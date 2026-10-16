//! Registry of data sources («ماژول‌ها») and what the sources share: per-source settings (API keys,
//! sealed), saving a found business, and the run loop of the search-driven sources (Phase 3.3).
//! Includes Google Maps, Divar, Sheypoor, Balad, Neshan, Telegram, Instagram, Website enrichment, and Excel/CSV import.

#[path = "divar.rs"]
pub mod divar;

use crate::db::{Contact, Db, Place, Res};
use crate::logic::Contacts;
use crate::scanner::{self, Emit, ScanEvent, ScanRequest, Settings, Summary};
use serde::Serialize;
use serde_json::{Map, Value};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub icon: &'static str,
    /// scan = finds new businesses (public business data, feeds the mega-bank)
    /// enrich = adds detail to found ones · import = the user's own files
    pub kind: &'static str,
    pub ready: bool,
}

pub const SOURCES: &[Source] = &[
    Source {
        id: "maps",
        name: "گوگل‌مپ و وب",
        desc: "کسب‌وکارهای دارای اطلاعات عمومی در نقشه و وب، با استان، شهرستان یا منطقه و کلمه کلیدی",
        icon: "map",
        kind: "scan",
        ready: true,
    },
    Source {
        id: "divar",
        name: "دیوار",
        desc: "استخراج خودکار شماره تماس فروشگاه‌ها، کسب‌وکارها و آگهی‌های دیوار بر اساس شهر و منطقه",
        icon: "store",
        kind: "scan",
        ready: true,
    },
    Source {
        id: "sheypoor",
        name: "شیپور",
        desc: "استخراج خودکار شماره تماس فروشگاه‌ها، کسب‌وکارها و آگهی‌های شیپور بر اساس شهر و منطقه",
        icon: "tag",
        kind: "scan",
        ready: true,
    },
    Source {
        id: "balad",
        name: "نقشه بلد",
        desc: "صفحه‌های عمومی کسب‌وکارها در نقشه بلد (نام، تلفن، آدرس و موقعیت)",
        icon: "map",
        kind: "scan",
        ready: true,
    },
    Source {
        id: "neshan",
        name: "نقشه نشان",
        desc: "فهرست کسب‌وکارهای ثبت‌شده در نقشه نشان و تکمیل خودکار شماره تماس از گوگل‌مپ",
        icon: "map",
        kind: "scan",
        ready: true,
    },
    Source {
        id: "telegram",
        name: "کانال‌های عمومی تلگرام",
        desc: "کانال‌های عمومی کسب‌وکارها بر اساس موضوع به همراه تعداد عضو و اطلاعات تماس بیو",
        icon: "telegram",
        kind: "scan",
        ready: true,
    },
    Source {
        id: "instagram",
        name: "پیج‌های کاری اینستاگرام",
        desc: "پیج‌های کسب‌وکاری اینستاگرام دارای شماره تماس، ایمیل یا وب‌سایت در بیو",
        icon: "instagram",
        kind: "scan",
        ready: true,
    },
    Source {
        id: "website",
        name: "سایت کسب‌وکار",
        desc: "ایمیل، شبکه‌های اجتماعی و نام مدیر از سایت خود کسب‌وکار (همراه استخراج)",
        icon: "globe",
        kind: "enrich",
        ready: true,
    },
    Source {
        id: "import",
        name: "ورود فایل",
        desc: "فهرست مشتریان خودتان از Excel یا CSV",
        icon: "upload",
        kind: "import",
        ready: true,
    },
];

pub fn get(id: &str) -> Option<&'static Source> {
    SOURCES.iter().find(|s| s.id == id)
}

// ---------------- on / off per source (the user's choice, on this PC) ----------------

pub fn switchable(id: &str) -> bool {
    let finders = SOURCES.iter().filter(|s| s.kind == "scan").count();
    get(id).is_some_and(|s| (s.kind == "scan" && finders > 1) || s.id == "website")
}

pub fn enabled(db: &Db, id: &str) -> bool {
    !switchable(id) || !db.kv_get("sources_off").unwrap_or_default().split(',').any(|x| x == id)
}

