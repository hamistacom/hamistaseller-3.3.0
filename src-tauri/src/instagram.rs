//! Instagram BUSINESS pages, found by keyword or hashtag + city through public web search (Phase 3.3).
//! * With Meta Graph API connected, reads bio, website and follower count from `business_discovery`.
//! * Without Graph API, reads the search snippet AND (when a browser tab is available) visits the profile
//!   page in the persistent browser session to read the full biography, external website link and follower count.
//! * Never reads followers, following, likers or commenters of any page.

use crate::db::{Db, Place, Res};
use crate::logic::{emails_in_text, host_path, links_in_text, phones_in_text, Contacts};
use crate::scanner::{goto, js};
use crate::websearch::Hit;
use chromiumoxide::Page;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

const RESERVED: &[&str] = &[
    "p",
    "reel",
    "reels",
    "explore",
    "accounts",
    "stories",
    "tv",
    "direct",
    "about",
    "legal",
    "developer",
    "web",
    "share",
    "challenge",
    "privacy",
    "help",
    "directory",
];

/// `https://www.instagram.com/cafe.lamiz/` → `cafe.lamiz`; posts / explore pages → None.
pub fn handle_of(url: &str) -> Option<String> {
    let (host, path) = host_path(url)?;
    if host != "instagram.com" && host != "m.instagram.com" {
        return None;
    }
    let first = path.split(['/', '?', '#']).find(|s| !s.is_empty())?;
    let ok = (2..=30).contains(&first.len()) && first.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') && !first.starts_with('.');
    (ok && !RESERVED.contains(&first.to_lowercase().as_str())).then(|| first.to_string())
}

/// "Café Lamiz (@cafe.lamiz) • Instagram photos and videos" → "Café Lamiz".
pub fn name_from_title(title: &str) -> String {
    let t = title.split(" (@").next().unwrap_or(title);
    let t = t.split(" • ").next().unwrap_or(t);
    t.split(" | ").next().unwrap_or(t).trim().to_string()
}

/// The user's own Instagram Business account for the official Graph API (optional).
pub struct Api {
    user_id: String,
    token: String,
}

impl Api {
    pub fn from_config(db: &Db) -> Option<Api> {
        let user_id = crate::sources::config_value(db, "instagram", "igUserId");
        let token = crate::sources::config_value(db, "instagram", "accessToken");
        (!user_id.is_empty() && !token.is_empty()).then_some(Api { user_id, token })
    }

    fn base() -> String {
        crate::logic::test_seam("HAMISTA_GRAPH_BASE").unwrap_or_else(|| "https://graph.facebook.com".into())
    }

