//! «دریافت لیست محصول»: the product list (name, price, photo, link, seller, brand, rating, date …)
//! of any shop URL, with a short analysis and an Excel file.
//!
//! Scope is strict and comes from the link itself:
//! * a Basalam booth (`basalam.com/<booth>`) → only that booth's products, never the rest of Basalam;
//! * a site's home page → the whole site (home + its category / listing pages, up to `pages`);
//! * any other page (category, search, product) → that page and its «next page» links, same site only;
//! * a public Telegram channel (`t.me/<name>`) → its posts: text, price written in the post, photo,
//!   post link, the product link in the post and the post date.
//!
//! Pages are read in the app's own browser (JS-rendered shops, the user's proxy/VPN, a visible window
//! when a site asks «I'm not a robot» — skippable). The page readers are the market engine's own
//! (`market::PAGE_JS` + `offers_in`): `__NEXT_DATA__`, JSON-LD and DOM price cards.
use crate::db::{err, Res};
use crate::logic::{ascii_digits, html_unescape};
use crate::market::{offers_in, Offer, PageData, PAGE_JS};
use crate::scanner::{self, Emit, ScanEvent, Settings};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ListReq {
    pub url: String,
    /// Most products to collect (default 300, at most 3000).
    pub limit: i64,
    /// Most pages to open (default 10 for a whole site, 5 otherwise; at most 60).
    pub pages: i64,
    /// Also open each product page for availability, brand and description (slower).
    pub details: bool,
    /// Optional: keep only products whose name has these words.
    pub query: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Item {
    pub title: String,
    pub price: i64, // Toman, 0 = not stated
    pub image: String,
    pub url: String,
    pub seller: String,
    pub seller_url: String,
    pub brand: String,
    pub rating: Option<f64>,
    pub reviews: i64,
    pub sales: i64,
    /// Listing / post date (ISO), when the source states one.
    pub date: String,
    pub location: String,
    pub availability: String,
    pub description: String,
    /// Telegram: views of the post · the product's own link written in the post.
    pub views: i64,
    pub link: String,
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Stats {
    pub count: usize,
    pub priced: usize,
    pub min: i64,
    pub median: i64,
    pub max: i64,
    pub avg: i64,
    pub sellers: Vec<(String, usize)>,
    pub brands: Vec<(String, usize)>,
    pub newest: String,
    pub oldest: String,
    /// Posts / listings per week between the oldest and newest date (Telegram channels).
    pub per_week: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ListReport {
    pub url: String,
    /// What was read, in words («فقط غرفه asal_darashkooh در باسلام» …).
    pub scope: String,
    pub items: Vec<Item>,
    pub pages: usize,
    pub stats: Stats,
    pub notes: Vec<String>,
}

// ---------------- scope ----------------

#[derive(Debug, PartialEq, Clone)]
pub enum Scope {
    Telegram(String),
    Booth { vendor: String },
    Site { host: String, whole: bool },
    /// Several shops at once (the «وب» source of product analysis): each one's own links only.
    Sites(Vec<String>),
}

pub fn normalize(url: &str) -> String {
    let u = url.trim();
    if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else {
        format!("https://{}", u.trim_start_matches('/'))
    }
}

fn path_of(url: &str) -> String {
    let rest = url.split("//").nth(1).unwrap_or(url);
    rest.find('/').map_or("/".to_string(), |i| rest[i..].split(['?', '#']).next().unwrap_or("/").to_string())
}

pub fn scope_of(url: &str) -> Res<Scope> {
    let url = normalize(url);
    let host = crate::links::host(&url);
    if host.is_empty() || !host.contains('.') {
        return Err("لینک معتبر نیست؛ مثلاً https://basalam.com/نام-غرفه یا آدرس سایت فروشگاه را وارد کنید.".into());
    }
    if ["t.me", "telegram.me"].contains(&host.as_str()) {
        return crate::telegram::handle_of(&url)
            .map(Scope::Telegram)
            .ok_or_else(|| "این لینک کانال عمومی تلگرام نیست (مثال: https://t.me/name).".into());
    }
    let path = path_of(&url);
    let first = path.split('/').find(|s| !s.is_empty()).unwrap_or("");
    if host == "basalam.com" && !first.is_empty() && !crate::links::RESERVED_BASALAM.contains(&first) {
        return Ok(Scope::Booth { vendor: first.to_string() });
    }
    Ok(Scope::Site {
        host,
        whole: first.is_empty(),
    })
}

impl Scope {
    fn describe(&self) -> String {
        match self {
            Scope::Telegram(h) => format!("پست‌های کانال عمومی تلگرام @{h}"),
            Scope::Booth { vendor } => format!("فقط محصولات غرفه «{vendor}» در باسلام (نه کل باسلام)"),
            Scope::Site { host, whole: true } => format!("کل سایت {host} (صفحه اصلی و صفحه‌های دسته‌بندی)"),
            Scope::Site { host, whole: false } => format!("همین صفحه از {host} و صفحه‌های بعدی آن"),
            Scope::Sites(h) => format!("{} سایت: {}", h.len(), h.join("، ")),
        }
    }

    /// Is this product inside what the user asked for? Strict: a booth keeps only its own product
    /// pages; a site keeps only its own links (other shops' ads and widgets are dropped).
    pub fn keeps(&self, url: &str, seller_url: &str) -> bool {
        match self {
            Scope::Telegram(_) => true,
            Scope::Booth { vendor } => {
                let own = |u: &str| crate::links::host(u) == "basalam.com" && path_of(u).to_lowercase().starts_with(&format!("/{}/", vendor.to_lowercase()));
                own(url) || (!url.is_empty() && path_of(seller_url).trim_end_matches('/').eq_ignore_ascii_case(&format!("/{vendor}")))
            }
            Scope::Site { host, .. } => same_host(url, host),
            Scope::Sites(hosts) => hosts.iter().any(|h| same_host(url, h)),
        }
    }
}

fn same_host(url: &str, host: &str) -> bool {
    let h = crate::links::host(url);
    !h.is_empty() && (h == host || h.ends_with(&format!(".{host}")))
}

// ---------------- prices written in text ----------------

/// The price written in a post / description: «۲۵۰,۰۰۰ تومان», «قیمت: ۱۲۰ هزار تومان», «۲ میلیون»,
/// «۱,۵۰۰,۰۰۰ ریال» (÷10). 0 when none.
pub fn price_from_text(text: &str) -> i64 {
    let t = ascii_digits(text).replace('٬', ",").replace('٫', ".");
    let chars: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || ((chars[i] == ',' || chars[i] == '.') && chars.get(i + 1).is_some_and(char::is_ascii_digit))) {
            i += 1;
        }
        let raw: String = chars[start..i].iter().collect();
        let n: f64 = if raw.matches('.').count() == 1 && !raw.contains(',') && raw.split('.').nth(1).is_some_and(|d| d.len() <= 2) {
            raw.parse().unwrap_or(0.0) // «۲.۵ میلیون»
        } else {
            raw.replace([',', '.'], "").parse().unwrap_or(0.0)
        };
        let after: String = chars[i..].iter().take(16).collect::<String>();
        let after = after.trim_start();
        let before: String = chars[start.saturating_sub(14)..start].iter().collect();
        let (mul, rest) = if let Some(r) = after.strip_prefix("هزار") {
            (1_000.0, r.trim_start())
        } else if let Some(r) = after.strip_prefix("میلیون") {
            (1_000_000.0, r.trim_start())
        } else {
            (1.0, after)
        };
        let word = |w: &str| rest.starts_with(w) && !rest[w.len()..].starts_with(|c: char| c.is_alphabetic());
        let v = if rest.starts_with("ریال") || rest.starts_with("ريال") {
            n * mul / 10.0
        } else if rest.starts_with("تومان") || rest.starts_with("تومن") || word("ت") || mul > 1.0 || before.contains("قیمت") {
            n * mul
        } else {
            0.0
        };
        if (1_000.0..100_000_000_000.0).contains(&v) {
            return v as i64;
        }
    }
    0
}

// ---------------- Telegram channel posts ----------------

fn between<'a>(s: &'a str, from: &str, to: &str) -> Option<&'a str> {
    let a = s.find(from)? + from.len();
    let b = s[a..].find(to)? + a;
    Some(&s[a..b])
}