pub fn set_enabled(db: &Db, id: &str, on: bool) -> Res<()> {
    if !switchable(id) {
        return Err("این ماژول قابل خاموش کردن نیست.".into());
    }
    let mut off: Vec<String> = db
        .kv_get("sources_off")
        .unwrap_or_default()
        .split(',')
        .filter(|x| !x.is_empty() && *x != id)
        .map(String::from)
        .collect();
    if !on {
        off.push(id.to_string());
        if SOURCES.iter().filter(|s| s.kind == "scan").all(|s| off.iter().any(|o| o == s.id)) {
            return Err("حداقل یک منبع استخراج باید روشن بماند.".into());
        }
    }
    db.kv_set("sources_off", &off.join(","))
}

pub fn stats(db: &Db) -> std::collections::HashMap<String, (i64, i64)> {
    let c = db.c();
    let Ok(mut st) = c.prepare(
        "SELECT source, count(*), sum(created_at >= datetime('now','localtime','-30 days')) FROM places p
         WHERE EXISTS(SELECT 1 FROM contacts c WHERE c.place_id=p.id) GROUP BY source",
    ) else {
        return Default::default();
    };
    st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            (r.get::<_, i64>(1)?, r.get::<_, Option<i64>>(2)?.unwrap_or(0)),
        ))
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

// ---------------- per-source settings (API keys), DPAPI-sealed ----------------

pub fn config_keys(id: &str) -> &'static [&'static str] {
    match id {
        "neshan" => &["apiKey"],
        "instagram" => &["igUserId", "accessToken"],
        _ => &[],
    }
}