    /// Public profile of a business/creator account; None for personal accounts or unknown names.
    async fn discover(&self, http: &reqwest::Client, handle: &str) -> Res<Option<Value>> {
        let fields = format!("business_discovery.username({handle}){{username,name,biography,website,followers_count}}");
        let resp = http
            .get(format!("{}/v21.0/{}", Self::base(), self.user_id))
            .query(&[("fields", fields.as_str()), ("access_token", self.token.as_str())])
            .send()
            .await
            .map_err(|e| format!("Instagram API: {}", e.without_url()))?;
        let ok = resp.status().is_success();
        let v: Value = resp.json().await.unwrap_or(Value::Null);
        if ok {
            return Ok(Some(v["business_discovery"].clone()).filter(|b| b.is_object()));
        }
        match v["error"]["code"].as_i64() {
            Some(100) | Some(110) => Ok(None),
            Some(190) => Err("توکن Instagram API منقضی یا نامعتبر است؛ در تنظیمات ← ماژول‌ها تازه کنید.".into()),
            Some(4) | Some(17) | Some(32) | Some(613) => Err("سقف ساعتی Instagram API پر شده است؛ کمی بعد ادامه دهید.".into()),
            _ => Err(format!("خطای Instagram API: {}", v["error"]["message"].as_str().unwrap_or("نامشخص"))),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ProfileDom {
    name: String,
    bio: String,
    website: String,
    meta_desc: String,
}

const PROFILE_JS: &str = r#"
const clean = s => String(s || '').replace(/\s+/g, ' ').trim();
const h1 = document.querySelector('header h1, header h2');
const bioEl = document.querySelector('header section div.-vDIg, header section h1 ~ div, header section span');
const headerText = document.querySelector('header') ? document.querySelector('header').innerText : '';
const extLink = [...document.querySelectorAll('header a[href^="http"]')].map(a => a.href).find(h => !h.includes('instagram.com') && !h.includes('facebook.com')) || '';
const meta = document.querySelector('meta[property="og:description"], meta[name="description"]');
return {
  name: clean(h1 && h1.innerText),
  bio: clean(bioEl ? bioEl.innerText : headerText),
  website: extLink,
  meta_desc: meta ? (meta.getAttribute('content') || '') : '',
};
"#;

pub type Found = (Place, Contacts, Vec<(&'static str, String)>);

pub async fn read(http: &reqwest::Client, api: Option<&Api>, hit: &Hit, place: Place) -> Res<Option<Found>> {
    read_with_page(http, api, None, hit, place).await
}

pub async fn read_with_page(http: &reqwest::Client, api: Option<&Api>, page: Option<&Page>, hit: &Hit, mut place: Place) -> Res<Option<Found>> {
    let Some(handle) = handle_of(&hit.url) else { return Ok(None) };
    let (mut name, mut bio, mut website, mut followers) = match api {
        Some(api) => match api.discover(http, &handle).await? {
            Some(b) => (
                b["name"].as_str().unwrap_or_default().to_string(),
                b["biography"].as_str().unwrap_or_default().to_string(),
                b["website"].as_str().unwrap_or_default().to_string(),
                b["followers_count"].as_i64(),
            ),
            None => return Ok(None),
        },
        None => (name_from_title(&hit.title), hit.snippet.clone(), String::new(), None),
    };

    let mut c = Contacts::default();
    phones_in_text(&bio).iter().for_each(|p| c.add_phone(p));
    emails_in_text(&bio).iter().for_each(|e| c.add_email(e));
    links_in_text(&bio).iter().for_each(|l| c.add_link(l));
    if !website.is_empty() {
        c.add_link(&website);
    }

    // If no Graph API is set and the search snippet didn't contain a phone/email/website,
    // visit the profile page in the browser session (in case the user logged in to Instagram)
    if api.is_none() && c.mobiles.is_empty() && c.landlines.is_empty() && c.emails.is_empty() && c.website.is_empty() {
        if let Some(p) = page {
            if crate::logic::test_seam("HAMISTA_SEARCH_BASE").is_none() {
                goto(p, &format!("https://www.instagram.com/{handle}/")).await;
                sleep(Duration::from_millis(1800)).await;
                if let Ok(dom) = js::<ProfileDom>(p, PROFILE_JS).await {
                    let combined = format!("{} {}", dom.bio, dom.meta_desc);
                    phones_in_text(&combined).iter().for_each(|ph| c.add_phone(ph));
                    emails_in_text(&combined).iter().for_each(|em| c.add_email(em));
                    links_in_text(&combined).iter().for_each(|ln| c.add_link(ln));
                    if !dom.website.is_empty() {
                        c.add_link(&dom.website);
                        website = dom.website;
                    }
                    if name.is_empty() && !dom.name.is_empty() {
                        name = dom.name;
                    }
                    if !dom.bio.is_empty() {
                        bio = format!("{bio} {}", dom.bio);
                    }
                    if followers.is_none() {
                        followers = crate::brand::parse_counts(&dom.meta_desc).0;
                    }
                }
            }
        }
    }

    let kw_in_bio = !place.keyword.trim().is_empty() && crate::logic::name_key(&format!("{name} {bio}")).contains(&crate::logic::name_key(&place.keyword));
    let has_contact = !(c.mobiles.is_empty() && c.landlines.is_empty() && c.emails.is_empty() && c.website.is_empty());
    let business = api.is_some() || has_contact || kw_in_bio;
    if !business {
        return Ok(None);
    }

    // the admin / shop id written in the bio («سفارش در دایرکت @shop_admin», «t.me/…»)
    for m in crate::logic::mentions_in_text(&bio).into_iter().filter(|m| !m.eq_ignore_ascii_case(&handle)) {
        c.add_link(&format!("https://instagram.com/{m}"));
    }
    c.add_link(&format!("https://instagram.com/{handle}"));
    place.maps_url = format!("https://instagram.com/{handle}"); // the page itself, one click away
    place.name = if name.is_empty() { format!("@{handle}") } else { name };
    let labels = followers.map(|f| vec![("instagram", format!("{f} دنبال‌کننده"))]).unwrap_or_default();
    Ok(Some((place, c, labels)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_and_titles() {
        assert_eq!(handle_of("https://www.instagram.com/cafe.lamiz/").as_deref(), Some("cafe.lamiz"));
        assert_eq!(handle_of("https://instagram.com/cafe_lamiz?igsh=x").as_deref(), Some("cafe_lamiz"));
        assert_eq!(handle_of("https://www.instagram.com/p/Cx12/"), None);
        assert_eq!(handle_of("https://www.instagram.com/explore/tags/cafe/"), None);
        assert_eq!(handle_of("https://example.com/cafe"), None);
        assert_eq!(name_from_title("Café Lamiz (@cafe.lamiz) • Instagram photos and videos"), "Café Lamiz");
        assert_eq!(name_from_title("کافه لمیز | Instagram"), "کافه لمیز");
    }
}
