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

const AVATAR_EXT: [(&str, &str); 4] = [("image/jpeg", "jpg"), ("image/png", "png"), ("image/webp", "webp"), ("image/gif", "gif")];

/// Where a business picture lives on disk: `cache/avatars/<sha256 of its address>.<ext>`.
fn avatar_file(dir: &std::path::Path, url: &str, ext: &str) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    let hash: String = Sha256::digest(url.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    dir.join(format!("{hash}.{ext}"))
}

/// A picture saved earlier by `avatar_data_url`, as a data: URL (None = not cached yet).
fn avatar_cached(dir: &std::path::Path, url: &str) -> Option<String> {
    AVATAR_EXT.iter().find_map(|(mime, ext)| {
        let bytes = std::fs::read(avatar_file(dir, url, ext)).ok()?;
        Some(format!("data:{mime};base64,{}", b64_std(&bytes)))
    })
}

/// A business picture for the card. The database keeps only the address (`places.photo`); the picture
/// itself is downloaded once into `cache/avatars` and served from there afterwards, so it also works
/// offline and never grows `hamista.db` or its daily backup. A value that already is a `data:image`
/// URL (older data) is returned as it is.
pub async fn avatar_data_url(dir: &std::path::Path, url: &str) -> crate::db::Res<String> {
    if url.starts_with("data:image") {
        return Ok(url.to_string());
    }
    if let Some(hit) = avatar_cached(dir, url) {
        return Ok(hit);
    }
    let data = image_data_url(url).await?;
    // "data:<mime>;base64,<body>" → bytes on disk
    if let Some((head, body)) = data.split_once(";base64,") {
        let mime = head.trim_start_matches("data:");
        if let (Some((_, ext)), Some(bytes)) = (AVATAR_EXT.iter().find(|(m, _)| *m == mime), b64_decode(body)) {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(avatar_file(dir, url, ext), bytes);
        }
    }
    Ok(data)
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let raw: Vec<u8> = s.bytes().filter(|c| *c != b'=').collect();
    let mut out = Vec::with_capacity(raw.len() * 3 / 4);
    for chunk in raw.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= val(*c)? << (18 - 6 * i);
        }
        let take = match chunk.len() {
            4 => 3,
            3 => 2,
            2 => 1,
            _ => return None,
        };
        out.extend_from_slice(&n.to_be_bytes()[1..1 + take]);
    }
    Some(out)
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
    fn base64_roundtrip_and_avatar_cache() {
        for sample in [&b"M"[..], b"Ma", b"Man", b"\x00\xff\x10 image bytes"] {
            assert_eq!(super::b64_decode(&super::b64_std(sample)).unwrap(), sample);
        }
        let dir = std::env::temp_dir().join(format!("hs-avatars-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let url = "https://lh5.googleusercontent.com/p/x=w408";
        assert!(super::avatar_cached(&dir, url).is_none(), "nothing cached yet");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(super::avatar_file(&dir, url, "png"), b"\x89PNG").unwrap();
        let hit = super::avatar_cached(&dir, url).unwrap();
        assert!(hit.starts_with("data:image/png;base64,"), "{hit}");
        assert!(super::avatar_cached(&dir, "https://other/x.jpg").is_none(), "keyed by address");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn std_base64() {
        assert_eq!(super::b64_std(b"Man"), "TWFu");
        assert_eq!(super::b64_std(b"Ma"), "TWE=");
        assert_eq!(super::b64_std(b"M"), "TQ==");
    }
}
