//! Web sources for «محصول و قیمت»: a few highly-relevant organic results (top 1–3), each read as a
//! structured product page, and B2B supplier discovery (manufacturers, wholesalers, distributors).
//!
//! Only public pages are read. Nothing is guessed: a price/brand/link is kept only when the page's
//! own structured data (JSON-LD `Product`/`Offer`, OpenGraph, meta) or a verified pattern gives it.
//! Pure parsing lives here and is unit-tested; the search itself uses the app's own browser
//! (`websearch`) and never spams the engine (at most a page or two).
use crate::logic::{extract_contacts, html_unescape, meta_content, Contacts};
use crate::market::{norm, tok_eq, words};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// What a web page is, for ranking and trust.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceQuality {
    Official,
    Manufacturer,
    Brand,
    Retailer,
    B2b,
    Marketplace,
    Directory,
    Blog,
    #[default]
    Unknown,
}

impl SourceQuality {
    pub fn fa(self) -> &'static str {
        match self {
            Self::Official => "سایت رسمی",
            Self::Manufacturer => "تولیدکننده",
            Self::Brand => "برند",
            Self::Retailer => "خرده‌فروش",
            Self::B2b => "عمده / B2B",
            Self::Marketplace => "بازارگاه",
            Self::Directory => "فهرست / دایرکتوری",
            Self::Blog => "وبلاگ / خبری",
            Self::Unknown => "نامشخص",
        }
    }
}

/// One web result shown next to the marketplace offers (at most 3).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WebResult {
    pub title: String,
    pub domain: String,
    pub url: String,
    /// The exact product page on that site, when the page itself is a product page.
    pub product_url: String,
    pub product_name: String,
    pub brand: String,
    pub price: i64, // Toman, 0 = none
    pub availability: String,
    pub quality: SourceQuality,
    pub page_type: String,
    pub relevance: i64,  // 0–100, from how well title/url match the query
    pub confidence: i64, // 0–100, from how much came from structured data
    pub fetched_at: String,
}

/// Marketplaces and aggregators that are not a shop's own site (already covered by the platforms).
const MARKETPLACES: &[&str] = &[
    "torob.com",
    "digikala.com",
    "basalam.com",
    "emalls.ir",
    "divar.ir",
    "sheypoor.com",
    "mizanplus",
    "banimode.com",
    "okala.com",
    "snapp.market",
    "digistyle.com",
    "timcheh.com",
    "esam.ir",
    "iranrng.com",
    "kilid.com",
];
const DIRECTORIES: &[&str] = &[
    "esfahan.ir",
    "parsbanfood",
    "tejaratbazar",
    "iranzamin",
    "irandoc",
    "bazarche",
    "niazerooz",
    "ependar",
    "istgah.com",
    "panakala",
    "tjpco",
    "pd() ",
    "aradbranding",
];
const BLOGS: &[&str] = &[
    "blog.",
    "virgool.io",
    "namnak.com",
    "beytoote.com",
    "tebyan.net",
    "blogfa.com",
    "wikipedia.org",
    "aparat.com",
    "youtube.com",
    "instagram.com",
    "t.me",
];

fn host(url: &str) -> String {
    url.split("//")
        .nth(1)
        .unwrap_or("")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_lowercase()
}

