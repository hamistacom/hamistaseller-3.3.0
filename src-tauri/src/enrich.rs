//! Reads a business website (home + up to two contact/about pages) for emails, socials,
//! extra numbers and the manager's name. Plain HTTP: no browser tab is spent on it.
use crate::logic::{contact_pages, extract_contacts, Contacts};
use std::time::Duration;

const MAX_PAGE: usize = 2 * 1024 * 1024;

/// Shared HTTP client for websites and sending APIs (Windows SChannel TLS, no extra build tools).
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36")
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_default()
}

async fn fetch_html(http: &reqwest::Client, url: &str) -> Option<String> {
    let mut r = tokio::time::timeout(Duration::from_secs(10), http.get(url).send()).await.ok()?.ok()?;
    let html_like = match r.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        Some(t) => t.contains("html"),
        None => true, // no Content-Type: try it anyway
    };
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

/// Adds what the site shows to `c`. Gives up silently: a dead site must never stop a scan.
pub async fn enrich(http: &reqwest::Client, c: &mut Contacts) {
    let site = c.website.clone();
    let Some(home) = fetch_html(http, &site).await else { return };
    extract_contacts(&home, c);
    for page in contact_pages(&home, &site) {
        if !c.needs_more() && !c.person.is_empty() {
            break;
        }
        if let Some(html) = fetch_html(http, &page).await {
            extract_contacts(&html, c);
        }
    }
}

/// Downloads a small picture (≤ 3 MB, image/* only) and returns it as a data: URL for the UI.
pub async fn image_data_url(url: &str) -> crate::db::Res<String> {
    const MAX: usize = 3 * 1024 * 1024;
    let r = client().get(url).send().await.map_err(|e| format!("عکس دریافت نشد: {e}"))?;
    let kind = r
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !r.status().is_success() || !kind.starts_with("image/") {
        return Ok(String::new());
    }
    let body = r.bytes().await.map_err(|e| format!("عکس دریافت نشد: {e}"))?;
    if body.len() > MAX {
        return Ok(String::new());
    }
    Ok(format!("data:{};base64,{}", kind.split(';').next().unwrap_or("image/jpeg"), b64_std(&body)))
}

fn b64_std(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn std_base64() {
        assert_eq!(super::b64_std(b"Man"), "TWFu");
        assert_eq!(super::b64_std(b"Ma"), "TWE=");
        assert_eq!(super::b64_std(b"M"), "TQ==");
    }
}
