//! Entity-level links. Every offer carries the link of *that* product (and, when known, of *that*
//! seller / booth): never a platform's home page, never its search page, never a photo.
//!
//! * `from_json` builds the links from the ids a platform's own data gives (Digikala's `dkp-` id,
//!   Basalam's vendor identifier + product id, Divar's post token …) — the most reliable source;
//! * `canonical` cleans a link (absolute, https, no tracking parameters, no fragment);
//! * `kind` says what a link points at, from the platform's URL patterns;
//! * `check` = "exact" only when the link is a product / ad page on that platform's own domain.
//!   Anything else is "unverified" and the UI says so; nothing is ever guessed.
use serde_json::{Map, Value};

/// The domain each platform's pages live on.
pub fn domain(platform: &str) -> Option<&'static str> {
    Some(match platform {
        "torob" => "torob.com",
        "digikala" => "digikala.com",
        "basalam" => "basalam.com",
        "emalls" => "emalls.ir",
        "divar" => "divar.ir",
        "sheypoor" => "sheypoor.com",
        _ => return None,
    })
}

pub fn host(url: &str) -> String {
    url.split("//").nth(1).unwrap_or("").split(['/', '?', '#']).next().unwrap_or("").trim_start_matches("www.").to_lowercase()
}

fn path(url: &str) -> &str {
    let rest = url.split("//").nth(1).unwrap_or(url);
    rest.find('/').map_or("/", |i| rest[i..].split(['?', '#']).next().unwrap_or("/"))
}

const TRACKING: &[&str] = &["utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content", "gclid", "fbclid", "yclid", "_ga", "ref", "spm", "from", "source"];

/// Absolute https link without tracking parameters or fragment ("" stays "").
pub fn canonical(url: &str) -> String {
    let u = url.trim();
    if u.is_empty() {
        return String::new();
    }
    let u = if let Some(r) = u.strip_prefix("//") { format!("https://{r}") } else { u.to_string() };
    let u = match u.strip_prefix("http://") {
        Some(r) => format!("https://{r}"),
        None => u,
    };
    if !u.starts_with("https://") {
        return u;
    }
    let u = u.split('#').next().unwrap_or("").to_string();
    let Some((base, query)) = u.split_once('?') else { return u };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|kv| !kv.is_empty())
        .filter(|kv| {
            let k = kv.split('=').next().unwrap_or("").to_lowercase();
            !TRACKING.contains(&k.as_str()) && !k.starts_with("utm_")
        })
        .collect();
    if kept.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", kept.join("&"))
    }
}

/// Site-relative link made absolute on the platform's domain.
pub fn absolute(platform: &str, url: &str) -> String {
    let u = url.trim();
    if u.starts_with('/') && !u.starts_with("//") {
        if let Some(d) = domain(platform) {
            let www = if matches!(platform, "digikala" | "sheypoor") { "www." } else { "" };
            return format!("https://{www}{d}{u}");
        }
    }
    u.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One product / one ad.
    Product,
    /// One seller, shop or booth.
    Seller,
    Search,
    Home,
    Image,
    Other,
}

const IMAGE_EXT: &[&str] = &[".jpg", ".jpeg", ".png", ".webp", ".gif", ".svg", ".avif"];

fn segs(p: &str) -> Vec<&str> {
    p.split('/').filter(|s| !s.is_empty()).collect()
}

/// What `url` points at on `platform` (any platform: "web").
pub fn kind(platform: &str, url: &str) -> Kind {
    let p = path(url);
    let low = p.to_lowercase();
    let h = host(url);
    if IMAGE_EXT.iter().any(|e| low.ends_with(e)) || h.contains("cdn") || h.starts_with("static") || h.starts_with("img") || h.starts_with("statics") {
        return Kind::Image;
    }
    if url.contains("?q=") || url.contains("&q=") || url.contains("query=") || low.contains("/search") || low.starts_with("/s/") {
        return Kind::Search;
    }
    let s = segs(p);
    if s.is_empty() {
        return Kind::Home;
    }
    match platform {
        "torob" => match s[0] {
            "p" if s.len() >= 2 => Kind::Product,
            "shop" if s.len() >= 2 => Kind::Seller,
            _ => Kind::Other,
        },
        "digikala" => match s[0] {
            "product" if s.get(1).is_some_and(|x| x.starts_with("dkp-")) => Kind::Product,
            "seller" if s.len() >= 2 => Kind::Seller,
            _ => Kind::Other,
        },
        "basalam" => {
            if s.contains(&"product") && s.last().is_some_and(|x| x.chars().all(|c| c.is_ascii_digit())) {
                Kind::Product
            } else if s.len() == 1 && !RESERVED_BASALAM.contains(&s[0]) {
                Kind::Seller
            } else {
                Kind::Other
            }
        }
        "emalls" => {
            if s[0].eq_ignore_ascii_case("shop") || s[0].to_lowercase().starts_with("shop") {
                Kind::Seller
            } else if s[0].split('_').next().is_some_and(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit())) {
                Kind::Product
            } else {
                Kind::Other
            }
        }
        "divar" | "sheypoor" => match s[0] {
            "v" if s.len() >= 2 => Kind::Product,
            "shops" | "shop" | "marketplace" | "brand" | "store" if s.len() >= 2 => Kind::Seller,
            _ => Kind::Other,
        },
        _ => Kind::Other,
    }
}