fn strip_tags(html: &str) -> String {
    let html = html.replace("<br/>", "\n").replace("<br>", "\n").replace("<br />", "\n");
    let mut out = String::new();
    let mut tag = false;
    for c in html.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    html_unescape(out.trim())
}

fn views(s: &str) -> i64 {
    let s = s.trim().to_uppercase();
    let (n, mul) = match s.chars().last() {
        Some('K') => (&s[..s.len() - 1], 1_000.0),
        Some('M') => (&s[..s.len() - 1], 1_000_000.0),
        _ => (s.as_str(), 1.0),
    };
    (n.parse::<f64>().unwrap_or(0.0) * mul) as i64
}

/// The posts of one `t.me/s/<name>` page (oldest first, as Telegram serves them) and the smallest
/// post id (for the next, older page).
pub fn telegram_posts(html: &str, handle: &str) -> (Vec<Item>, Option<i64>) {
    let channel = crate::logic::meta_content(html, "og:title").unwrap_or_else(|| format!("@{handle}"));
    let mut out = vec![];
    let mut min_id: Option<i64> = None;
    for chunk in html.split("tgme_widget_message_wrap").skip(1) {
        let Some(post) = between(chunk, "data-post=\"", "\"") else { continue };
        let id: i64 = post.rsplit('/').next().and_then(|x| x.parse().ok()).unwrap_or(0);
        if id > 0 {
            min_id = Some(min_id.map_or(id, |m| m.min(id)));
        }
        let text_html = chunk
            .find("js-message_text")
            .and_then(|i| chunk[i..].find('>').map(|g| &chunk[i + g + 1..]))
            .and_then(|rest| rest.find("</div>").map(|e| &rest[..e]))
            .unwrap_or("");
        let text = strip_tags(text_html);
        let image = between(chunk, "tgme_widget_message_photo_wrap", "')")
            .and_then(|s| s.split("background-image:url('").nth(1))
            .unwrap_or("")
            .to_string();
        if text.is_empty() && image.is_empty() {
            continue;
        }
        // the first outside link in the post: usually the product page on the shop's site
        let link = text_html
            .split("href=\"")
            .skip(1)
            .filter_map(|s| s.split('"').next())
            .map(html_unescape)
            .find(|u| u.starts_with("http") && !u.contains("t.me/"))
            .unwrap_or_default();
        let title = text.lines().map(str::trim).find(|l| l.chars().count() >= 3).unwrap_or("").chars().take(120).collect::<String>();
        out.push(Item {
            title: if title.is_empty() { format!("پست {id}") } else { title },
            price: price_from_text(&text),
            image,
            url: format!("https://t.me/{post}"),
            seller: channel.clone(),
            seller_url: format!("https://t.me/{handle}"),
            date: between(chunk, "<time datetime=\"", "\"").map(|d| d.replace('T', " ").chars().take(19).collect()).unwrap_or_default(),
            description: text.chars().take(600).collect(),
            views: between(chunk, "tgme_widget_message_views\">", "<").map(views).unwrap_or(0),
            link,
            source: "t.me".into(),
            ..Default::default()
        });
    }
    (out, min_id)
}