/// B2B / wholesale cues in a page's text (and strong negatives). Used to tell a real supplier from a
/// single retail listing.
const B2B_POS: &[&str] = &[
    "عمده",
    "فروش عمده",
    "قیمت عمده",
    "خرید عمده",
    "بنکداری",
    "بنکدار",
    "پخش",
    "پخش سراسری",
    "تأمین",
    "تامین کننده",
    "تامین‌کننده",
    "تولیدکننده",
    "تولید کننده",
    "کارخانه",
    "کارخانجات",
    "صنایع",
    "واردکننده",
    "واردات",
    "نمایندگی",
    "نماینده",
    "همکاری",
    "قیمت همکاری",
    "قیمت نمایندگی",
    "حداقل سفارش",
    "تیراژ",
    "فاکتور رسمی",
    "ارسال به سراسر",
    "انبار",
    "b2b",
    "moq",
    "wholesale",
    "عمده‌فروشی",
    "عمده فروشی",
];
const B2B_NEG: &[&str] = &["افزودن به سبد", "add to cart", "خرید تکی", "گارانتی ۱۸ ماهه"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SupplierKind {
    Manufacturer,
    Wholesaler,
    Distributor,
    Importer,
    Representative,
    Retailer,
    #[default]
    Unknown,
}

impl SupplierKind {
    pub fn fa(self) -> &'static str {
        match self {
            Self::Manufacturer => "تولیدکننده",
            Self::Wholesaler => "عمده‌فروش",
            Self::Distributor => "پخش‌کننده",
            Self::Importer => "واردکننده",
            Self::Representative => "نمایندگی",
            Self::Retailer => "خرده‌فروش",
            Self::Unknown => "نامشخص",
        }
    }
}

/// A B2B supplier card (only facts the page stated; «اعلام نشده» fields stay empty).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Supplier {
    pub name: String,
    pub kind: String, // SupplierKind.fa()
    pub category: String,
    pub products: Vec<String>,
    pub city: String,
    pub province: String,
    pub website: String,
    pub phone: String, // public business number when the page shows one
    pub email: String,
    pub social: Vec<String>,
    pub moq: String,
    pub wholesale: bool,
    pub shipping: String,
    pub terms: Vec<String>,
    pub confidence: i64,
    pub source: String,
    pub url: String,
    pub last_checked: String,
}

/// The scoring behind a supplier classification (what made it a manufacturer / wholesaler …).
pub fn classify_supplier(domain: &str, title: &str, text: &str) -> (SupplierKind, i64, Vec<String>) {
    let t = norm(&format!("{title} {text}"));
    let has = |k: &str| t.contains(&norm(k));
    let mut signals = vec![];
    let mut score = 0i64;
    let own_site = !MARKETPLACES.iter().any(|m| domain.contains(m)) && !BLOGS.iter().any(|b| domain.contains(b)) && domain.contains('.');
    if own_site {
        score += 20;
        signals.push("سایت اختصاصی".into());
    }
    let pos = B2B_POS.iter().filter(|k| has(k)).count();
    score += (pos as i64 * 12).min(48);
    if B2B_NEG.iter().any(|k| has(k)) && pos <= 1 {
        score -= 25;
    }
    for (needle, label) in [
        ("حداقل سفارش", "حداقل سفارش"),
        ("moq", "MOQ"),
        ("قیمت همکاری", "قیمت همکاری"),
        ("کاتالوگ", "کاتالوگ"),
        ("انبار", "انبار"),
        ("فاکتور رسمی", "فاکتور رسمی"),
    ] {
        if has(needle) {
            score += 6;
            signals.push(label.into());
        }
    }
    let kind = if has("کارخانه") || has("تولیدکننده") || has("تولید کننده") || has("صنایع") {
        SupplierKind::Manufacturer
    } else if has("واردکننده") || has("واردات") {
        SupplierKind::Importer
    } else if has("پخش") || has("بنکدار") || has("بنکداری") {
        SupplierKind::Distributor
    } else if has("نمایندگی") || has("نماینده") {
        SupplierKind::Representative
    } else if pos >= 2 || has("عمده") {
        SupplierKind::Wholesaler
    } else if own_site {
        SupplierKind::Retailer
    } else {
        SupplierKind::Unknown
    };
    (kind, score.clamp(0, 100), signals)
}

/// Query expansion for B2B discovery (added to the user's product to find suppliers).
pub fn b2b_queries(product: &str) -> Vec<String> {
    let p = product.trim();
    [
        "قیمت عمده",
        "فروش عمده",
        "تولیدکننده",
        "کارخانه",
        "تأمین کننده",
        "پخش عمده",
        "نمایندگی",
        "قیمت همکاری",
    ]
    .iter()
    .map(|suffix| format!("{p} {suffix}"))
    .collect()
}

