//! Balad (balad.ir) business pages. Balad has no public API, so its public place pages
//! (`balad.ir/p/<name>-<area>_<category>-<id>`) are found through web search and read in the app's
//! headless browser: name, phone, address, coordinates and links shown on the page.
//! Page reading is generic (structured data → tel: links) and can be replaced remotely
//! (scenario key `baladJs`) if Balad changes its pages.
use crate::db::{Place, Res};
use crate::logic::{host_path, Contacts};
use crate::scanner::{goto, js, script};
use crate::websearch::Hit;
use tokio_util::sync::CancellationToken;
use chromiumoxide::Page;
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// `https://balad.ir/p/کافه-…_coffee-shop-4ctFksRaCpebRg` → `balad:4ctfksracpebrg`.
pub fn key_of(url: &str) -> Option<String> {
    let (host, path) = host_path(url)?;
    // tests serve the pages from a local mock (test builds only)
    let host_ok = host == "balad.ir" || crate::logic::test_seam("HAMISTA_SEARCH_BASE").is_some();
    let path = path.split(['?', '#']).next()?;
    let slug = path.strip_prefix("/p/")?.trim_end_matches('/');
    let id = slug.rsplit('-').next()?;
    (host_ok && id.len() >= 6 && id.chars().all(|c| c.is_ascii_alphanumeric())).then(|| format!("balad:{}", id.to_lowercase()))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Info {
    ready: bool,
    name: String,
    address: String,
    phones: Vec<String>,
    links: Vec<String>,
    lat: Option<f64>,
    lng: Option<f64>,
}

const READER_JS: &str = r#"
const t = s => String(s || '').replace(/\s+/g, ' ').trim();
const ld = [...document.querySelectorAll('script[type="application/ld+json"]')].flatMap(s => {
  try { const j = JSON.parse(s.textContent); return [].concat(j['@graph'] || j); } catch (e) { return []; }
});
const biz = ld.find(o => o && (o.telephone || o.address || o.geo)) || {};
const a = biz.address;
const addr = typeof a === 'string' ? a : a ? [a.streetAddress, a.addressLocality, a.addressRegion].filter(Boolean).join('، ') : '';
const phones = [...document.querySelectorAll('a[href^="tel:"]')].map(x => decodeURIComponent(x.getAttribute('href').slice(4)));
[].concat(biz.telephone || []).forEach(p => phones.push(String(p)));
// only links inside the page's main content, never Balad's own social accounts (header / footer)
const main = document.querySelector('main, article, [role="main"]') || document.body;
const links = [...main.querySelectorAll('a[href]')].filter(x => !x.closest('header, footer, nav')).map(x => x.href)
  .filter(h => /instagram\.com|t\.me\/|wa\.me|eitaa\.com|ble\.ir|rubika\.ir/.test(h) && !/balad/i.test(h));
[].concat(biz.sameAs || []).forEach(h => links.push(String(h)));
if (biz.url && !/balad\.ir/.test(biz.url)) links.push(String(biz.url));
const h1 = document.querySelector('h1');
return {
  ready: !!(h1 || biz.name),
  name: t(biz.name || (h1 && h1.innerText)),
  address: t(addr || (document.querySelector('address') || {}).innerText),
  phones, links,
  lat: biz.geo ? Number(biz.geo.latitude) || null : null,
  lng: biz.geo ? Number(biz.geo.longitude) || null : null,
};
"#;

// ---------------- Balad's own search ----------------

/// Balad's search page for `q`; the scenario key `baladSearchUrl` can replace it if Balad moves it.
fn search_url(q: &str) -> String {
    let base = crate::logic::test_seam("HAMISTA_BALAD_BASE")
        .map(|b| format!("{b}/search?q={{q}}"))
        .unwrap_or_else(|| script("baladSearchUrl", "https://balad.ir/search?q={q}"));
    base.replace("{q}", &crate::logic::pct_encode(q))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Results {
    anchors: Vec<(String, String)>, // href, visible text
    html: String,                   // the page source (Balad also embeds its results as JSON)
    ready: bool,
}

const SEARCH_JS: &str = r#"
const a = [...document.querySelectorAll('a[href*="/p/"]')].map(x => [x.href, (x.innerText || '').replace(/\s+/g, ' ').trim()]);
const html = document.documentElement.innerHTML.slice(0, 1500000);
window.scrollTo(0, document.body.scrollHeight);
return { anchors: a, html, ready: document.readyState === 'complete' && (a.length > 0 || html.includes('/p/')) };
"#;

/// Every `/p/<slug>` path written in a page source (anchors and embedded JSON alike), as full URLs (pure).
pub fn place_links(html: &str, origin: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let unescaped = html.replace("\\/", "/"); // JSON writes «/» as «\/»
    let mut rest = unescaped.as_str();
    while let Some(i) = rest.find("/p/") {
        let tail = &rest[i..];
        let end = tail.find(['"', '\'', ' ', '<', '>', '\\', ')', '\n', '?', '#']).unwrap_or(tail.len());
        let url = format!("{origin}{}", &tail[..end]);
        if key_of(&url).is_some() && !out.contains(&url) {
            out.push(url);
        }
        rest = &tail[3..];
    }
    out
}

/// Searches Balad itself in the app's browser (no search engine): the page is scrolled until `limit` places
/// are listed or nothing new appears. Empty when Balad shows no results or has changed its page.
pub async fn search(page: &Page, query: &str, limit: usize, token: &CancellationToken) -> Res<Vec<Hit>> {
    goto(page, &search_url(query)).await;
    let t = Instant::now();
    let mut hits: Vec<Hit> = vec![];
    let mut stale = 0;
    while t.elapsed() < Duration::from_secs(25) && hits.len() < limit && stale < 3 && !token.is_cancelled() {
        let r: Results = js(page, &script("baladSearchJs", SEARCH_JS)).await.unwrap_or_default();
        let before = hits.len();
        let origin = crate::logic::test_seam("HAMISTA_BALAD_BASE").unwrap_or_else(|| "https://balad.ir".into());
        let found = r.anchors.iter().map(|(u, title)| (u.clone(), title.clone())).chain(place_links(&r.html, &origin).into_iter().map(|u| (u, String::new())));
        for (url, title) in found {
            if let Some(k) = key_of(&url) {
                if !hits.iter().any(|h| key_of(&h.url).as_deref() == Some(k.as_str())) {
                    hits.push(Hit { url, title, snippet: String::new() });
                }
            }
        }
        stale = if hits.len() == before && r.ready { stale + 1 } else { 0 };
        sleep(Duration::from_millis(900)).await;
    }
    hits.truncate(limit);
    Ok(hits)
}

pub type Found = (Place, Contacts, Vec<(&'static str, String)>);

pub async fn read(page: &Page, url: &str, mut place: Place) -> Res<Option<Found>> {
    goto(page, url).await;
    let t = Instant::now();
    let info = loop {
        let i: Info = js(page, &script("baladJs", READER_JS)).await.unwrap_or_default();
        // phone buttons can render a moment after the title
        if i.ready && (!i.phones.is_empty() || t.elapsed() > Duration::from_secs(4)) {
            break i;
        }
        if t.elapsed() > Duration::from_secs(12) {
            return Err(format!("صفحه بلد بارگذاری نشد: {url}"));
        }
        sleep(Duration::from_millis(300)).await;
    };
    let mut c = Contacts::default();
    info.phones.iter().for_each(|p| c.add_phone(p));
    if !c.mobiles.is_empty() || !c.landlines.is_empty() {
        // a Balad listing is kept for its phone; links alone are not enough to be sure they are the business's
        info.links.iter().filter(|l| !l.to_lowercase().contains("balad")).for_each(|l| c.add_link(l));
    }
    place.name = info.name;
    place.address = info.address;
    (place.lat, place.lng) = (info.lat, info.lng);
    place.maps_url = url.to_string();
    Ok(Some((place, c, vec![])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_found_in_the_page_source() {
        let html = r#"<a href="/p/%DA%A9%D8%A7%D9%81%D9%87-tehran_coffee-shop-4ctFksRaCpebRg">x</a>
            {"url":"\/p\/%D8%B3%D9%87-area_bakery-9AbCdEfGhIjKlM"} <a href="/p/%DA%A9%D8%A7%D9%81%D9%87-tehran_coffee-shop-4ctFksRaCpebRg?x=1">dup</a>
            <a href="/blog/cafe-4ctFksRaCpebRg">blog</a> /p/ <a href="/p/short-ab">short</a>"#;
        let l = place_links(html, "https://balad.ir");
        assert_eq!(l.len(), 2, "{l:?}");
        assert!(l.iter().all(|u| u.starts_with("https://balad.ir/p/")));
        assert!(search_url("کافه").starts_with("https://balad.ir/search?q=%DA%A9"));
    }

    #[test]
    fn place_keys() {
        assert_eq!(
            key_of("https://balad.ir/p/%DA%A9%D8%A7%D9%81%D9%87-tehran-nei-pasdaran_coffee-shop-4ctFksRaCpebRg").as_deref(),
            Some("balad:4ctfksracpebrg")
        );
        assert_eq!(key_of("https://balad.ir/blog/best-cafe-tehran/"), None);
        assert_eq!(key_of("https://example.com/p/x-4ctFksRaCpebRg"), None);
    }
}
