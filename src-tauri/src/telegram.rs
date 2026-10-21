//! Public Telegram channels and groups of businesses, found by topic. Reads only the public page
//! (t.me/<name>): title, description, subscriber / member count and the contacts written in the
//! description — phones, e-mails, links and the admin's @id (saved as a t.me link). Personal
//! accounts are skipped; members of anything are never read.
use crate::db::{Place, Res};
use crate::logic::{emails_in_text, host_path, links_in_text, mentions_in_text, meta_content, phones_in_text, Contacts};
use crate::scanner::{goto, js, script};
use crate::websearch::Hit;
use chromiumoxide::Page;
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

const RESERVED: &[&str] = &[
    "s",
    "joinchat",
    "c",
    "share",
    "addstickers",
    "addemoji",
    "proxy",
    "socks",
    "iv",
    "login",
    "contact",
    "setlanguage",
    "boost",
    "addlist",
    "giftcode",
    "invoice",
    "bg",
];

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

/// "12 345 subscribers" → (12345, false); "1 204 members, 30 online" → (1204, true) for a group;
/// a person's page says neither → None.
pub fn audience(html: &str) -> Option<(i64, bool)> {
    let extra = html.split("tgme_page_extra\">").nth(1)?.split('<').next()?.to_lowercase();
    let group = extra.contains("member");
    if !group && !extra.contains("subscriber") {
        return None;
    }
    let first = extra.split(',').next().unwrap_or("");
    let digits: String = first.chars().filter(char::is_ascii_digit).collect();
    Some((digits.parse().unwrap_or(0), group))
}