/// A ready-to-send price-enquiry (RFQ) message for a supplier. Nothing is sent — the user copies it.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Rfq {
    pub product: String,
    pub quantity: String,
    pub city: String,
    pub target_price: String,
    pub packaging: String,
    pub payment: String,
    pub lead_time: String,
}

/// Builds a professional Persian price-enquiry from whatever fields the user filled.
pub fn rfq_text(r: &Rfq) -> String {
    let mut lines = vec!["با سلام و احترام،".to_string()];
    let mut ask = format!("برای تأمین «{}»", r.product.trim());
    if !r.quantity.trim().is_empty() {
        ask.push_str(&format!(" به تعداد/مقدار {}", r.quantity.trim()));
    }
    if !r.city.trim().is_empty() {
        ask.push_str(&format!("، مقصد {}", r.city.trim()));
    }
    ask.push_str(" نیازمند استعلام قیمت هستیم.");
    lines.push(ask);
    let mut wants = vec!["قیمت همکاری/عمده", "حداقل سفارش (MOQ)", "شرایط و زمان ارسال", "زمان تحویل"];
    if !r.packaging.trim().is_empty() {
        lines.push(format!("نوع بسته‌بندی مدنظر: {}.", r.packaging.trim()));
    }
    if !r.payment.trim().is_empty() {
        lines.push(format!("شرایط پرداخت: {}.", r.payment.trim()));
    }
    if !r.target_price.trim().is_empty() {
        lines.push(format!(
            "قیمت هدف ما حدود {} است؛ در صورت امکان بر همین مبنا پیشنهاد دهید.",
            r.target_price.trim()
        ));
    }
    if !r.lead_time.trim().is_empty() {
        wants.retain(|w| *w != "زمان تحویل");
        lines.push(format!("زمان تحویل مورد انتظار: {}.", r.lead_time.trim()));
    }
    lines.push(format!("لطفاً {} را اعلام بفرمایید.", wants.join("، ")));
    lines.push("با تشکر از همکاری شما.".into());
    lines.join("\n")
}

// ---------------- structured product extraction ----------------

/// Every `<script type="application/ld+json">` block as parsed JSON.
fn ld_json(html: &str) -> Vec<Value> {
    let mut out = vec![];
    let needle = "application/ld+json";
    let mut rest = html;
    while let Some(i) = rest.find(needle) {
        rest = &rest[i + needle.len()..];
        let Some(gt) = rest.find('>') else { break };
        let body = &rest[gt + 1..];
        let Some(end) = body.find("</script") else { break };
        if let Ok(v) = serde_json::from_str::<Value>(html_unescape(body[..end].trim()).trim()) {
            out.push(v);
        }
        rest = &body[end..];
    }
    out
}

/// Does any `@type` in this node (string or array) equal `want` (case-insensitive)?
fn is_type(o: &serde_json::Map<String, Value>, want: &str) -> bool {
    match o.get("@type") {
        Some(Value::String(s)) => s.eq_ignore_ascii_case(want),
        Some(Value::Array(a)) => a.iter().any(|x| x.as_str().is_some_and(|s| s.eq_ignore_ascii_case(want))),
        _ => false,
    }
}

/// Finds the first object of JSON-LD `@type` `want`, looking through `@graph` and arrays.
fn find_type<'a>(v: &'a Value, want: &str) -> Option<&'a serde_json::Map<String, Value>> {
    match v {
        Value::Array(a) => a.iter().find_map(|x| find_type(x, want)),
        Value::Object(o) => {
            if is_type(o, want) {
                return Some(o);
            }
            if let Some(g) = o.get("@graph") {
                return find_type(g, want);
            }
            None
        }
        _ => None,
    }
}