const RESERVED_BASALAM: &[&str] = &["search", "accounts", "cart", "profile", "category", "categories", "blog", "about", "help", "landing", "p", "product"];

/// "exact": a product / ad page on the platform's own domain; "unverified": a link that could not
/// be proven to be that product's page; "": no link.
pub fn check(platform: &str, url: &str) -> &'static str {
    if url.is_empty() {
        return "";
    }
    let on_domain = domain(platform).is_some_and(|d| host(url) == d || host(url).ends_with(&format!(".{d}")));
    if on_domain && kind(platform, url) == Kind::Product {
        "exact"
    } else {
        "unverified"
    }
}

fn s<'a>(o: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    o.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty())
}

fn id(o: &Map<String, Value>, k: &str) -> Option<String> {
    match o.get(k)? {
        Value::Number(n) => n.as_u64().map(|n| n.to_string()),
        Value::String(x) if !x.trim().is_empty() && x.trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') => Some(x.trim().to_string()),
        _ => None,
    }
}

fn obj<'a>(o: &'a Map<String, Value>, path: &[&str]) -> Option<&'a Map<String, Value>> {
    path.iter().try_fold(o, |cur, k| cur.get(*k).and_then(Value::as_object))
}

/// A link held as text or as `{uri: "/product/…"}` / `{url: "…"}` / `{base, uri}`.
fn link_value(v: &Value) -> Option<String> {
    match v {
        Value::String(x) if !x.trim().is_empty() => Some(x.trim().to_string()),
        Value::Object(o) => ["uri", "url", "href"].iter().find_map(|k| o.get(*k).and_then(Value::as_str)).map(|u| {
            let base = o.get("base").and_then(Value::as_str).unwrap_or("");
            format!("{base}{u}")
        }),
        _ => None,
    }
}