pub type Found = (Place, Contacts, Vec<(&'static str, String)>);

// ---------------- Telegram's own search (signed-in session) ----------------

#[derive(Deserialize, Default)]
#[serde(default)]
struct Rows {
    /// "login" | "ready" | "" (still loading)
    state: String,
    /// (title, subtitle) of the global-search rows
    rows: Vec<(String, String)>,
}

/// Telegram Web (K) with the user's own session: types the query into the search box and lists the rows.
const SEARCH_JS: &str = r#"
const t = el => (el && el.innerText || '').replace(/\s+/g, ' ').trim();
const q = %Q%;
if (document.querySelector('.page-sign, #auth-pages, .auth-image')) return { state: 'login', rows: [] };
const input = document.querySelector('.input-search input, input.input-search-input');
if (!input) return { state: '', rows: [] };
if (input.value !== q) { input.focus(); input.value = q; input.dispatchEvent(new Event('input', { bubbles: true })); return { state: '', rows: [] }; }
const rows = [...document.querySelectorAll('.search-group .chatlist-chat, .search-group a.chatlist-chat')]
  .map(r => [t(r.querySelector('.peer-title')), t(r.querySelector('.dialog-subtitle, .user-last-message, .row-subtitle'))]);
return { state: 'ready', rows };
"#;

/// «@name, 12.5K subscribers» / «@name, 1 204 members» → `name` of a public channel or group; a person's row
/// («last seen recently», no member count) and private chats give None (pure).
pub fn channel_from_row(subtitle: &str) -> Option<String> {
    let low = subtitle.to_lowercase();
    if !["subscriber", "member", "مشترک", "عضو"].iter().any(|w| low.contains(w)) {
        return None;
    }
    crate::logic::mentions_in_text(subtitle).into_iter().find(|m| m.len() >= 4)
}

/// Searches Telegram itself for public channels and groups on `query`. Needs the Telegram sign-in saved by
/// «ورود به تلگرام»; Err says so (the caller then falls back to the web search). Reads only the search rows —
/// never members, chats or messages.
pub async fn search(page: &Page, query: &str, token: &CancellationToken) -> Res<Vec<Hit>> {
    goto(page, "https://web.telegram.org/k/").await;
    let body = script("telegramSearchJs", SEARCH_JS).replace("%Q%", &serde_json::to_string(query.trim()).unwrap_or_default());
    let t = Instant::now();
    let mut quiet_since: Option<Instant> = None;
    let mut last = 0;
    let rows = loop {
        if token.is_cancelled() || t.elapsed() > Duration::from_secs(40) {
            return Err("نتیجه‌ای از جستجوی تلگرام نیامد.".into());
        }
        let r: Rows = js(page, &body).await.unwrap_or_default();
        if r.state == "login" {
            return Err("وارد تلگرام نشده‌اید؛ از «ورود به تلگرام» وارد شوید تا جستجوی بومی کار کند.".into());
        }
        if r.state == "ready" && !r.rows.is_empty() {
            // the list grows for a moment after the first rows appear
            if r.rows.len() != last {
                (last, quiet_since) = (r.rows.len(), Some(Instant::now()));
            } else if quiet_since.is_some_and(|q| q.elapsed() > Duration::from_millis(1800)) {
                break r.rows;
            }
        }
        sleep(Duration::from_millis(400)).await;
    };
    let mut hits: Vec<Hit> = vec![];
    for (title, sub) in rows {
        if let Some(name) = channel_from_row(&sub) {
            if !hits.iter().any(|h| h.url.ends_with(&format!("/{name}"))) {
                hits.push(Hit { url: format!("https://t.me/{name}"), title, snippet: sub });
            }
        }
    }
    Ok(hits)
}

pub async fn read(http: &reqwest::Client, hit: &Hit, mut place: Place) -> Res<Option<Found>> {
    let Some(handle) = handle_of(&hit.url) else { return Ok(None) };
    let resp = http.get(format!("{}/{handle}", base())).send().await.map_err(|e| format!("@{handle}: {e}"))?;
    let html = resp.text().await.map_err(|e| format!("@{handle}: {e}"))?;
    let Some((count, group)) = audience(&html) else {
        return Ok(None); // a person: not a business channel / group
    };
    let title = meta_content(&html, "og:title").unwrap_or_default();
    let desc = meta_content(&html, "og:description").unwrap_or_default();
    let mut c = Contacts::default();
    // the admin / order / support id written in the description is the real way to reach them
    for m in mentions_in_text(&desc).into_iter().filter(|m| !m.eq_ignore_ascii_case(&handle)) {
        c.add_link(&format!("https://t.me/{m}"));
    }
    phones_in_text(&desc).iter().for_each(|p| c.add_phone(p));
    emails_in_text(&desc).iter().for_each(|e| c.add_email(e));
    links_in_text(&desc).iter().for_each(|l| c.add_link(l));
    place.maps_url = format!("https://t.me/{handle}"); // the channel / group itself, one click away
    place.name = if title.is_empty() { format!("@{handle}") } else { title };
    let kind = if group { "عضو گروه" } else { "عضو کانال" };
    Ok(Some((place, c, vec![("telegram", format!("{count} {kind}"))])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_channels_and_groups_come_from_search_rows() {
        assert_eq!(channel_from_row("@lamiz_cafe, 12.5K subscribers").as_deref(), Some("lamiz_cafe"));
        assert_eq!(channel_from_row("@lamiz_shop, 1 204 members").as_deref(), Some("lamiz_shop"));
        assert_eq!(channel_from_row("@lamiz_shop، ۱۲۰۰ مشترک").as_deref(), Some("lamiz_shop"));
        assert_eq!(channel_from_row("@ali_amini, last seen recently"), None, "a person");
        assert_eq!(channel_from_row("1 204 members"), None, "no public name");
        assert_eq!(channel_from_row(""), None);
    }

    #[test]
    fn handles_and_channel_pages() {
        assert_eq!(handle_of("https://t.me/lamiz_cafe").as_deref(), Some("lamiz_cafe"));
        assert_eq!(handle_of("https://t.me/s/lamiz_cafe/120").as_deref(), Some("lamiz_cafe"));
        assert_eq!(handle_of("https://telegram.me/LamizCafe?start=1").as_deref(), Some("LamizCafe"));
        assert_eq!(handle_of("https://t.me/joinchat/AAAA"), None);
        assert_eq!(handle_of("https://t.me/+AbCdEf"), None);
        assert_eq!(handle_of("https://t.me/abc"), None, "too short for a public name");
        assert_eq!(handle_of("https://example.com/lamiz"), None);
        assert_eq!(audience(r#"<div class="tgme_page_extra">9 428 096 subscribers</div>"#), Some((9_428_096, false)));
        assert_eq!(audience(r#"<div class="tgme_page_extra">1 204 members, 30 online</div>"#), Some((1204, true)));
        assert_eq!(audience(r#"<div class="tgme_page_extra">@lamiz</div>"#), None);
        assert_eq!(audience("<div>no extra</div>"), None);
    }
}