fn str_of(o: &serde_json::Map<String, Value>, key: &str) -> String {
    match o.get(key) {
        Some(Value::String(s)) => html_unescape(s.trim()),
        Some(Value::Object(c)) => c.get("name").and_then(Value::as_str).map(|s| html_unescape(s.trim())).unwrap_or_default(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Price (Toman) from an Offer/AggregateOffer node: Rial prices (> 1,000,000 with a Rial currency or
/// no currency and very large) are divided by 10.
fn price_of(offer: &serde_json::Map<String, Value>) -> i64 {
    let raw = ["price", "lowPrice", "highPrice"].iter().find_map(|k| match offer.get(*k) {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => crate::logic::ascii_digits(s)
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok(),
        _ => None,
    });
    let Some(mut p) = raw.filter(|p| *p > 0.0) else { return 0 };
    let cur = offer.get("priceCurrency").and_then(Value::as_str).unwrap_or("").to_uppercase();
    if cur == "IRR" || cur.contains("RIAL") || (cur.is_empty() && p >= 5_000_000.0) {
        p /= 10.0;
    }
    p as i64
}

#[derive(Debug, Default, PartialEq)]
pub struct WebProduct {
    pub name: String,
    pub brand: String,
    pub price: i64,
    pub availability: String,
    pub is_product: bool,
    /// How much of this came from real structured data (0–100).
    pub confidence: i64,
}

/// Reads a product page: JSON-LD Product/Offer first, then OpenGraph, then meta. Returns what was
/// actually present — never invents a price or brand.
pub fn parse_product(html: &str) -> WebProduct {
    let mut p = WebProduct::default();
    for v in ld_json(html) {
        if let Some(prod) = find_type(&v, "Product") {
            p.is_product = true;
            p.name = str_of(prod, "name");
            p.brand = str_of(prod, "brand");
            p.confidence = 70;
            if let Some(off) = prod
                .get("offers")
                .and_then(|o| find_type(o, "Offer").or_else(|| find_type(o, "AggregateOffer")).or_else(|| o.as_object()))
            {
                p.price = price_of(off);
                let avail = str_of(off, "availability");
                p.availability = availability_fa(&avail);
                if p.price > 0 {
                    p.confidence = 90;
                }
            }
            break;
        }
    }
    // OpenGraph / product meta fills the gaps
    if p.name.is_empty() {
        if let Some(t) = meta_content(html, "og:title") {
            p.name = t;
            p.confidence = p.confidence.max(40);
        }
    }
    if p.price == 0 {
        if let Some(pr) = meta_content(html, "product:price:amount").or_else(|| meta_content(html, "og:price:amount")) {
            if let Ok(v) = crate::logic::ascii_digits(&pr)
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<f64>()
            {
                let cur = meta_content(html, "product:price:currency").unwrap_or_default().to_uppercase();
                p.price = (if cur == "IRR" || cur.contains("RIAL") || (cur.is_empty() && v >= 5_000_000.0) {
                    v / 10.0
                } else {
                    v
                }) as i64;
                p.confidence = p.confidence.max(55);
                p.is_product = true;
            }
        }
    }
    if p.brand.is_empty() {
        p.brand = meta_content(html, "product:brand").unwrap_or_default();
    }
    p
}

fn availability_fa(s: &str) -> String {
    let l = s.to_lowercase();
    if l.contains("instock") || l.contains("in_stock") {
        "موجود".into()
    } else if l.contains("outofstock") || l.contains("out_of_stock") || l.contains("soldout") {
        "ناموجود".into()
    } else if l.contains("preorder") {
        "پیش‌فروش".into()
    } else {
        String::new()
    }
}

/// Classifies a page from its domain and (optionally) its text.
pub fn classify_source(domain: &str, text: &str) -> SourceQuality {
    let d = domain.to_lowercase();
    if MARKETPLACES.iter().any(|m| d.contains(m)) {
        return SourceQuality::Marketplace;
    }
    if BLOGS.iter().any(|b| d.contains(b)) {
        return SourceQuality::Blog;
    }
    if DIRECTORIES.iter().any(|b| d.contains(b)) {
        return SourceQuality::Directory;
    }
    let (kind, score, _) = classify_supplier(&d, "", text);
    match kind {
        SupplierKind::Manufacturer => SourceQuality::Manufacturer,
        SupplierKind::Wholesaler | SupplierKind::Distributor | SupplierKind::Importer | SupplierKind::Representative if score >= 40 => SourceQuality::B2b,
        _ if d.contains('.') => SourceQuality::Retailer,
        _ => SourceQuality::Unknown,
    }
}

/// How well a result's title + url fit the query (0–100), so position alone never decides.
pub fn relevance(query: &str, title: &str, url: &str) -> i64 {
    let q = words(query);
    if q.is_empty() {
        return 0;
    }
    let t = words(&format!("{title} {}", url.replace(['/', '-', '_'], " ")));
    let hit = q.iter().filter(|w| t.iter().any(|x| tok_eq(x, w))).count();
    (hit * 100 / q.len()) as i64
}

pub(crate) fn client() -> reqwest::Client {
    // follows the system proxy (VPN): web results are often on sites that need the user's proxy
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36")
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(6))
        .build()
        .unwrap_or_default()
}

/// Top web results for a product: searches once, keeps the most relevant non-marketplace hits,
/// reads each as a structured page. `want` is capped at 3. The engine is never spammed (one search
/// page); if it is unavailable the caller still gets an empty list, not a crash.
pub async fn results(page: &chromiumoxide::Page, query: &str, want: usize, token: &CancellationToken) -> Vec<WebResult> {
    let want = want.clamp(1, 3);
    let hits = match crate::websearch::search(page, query, 1, token).await {
        Ok(h) => h,
        Err(_) => return vec![],
    };
    let mut scored: Vec<(i64, crate::websearch::Hit)> = hits
        .into_iter()
        .filter(|h| !MARKETPLACES.iter().any(|m| host(&h.url).contains(m)))
        .map(|h| (relevance(query, &h.title, &h.url), h))
        .filter(|(r, _)| *r >= 50)
        .collect();
    scored.sort_by_key(|(r, _)| std::cmp::Reverse(*r));
    let now = crate::market::iso_utc(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64),
    );
    let mut out = vec![];
    for (rel, h) in scored.into_iter().take(want) {
        if token.is_cancelled() {
            break;
        }
        let domain = host(&h.url);
        let body = tokio::time::timeout(Duration::from_secs(18), client().get(&h.url).send())
            .await
            .ok()
            .and_then(|r| r.ok())
            .map(|r| async move { r.text().await.unwrap_or_default() });
        let html = match body {
            Some(f) => f.await,
            None => String::new(),
        };
        let prod = parse_product(&html);
        out.push(WebResult {
            title: if h.title.is_empty() { domain.clone() } else { h.title.clone() },
            domain: domain.clone(),
            url: h.url.clone(),
            product_url: if prod.is_product { h.url.clone() } else { String::new() },
            product_name: prod.name,
            brand: prod.brand,
            price: prod.price,
            availability: prod.availability,
            quality: classify_source(&domain, &html),
            page_type: classify_source(&domain, &html).fa().into(),
            relevance: rel,
            confidence: prod.confidence,
            fetched_at: now.clone(),
        });
    }
    out
}

// ---------------- «وب» as a product source ----------------

/// Does a shop's page show the eNamad trust seal (Iran's e-commerce licence)?
pub fn has_enamad(html: &str) -> bool {
    let l = html.to_lowercase();
    l.contains("trustseal.enamad.ir") || l.contains("enamad.ir/")
}

/// A shop's own site worth reading for prices: not a marketplace (they are their own sources),
/// blog, directory, social page or video site.
pub fn is_shop_site(domain: &str) -> bool {
    !matches!(classify_source(domain, ""), SourceQuality::Marketplace | SourceQuality::Blog | SourceQuality::Directory)
}

/// The «وب» source of product analysis: products with prices from brands' own sites and shops
/// (eNamad-licensed shops first), read with the same engine as «دریافت لیست محصول». With `site`,
/// only that source is read (a site, a Basalam booth, one page or a Telegram channel).
/// Returns the offers (platform "web") and an error text when nothing could be read.
pub async fn shop_offers(
    s: &crate::scanner::Settings,
    profile: &std::path::Path,
    query: &str,
    site: &str,
    limit: usize,
    token: &CancellationToken,
    emit: crate::scanner::Emit<'_>,
) -> (Vec<crate::market::Offer>, Option<String>) {
    use crate::products::{self, Scope};
    let site = site.trim();
    let limit = limit.clamp(10, 300);
    // 1) where to read: the given source, or the best shop sites a web search finds
    let (starts, scope): (Vec<String>, Scope) = if site.is_empty() {
        match search_shops(s, profile, &format!("{query} قیمت خرید"), token).await {
            Ok(hits) if !hits.is_empty() => {
                let hosts: Vec<String> = hits.iter().map(|u| host(u)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                (hits, Scope::Sites(hosts))
            }
            Ok(_) => return (vec![], Some("در جستجوی وب، سایت فروشگاهی مرتبطی پیدا نشد.".into())),
            Err(e) => return (vec![], Some(e)),
        }
    } else {
        let url = products::normalize(site);
        match products::scope_of(&url) {
            Err(e) => return (vec![], Some(e)),
            Ok(Scope::Site { host: h, whole: true }) => {
                // a whole shop: its own pages about this product (site search), else its home page
                let hits = search_shops(s, profile, &format!("site:{h} {query}"), token).await.unwrap_or_default();
                let hits: Vec<String> = hits.into_iter().filter(|u| host(u).ends_with(&h)).collect();
                (if hits.is_empty() { vec![url] } else { hits }, Scope::Site { host: h, whole: false })
            }
            Ok(sc) => (vec![url], sc),
        }
    };
    if token.is_cancelled() {
        return (vec![], None);
    }
    // 2) read them
    let items = if let Scope::Telegram(_) = scope {
        let req = products::ListReq {
            url: starts[0].clone(),
            limit: limit as i64,
            ..Default::default()
        };
        match products::run(s, profile, &req, token, emit).await {
            Ok(r) => r.items,
            Err(e) => return (vec![], Some(e)),
        }
    } else {
        let pages = starts.len() + 4;
        match products::crawl(s, profile, &starts, &scope, limit, pages, token, emit).await {
            Ok((i, _, _)) => i,
            Err(e) => return (vec![], Some(e)),
        }
    };
    // 3) trust: shops showing the eNamad seal come first and say so
    let hosts: std::collections::BTreeSet<String> = items.iter().map(|i| host(&i.url)).filter(|h| !h.is_empty() && h != "t.me").collect();
    let http = client();
    let checks = futures::future::join_all(hosts.iter().map(|h| {
        let http = http.clone();
        async move {
            let html = async { http.get(format!("https://{h}/")).send().await.ok()?.text().await.ok() };
            let ok = tokio::time::timeout(Duration::from_secs(10), html).await.ok().flatten().is_some_and(|x| has_enamad(&x));
            (h.clone(), ok)
        }
    }))
    .await;
    let enamad: std::collections::HashSet<String> = checks.into_iter().filter(|(_, ok)| *ok).map(|(h, _)| h).collect();
    let mut out: Vec<crate::market::Offer> = items
        .into_iter()
        .map(|i| {
            let h = host(&i.url);
            let trusted = enamad.contains(&h);
            let seller = if !i.seller.is_empty() { i.seller } else { h.clone() };
            crate::market::Offer {
                platform: "web".into(),
                title: i.title,
                brand: i.brand,
                price: i.price,
                seller: if trusted { format!("{seller} ✓ اینماد") } else { seller },
                rating: i.rating,
                reviews: i.reviews,
                url: crate::links::canonical(&i.url),
                image: i.image,
                sales: i.sales,
                updated: i.date,
                location: i.location,
                seller_url: if i.seller_url.is_empty() { format!("https://{h}") } else { i.seller_url },
                link: "exact".into(), // read from the shop's own page
                ..Default::default()
            }
        })
        .collect();
    out.sort_by_key(|o| !o.seller.ends_with("اینماد")); // stable: site order kept within each group
    (out, None)
}

/// Result links of a web search that point at shop sites (not marketplaces / blogs), at most 8,
/// one or two per site. Uses the app's browser for one search (Bing, then DuckDuckGo).
async fn search_shops(s: &crate::scanner::Settings, profile: &std::path::Path, q: &str, token: &CancellationToken) -> Result<Vec<String>, String> {
    let eng = crate::scanner::launch(s, profile, s.headless, 0).await?;
    let hits = crate::websearch::search(&eng.list, q, 2, token).await;
    crate::scanner::shut(Some(eng)).await;
    let mut per_host: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    Ok(hits?
        .into_iter()
        .map(|h| h.url)
        .filter(|u| u.starts_with("http") && is_shop_site(&host(u)))
        .filter(|u| {
            let n = per_host.entry(host(u)).or_default();
            *n += 1;
            *n <= 2
        })
        .take(8)
        .collect())
}

/// Turns a supplier page into a card: classifies it, and reads only the public business contact the
/// page itself shows (never the advertiser-contact flow of a classifieds ad).
pub fn supplier_from(domain: &str, title: &str, html: &str, url: &str, category: &str) -> Option<Supplier> {
    let (kind, confidence, signals) = classify_supplier(domain, title, html);
    if matches!(kind, SupplierKind::Unknown | SupplierKind::Retailer) && confidence < 40 {
        return None; // not enough to call it a supplier — don't pretend
    }
    let mut c = Contacts::default();
    extract_contacts(html, &mut c);
    let now = crate::market::iso_utc(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64),
    );
    Some(Supplier {
        name: title.trim().chars().take(80).collect(),
        kind: kind.fa().into(),
        category: category.to_string(),
        website: format!("https://{domain}"),
        phone: c.landlines.first().or_else(|| c.mobiles.first()).cloned().unwrap_or_default(),
        email: c.emails.first().cloned().unwrap_or_default(),
        social: [("اینستاگرام", &c.instagram), ("تلگرام", &c.telegram)]
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, _)| k.to_string())
            .collect(),
        moq: if norm(html).contains(&norm("حداقل سفارش")) {
            "دارد (در صفحه)".into()
        } else {
            String::new()
        },
        wholesale: true,
        terms: signals,
        confidence,
        source: domain.to_string(),
        url: url.to_string(),
        last_checked: now,
        ..Default::default()
    })
}

