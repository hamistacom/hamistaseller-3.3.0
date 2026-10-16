//! Campaigns: one personalised message per recipient through an official channel
//! (SMS panel, SMTP or Bale Safir), paced per account, honouring the do-not-contact list
//! and "already messaged in the last N days". Sent from inside the app — no browser.
use crate::channels::Loaded;
use crate::db::{Db, Filter, Place, Res};
use crate::logic::{normalize_phone, render};
use crate::scanner::{log, status, Emit, ScanEvent, Summary};
use serde::Deserialize;
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CampaignReq {
    pub filter: Filter,
    pub account_id: String,
    pub subject: String,
    pub message: String,
    pub optout_footer: bool,
    pub skip_recent_days: i64, // 0 = allow re-sending
    pub every_number: bool,    // every mobile / email of a business, not only the first
    pub test_to: String,       // non-empty: send one test message only
}

const SMS_FOOTER: &str = "\nلغو۱۱";
const EMAIL_FOOTER: &str = "\n\n—\nاگر مایل به دریافت این ایمیل‌ها نیستید، در پاسخ فقط بنویسید «لغو».";

/// Placeholders a template can use: {name} {person} {role} {city} {province} {keyword} {category}.
pub fn vars(p: &Place) -> Vec<(&'static str, String)> {
    let city = if p.city.is_empty() { p.province.clone() } else { p.city.clone() };
    vec![
        ("name", p.name.clone()),
        ("person", p.contact_person.clone()),
        ("role", p.contact_role.clone()),
        ("city", city),
        ("province", p.province.clone()),
        ("keyword", p.keyword.clone()),
        ("category", p.category.clone()),
    ]
}

pub fn compose(tpl: &str, p: &Place) -> String {
    let v = vars(p);
    render(tpl.trim(), &v.iter().map(|(k, s)| (*k, s.as_str())).collect::<Vec<_>>())
}

struct Target<'a> {
    place: &'a Place,
    contact_id: i64,
    to: String,
}

pub async fn run_campaign(db: &Db, req: &CampaignReq, acc: Loaded, token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    if req.message.trim().is_empty() {
        return Err("متن پیام خالی است.".into());
    }
    let target_kind = acc.sender.target();
    if target_kind == "email" && req.subject.trim().is_empty() {
        return Err("موضوع ایمیل را وارد کنید.".into());
    }
    let footer = match (req.optout_footer, acc.kind.as_str()) {
        (false, _) => "",
        (true, "sms") => SMS_FOOTER,
        (true, "email") => EMAIL_FOOTER,
        _ => "",
    };
    let build = |p: &Place| (compose(&req.subject, p), format!("{}{footer}", compose(&req.message, p)));

    let (rows, _) = db.list(&req.filter, 0, 0)?;
    let mut seen = HashSet::new();
    let mut targets: Vec<Target> = vec![];
    for p in &rows {
        let mut of_kind = p.contacts.iter().filter(|c| c.kind == target_kind);
        let picked: Vec<_> = if req.every_number {
            of_kind.collect()
        } else {
            of_kind.next().into_iter().collect()
        };
        for c in picked {
            if seen.insert(c.value.clone()) {
                targets.push(Target {
                    place: p,
                    contact_id: c.id,
                    to: c.value.clone(),
                });
            }
        }
    }

    if !req.test_to.trim().is_empty() {
        let sample = targets.first().map(|t| t.place.clone()).unwrap_or(Place {
            name: "نمونه".into(),
            city: "تهران".into(),
            ..Default::default()
        });
        let (subject, body) = build(&sample);
        let to = if target_kind == "email" {
            req.test_to.trim().to_lowercase()
        } else {
            normalize_phone(&req.test_to)
        };
        acc.sender
            .send(&to, &subject, &body, &format!("hs-test-{}", db.stamp()))
            .await
            .map_err(|e| e.msg)?;
        return Ok(Summary {
            message: format!("پیام آزمایشی به {} ارسال شد.", req.test_to.trim()),
            ..Default::default()
        });
    }
    if targets.is_empty() {
        return Err(if target_kind == "email" {
            "در این انتخاب هیچ ایمیلی نیست."
        } else {
            "در این انتخاب هیچ شماره موبایلی نیست."
        }
        .into());
    }

    let total = targets.len() as i64;
    let id = db.campaign_start(&acc.kind, req.filter.list_id, &acc.id, &acc.name, &req.subject, &req.message, total)?;
    let gap = Duration::from_millis(60_000 / acc.per_minute.clamp(1, 1200) as u64);
    let (mut sent, mut failed, mut skipped, mut streak) = (0i64, 0i64, 0i64, 0);
    let mut fatal = None;
    status(emit, format!("ارسال {total} پیام از طریق «{}»…", acc.name));
    for t in targets {
        if token.is_cancelled() {
            break;
        }
        if db.opted_out(&t.to) || (req.skip_recent_days > 0 && db.messaged_within(&t.to, &acc.kind, req.skip_recent_days)) {
            skipped += 1;
            db.log_message(id, t.place.id, t.contact_id, &t.to, &acc.kind, "skipped", "", "");
            emit(ScanEvent::Sending { sent, failed, skipped, total });
            continue;
        }
        let (subject, body) = build(t.place);
        let request_id = format!("hs{id}-{}", t.contact_id); // Bale Safir sends a retried request only once
                                                             // not raced against Stop: the panel may already have taken the message, so its result
                                                             // must be recorded (each send is time-limited by its HTTP / SMTP client)
        let result = acc.sender.send(&t.to, &subject, &body, &request_id).await;
        match result {
            Ok(provider_id) => {
                sent += 1;
                streak = 0;
                db.log_message(id, t.place.id, t.contact_id, &t.to, &acc.kind, "sent", "", &provider_id);
                db.set_delivery(t.contact_id, "sent");
                db.mark_contacted(t.place.id);
            }
            Err(e) => {
                failed += 1;
                db.log_message(id, t.place.id, t.contact_id, &t.to, &acc.kind, "failed", &e.msg, "");
                db.set_delivery(t.contact_id, e.delivery);
                log(emit, "warning", format!("{}: {}", t.to, e.msg));
                // a bad number says nothing about the account; 5 account errors in a row do
                if !e.recipient {
                    streak += 1;
                    if streak >= 5 {
                        fatal = Some(format!("۵ ارسال پشت‌سرهم ناموفق بود و کمپین متوقف شد. آخرین خطا: {}", e.msg));
                        break;
                    }
                }
            }
        }
        emit(ScanEvent::Sending { sent, failed, skipped, total });
        tokio::select! {
            _ = sleep(gap) => {}
            _ = token.cancelled() => break,
        }
    }
    let state = if fatal.is_some() {
        "failed"
    } else if token.is_cancelled() {
        "cancelled"
    } else {
        "done"
    };
    db.campaign_finish(id, sent, failed, skipped, state, fatal.as_deref().unwrap_or(""))?;
    db.log("campaign", &format!("کمپین «{}»: {sent} ارسال موفق از {total}", acc.name));
    let message = fatal.clone().unwrap_or_else(|| {
        let head = if token.is_cancelled() {
            "کمپین متوقف شد."
        } else {
            "کمپین تمام شد."
        };
        format!("{head} {sent} ارسال موفق، {failed} ناموفق، {skipped} ردشده (لغو یا ارسال اخیر).")
    });
    Ok(Summary {
        checked: sent + failed + skipped,
        found: sent,
        cancelled: token.is_cancelled(),
        error: fatal.is_some(),
        message,
    })
}