pub fn config(db: &Db, id: &str) -> Map<String, Value> {
    db.kv_get(&format!("src_cfg_{id}"))
        .and_then(|b| crate::logic::b64_decode(&b))
        .and_then(|raw| crate::secrets::open_str(&raw).ok())
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

pub fn config_value(db: &Db, id: &str, key: &str) -> String {
    config(db, id).get(key).and_then(Value::as_str).unwrap_or_default().trim().to_string()
}

pub fn config_save(db: &Db, id: &str, patch: &Map<String, Value>) -> Res<()> {
    let keys = config_keys(id);
    let mut cfg = config(db, id);
    for (k, v) in patch {
        if !keys.contains(&k.as_str()) {
            continue;
        }
        match v {
            Value::Null => {
                cfg.remove(k);
            }
            Value::String(s) if !s.trim().is_empty() => {
                cfg.insert(k.clone(), Value::String(s.trim().to_string()));
            }
            _ => {}
        }
    }
    let sealed = crate::secrets::seal_str(&Value::Object(cfg).to_string())?;
    db.kv_set(&format!("src_cfg_{id}"), &crate::logic::b64_encode(&sealed))
}

pub fn config_status(db: &Db, id: &str) -> Map<String, Value> {
    let cfg = config(db, id);
    config_keys(id)
        .iter()
        .map(|k| (k.to_string(), Value::Bool(cfg.get(*k).and_then(Value::as_str).is_some_and(|v| !v.is_empty()))))
        .collect()
}

// ---------------- saving a found business ----------------

pub fn save(db: &Db, emit: Emit<'_>, mut place: Place, c: &Contacts, origin: &str, labels: &[(&str, String)]) -> bool {
    if !c.has_any() {
        let _ = db.insert_place(&place);
        scanner::log(emit, "muted", format!("{} — راه ارتباطی عمومی نداشت", place.name));
        return false;
    }
    if place.website.is_empty() {
        place.website = c.website.clone();
    }
    place.contacts = c
        .pairs()
        .into_iter()
        .map(|(kind, value)| Contact {
            label: labels.iter().find(|(k, _)| *k == kind).map(|(_, l)| l.clone()).unwrap_or_default(),
            origin: origin.into(),
            kind: kind.into(),
            value,
            ..Default::default()
        })
        .collect();
    match db.insert_place(&place) {
        Ok((true, added)) if added > 0 => {
            let first = place.contacts.first().map(|c| c.value.clone()).unwrap_or_default();
            scanner::log(emit, "success", format!("{} — {first}", place.name));
            if let Ok(saved) = db.get_by_key(place.list_id, &place.place_key) {
                emit(ScanEvent::Found { place: Box::new(saved) });
            }
            true
        }
        Ok((true, _)) => {
            scanner::log(emit, "muted", format!("{} — راه‌های ارتباطی‌اش قبلاً در این بانک ثبت شده", place.name));
            false
        }
        Ok((false, _)) => false,
        Err(e) => {
            scanner::log(emit, "error", format!("ذخیره نشد: {e}"));
            false
        }
    }
}

// ---------------- one source, or several in one job ----------------

pub async fn run_one(db: &Db, s: &Settings, profile: &Path, req: &ScanRequest, token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    match req.source.as_str() {
        "" | "maps" => scanner::run(db, s, profile, req, token, emit).await,
        "divar" => divar::run_divar(db, s, profile, req, token, emit).await,
        "sheypoor" => divar::run_sheypoor(db, s, profile, req, token, emit).await,
        "neshan" if !req.lookups.is_empty() => {
            let mut sum = scanner::run(db, s, profile, req, token, emit).await?;
            if !req.rest.is_empty() && !token.is_cancelled() {
                let rest = ScanRequest {
                    keywords: req.rest.clone(),
                    lookups: vec![],
                    rest: vec![],
                    first_limit: 0,
                    ..req.clone()
                };
                let more = crate::neshan::run(db, s, profile, &rest, token, emit).await?;
                (sum.checked, sum.found, sum.cancelled) = (sum.checked + more.checked, sum.found + more.found, more.cancelled);
                sum.message = more.message;
            }
            Ok(sum)
        }
        "neshan" => crate::neshan::run(db, s, profile, req, token, emit).await,
        _ => run_search(db, s, profile, req, token, emit).await,
    }
}

pub fn requested(req: &ScanRequest) -> Vec<String> {
    let wanted: Vec<&str> = if req.sources.is_empty() {
        vec![if req.source.is_empty() { "maps" } else { req.source.as_str() }]
    } else {
        req.sources.iter().map(String::as_str).collect()
    };
    SOURCES.iter().filter(|s| wanted.contains(&s.id)).map(|s| s.id.to_string()).collect()
}

#[allow(clippy::too_many_arguments)]
pub async fn run_all(
    db: &Db,
    s: &Settings,
    profile: &Path,
    req: &ScanRequest,
    list: &[String],
    token: &CancellationToken,
    emit: Emit<'_>,
    mut each: impl FnMut(&str, &Summary),
) -> Res<Summary> {
    if list.len() == 1 {
        let one = ScanRequest {
            source: list[0].clone(),
            sources: vec![],
            ..req.clone()
        };
        let r = run_one(db, s, profile, &one, token, emit).await?;
        each(&list[0], &r);
        return Ok(r);
    }
    let mut sum = Summary::default();
    let mut lines = vec![];
    for (n, id) in list.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }
        let name = get(id).map_or(id.as_str(), |x| x.name);
        scanner::status(emit, format!("منبع {} از {}: {name}", n + 1, list.len()));
        scanner::log(emit, "muted", format!("—— {name} ——"));
        let one = ScanRequest {
            source: id.clone(),
            sources: vec![],
            ..req.clone()
        };
        match run_one(db, s, profile, &one, token, emit).await {
            Ok(r) => {
                each(id, &r);
                sum.checked += r.checked;
                sum.found += r.found;
                lines.push(format!("{name}: {} مخاطب جدید", r.found));
                if r.error {
                    scanner::log(emit, "warning", format!("{name}: {}", r.message));
                }
            }
            Err(e) => {
                scanner::log(emit, "error", format!("{name}: {e}"));
                lines.push(format!("{name}: انجام نشد"));
                sum.error = true;
            }
        }
    }
    sum.cancelled = token.is_cancelled();
    let head = if sum.cancelled { "متوقف شد." } else { "پایان استخراج از همه منابع." };
    sum.message = format!("{head} {} مورد بررسی و {} مخاطب جدید — {}", sum.checked, sum.found, lines.join("، "));
    sum.error = sum.error && sum.found == 0;
    Ok(sum)
}