async fn telegram(handle: &str, limit: usize, token: &CancellationToken, emit: Emit<'_>) -> Res<(Vec<Item>, usize)> {
    let http = crate::enrich::client();
    let base = crate::logic::test_seam("HAMISTA_TME_BASE").unwrap_or_else(|| "https://t.me".into());
    let mut items: Vec<Item> = vec![];
    let mut before: Option<i64> = None;
    let mut pages = 0;
    while items.len() < limit && !token.is_cancelled() && pages < 200 {
        let url = match before {
            Some(b) => format!("{base}/s/{handle}?before={b}"),
            None => format!("{base}/s/{handle}"),
        };
        let html = tokio::select! {
            r = async { http.get(&url).send().await?.text().await } => r.map_err(|e| format!("تلگرام: {e} — اگر تلگرام در شبکه شما فیلتر است VPN/پروکسی سیستم را روشن کنید."))?,
            _ = token.cancelled() => break,
        };
        pages += 1;
        let (mut posts, min_id) = telegram_posts(&html, handle);
        if posts.is_empty() {
            if pages == 1 && !html.contains("tgme_widget_message") {
                return Err(format!("کانال @{handle} پیدا نشد یا عمومی نیست (پیش‌نمایش عمومی ندارد)."));
            }
            break;
        }
        posts.reverse(); // newest first
        items.extend(posts);
        scanner::status(emit, format!("خواندن پست‌های @{handle}: {} پست", items.len()));
        match min_id {
            Some(m) if Some(m) != before && m > 1 => before = Some(m),
            _ => break,
        }
        tokio::time::sleep(Duration::from_millis(crate::logic::jitter(700, 1400))).await;
    }
    items.truncate(limit);
    Ok((items, pages))
}

// ---------------- shops, in the browser ----------------

const NAV_JS: &str = r#"
const abs = (h) => { try { return h ? new URL(h, location.href).href : ''; } catch (e) { return ''; } };
let next = '';
const rel = document.querySelector('a[rel="next"], link[rel="next"]');
if (rel) next = abs(rel.getAttribute('href'));
if (!next) {
  for (const a of document.querySelectorAll('a[href]')) {
    const t = (a.innerText || a.getAttribute('aria-label') || '').trim();
    if (/^(بعدی|صفحه بعد|صفحه بعدی|next|›|»)$/i.test(t)) { next = abs(a.getAttribute('href')); break; }
  }
}
const cats = [];
for (const a of document.querySelectorAll('a[href]')) {
  const u = abs(a.getAttribute('href'));
  if (!u) continue;
  let p = '';
  try { p = decodeURIComponent(new URL(u).pathname); } catch (e) { continue; }
  if (/\/(category|categories|cat|product-category|collections?|shop|store|products|دسته|دسته-بندی|محصولات|فروشگاه)(\/|-|$)/i.test(p) && !cats.includes(u)) cats.push(u);
  if (cats.length >= 80) break;
}
return { next, cats, href: location.href };
"#;

#[derive(Deserialize, Default)]
#[serde(default)]
struct Nav {
    next: String,
    cats: Vec<String>,
    href: String,
}

/// Products on the page that is open now (all readers of the market engine), with links made
/// absolute against the page and Rial prices converted.
fn page_items(d: &PageData, page_url: &str) -> Vec<Item> {
    let host = crate::links::host(page_url);
    // a known marketplace keeps its own link rules and Rial prices
    let (pid, rial) = crate::market::PLATFORMS
        .iter()
        .find(|p| host.split('.').any(|x| x == p.id))
        .map_or(("web", false), |p| (p.id, p.rial));
    let origin = format!("https://{}", page_url.split("//").nth(1).unwrap_or("").split('/').next().unwrap_or(""));
    let fix = |u: &str| -> String {
        let u = u.trim();
        if u.starts_with("//") {
            format!("https:{u}")
        } else if u.starts_with('/') {
            format!("{origin}{u}")
        } else {
            u.to_string()
        }
    };
    let mut offers: Vec<Offer> = vec![];
    for v in &d.data {
        if let Some(cards) = v.get("cards").and_then(Value::as_array) {
            for c in cards {
                let r = c.get("unit").and_then(Value::as_str) == Some("ریال");
                offers.extend(offers_in(c, pid, r));
            }
        } else {
            // JSON-LD / app data in Rial when it says so
            let r = rial || v.to_string().contains("\"IRR\"");
            offers.extend(offers_in(v, pid, r));
        }
    }
    offers
        .into_iter()
        .map(|o| Item {
            title: o.title,
            price: o.price,
            image: fix(&o.image),
            url: fix(&o.url),
            seller: o.seller,
            seller_url: fix(&o.seller_url),
            brand: o.brand,
            rating: o.rating,
            reviews: o.reviews,
            sales: o.sales,
            date: o.updated,
            location: o.location,
            source: host.clone(),
            ..Default::default()
        })
        .collect()
}

/// Scrolls an endless list until `want` products are on it or it stops growing.
async fn read_page(page: &chromiumoxide::Page, want: usize, token: &CancellationToken) -> PageData {
    let mut d: PageData = scanner::js(page, PAGE_JS).await.unwrap_or_default();
    let mut still = 0;
    for _ in 0..60 {
        if d.cards >= want || d.blocked || d.login || token.is_cancelled() || still >= 2 {
            break;
        }
        let _ = scanner::js::<Value>(page, "window.scrollTo(0, document.body.scrollHeight); return 1;").await;
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let next: PageData = scanner::js(page, PAGE_JS).await.unwrap_or_default();
        still = if next.cards > d.cards { 0 } else { still + 1 };
        d = next;
    }
    d
}

