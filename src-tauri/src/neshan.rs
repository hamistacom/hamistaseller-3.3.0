//! Neshan map (official API, the user's own API key). Neshan lists Iranian businesses well but its
//! search API returns no phone numbers, so each business found is then looked up by exact name on
//! Google Maps (the existing engine, `ScanRequest.lookups`) to read its phone, site and socials.
//! Saved places carry source "neshan".
use crate::db::{Db, Res};
use crate::scanner::{self, Emit, ScanRequest, Settings, Summary};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn base() -> String {
    crate::logic::test_seam("HAMISTA_NESHAN_BASE").unwrap_or_else(|| "https://api.neshan.org".into())
}

async fn get(http: &reqwest::Client, key: &str, url: String) -> Res<Value> {
    let resp = http.get(url).header("Api-Key", key).send().await.map_err(|e| format!("اتصال به نشان برقرار نشد: {e}"))?;
    match resp.status().as_u16() {
        200 => resp.json().await.map_err(|e| e.to_string()),
        401 | 403 => Err("کلید API نشان نامعتبر است یا سرویس «جستجو» و «ژئوکدینگ» برایش فعال نیست (پنل نشان).".into()),
        429 => Err("سهمیه روزانه API نشان تمام شده است.".into()),
        s => Err(format!("خطای سرور نشان ({s})")),
    }
}

/// City centre (lat, lng) from Neshan's geocoder.
async fn locate(http: &reqwest::Client, key: &str, area: &str) -> Res<(f64, f64)> {
    let v = get(http, key, format!("{}/v4/geocoding?address={}", base(), crate::logic::pct_encode(area))).await?;
    match (v["location"]["y"].as_f64(), v["location"]["x"].as_f64()) {
        (Some(lat), Some(lng)) => Ok((lat, lng)),
        _ => Err(format!("موقعیت «{area}» در نشان پیدا نشد.")),
    }
}

/// Search points: the centre, then two rings (~3 km and ~6 km) — each Neshan search returns ≤30.
pub fn grid(lat: f64, lng: f64) -> Vec<(f64, f64)> {
    let mut v = vec![(lat, lng)];
    for d in [0.03, 0.06] {
        for (a, b) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            v.push((lat + a * d, lng + b * d));
        }
    }
    v
}

/// Business names from one Neshan search result (streets, regions and duplicates dropped).
pub fn business_titles(v: &Value, out: &mut Vec<String>) {
    for it in v["items"].as_array().into_iter().flatten() {
        let cat = it["category"].as_str().unwrap_or_default();
        if cat == "region" || cat == "municipal" || it["type"].as_str().is_some_and(|t| ["street", "primary", "secondary", "neighbourhood", "city", "county"].contains(&t)) {
            continue;
        }
        let title = it["title"].as_str().unwrap_or_default().trim().to_string();
        let key = crate::logic::name_key(&title);
        if !key.is_empty() && !out.iter().any(|x| crate::logic::name_key(x) == key) {
            out.push(title);
        }
    }
}

pub async fn run(db: &Db, s: &Settings, profile: &Path, req: &ScanRequest, token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    let key = crate::sources::config_value(db, "neshan", "apiKey");
    if key.is_empty() {
        return Err("کلید API نشان را در تنظیمات ← ماژول‌ها وارد کنید (رایگان از پنل platform.neshan.org).".into());
    }
    let area = req.area();
    let area = area.as_str();
    if area.is_empty() {
        return Err("استان یا شهر را انتخاب کنید.".into());
    }
    let keywords: Vec<String> = req.keywords.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect();
    if keywords.is_empty() {
        return Err("حداقل یک کلمه کلیدی وارد کنید.".into());
    }
    let limit = req.limit.clamp(1, 1000) as usize;
    let http = crate::enrich::client();
    scanner::status(emit, format!("یافتن «{area}» در نشان…"));
    let (lat, lng) = locate(&http, &key, area).await?;
    let mut sum = Summary::default();
    for (i, kw) in keywords.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }
        scanner::status(emit, format!("جستجوی «{kw}» در نقشه نشان…"));
        let mut titles: Vec<String> = vec![];
        for (la, ln) in grid(lat, lng) {
            if titles.len() >= limit || token.is_cancelled() {
                break;
            }
            let url = format!("{}/v1/search?term={}&lat={la:.6}&lng={ln:.6}", base(), crate::logic::pct_encode(kw));
            match get(&http, &key, url).await {
                Ok(v) => business_titles(&v, &mut titles),
                Err(e) if titles.is_empty() && sum.checked == 0 => return Err(e),
                Err(e) => {
                    scanner::log(emit, "warning", e);
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        titles.truncate(limit);
        if titles.is_empty() {
            scanner::log(emit, "warning", format!("نشان برای «{kw}» در {area} کسب‌وکاری نداشت."));
            continue;
        }
        scanner::log(emit, "muted", format!("نشان: {} کسب‌وکار «{kw}» پیدا شد؛ یافتن شماره‌ها در گوگل‌مپ…", titles.len()));
        let sub = ScanRequest {
            keywords: vec![kw.clone()],
            lookups: titles.clone(),
            rest: keywords[i + 1..].to_vec(),
            limit: titles.len() as i64,
            first_limit: 0,
            source: "neshan".into(),
            ..req.clone()
        };
        let r = scanner::run(db, s, profile, &sub, token, emit).await?;
        sum.checked += r.checked;
        sum.found += r.found;
        if r.error {
            sum.error = true;
            sum.message = r.message;
            break;
        }
    }
    sum.cancelled = token.is_cancelled();
    if sum.message.is_empty() {
        let head = if sum.cancelled { "متوقف شد." } else { "پایان استخراج از نشان." };
        sum.message = format!("{head} {} کسب‌وکار بررسی و {} مخاطب جدید ذخیره شد.", sum.checked, sum.found);
    }
    Ok(sum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn grid_and_titles() {
        assert_eq!(grid(35.7, 51.4).len(), 17);
        let v = json!({"count": 4, "items": [
            {"title": "کافه روبرتو", "category": "place", "type": "cafe"},
            {"title": "كافه روبرتو", "category": "place", "type": "cafe"},
            {"title": "خیابان پاسداران", "category": "municipal", "type": "primary"},
            {"title": "پاسداران", "category": "region", "type": "neighbourhood"},
            {"title": "کافه لمیز", "category": "place", "type": "cafe"}
        ]});
        let mut out = vec![];
        business_titles(&v, &mut out);
        assert_eq!(out, vec!["کافه روبرتو", "کافه لمیز"]);
    }
}
