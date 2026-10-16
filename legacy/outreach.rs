//! Website enrichment + SMS / email campaigns.
use crate::db::{err, Db, Filter, Place, Res};
use crate::logic::{accepted, contact_pages, extract_contacts, pct_encode, render, Contacts};
use crate::scanner::{log, status, Emit, ScanEvent, Summary};
use lettre::message::header::{ContentType, HeaderName, HeaderValue};
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36")
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_default()
}

// ---------------- website enrichment ----------------

const MAX_PAGE: usize = 2 * 1024 * 1024;

async fn fetch_html(http: &reqwest::Client, url: &str) -> Option<String> {
    let mut r = tokio::time::timeout(Duration::from_secs(10), http.get(url).send()).await.ok()?.ok()?;
    let html_like = r
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map_or(true, |t| t.contains("html"));
    if !r.status().is_success() || !html_like {
        return None;
    }
    let mut body = Vec::new();
    while let Ok(Ok(Some(chunk))) = tokio::time::timeout(Duration::from_secs(10), r.chunk()).await {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_PAGE {
            break;
        }
    }
    Some(String::from_utf8_lossy(&body).into_owned())
}

/// Reads the homepage (and up to two contact/about pages) for emails, socials and extra numbers.
pub async fn enrich(http: &reqwest::Client, c: &mut Contacts) {
    let site = c.website.clone();
    let Some(home) = fetch_html(http, &site).await else { return };
    extract_contacts(&home, c);
    for page in contact_pages(&home, &site) {
        if !c.needs_more() {
            break;
        }
        if let Some(html) = fetch_html(http, &page).await {
            extract_contacts(&html, c);
        }
    }
}

// ---------------- accounts ----------------

/// Any HTTP SMS gateway, described as data. Presets fill it in; users can edit every field.
/// Placeholders: {apikey} {username} {password} {from} {to} (09…) {to98} (989…) {text}.
/// In JSON bodies a value written as "{#from}" is sent as a number.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SmsAccount {
    pub id: String,
    pub name: String,
    pub preset: String,
    pub method: String, // GET | POST
    pub url: String,
    pub body_type: String, // json | form | none
    pub body: Value,
    pub headers: Vec<(String, String)>,
    pub api_key: String,
    pub username: String,
    pub password: String,
    pub sender: String,
    pub ok_path: String,        // "" = any 2xx; "." = whole body; "a.b.0" = JSON path
    pub ok_values: Vec<String>, // exact matches, or ">N" for a number greater than N
    pub per_minute: u32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct EmailAccount {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub security: String, // tls | starttls | none
    pub username: String,
    pub password: String,
    pub from_name: String,
    pub from_email: String,
    pub per_minute: u32,
}

fn fill(tpl: &str, vars: &[(&str, &str)], encode: bool) -> String {
    let enc: Vec<(&str, String)> = vars.iter().map(|(k, v)| (*k, if encode { pct_encode(v) } else { v.to_string() })).collect();
    render(tpl, &enc.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>())
}

fn fill_json(v: &Value, vars: &[(&str, &str)]) -> Value {
    match v {
        Value::String(s) if s.starts_with("{#") && s.ends_with('}') => {
            let raw = fill(&format!("{{{}", &s[2..]), vars, false);
            raw.parse::<i64>().map(Value::from).unwrap_or(Value::String(raw))
        }
        Value::String(s) => Value::String(fill(s, vars, false)),
        Value::Array(a) => Value::Array(a.iter().map(|x| fill_json(x, vars)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), fill_json(x, vars))).collect()),
        other => other.clone(),
    }
}

/// Dot path with array indexes; keys match case-insensitively (`IsSuccess` == `isSuccess`).
fn json_at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, k| match (v, k.parse::<usize>()) {
        (Value::Array(a), Ok(i)) => a.get(i),
        (Value::Object(o), _) => o.get(k).or_else(|| o.iter().find(|(kk, _)| kk.eq_ignore_ascii_case(k)).map(|(_, x)| x)),
        _ => None,
    })
}

fn snippet(s: &str) -> String {
    s.chars().take(200).collect()
}

