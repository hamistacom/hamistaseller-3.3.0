//! Public Telegram channels of businesses, found by topic. Reads only the channel's own public page
//! (t.me/<name>): title, description, subscriber count and the contacts written in the description.
//! Groups, private chats and personal accounts are skipped; members of anything are never read.
use crate::db::{Place, Res};
use crate::logic::{emails_in_text, host_path, links_in_text, meta_content, phones_in_text, Contacts};
use crate::websearch::Hit;

const RESERVED: &[&str] = &["s", "joinchat", "c", "share", "addstickers", "addemoji", "proxy", "socks", "iv", "login", "contact", "setlanguage", "boost", "addlist", "giftcode", "invoice", "bg"];

/// `https://t.me/s/lamiz_cafe/120` → `lamiz_cafe`. None for invite links, posts-only links, etc.
pub fn handle_of(url: &str) -> Option<String> {
    let (host, path) = host_path(url)?;
    if !["t.me", "telegram.me", "telegram.dog"].contains(&host.as_str()) {
        return None;
    }
    let mut segs = path.split(['/', '?', '#']).filter(|s| !s.is_empty());
    let mut first = segs.next()?;
    if first == "s" {
        first = segs.next()?;
    }
    let ok = first.len() >= 4
        && first.len() <= 32
        && first.starts_with(|c: char| c.is_ascii_alphabetic())
        && first.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    (ok && !RESERVED.contains(&first.to_lowercase().as_str())).then(|| first.to_string())
}

fn base() -> String {
    crate::logic::test_seam("HAMISTA_TME_BASE").unwrap_or_else(|| "https://t.me".into())
}

/// "12 345 subscribers" → Some(12345); groups say "members", people say nothing.
pub fn subscribers(html: &str) -> Option<i64> {
    let extra = html.split("tgme_page_extra\">").nth(1)?.split('<').next()?;
    if !extra.to_lowercase().contains("subscriber") {
        return None;
    }
    let digits: String = extra.chars().filter(char::is_ascii_digit).collect();
    digits.parse().ok().or(Some(0))
}

pub type Found = (Place, Contacts, Vec<(&'static str, String)>);

pub async fn read(http: &reqwest::Client, hit: &Hit, mut place: Place) -> Res<Option<Found>> {
    let Some(handle) = handle_of(&hit.url) else { return Ok(None) };
    let resp = http
        .get(format!("{}/{handle}", base()))
        .send()
        .await
        .map_err(|e| format!("@{handle}: {e}"))?;
    let html = resp.text().await.map_err(|e| format!("@{handle}: {e}"))?;
    let Some(subs) = subscribers(&html) else {
        return Ok(None); // a group or a person: not a business channel
    };
    let title = meta_content(&html, "og:title").unwrap_or_default();
    let desc = meta_content(&html, "og:description").unwrap_or_default();
    let mut c = Contacts::default();
    c.add_link(&format!("https://t.me/{handle}"));
    phones_in_text(&desc).iter().for_each(|p| c.add_phone(p));
    emails_in_text(&desc).iter().for_each(|e| c.add_email(e));
    links_in_text(&desc).iter().for_each(|l| c.add_link(l));
    place.name = if title.is_empty() { format!("@{handle}") } else { title };
    Ok(Some((place, c, vec![("telegram", format!("{subs} عضو"))])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_and_channel_pages() {
        assert_eq!(handle_of("https://t.me/lamiz_cafe").as_deref(), Some("lamiz_cafe"));
        assert_eq!(handle_of("https://t.me/s/lamiz_cafe/120").as_deref(), Some("lamiz_cafe"));
        assert_eq!(handle_of("https://telegram.me/LamizCafe?start=1").as_deref(), Some("LamizCafe"));
        assert_eq!(handle_of("https://t.me/joinchat/AAAA"), None);
        assert_eq!(handle_of("https://t.me/+AbCdEf"), None);
        assert_eq!(handle_of("https://t.me/abc"), None, "too short for a public name");
        assert_eq!(handle_of("https://example.com/lamiz"), None);
        assert_eq!(subscribers(r#"<div class="tgme_page_extra">9 428 096 subscribers</div>"#), Some(9_428_096));
        assert_eq!(subscribers(r#"<div class="tgme_page_extra">1 204 members, 30 online</div>"#), None);
        assert_eq!(subscribers("<div>no extra</div>"), None);
    }
}