/// B2B suppliers for a product: expands the query («… عمده / تولیدکننده / پخش …»), searches once per
/// cue (paced, never spammed), fetches each distinct own-site hit and classifies it. Retail listings
/// and marketplaces are left out. At most ~8 suppliers.
pub async fn suppliers(page: &chromiumoxide::Page, product: &str, token: &CancellationToken) -> Vec<Supplier> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<Supplier> = vec![];
    for q in b2b_queries(product).into_iter().take(4) {
        if token.is_cancelled() || out.len() >= 8 {
            break;
        }
        let hits = crate::websearch::search(page, &q, 1, token).await.unwrap_or_default();
        for h in hits.into_iter().take(6) {
            let domain = host(&h.url);
            if domain.is_empty()
                || MARKETPLACES.iter().any(|m| domain.contains(m))
                || BLOGS.iter().any(|b| domain.contains(b))
                || DIRECTORIES.iter().any(|b| domain.contains(b))
            {
                continue;
            }
            if !seen.insert(domain.clone()) {
                continue;
            }
            let html = tokio::time::timeout(Duration::from_secs(16), client().get(&h.url).send())
                .await
                .ok()
                .and_then(|r| r.ok());
            let html = match html {
                Some(r) => r.text().await.unwrap_or_default(),
                None => continue,
            };
            if let Some(s) = supplier_from(&domain, &h.title, &html, &h.url, product) {
                out.push(s);
            }
            if out.len() >= 8 {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(700)).await; // polite pacing
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.confidence));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LD: &str = r#"<html><head>
      <script type="application/ld+json">{"@context":"https://schema.org","@type":"Product","name":"عسل طبیعی خوانسار ۱ کیلویی","brand":{"@type":"Brand","name":"خوانسار"},"offers":{"@type":"Offer","price":"5800000","priceCurrency":"IRR","availability":"https://schema.org/InStock"}}</script>
      <meta property="og:title" content="x"></head><body>قیمت عمده و فروش عمده مستقیم از تولیدکننده، حداقل سفارش ۱۰ کیلو</body></html>"#;

    #[test]
    fn shop_sources() {
        assert!(has_enamad(r#"<a referrerpolicy="origin" href="https://trustseal.enamad.ir/?id=123"><img src="https://trustseal.enamad.ir/logo.aspx?id=123"></a>"#));
        assert!(!has_enamad("<html>no seal</html>"));
        assert!(is_shop_site("asal-khansar.ir") && !is_shop_site("torob.com") && !is_shop_site("virgool.io") && !is_shop_site("t.me"));
    }

    #[test]
    fn json_ld_product() {
        let p = parse_product(LD);
        assert!(p.is_product);
        assert_eq!(p.name, "عسل طبیعی خوانسار ۱ کیلویی");
        assert_eq!(p.brand, "خوانسار");
        assert_eq!(p.price, 580_000, "Rial → Toman");
        assert_eq!(p.availability, "موجود");
        assert_eq!(p.confidence, 90);
        // a page with no structured data: nothing invented
        let empty = parse_product("<html><body>قیمت تماس بگیرید</body></html>");
        assert!(!empty.is_product && empty.price == 0 && empty.brand.is_empty());
    }

    #[test]
    fn og_fallback() {
        let og = r#"<meta property="og:title" content="عسل سبلان"><meta property="product:price:amount" content="640000"><meta property="product:price:currency" content="IRR">"#;
        let p = parse_product(og);
        assert_eq!(p.name, "عسل سبلان");
        assert_eq!(p.price, 64_000);
    }

    #[test]
    fn supplier_classification() {
        let (k, score, sig) = classify_supplier(
            "asalkhoonsar.ir",
            "تولید و پخش عسل",
            "فروش عمده مستقیم از کارخانه، قیمت همکاری و حداقل سفارش ۱۰ کیلو، فاکتور رسمی",
        );
        assert_eq!(k, SupplierKind::Manufacturer);
        assert!(score >= 60, "{score}");
        assert!(sig.iter().any(|s| s == "قیمت همکاری") && sig.iter().any(|s| s == "فاکتور رسمی"));
        // a single retail listing is not a supplier
        let (k2, s2, _) = classify_supplier("digikala.com", "عسل خوانسار", "افزودن به سبد خرید، گارانتی ۱۸ ماهه");
        assert!(matches!(k2, SupplierKind::Retailer | SupplierKind::Unknown) && s2 < 40, "{k2:?} {s2}");
        assert!(supplier_from("digikala.com", "عسل", "افزودن به سبد", "https://digikala.com/x", "عسل").is_none());
    }

    #[test]
    fn source_quality_and_relevance() {
        assert_eq!(classify_source("digikala.com", ""), SourceQuality::Marketplace);
        assert_eq!(classify_source("virgool.io", ""), SourceQuality::Blog);
        assert_eq!(classify_source("asalkhoonsar.ir", "تولیدکننده و کارخانه عسل"), SourceQuality::Manufacturer);
        assert_eq!(relevance("عسل خوانسار", "عسل طبیعی خوانسار ۱ کیلویی", "https://x.ir/asal"), 100);
        assert!(relevance("عسل خوانسار", "فروش لوازم خانگی", "https://x.ir/home") < 50);
        assert_eq!(b2b_queries("عسل").len(), 8);
    }

    #[test]
    fn rfq_message() {
        let t = rfq_text(&Rfq {
            product: "عسل خوانسار".into(),
            quantity: "۵۰ کیلوگرم".into(),
            city: "اصفهان".into(),
            packaging: "سطل ۱ کیلویی".into(),
            ..Default::default()
        });
        assert!(t.contains("عسل خوانسار") && t.contains("۵۰ کیلوگرم") && t.contains("اصفهان"));
        assert!(t.contains("قیمت همکاری/عمده") && t.contains("حداقل سفارش"));
        assert!(t.contains("سطل ۱ کیلویی"));
        // a near-empty request still produces a valid message
        assert!(rfq_text(&Rfq {
            product: "زعفران".into(),
            ..Default::default()
        })
        .contains("زعفران"));
    }
}