pub async fn send_sms(http: &reqwest::Client, a: &SmsAccount, to: &str, text: &str) -> Res<()> {
    let to98 = format!("98{}", to.trim_start_matches('0'));
    let vars = [
        ("apikey", a.api_key.as_str()),
        ("username", &a.username),
        ("password", &a.password),
        ("from", &a.sender),
        ("to", to),
        ("to98", &to98),
        ("text", text),
    ];
    let method = if a.method.eq_ignore_ascii_case("GET") {
        reqwest::Method::GET
    } else {
        reqwest::Method::POST
    };
    let mut rb = http.request(method, fill(&a.url, &vars, true));
    for (k, v) in &a.headers {
        rb = rb.header(k.as_str(), fill(v, &vars, false));
    }
    rb = match a.body_type.as_str() {
        "json" => rb.json(&fill_json(&a.body, &vars)),
        "form" => {
            let form: Vec<(String, String)> = a
                .body
                .as_object()
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| (k.clone(), fill(v.as_str().unwrap_or(&v.to_string()), &vars, false)))
                        .collect()
                })
                .unwrap_or_default();
            rb.form(&form)
        }
        _ => rb,
    };
    let resp = rb.send().await.map_err(|e| format!("اتصال به پنل پیامک ناموفق بود: {e}"))?;
    let code = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !code.is_success() {
        return Err(format!("HTTP {}: {}", code.as_u16(), snippet(&body)));
    }
    if a.ok_path.trim().is_empty() {
        return Ok(());
    }
    let got = if a.ok_path == "." {
        body.clone()
    } else {
        let v: Value = serde_json::from_str(&body).map_err(|_| format!("پاسخ نامعتبر: {}", snippet(&body)))?;
        json_at(&v, &a.ok_path)
            .map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string()))
            .unwrap_or_default()
    };
    if accepted(&got, &a.ok_values) {
        Ok(())
    } else {
        Err(format!("پنل پیامک خطا داد: {}", snippet(&body)))
    }
}

type Mailer = AsyncSmtpTransport<Tokio1Executor>;

fn mailer(a: &EmailAccount) -> Res<Mailer> {
    let host = a.host.trim();
    let b = match a.security.as_str() {
        "tls" => Mailer::relay(host).map_err(err)?,
        "starttls" => Mailer::starttls_relay(host).map_err(err)?,
        _ => Mailer::builder_dangerous(host),
    };
    let b = if a.port > 0 { b.port(a.port) } else { b };
    let b = if a.username.trim().is_empty() {
        b
    } else {
        b.credentials(Credentials::new(a.username.trim().to_string(), a.password.clone()))
    };
    Ok(b.timeout(Some(Duration::from_secs(30))).build())
}

async fn send_email(m: &Mailer, a: &EmailAccount, to: &str, subject: &str, body: &str) -> Res<()> {
    let from = Mailbox::new(
        Some(a.from_name.clone()).filter(|s| !s.is_empty()),
        a.from_email.parse().map_err(|_| "ایمیل فرستنده نامعتبر است.")?,
    );
    let to: Mailbox = to.parse().map_err(|_| format!("ایمیل گیرنده نامعتبر است: {to}"))?;
    let unsubscribe = format!("<mailto:{}?subject=unsubscribe>", a.from_email);
    let msg = Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .raw_header(HeaderValue::new(HeaderName::new_from_ascii_str("List-Unsubscribe"), unsubscribe))
        .body(body.to_string())
        .map_err(err)?;
    m.send(msg).await.map(|_| ()).map_err(|e| format!("ارسال ایمیل ناموفق بود: {e}"))
}

// ---------------- campaigns ----------------

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CampaignReq {
    pub channel: String, // sms | email
    pub filter: Filter,
    pub account_id: String,
    pub subject: String,
    pub message: String,
    pub optout_footer: bool,
    pub skip_recent_days: i64, // 0 = allow re-sending
    pub test_to: String,       // non-empty: send one test message only
}

pub enum Account {
    Sms(SmsAccount),
    Email(EmailAccount),
}

enum Channel {
    Sms(SmsAccount, reqwest::Client),
    Email(EmailAccount, Mailer),
}

impl Channel {
    async fn send(&self, to: &str, subject: &str, body: &str) -> Res<()> {
        match self {
            Channel::Sms(a, http) => send_sms(http, a, to, body).await,
            Channel::Email(a, m) => send_email(m, a, to, subject, body).await,
        }
    }
}

const SMS_FOOTER: &str = "\nلغو۱۱";
const EMAIL_FOOTER: &str = "\n\n—\nاگر مایل به دریافت این ایمیل‌ها نیستید، در پاسخ فقط بنویسید «لغو».";