/// Opens `url` and waits for the page to show something (or a block page).
async fn open(page: &chromiumoxide::Page, url: &str, token: &CancellationToken) {
    scanner::goto(page, url).await;
    for _ in 0..8 {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(900)) => {}
            _ = token.cancelled() => return,
        }
        let d: PageData = scanner::js(page, PAGE_JS).await.unwrap_or_default();
        if d.cards > 0 || d.blocked || d.login || d.data.len() > 1 {
            return;
        }
    }
}

/// The site asked «I'm not a robot» or to sign in: a visible window, never stuck — the user passes
/// the check, closes the window, presses «رد کردن و ادامه», Stop, or 5 minutes pass.
async fn attended(s: &Settings, profile: &Path, url: &str, token: &CancellationToken, emit: Emit<'_>) -> Res<Option<scanner::Engine>> {
    scanner::clear_wait_skip();
    let host = crate::links::host(url);
    emit(ScanEvent::Captcha {
        message: format!("یک پنجره مرورگر باز شد: «{host}» تأیید «ربات نیستم» یا ورود خواست. آن را در همان پنجره انجام دهید؛ دریافت خودکار ادامه پیدا می‌کند. برای رد کردن، «رد کردن و ادامه» را بزنید."),
    });
    let mut eng = scanner::launch(s, profile, false, 0).await?;
    scanner::goto(&eng.list, url).await;
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(1500)) => {}
            _ = token.cancelled() => break,
        }
        if scanner::take_wait_skip() || eng.proc.exited() || !eng.alive.load(std::sync::atomic::Ordering::Relaxed) || std::time::Instant::now() > deadline {
            break;
        }
        let d: PageData = scanner::js(&eng.list, PAGE_JS).await.unwrap_or_default();
        if !d.blocked && !d.login && (d.cards > 0 || d.data.len() > 1) {
            scanner::status(emit, format!("ادامه دریافت از {host}…"));
            return Ok(Some(eng));
        }
    }
    scanner::shut(Some(eng)).await;
    Ok(None)
}

#[allow(clippy::too_many_arguments)] // one engine, three callers; a struct would only rename these
/// Reads a shop (booth / site / page) within `scope`. Used by «دریافت لیست محصول», by the web
/// source of «تحلیل محصول» (a user-given site) and by brand analysis (the brand's own store).
pub async fn crawl(
    s: &Settings,
    profile: &Path,
    starts: &[String],
    scope: &Scope,
    limit: usize,
    max_pages: usize,
    token: &CancellationToken,
    emit: Emit<'_>,
) -> Res<(Vec<Item>, usize, Vec<String>)> {
    let mut notes = vec![];
    let mut eng = Some(tokio::select! {
        e = scanner::launch(s, profile, s.headless, 0) => e?,
        _ = token.cancelled() => return Ok((vec![], 0, notes)),
    });
    let mut queue: VecDeque<String> = starts.iter().cloned().collect();
    let mut visited: HashSet<String> = HashSet::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut items: Vec<Item> = vec![];
    let mut dropped = 0usize;
    while let Some(url) = queue.pop_front() {
        if token.is_cancelled() || items.len() >= limit || visited.len() >= max_pages {
            break;
        }
        if !visited.insert(url.trim_end_matches('/').to_string()) {
            continue;
        }
        let Some(e) = eng.as_ref() else { break };
        scanner::status(emit, format!("خواندن صفحه {} — {} محصول تا اینجا", visited.len(), items.len()));
        open(&e.list, &url, token).await;
        let mut d = read_page(&e.list, limit - items.len(), token).await;
        if d.blocked || d.login {
            scanner::shut(eng.take()).await;
            match attended(s, profile, &url, token, emit).await? {
                Some(v) => {
                    d = read_page(&v.list, limit - items.len(), token).await;
                    eng = Some(v);
                }
                None => {
                    notes.push(format!("{}: تأیید «ربات نیستم» انجام نشد؛ این صفحه رد شد.", crate::links::host(&url)));
                    eng = scanner::launch(s, profile, s.headless, 0).await.ok();
                    continue;
                }
            }
        }
        let Some(e) = eng.as_ref() else { break };
        let here = scanner::js::<Nav>(&e.list, NAV_JS).await.unwrap_or_default();
        let page_url = if here.href.is_empty() { url.clone() } else { here.href.clone() };
        for mut it in page_items(&d, &page_url) {
            if it.url.is_empty() && !matches!(scope, Scope::Booth { .. }) {
                it.url = page_url.clone(); // a single product page's own data
            }
            if !scope.keeps(&it.url, &it.seller_url) {
                dropped += 1;
                continue;
            }
            let key = if it.url.is_empty() || it.url == page_url { format!("{}|{}", it.title, it.price) } else { it.url.clone() };
            if seen.insert(key) && items.len() < limit {
                items.push(it);
            }
        }
        emit(ScanEvent::Progress {
            keyword: starts.first().cloned().unwrap_or_default(),
            collected: items.len() as i64,
            checked: visited.len() as i64,
            found: items.len() as i64,
            target: limit as i64,
        });
        let same_site = |u: &str| match scope {
            Scope::Site { host, .. } => same_host(u, host),
            Scope::Sites(_) => same_host(u, &crate::links::host(&page_url)),
            Scope::Booth { vendor } => crate::links::host(u) == "basalam.com" && path_of(u).to_lowercase().starts_with(&format!("/{}", vendor.to_lowercase())),
            Scope::Telegram(_) => false,
        };
        if !here.next.is_empty() && same_site(&here.next) {
            queue.push_front(here.next.clone());
        }
        if matches!(scope, Scope::Site { whole: true, .. }) {
            queue.extend(here.cats.into_iter().filter(|u| same_site(u)));
        }
        tokio::time::sleep(Duration::from_millis(crate::logic::jitter(s.delay_ms, s.delay_ms * 2))).await;
    }
    scanner::shut(eng).await;
    if dropped > 0 {
        notes.push(format!("{dropped} مورد خارج از محدوده لینک (محصول غرفه‌ها یا سایت‌های دیگر، پیشنهادها و تبلیغات) کنار گذاشته شد."));
    }
    Ok((items, visited.len(), notes))
}