// ---------------- search-driven sources (Balad, Telegram, Instagram) ----------------

pub fn site_query(source: &str, keyword: &str, area: &str) -> String {
    let site = match source {
        "balad" => "site:balad.ir/p",
        "telegram" => "site:t.me",
        "instagram" => "site:instagram.com",
        _ => "",
    };
    format!("{site} {} {}", keyword.trim(), area.trim()).trim().to_string()
}

pub async fn run_search(db: &Db, s: &Settings, profile: &Path, req: &ScanRequest, token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    let source = req.source.as_str();
    let keywords: Vec<String> = req.keywords.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect();
    if keywords.is_empty() {
        return Err("حداقل یک کلمه کلیدی وارد کنید.".into());
    }

    // Interactive login mode for Instagram in the browser
    if source == "instagram" && keywords.len() == 1 && keywords[0] == "__login__" {
        scanner::status(emit, "باز کردن پنجره مرورگر برای ورود به حساب اینستاگرام…");
        let mut eng = scanner::launch(s, profile, false, 0).await?;
        scanner::goto(&eng.list, "https://www.instagram.com/accounts/login/").await;
        emit(ScanEvent::Captcha {
            message: "پنجره مرورگر باز شد. لطفاً وارد حساب اینستاگرام خود شوید و پس از ورود، پنجره را ببندید تا نشست شما ذخیره شود.".into(),
        });
        let deadline = Instant::now() + Duration::from_secs(600);
        while Instant::now() < deadline && !token.is_cancelled() {
            if !eng.alive.load(std::sync::atomic::Ordering::Relaxed) || eng.proc.exited() {
                break;
            }
            sleep(Duration::from_secs(1)).await;
        }
        scanner::shut(Some(eng)).await;
        return Ok(Summary {
            checked: 0,
            found: 0,
            cancelled: false,
            error: false,
            message: "نشست حساب اینستاگرام ذخیره شد. اکنون می‌توانید استخراج را شروع کنید.".into(),
        });
    }

    if source == "balad" && req.area().is_empty() {
        return Err("استان یا شهر را انتخاب کنید.".into());
    }
    let ig = crate::instagram::Api::from_config(db);
    let limit = req.limit.clamp(1, 500);
    let area = req.area();
    let area = area.as_str();
    let http = crate::enrich::client();
    let mut sum = Summary::default();
    let mut failure: Option<String> = None;
    scanner::status(emit, "در حال آماده‌سازی مرورگر…");
    let eng = tokio::select! {
        e = scanner::launch(s, profile, s.headless, 1) => e?,
        _ = token.cancelled() => return Ok(Summary { cancelled: true, message: "متوقف شد.".into(), ..Default::default() }),
    };
    for (i, kw) in keywords.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }
        let run_id = match db.run_start(req.list_id, kw, &req.province, &req.run_city(), limit) {
            Ok(id) => id,
            Err(e) => {
                failure = Some(e);
                break;
            }
        };
        let resume = ScanRequest {
            keywords: keywords[i..].to_vec(),
            ..req.clone()
        };
        let resume = serde_json::to_string(&resume).unwrap_or_default();
        db.run_checkpoint(run_id, 0, 0, &resume);
        let query = site_query(source, kw, area);
        scanner::status(emit, format!("جستجوی «{query}»…"));
        let pages = ((limit as usize).div_ceil(10) + 1).min(10);
        let hits = match crate::websearch::search_attended(&eng.list, s, profile, &query, pages, token, emit).await {
            Ok(h) => h,
            Err(e) => {
                let _ = db.run_finish(run_id, 0, 0, "failed", &e);
                failure = Some(e);
                break;
            }
        };
        let (mut checked, mut found) = (0i64, 0i64);
        let mut seen = std::collections::HashSet::new();
        let progress = |checked: i64, found: i64| ScanEvent::Progress {
            keyword: kw.clone(),
            collected: hits.len() as i64,
            checked,
            found,
            target: limit,
        };
        emit(progress(0, 0));
        for hit in &hits {
            if checked >= limit || token.is_cancelled() {
                break;
            }
            let key = match source {
                "balad" => crate::balad::key_of(&hit.url),
                "telegram" => crate::telegram::handle_of(&hit.url).map(|h| format!("tg:{}", h.to_lowercase())),
                "instagram" => crate::instagram::handle_of(&hit.url).map(|h| format!("ig:{}", h.to_lowercase())),
                _ => None,
            };
            let Some(key) = key else { continue };
            if !seen.insert(key.clone()) || db.known(req.list_id, &key) {
                continue;
            }
            checked += 1;
            let base_key = Place {
                list_id: req.list_id,
                place_key: key.clone(),
                source: source.into(),
                ..Default::default()
            };
            let base = Place {
                list_id: req.list_id,
                place_key: key,
                keyword: kw.clone(),
                category: kw.clone(),
                province: req.province.trim().to_string(),
                city: req.city.trim().to_string(),
                status: "new".into(),
                source: source.into(),
                ..Default::default()
            };
            let read = match source {
                "balad" => tokio::select! {
                    r = crate::balad::read(&eng.workers[0], &hit.url, base) => r,
                    _ = token.cancelled() => Ok(None),
                },
                "telegram" => crate::telegram::read(&http, hit, base).await,
                _ => tokio::select! {
                    r = crate::instagram::read_with_page(&http, ig.as_ref(), Some(&eng.workers[0]), hit, base) => r,
                    _ = token.cancelled() => Ok(None),
                },
            };
            match read {
                Ok(Some((place, c, labels))) => {
                    if save(db, emit, place, &c, source, &labels) {
                        found += 1;
                    }
                }
                Ok(None) => {
                    let _ = db.insert_place(&Place {
                        name: String::new(),
                        category: String::new(),
                        ..base_key
                    });
                }
                Err(e) => scanner::log(emit, "warning", e),
            }
            emit(progress(checked, found));
            db.run_checkpoint(run_id, checked, found, &resume);
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(crate::logic::jitter(s.delay_ms, s.delay_ms * 2))) => {}
                _ = token.cancelled() => break,
            }
        }
        if hits.is_empty() {
            scanner::log(emit, "warning", format!("برای «{kw}» نتیجه‌ای پیدا نشد."));
        }
        sum.checked += checked;
        sum.found += found;
        let status = if token.is_cancelled() { "cancelled" } else { "done" };
        let _ = db.run_finish(run_id, checked, found, status, "");
        if token.is_cancelled() {
            break;
        }
    }
    scanner::status(emit, "در حال بستن مرورگر…");
    scanner::shut(Some(eng)).await;
    sum.cancelled = token.is_cancelled();
    sum.error = failure.is_some();
    let name = get(source).map_or(source, |s| s.name);
    sum.message = match (&failure, sum.cancelled) {
        (Some(e), _) => e.clone(),
        (None, true) => format!("متوقف شد. {} مورد از {name} بررسی و {} مخاطب جدید ذخیره شد.", sum.checked, sum.found),
        (None, false) => format!("پایان استخراج از {name}. {} مورد بررسی و {} مخاطب جدید ذخیره شد.", sum.checked, sum.found),
    };
    match failure {
        Some(e) if sum.checked == 0 => Err(e),
        _ => Ok(sum),
    }
}

pub fn public_sql_list() -> String {
    SOURCES
        .iter()
        .filter(|s| s.kind == "scan")
        .map(|s| format!("'{}'", s.id))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_sane() {
        let mut ids: Vec<_> = SOURCES.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), SOURCES.len(), "duplicate source id");
        assert!(SOURCES.iter().all(|s| s.id.bytes().all(|b| b.is_ascii_lowercase())));
        let l = public_sql_list();
        assert!(l.contains("'maps'") && l.contains("'divar'") && !l.contains("import") && !l.contains("website"));
        assert!(SOURCES.iter().all(|s| s.ready), "every listed source ships its engine");
        assert_eq!(site_query("telegram", " کافه ", "تهران"), "site:t.me کافه تهران");
        assert_eq!(site_query("balad", "کافه", ""), "site:balad.ir/p کافه");
        assert!(switchable("maps") && switchable("divar") && switchable("website") && !switchable("import"));
    }
}