fn vars(p: &Place) -> [(&'static str, String); 4] {
    let city = if p.city.is_empty() { p.province.clone() } else { p.city.clone() };
    [
        ("name", p.name.clone()),
        ("city", city),
        ("province", p.province.clone()),
        ("keyword", p.keyword.clone()),
    ]
}

pub async fn run_campaign(db: &Db, req: &CampaignReq, account: Account, token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    if req.message.trim().is_empty() {
        return Err("متن پیام خالی است.".into());
    }
    let (channel, per_minute, account_name) = match account {
        Account::Sms(a) => {
            let (pm, name) = (a.per_minute, a.name.clone());
            (Channel::Sms(a, client()), pm, name)
        }
        Account::Email(a) => {
            if req.subject.trim().is_empty() {
                return Err("موضوع ایمیل را وارد کنید.".into());
            }
            let (pm, name, m) = (a.per_minute, a.name.clone(), mailer(&a)?);
            (Channel::Email(a, m), pm, name)
        }
    };
    let is_sms = matches!(channel, Channel::Sms(..));
    let kind = if is_sms { "sms" } else { "email" };
    let footer = if !req.optout_footer {
        ""
    } else if is_sms {
        SMS_FOOTER
    } else {
        EMAIL_FOOTER
    };
    let compose = |p: &Place| {
        let v = vars(p);
        let v: Vec<(&str, &str)> = v.iter().map(|(k, s)| (*k, s.as_str())).collect();
        (render(req.subject.trim(), &v), format!("{}{footer}", render(req.message.trim(), &v)))
    };

    let (rows, _) = db.list(&req.filter, 0, 0)?;
    let mut seen = HashSet::new();
    let targets: Vec<(&Place, String)> = rows
        .iter()
        .filter_map(|p| {
            let to = Place::first(if is_sms { &p.mobiles } else { &p.emails }).to_string();
            (!to.is_empty() && seen.insert(to.clone())).then_some((p, to))
        })
        .collect();

    if !req.test_to.trim().is_empty() {
        let sample = targets.first().map(|(p, _)| (*p).clone()).unwrap_or(Place {
            name: "نمونه".into(),
            city: "تهران".into(),
            ..Default::default()
        });
        let (subject, body) = compose(&sample);
        let to = if is_sms {
            crate::logic::normalize_phone(&req.test_to)
        } else {
            req.test_to.trim().to_lowercase()
        };
        channel.send(&to, &subject, &body).await?;
        return Ok(Summary {
            message: format!("پیام آزمایشی به {} ارسال شد.", req.test_to.trim()),
            ..Default::default()
        });
    }
    if targets.is_empty() {
        return Err(if is_sms {
            "در این انتخاب هیچ شماره موبایلی نیست."
        } else {
            "در این انتخاب هیچ ایمیلی نیست."
        }
        .into());
    }

    let total = targets.len() as i64;
    let id = db.campaign_start(kind, req.filter.list_id, &account_name, &req.subject, &req.message, total)?;
    let per_minute = if per_minute == 0 { 60 } else { per_minute.min(1200) };
    let gap = Duration::from_millis(60_000 / per_minute as u64);
    let (mut sent, mut failed, mut skipped, mut streak) = (0i64, 0i64, 0i64, 0);
    let mut fatal = None;
    status(emit, format!("ارسال {} پیام از طریق «{account_name}»…", total));
    for (p, to) in targets {
        if token.is_cancelled() {
            break;
        }
        if db.opted_out(to.as_str()) || (req.skip_recent_days > 0 && db.messaged_within(&to, kind, req.skip_recent_days)) {
            skipped += 1;
            db.log_message(id, p.id, &to, kind, "skipped", "");
            emit(ScanEvent::Sending { sent, failed, skipped, total });
            continue;
        }
        let (subject, body) = compose(p);
        let result = tokio::select! {
            r = channel.send(&to, &subject, &body) => r,
            _ = token.cancelled() => break,
        };
        match result {
            Ok(()) => {
                sent += 1;
                streak = 0;
                db.log_message(id, p.id, &to, kind, "sent", "");
                let _ = db.mark_contacted(p.id);
            }
            Err(e) => {
                failed += 1;
                streak += 1;
                db.log_message(id, p.id, &to, kind, "failed", &e);
                log(emit, "warning", format!("{to}: {e}"));
                if streak >= 5 {
                    fatal = Some(format!("۵ ارسال پشت‌سرهم ناموفق بود و کمپین متوقف شد. آخرین خطا: {e}"));
                    break;
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
    db.campaign_finish(id, sent, failed, skipped, state)?;
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