/// Opens each product page (plain HTTP, 6 at a time) for availability, brand and description.
async fn details(items: &mut [Item], token: &CancellationToken, emit: Emit<'_>) {
    use futures::StreamExt;
    let http = crate::web::client();
    let n = items.len().min(300);
    scanner::status(emit, format!("خواندن جزئیات {n} محصول…"));
    let urls: Vec<(usize, String)> = items.iter().take(n).map(|i| i.url.clone()).enumerate().collect();
    let got: Vec<(usize, String)> = futures::stream::iter(urls)
        .map(|(i, u)| {
            let http = http.clone();
            async move {
                let html = async { http.get(&u).send().await.ok()?.text().await.ok() };
                (i, tokio::time::timeout(Duration::from_secs(15), html).await.ok().flatten().unwrap_or_default())
            }
        })
        .buffer_unordered(6)
        .take_until(token.cancelled())
        .collect()
        .await;
    for (i, html) in got {
        let p = crate::web::parse_product(&html);
        let it = &mut items[i];
        if it.brand.is_empty() {
            it.brand = p.brand;
        }
        if it.price == 0 {
            it.price = p.price;
        }
        it.availability = p.availability;
        if let Some(d) = crate::logic::meta_content(&html, "og:description").or_else(|| crate::logic::meta_content(&html, "description")) {
            it.description = d.chars().take(400).collect();
        }
    }
}

pub fn stats(items: &[Item]) -> Stats {
    let mut prices: Vec<i64> = items.iter().map(|i| i.price).filter(|p| *p > 0).collect();
    prices.sort_unstable();
    let top = |f: fn(&Item) -> &str| {
        let mut m: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for i in items {
            let k = f(i).trim();
            if !k.is_empty() {
                *m.entry(k.to_string()).or_default() += 1;
            }
        }
        let mut v: Vec<_> = m.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(10);
        v
    };
    let mut dates: Vec<&str> = items.iter().map(|i| i.date.as_str()).filter(|d| d.len() >= 10).collect();
    dates.sort_unstable();
    let days = |d: &str| -> i64 {
        let n = |a: usize, b: usize| d.get(a..b).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
        n(0, 4) * 365 + n(5, 7) * 30 + n(8, 10)
    };
    let per_week = match (dates.first(), dates.last()) {
        (Some(a), Some(b)) if dates.len() > 1 => dates.len() as f64 * 7.0 / ((days(b) - days(a)).max(1) as f64),
        _ => 0.0,
    };
    Stats {
        count: items.len(),
        priced: prices.len(),
        min: prices.first().copied().unwrap_or(0),
        median: if prices.is_empty() { 0 } else { crate::market::pct(&prices, 0.5) },
        max: prices.last().copied().unwrap_or(0),
        avg: if prices.is_empty() { 0 } else { prices.iter().sum::<i64>() / prices.len() as i64 },
        sellers: top(|i| &i.seller),
        brands: top(|i| &i.brand),
        newest: dates.last().map(|d| d.to_string()).unwrap_or_default(),
        oldest: dates.first().map(|d| d.to_string()).unwrap_or_default(),
        per_week: (per_week * 10.0).round() / 10.0,
    }
}

/// «دریافت لیست محصول» for one link.
pub async fn run(s: &Settings, profile: &Path, req: &ListReq, token: &CancellationToken, emit: Emit<'_>) -> Res<ListReport> {
    let url = normalize(&req.url);
    let scope = scope_of(&url)?;
    let limit = if req.limit > 0 { req.limit.min(3000) as usize } else { 300 };
    let whole = matches!(scope, Scope::Site { whole: true, .. });
    let pages = if req.pages > 0 { req.pages.min(60) as usize } else if whole { 10 } else { 5 };
    scanner::status(emit, format!("محدوده: {}", scope.describe()));
    let (mut items, pages, mut notes) = match &scope {
        Scope::Telegram(h) => {
            let (i, p) = telegram(h, limit, token, emit).await?;
            (i, p, vec![])
        }
        _ => crawl(s, profile, std::slice::from_ref(&url), &scope, limit, pages, token, emit).await?,
    };
    if !req.query.trim().is_empty() {
        let before = items.len();
        let groups = crate::market::parse_query(req.query.trim());
        items.retain(|i| crate::market::matches_groups(&i.title, &groups, false));
        notes.push(format!("{} از {before} مورد با «{}» جور بود.", items.len(), req.query.trim()));
    }
    if req.details && !items.is_empty() && !matches!(scope, Scope::Telegram(_)) && !token.is_cancelled() {
        details(&mut items, token, emit).await;
    }
    if items.is_empty() && !token.is_cancelled() {
        return Err(format!(
            "در {} محصولی با قیمت پیدا نشد. اگر سایت فقط پس از ورود قیمت نشان می‌دهد، «نمایش پنجره مرورگر» را در تنظیمات روشن کنید؛ یا لینک صفحه دسته‌بندی / غرفه را بدهید.",
            scope.describe()
        ));
    }
    Ok(ListReport {
        url,
        scope: scope.describe(),
        stats: stats(&items),
        items,
        pages,
        notes,
    })
}

// ---------------- Excel ----------------