/// (product link, seller link) from one product object of a platform's JSON. Only links built from
/// the platform's own ids or its own link fields; empty when there is none.
pub fn from_json(platform: &str, o: &Map<String, Value>) -> (String, String) {
    let fix = |u: String| canonical(&absolute(platform, &u));
    let own = |u: &str| kind(platform, &fix(u.to_string()));
    let mut product = String::new();
    let mut seller = String::new();
    match platform {
        "digikala" => {
            if let Some(u) = o.get("url").and_then(link_value).filter(|u| own(u) == Kind::Product) {
                product = fix(u);
            } else if let Some(i) = id(o, "id").filter(|i| i.chars().all(|c| c.is_ascii_digit())) {
                product = format!("https://www.digikala.com/product/dkp-{i}/");
            }
            let sel = obj(o, &["default_variant", "seller"]).or_else(|| obj(o, &["seller"]));
            if let Some(sel) = sel {
                if let Some(u) = sel.get("url").and_then(link_value).filter(|u| own(u) == Kind::Seller) {
                    seller = fix(u);
                } else if let Some(c) = id(sel, "code") {
                    seller = format!("https://www.digikala.com/seller/{c}/");
                }
            }
        }
        "basalam" => {
            let vendor = obj(o, &["vendor"]);
            let ident = vendor.and_then(|v| s(v, "identifier")).filter(|x| x.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'));
            if let Some(u) = ["url", "link", "product_url"].iter().find_map(|k| o.get(*k).and_then(link_value)).filter(|u| own(u) == Kind::Product) {
                product = fix(u);
            } else if let (Some(v), Some(i)) = (ident, id(o, "id")) {
                product = format!("https://basalam.com/{v}/product/{i}");
            }
            if let Some(v) = ident {
                seller = format!("https://basalam.com/{v}");
            }
        }
        "divar" => {
            if let Some(t) = id(o, "token") {
                product = format!("https://divar.ir/v/{t}");
            }
        }
        _ => {}
    }
    if product.is_empty() {
        // the platform's own link fields, accepted only when they point at a product page
        for k in ["web_client_absolute_url", "url", "link", "uri", "page_url", "href"] {
            if let Some(u) = o.get(k).and_then(link_value) {
                let f = fix(u);
                if kind(platform, &f) == Kind::Product {
                    product = f;
                    break;
                }
            }
        }
    }
    if seller.is_empty() {
        for k in ["shop_url", "seller_url", "vendor_url", "store_url"] {
            if let Some(u) = o.get(k).and_then(link_value) {
                let f = fix(u);
                if kind(platform, &f) == Kind::Seller {
                    seller = f;
                    break;
                }
            }
        }
    }
    (product, seller)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn m(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn canonical_links() {
        assert_eq!(canonical("http://torob.com/p/abc/?utm_source=x&a=1#top"), "https://torob.com/p/abc/?a=1");
        assert_eq!(canonical("//basalam.com/x/product/5?utm_medium=y"), "https://basalam.com/x/product/5");
        assert_eq!(canonical(""), "");
        assert_eq!(absolute("digikala", "/product/dkp-1/"), "https://www.digikala.com/product/dkp-1/");
        assert_eq!(absolute("torob", "https://x.ir/a"), "https://x.ir/a");
    }

    #[test]
    fn kinds_and_checks() {
        assert_eq!(kind("basalam", "https://basalam.com/shahd_azadi/product/123456"), Kind::Product);
        assert_eq!(kind("basalam", "https://basalam.com/shahd_azadi"), Kind::Seller);
        assert_eq!(kind("basalam", "https://basalam.com/"), Kind::Home);
        assert_eq!(kind("basalam", "https://basalam.com/search?q=عسل"), Kind::Search);
        assert_eq!(kind("basalam", "https://statics.basalam.com/public/a.jpg"), Kind::Image);
        assert_eq!(kind("digikala", "https://www.digikala.com/product/dkp-998/عسل/"), Kind::Product);
        assert_eq!(kind("digikala", "https://www.digikala.com/seller/abc12/"), Kind::Seller);
        assert_eq!(kind("torob", "https://torob.com/p/1a2b/عسل-خوانسار/"), Kind::Product);
        assert_eq!(kind("divar", "https://divar.ir/v/عسل/gaEHxMzg"), Kind::Product);
        assert_eq!(kind("divar", "https://divar.ir/s/tehran?q=عسل"), Kind::Search);
        assert_eq!(kind("emalls", "https://emalls.ir/4471100_عسل/"), Kind::Product);
        assert_eq!(kind("emalls", "https://emalls.ir/Shop/123_x/"), Kind::Seller);
        assert_eq!(check("basalam", "https://basalam.com/shahd/product/12"), "exact");
        assert_eq!(check("basalam", "https://basalam.com/"), "unverified", "home page is never «exact»");
        assert_eq!(check("basalam", "https://torob.com/p/x/"), "unverified", "another domain");
        assert_eq!(check("torob", ""), "");
    }

    #[test]
    fn links_from_platform_json() {
        // Digikala: {url: {uri}} and the seller's code
        let dk = m(json!({"id": 998, "title_fa": "عسل", "url": {"base": null, "uri": "/product/dkp-998/honey/"}, "default_variant": {"seller": {"code": "a1b2", "title": "الف"}}}));
        assert_eq!(from_json("digikala", &dk), ("https://www.digikala.com/product/dkp-998/honey/".into(), "https://www.digikala.com/seller/a1b2/".into()));
        let dk2 = m(json!({"id": 7, "title_fa": "عسل"}));
        assert_eq!(from_json("digikala", &dk2).0, "https://www.digikala.com/product/dkp-7/");
        // Basalam: vendor identifier + product id; a photo url is never taken as the product link
        let bs = m(json!({"id": 123456, "name": "عسل", "photo": {"url": "https://statics.basalam.com/p.jpg"}, "vendor": {"identifier": "shahd_azadi", "name": "شهد آزادی"}}));
        assert_eq!(from_json("basalam", &bs), ("https://basalam.com/shahd_azadi/product/123456".into(), "https://basalam.com/shahd_azadi".into()));
        let bs2 = m(json!({"id": 1, "name": "عسل", "url": "https://statics.basalam.com/p.jpg"}));
        assert_eq!(from_json("basalam", &bs2), (String::new(), String::new()), "no guessed link");
        // Torob: relative product page
        let tb = m(json!({"name1": "عسل", "web_client_absolute_url": "/p/abc/عسل/"}));
        assert_eq!(from_json("torob", &tb).0, "https://torob.com/p/abc/عسل/");
        // Divar post token
        assert_eq!(from_json("divar", &m(json!({"token": "gaEHxMzg", "title": "عسل"}))).0, "https://divar.ir/v/gaEHxMzg");
    }
}