pub fn export_xlsx(r: &ListReport, dir: &Path) -> Res<String> {
    use rust_xlsxwriter::{Format, Workbook};
    std::fs::create_dir_all(dir).map_err(err)?;
    let host = crate::links::host(&r.url).replace(['.', '/'], "-");
    let stamp = crate::market::iso_utc(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64))
        .replace([' ', ':'], "-");
    let path = dir.join(format!("products-{host}-{stamp}.xlsx"));
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("محصولات").map_err(err)?;
    ws.set_right_to_left(true);
    let bold = Format::new().set_bold();
    let money = Format::new().set_num_format("#,##0");
    const HEAD: &[&str] = &[
        "ردیف", "نام محصول", "قیمت (تومان)", "لینک محصول", "عکس", "فروشنده / غرفه", "لینک فروشنده", "برند", "امتیاز", "تعداد نظر", "فروش", "تاریخ", "موقعیت", "موجودی", "بازدید", "لینک محصول در پست", "توضیح", "منبع",
    ];
    for (c, h) in HEAD.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &bold).map_err(err)?;
    }
    let link = |ws: &mut rust_xlsxwriter::Worksheet, row: u32, col: u16, u: &str| -> Res<()> {
        if u.is_empty() {
            return Ok(());
        }
        if u.len() < 2000 && ws.write_url(row, col, u).is_ok() {
            return Ok(());
        }
        ws.write_string(row, col, u).map(|_| ()).map_err(err)
    };
    for (i, it) in r.items.iter().enumerate() {
        let row = i as u32 + 1;
        ws.write_number(row, 0, (i + 1) as f64).map_err(err)?;
        ws.write_string(row, 1, &it.title).map_err(err)?;
        if it.price > 0 {
            ws.write_number_with_format(row, 2, it.price as f64, &money).map_err(err)?;
        }
        link(ws, row, 3, &it.url)?;
        link(ws, row, 4, &it.image)?;
        ws.write_string(row, 5, &it.seller).map_err(err)?;
        link(ws, row, 6, &it.seller_url)?;
        ws.write_string(row, 7, &it.brand).map_err(err)?;
        if let Some(x) = it.rating {
            ws.write_number(row, 8, x).map_err(err)?;
        }
        for (col, v) in [(9u16, it.reviews), (10, it.sales), (14, it.views)] {
            if v > 0 {
                ws.write_number(row, col, v as f64).map_err(err)?;
            }
        }
        ws.write_string(row, 11, &it.date).map_err(err)?;
        ws.write_string(row, 12, &it.location).map_err(err)?;
        ws.write_string(row, 13, &it.availability).map_err(err)?;
        link(ws, row, 15, &it.link)?;
        ws.write_string(row, 16, &it.description).map_err(err)?;
        ws.write_string(row, 17, &it.source).map_err(err)?;
    }
    ws.set_freeze_panes(1, 0).map_err(err)?;
    ws.autofilter(0, 0, r.items.len() as u32, HEAD.len() as u16 - 1).map_err(err)?;
    ws.set_column_width(1, 48).map_err(err)?;
    ws.set_column_width(16, 60).map_err(err)?;

    let ws = wb.add_worksheet();
    ws.set_name("خلاصه").map_err(err)?;
    ws.set_right_to_left(true);
    let st = &r.stats;
    let rows: Vec<(&str, String)> = vec![
        ("لینک", r.url.clone()),
        ("محدوده", r.scope.clone()),
        ("صفحه‌های خوانده‌شده", r.pages.to_string()),
        ("تعداد محصول", st.count.to_string()),
        ("دارای قیمت", st.priced.to_string()),
        ("کمترین قیمت (تومان)", st.min.to_string()),
        ("میانه قیمت (تومان)", st.median.to_string()),
        ("میانگین قیمت (تومان)", st.avg.to_string()),
        ("بیشترین قیمت (تومان)", st.max.to_string()),
        ("جدیدترین تاریخ", st.newest.clone()),
        ("قدیمی‌ترین تاریخ", st.oldest.clone()),
        ("انتشار در هفته", st.per_week.to_string()),
    ];
    for (i, (k, v)) in rows.iter().enumerate() {
        ws.write_string_with_format(i as u32, 0, *k, &bold).map_err(err)?;
        ws.write_string(i as u32, 1, v).map_err(err)?;
    }
    let mut row = rows.len() as u32 + 1;
    for (title, list) in [("فروشندگان پرتکرار", &st.sellers), ("برندهای پرتکرار", &st.brands)] {
        if list.is_empty() {
            continue;
        }
        ws.write_string_with_format(row, 0, title, &bold).map_err(err)?;
        row += 1;
        for (name, n) in list {
            ws.write_string(row, 0, name).map_err(err)?;
            ws.write_number(row, 1, *n as f64).map_err(err)?;
            row += 1;
        }
        row += 1;
    }
    for n in &r.notes {
        ws.write_string(row, 0, n).map_err(err)?;
        row += 1;
    }
    ws.set_column_width(0, 28).map_err(err)?;
    ws.set_column_width(1, 70).map_err(err)?;
    wb.save(&path).map_err(err)?;
    Ok(path.display().to_string())
}

// ---------------- store exports ----------------

/// A stable code for a product that has none: the same product always gets the same one.
fn sku_of(it: &Item) -> String {
    use sha2::{Digest, Sha256};
    let key = if it.url.is_empty() { &it.title } else { &it.url };
    let h = Sha256::digest(key.as_bytes());
    format!("HS-{}", h[..4].iter().map(|b| format!("{b:02X}")).collect::<String>())
}

fn csv_cell(s: &str) -> String {
    let clean: String = s.chars().map(|c| if c == '\r' || c == '\n' { ' ' } else { c }).collect();
    // a cell that starts like a formula is text, not a formula (spreadsheet injection)
    let clean = if clean.starts_with(['=', '+', '@']) || (clean.starts_with('-') && clean.len() > 1 && !clean[1..].starts_with(|c: char| c.is_ascii_digit())) { format!("'{clean}") } else { clean };
    format!("\"{}\"", clean.replace('"', "\"\""))
}

fn in_stock(it: &Item) -> bool {
    !(it.availability.contains("ناموجود") || it.availability.contains("اتمام") || it.availability.to_lowercase().contains("out"))
}

/// WooCommerce's product-importer CSV (UTF-8 with BOM, so Persian opens right in Excel and WordPress).
/// Prices stay in Toman — set the store currency to Toman, or ×10 for Rial.
pub fn woocommerce_csv(items: &[Item]) -> String {
    const HEAD: &[&str] = &[
        "Type", "SKU", "Name", "Published", "Is featured?", "Visibility in catalog", "Short description", "Description", "Tax status", "In stock?", "Regular price", "Categories", "Tags", "Images",
    ];
    let mut out = String::from("\u{FEFF}");
    out.push_str(&HEAD.iter().map(|h| csv_cell(h)).collect::<Vec<_>>().join(","));
    out.push_str("\r\n");
    for it in items {
        let group = crate::fees::group_name(crate::fees::group_of(&it.title));
        let short: String = it.description.chars().take(160).collect();
        let cells = [
            "simple".to_string(),
            sku_of(it),
            it.title.clone(),
            "1".into(),
            "0".into(),
            "visible".into(),
            short,
            it.description.clone(),
            "taxable".into(),
            if in_stock(it) { "1" } else { "0" }.into(),
            if it.price > 0 { it.price.to_string() } else { String::new() },
            group.to_string(),
            it.brand.clone(),
            it.image.clone(),
        ];
        out.push_str(&cells.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    out
}

fn export_path(dir: &Path, stem: &str, ext: &str) -> Res<std::path::PathBuf> {
    std::fs::create_dir_all(dir).map_err(err)?;
    let stamp = crate::market::iso_utc(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)).replace([' ', ':'], "-");
    Ok(dir.join(format!("{stem}-{stamp}.{ext}")))
}

pub fn export_woocommerce_csv(items: &[Item], dir: &Path) -> Res<String> {
    if items.is_empty() {
        return Err("محصولی انتخاب نشده است.".into());
    }
    let path = export_path(dir, "woocommerce", "csv")?;
    std::fs::write(&path, woocommerce_csv(items)).map_err(err)?;
    Ok(path.display().to_string())
}

/// A bulk-upload sheet for Basalam's seller panel: name, price (Toman), stock, category, brand, description
/// and up to three picture links. The panel's own column names can change; map them on upload if it asks.
pub fn export_basalam_xlsx(items: &[Item], dir: &Path) -> Res<String> {
    use rust_xlsxwriter::{Format, Workbook};
    if items.is_empty() {
        return Err("محصولی انتخاب نشده است.".into());
    }
    let path = export_path(dir, "basalam", "xlsx")?;
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("محصولات").map_err(err)?;
    ws.set_right_to_left(true);
    let bold = Format::new().set_bold();
    let money = Format::new().set_num_format("#,##0");
    const HEAD: &[&str] = &["کد کالا", "نام محصول", "قیمت (تومان)", "موجودی", "دسته‌بندی", "برند", "توضیحات", "تصویر ۱", "تصویر ۲", "تصویر ۳"];
    for (c, h) in HEAD.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &bold).map_err(err)?;
    }
    for (i, it) in items.iter().enumerate() {
        let row = i as u32 + 1;
        ws.write_string(row, 0, sku_of(it)).map_err(err)?;
        ws.write_string(row, 1, it.title.chars().take(120).collect::<String>()).map_err(err)?;
        if it.price > 0 {
            ws.write_number_with_format(row, 2, it.price as f64, &money).map_err(err)?;
        }
        ws.write_number(row, 3, if in_stock(it) { 1.0 } else { 0.0 }).map_err(err)?;
        ws.write_string(row, 4, crate::fees::group_name(crate::fees::group_of(&it.title))).map_err(err)?;
        ws.write_string(row, 5, &it.brand).map_err(err)?;
        ws.write_string(row, 6, &it.description).map_err(err)?;
        if !it.image.is_empty() {
            ws.write_string(row, 7, &it.image).map_err(err)?;
        }
    }
    ws.set_freeze_panes(1, 0).map_err(err)?;
    ws.set_column_width(1, 48).map_err(err)?;
    ws.set_column_width(6, 60).map_err(err)?;
    wb.save(&path).map_err(err)?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn woocommerce_csv_is_importable() {
        let items = vec![
            Item { title: "عسل گون \"ویژه\" ۵۰۰ گرمی".into(), price: 450_000, url: "https://x.ir/p/1".into(), image: "https://x.ir/1.jpg".into(), brand: "خوانسار".into(), description: "خط۱\nخط۲".into(), ..Default::default() },
            Item { title: "=HYPERLINK(\"x\")".into(), availability: "ناموجود".into(), ..Default::default() },
        ];
        let csv = woocommerce_csv(&items);
        assert!(csv.starts_with('\u{FEFF}'), "BOM for Persian text");
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("\"Regular price\"") && lines[0].contains("\"Images\""));
        assert!(lines[1].contains("\"عسل گون \"\"ویژه\"\" ۵۰۰ گرمی\"") && lines[1].contains("\"450000\"") && lines[1].contains("\"مواد غذایی\""), "{}", lines[1]);
        assert!(lines[1].contains("خط۱ خط۲"), "a newline would break the row");
        assert!(lines[2].contains("\"'=HYPERLINK") && lines[2].contains("\"0\",\"\""), "formula neutralised, out of stock: {}", lines[2]);
        assert_eq!(sku_of(&items[0]), sku_of(&items[0].clone()));
        assert!(sku_of(&items[0]).starts_with("HS-") && sku_of(&items[0]) != sku_of(&items[1]));
    }

    #[test]
    fn store_exports_write_files() {
        let dir = std::env::temp_dir().join(format!("hs-exp-{}", std::process::id()));
        let items = vec![Item { title: "کالا".into(), price: 1000, ..Default::default() }];
        let csv = export_woocommerce_csv(&items, &dir).unwrap();
        let xlsx = export_basalam_xlsx(&items, &dir).unwrap();
        assert!(csv.ends_with(".csv") && std::fs::metadata(&csv).unwrap().len() > 50);
        assert!(xlsx.ends_with(".xlsx") && std::fs::metadata(&xlsx).unwrap().len() > 500);
        assert!(export_woocommerce_csv(&[], &dir).is_err() && export_basalam_xlsx(&[], &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scope_is_strict() {
        let booth = scope_of("basalam.com/asal_darashkooh").unwrap();
        assert_eq!(booth, Scope::Booth { vendor: "asal_darashkooh".into() });
        assert!(booth.keeps("https://basalam.com/asal_darashkooh/product/123", ""));
        assert!(!booth.keeps("https://basalam.com/another_booth/product/9", ""), "another booth's product");
        assert!(!booth.keeps("https://basalam.com/asal_darashkooh_2/product/9", ""), "prefix of another name");
        assert!(!booth.keeps("", "https://basalam.com/asal_darashkooh"), "no product link → not proven");
        assert_eq!(scope_of("https://basalam.com/").unwrap(), Scope::Site { host: "basalam.com".into(), whole: true });
        assert_eq!(scope_of("https://basalam.com/search?q=عسل").unwrap(), Scope::Site { host: "basalam.com".into(), whole: false });
        let site = scope_of("https://www.shop.ir/category/honey").unwrap();
        assert_eq!(site, Scope::Site { host: "shop.ir".into(), whole: false });
        assert!(site.keeps("https://shop.ir/product/a", "") && site.keeps("https://cdn.shop.ir/p", ""));
        assert!(!site.keeps("https://othershop.ir/p", "") && !site.keeps("https://digikala.com/p", ""));
        assert_eq!(scope_of("https://t.me/s/lamiz_shop").unwrap(), Scope::Telegram("lamiz_shop".into()));
        assert!(scope_of("nothing").is_err());
    }

    #[test]
    fn prices_in_text() {
        assert_eq!(price_from_text("عسل گون ۱ کیلویی\nقیمت: ۴۵۰,۰۰۰ تومان"), 450_000);
        assert_eq!(price_from_text("فقط ۱۲۰ هزار تومان"), 120_000);
        assert_eq!(price_from_text("۲.۵ میلیون"), 2_500_000);
        assert_eq!(price_from_text("1,500,000 ریال"), 150_000);
        assert_eq!(price_from_text("قیمت 98000"), 98_000);
        assert_eq!(price_from_text("۱ کیلو ۱۴۰۳ تولید"), 0, "a weight or a year is not a price");
        assert_eq!(price_from_text("۳۵۰ت"), 0, "below 1000 Toman is not a real price");
        assert_eq!(price_from_text("۳۵۰۰۰ ت ارسال رایگان"), 35_000);
    }

    #[test]
    fn telegram_channel_posts() {
        let html = r#"<meta property="og:title" content="Lamiz Shop">
<div class="tgme_widget_message_wrap js-widget_message_wrap"><div class="tgme_widget_message" data-post="lamiz_shop/41">
<a class="tgme_widget_message_photo_wrap" style="width:800px;background-image:url('https://cdn4.telesco.pe/file/a.jpg')"></a>
<div class="tgme_widget_message_text js-message_text" dir="auto">عسل گون طبیعی<br/>قیمت: ۴۵۰,۰۰۰ تومان<br/>خرید: <a href="https://lamiz.ir/p/12">lamiz.ir/p/12</a> سفارش @lamiz_admin</div>
<span class="tgme_widget_message_views">1.2K</span><time datetime="2026-10-01T09:30:00+00:00" class="time">09:30</time></div></div>
<div class="tgme_widget_message_wrap js-widget_message_wrap"><div class="tgme_widget_message" data-post="lamiz_shop/42">
<div class="tgme_widget_message_text js-message_text" dir="auto">ساعات کاری جدید</div><time datetime="2026-10-08T09:30:00+00:00"></time></div></div>"#;
        let (posts, min) = telegram_posts(html, "lamiz_shop");
        assert_eq!(min, Some(41));
        assert_eq!(posts.len(), 2);
        let p = &posts[0];
        assert_eq!(p.title, "عسل گون طبیعی");
        assert_eq!(p.price, 450_000);
        assert_eq!(p.url, "https://t.me/lamiz_shop/41");
        assert_eq!(p.image, "https://cdn4.telesco.pe/file/a.jpg");
        assert_eq!(p.link, "https://lamiz.ir/p/12");
        assert_eq!(p.views, 1200);
        assert_eq!(p.date, "2026-10-01 09:30:00");
        assert_eq!(p.seller, "Lamiz Shop");
        assert_eq!(posts[1].price, 0);
        let st = stats(&posts);
        assert_eq!((st.count, st.priced, st.min), (2, 1, 450_000));
        assert_eq!(st.per_week, 2.0);
    }

    #[test]
    fn excel_file_is_written() {
        let dir = std::env::temp_dir().join(format!("hs-plist-{}", std::process::id()));
        let r = ListReport {
            url: "https://shop.ir/".into(),
            items: vec![Item {
                title: "عسل".into(),
                price: 1000,
                url: "https://shop.ir/p/1".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let p = export_xlsx(&r, &dir).unwrap();
        assert!(std::fs::metadata(&p).unwrap().len() > 1000);
        let _ = std::fs::remove_dir_all(dir);
    }
}
