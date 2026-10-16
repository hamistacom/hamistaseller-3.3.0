//! Product & price intelligence («محصول و قیمت»): public product listings of Iranian shops
//! (Torob, Digikala, Basalam, Emalls, Divar, Sheypoor) → one list of offers → market statistics, a price
//! recommendation, a supplier value ranking and a buy/sell recommendation.
//!
//! Collecting is deliberately schema-tolerant: every platform is read as JSON when it offers it
//! (its public search API, or the page's embedded `__NEXT_DATA__` / `__PRELOADED_STATE__`), and any JSON is walked for
//! "objects that look like a product with a price". Only public business data is read: product
//! names, prices, shop names, ratings — never personal data.
use crate::db::{Db, Res};
use crate::scanner::{self, Emit, ScanEvent, Settings};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

// ---------------- platforms ----------------

/// Sort ids (UI labels in pages/market.js): relevance, popular, bestselling, reviews, cheap,
/// expensive, newest. Each platform maps the ones it supports; the rest are applied locally.
pub struct Platform {
    pub id: &'static str,
    pub name: &'static str,
    /// Public search API (JSON). Placeholders: `{q}` query, `{page}` 0-based / `{page1}` 1-based
    /// page, `{from}` offset, `{sort}` the platform's sort value.
    api: &'static str,
    /// Public search page (opened in the browser when the API does not answer). `{city}` = the
    /// classifieds' city slug.
    page: &'static str,
    /// Prices the platform shows in Rial (÷10 → Toman).
    rial: bool,
    per_page: usize,
    sorts: &'static [(&'static str, &'static str)],
    /// Autocomplete API: the platform's own popular / related searches.
    suggest: &'static str,
    /// Where the user signs in, for platforms that show more to signed-in users.
    pub login: &'static str,
}

pub const PLATFORMS: &[Platform] = &[
    Platform {
        id: "torob",
        name: "ترب",
        api: "https://api.torob.com/v4/base-product/search/?page={page}&sort={sort}&size=24&query={q}&q={q}&source=next_desktop",
        page: "https://torob.com/search/?query={q}",
        rial: false,
        per_page: 24,
        sorts: &[
            ("relevance", "popularity"),
            ("popular", "popularity"),
            ("bestselling", "popularity"),
            ("reviews", "popularity"),
            ("cheap", "price"),
            ("expensive", "-price"),
            ("newest", "date_added"),
        ],
        suggest: "https://api.torob.com/v4/base-product/suggestion/?q={q}",
        login: "",
    },
    Platform {
        id: "digikala",
        name: "دیجی‌کالا",
        api: "https://api.digikala.com/v1/search/?q={q}&page={page1}&sort={sort}",
        page: "https://www.digikala.com/search/?q={q}",
        rial: true,
        per_page: 20,
        sorts: &[
            ("relevance", "22"),
            ("popular", "4"),
            ("bestselling", "7"),
            ("reviews", "27"),
            ("cheap", "20"),
            ("expensive", "21"),
            ("newest", "1"),
        ],
        suggest: "https://api.digikala.com/v1/autocomplete/?q={q}",
        login: "https://www.digikala.com/users/login/",
    },
    Platform {
        id: "basalam",
        name: "باسلام",
        api: "https://search.basalam.com/ai-engine/api/v2.0/product/search?from={from}&q={q}&dynamicFacets=true&size=24",
        page: "https://basalam.com/search?q={q}",
        rial: true,
        per_page: 24,
        sorts: &[],
        suggest: "",
        login: "https://basalam.com/accounts/login",
    },
    Platform {
        id: "emalls",
        name: "ایمالز",
        api: "",
        page: "https://emalls.ir/Search.aspx?q={q}",
        rial: false,
        per_page: 30,
        sorts: &[],
        suggest: "",
        login: "",
    },
    // classifieds: only the asking price, title, photo, city and time of public ads are read —
    // never the advertiser's contact details
    Platform {
        id: "divar",
        name: "دیوار",
        api: "",
        page: "https://divar.ir/s/{city}?q={q}",
        rial: false,
        per_page: 24,
        sorts: &[],
        suggest: "",
        login: "https://divar.ir/my-divar",
    },
    Platform {
        id: "sheypoor",
        name: "شیپور",
        api: "",
        page: "https://www.sheypoor.com/s/{city}?q={q}",
        rial: false,
        per_page: 24,
        sorts: &[],
        suggest: "",
        login: "https://www.sheypoor.com/session",
    },
];

/// City slugs the classifieds use in their URLs (province centres and large cities).
const CITY_SLUGS: &[(&str, &str)] = &[
    ("تهران", "tehran"),
    ("کرج", "karaj"),
    ("مشهد", "mashhad"),
    ("اصفهان", "isfahan"),
    ("تبریز", "tabriz"),
    ("شیراز", "shiraz"),
    ("اهواز", "ahvaz"),
    ("قم", "qom"),
    ("کرمانشاه", "kermanshah"),
    ("ارومیه", "urmia"),
    ("رشت", "rasht"),
    ("زاهدان", "zahedan"),
    ("همدان", "hamedan"),
    ("کرمان", "kerman"),
    ("یزد", "yazd"),
    ("اردبیل", "ardabil"),
    ("بندرعباس", "bandar-abbas"),
    ("اراک", "arak"),
    ("زنجان", "zanjan"),
    ("سنندج", "sanandaj"),
    ("قزوین", "qazvin"),
    ("خرم‌آباد", "khorramabad"),
    ("گرگان", "gorgan"),
    ("ساری", "sari"),
    ("بوشهر", "bushehr"),
    ("بیرجند", "birjand"),
    ("بجنورد", "bojnurd"),
    ("ایلام", "ilam"),
    ("سمنان", "semnan"),
    ("شهرکرد", "shahrekord"),
    ("یاسوج", "yasuj"),
    ("کاشان", "kashan"),
    ("بابل", "babol"),
    ("آمل", "amol"),
    ("دزفول", "dezful"),
    ("نجف‌آباد", "najafabad"),
    ("نیشابور", "neyshabur"),
    ("سبزوار", "sabzevar"),
    ("اسلامشهر", "eslamshahr"),
    ("شهریار", "shahriar"),
    ("قدس", "qods"),
    ("پاکدشت", "pakdasht"),
];

const PROVINCE_CENTRES: &[(&str, &str)] = &[
    ("آذربایجان شرقی", "تبریز"),
    ("آذربایجان غربی", "ارومیه"),
    ("اردبیل", "اردبیل"),
    ("اصفهان", "اصفهان"),
    ("البرز", "کرج"),
    ("ایلام", "ایلام"),
    ("بوشهر", "بوشهر"),
    ("تهران", "تهران"),
    ("چهارمحال و بختیاری", "شهرکرد"),
    ("خراسان جنوبی", "بیرجند"),
    ("خراسان رضوی", "مشهد"),
    ("خراسان شمالی", "بجنورد"),
    ("خوزستان", "اهواز"),
    ("زنجان", "زنجان"),
    ("سمنان", "سمنان"),
    ("سیستان و بلوچستان", "زاهدان"),
    ("فارس", "شیراز"),
    ("قزوین", "قزوین"),
    ("قم", "قم"),
    ("کردستان", "سنندج"),
    ("کرمان", "کرمان"),
    ("کرمانشاه", "کرمانشاه"),
    ("کهگیلویه و بویراحمد", "یاسوج"),
    ("گلستان", "گرگان"),
    ("گیلان", "رشت"),
    ("لرستان", "خرم‌آباد"),
    ("مازندران", "ساری"),
    ("مرکزی", "اراک"),
    ("هرمزگان", "بندرعباس"),
    ("همدان", "همدان"),
    ("یزد", "یزد"),
];

/// The classifieds' slug for a city (or the province centre); "iran" = the whole country.
pub fn city_slug(province: &str, city: &str) -> &'static str {
    let by_name = |c: &str| CITY_SLUGS.iter().find(|(n, _)| norm(n) == norm(c)).map(|(_, s)| *s);
    by_name(city)
        .or_else(|| PROVINCE_CENTRES.iter().find(|(p, _)| norm(p) == norm(province)).and_then(|(_, c)| by_name(c)))
        .unwrap_or("iran")
}

/// Site-relative links made absolute and cleaned; a link that is not this product's own page
/// (home, search, photo, another site) is dropped instead of being shown as the product's link.
pub fn verify_links(o: &mut Offer) {
    use crate::links::{absolute, canonical, check, kind, Kind};
    o.url = canonical(&absolute(&o.platform, &o.url));
    if matches!(kind(&o.platform, &o.url), Kind::Home | Kind::Search | Kind::Image) {
        o.url.clear();
    }
    o.link = check(&o.platform, &o.url).to_string();
    o.seller_url = canonical(&absolute(&o.platform, &o.seller_url));
    if !o.seller_url.is_empty() && kind(&o.platform, &o.seller_url) != Kind::Seller {
        o.seller_url.clear();
    }
}

pub fn platform(id: &str) -> Option<&'static Platform> {
    PLATFORMS.iter().find(|p| p.id == id)
}

// ---------------- offers ----------------

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Offer {
    pub platform: String,
    pub title: String,
    pub brand: String,
    pub price: i64, // Toman
    pub seller: String,
    pub rating: Option<f64>, // 0..5
    pub reviews: i64,
    pub url: String,
    pub shops: i64,              // aggregators: how many shops sell it
    pub unit_price: Option<i64>, // Toman per kg / litre, when the title states a size
    pub value: f64,              // «امتیاز ارزش خرید» 0..100 (set by `analyze`)
    pub image: String,           // product photo (thumbnail)
    pub sales: i64,              // sold count, when the platform shows it
    pub updated: String,         // last price/listing update (ISO date-time), when known
    pub location: String,        // seller / ad city, when known
    /// This seller's / booth's own page on the platform, when the platform's data names it.
    pub seller_url: String,
    /// "exact" = `url` is proven to be this product's page on the platform; "unverified"; "" = no link.
    pub link: String,
}

const TITLE_KEYS: &[&str] = &[
    "title_fa",
    "title",
    "name1",
    "name",
    "product_name",
    "persian_name",
    "titleFa",
];
const PRICE_KEYS: &[&str] = &[
    "selling_price",
    "final_price",
    "price",
    "min_price",
    "price_toman",
    "minPrice",
    "sellingPrice",
    "primaryPrice",
    "lowest_price",
    "normal_text",
    "price_text",
    "amount",
];
const SELLER_KEYS: &[&str] = &[
    "shop_name",
    "shop_title",
    "seller_name",
    "vendor_name",
    "store_name",
    "shop",
    "seller",
    "vendor",
    "store",
    "bottom_description_text",
];
const RATING_KEYS: &[&str] = &[
    "rate",
    "rating",
    "average",
    "score",
    "total_rate",
    "rating_average",
    "shop_score",
];
const REVIEW_KEYS: &[&str] = &[
    "count",
    "rating_count",
    "review_count",
    "reviews_count",
    "comments_count",
    "rates_count",
    "signals",
];
const URL_KEYS: &[&str] = &[
    "web_client_absolute_url",
    "url",
    "link",
    "uri",
    "more_info_url",
    "page_url",
];
const BRAND_KEYS: &[&str] = &[
    "brand",
    "brand_name",
    "brand_title",
    "brandName",
];
const IMAGE_KEYS: &[&str] = &[
    "image_url",
    "image",
    "main_image",
    "thumbnail",
    "thumbnail_url",
    "imageUrl",
    "photo",
    "images",
    "img",
    "picture",
    "main",
];
const SALES_KEYS: &[&str] = &[
    "sales_count",
    "salesCount",
    "sold_count",
    "sold",
    "order_count",
    "orders_count",
    "purchase_count",
    "sales",
];
const DATE_KEYS: &[&str] = &[
    "price_updated_at",
    "last_price_update",
    "updated_at",
    "updatedAt",
    "last_update",
    "modified_at",
    "modified",
    "updated",
    "published_at",
    "created_at",
    "date",
];
const LOCATION_KEYS: &[&str] = &[
    "location",
    "city",
    "vendor_city",
    "city_name",
    "province",
    "district",
];

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let d: String = crate::logic::ascii_digits(s)
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            d.parse().ok()
        }
        _ => None,
    }
}

fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Object(o) => TITLE_KEYS
            .iter()
            .find_map(|k| o.get(*k).and_then(text)),
        _ => None,
    }
}

/// First value under any of `keys`, looking into nested objects up to `depth` levels.
fn find<'a>(o: &'a serde_json::Map<String, Value>, keys: &[&str], depth: usize) -> Option<&'a Value> {
    for k in keys {
        if let Some(v) = o.get(*k) {
            if !v.is_null() {
                return Some(v);
            }
        }
    }
    if depth == 0 {
        return None;
    }
    o.values()
        .filter_map(Value::as_object)
        .find_map(|c| find(c, keys, depth - 1))
}

/// First number under any of `keys` (a key holding an object is searched inside), up to `depth`.
fn find_num(o: &serde_json::Map<String, Value>, keys: &[&str], depth: usize) -> Option<f64> {
    for k in keys {
        match o.get(*k) {
            Some(Value::Object(c)) => {
                if let Some(n) = find_num(c, keys, depth.saturating_sub(1)) {
                    return Some(n);
                }
            }
            Some(v) => {
                if let Some(n) = num(v) {
                    return Some(n);
                }
            }
            None => {}
        }
    }
    if depth == 0 {
        return None;
    }
    o.values()
        .filter_map(Value::as_object)
        .find_map(|c| find_num(c, keys, depth - 1))
}

fn price_in(o: &serde_json::Map<String, Value>) -> Option<f64> {
    price_at(o, 3)
}

/// A price may be a number, a string, or an object ({selling_price, rrp_price} …), possibly nested
/// a few levels down (default_variant → price → selling_price). Listed keys win over nested ones.
fn price_at(o: &serde_json::Map<String, Value>, depth: usize) -> Option<f64> {
    for k in PRICE_KEYS {
        if let Some(v) = o.get(*k) {
            if let Some(n) = num(v).filter(|n| *n > 0.0) {
                return Some(n);
            }
            if let Some(p) = v.as_object().and_then(|c| price_at(c, depth.saturating_sub(1))) {
                return Some(p);
            }
        }
    }
    if depth == 0 {
        return None;
    }
    // nested product parts (not other products: arrays are skipped)
    o.iter()
        .filter(|(k, _)| !TITLE_KEYS.contains(&k.as_str()))
        .filter_map(|(_, v)| v.as_object())
        .filter(|c| !TITLE_KEYS.iter().any(|t| c.contains_key(*t)) || c.len() < 4)
        .take(8)
        .find_map(|c| price_at(c, depth - 1))
}

/// The first image URL in a value: a string, `{url: […]}`, `{MEDIUM: …}`, a list …
fn image_of(v: &Value, depth: usize) -> Option<String> {
    let ok = |u: &str| {
        let u = u.trim();
        let u = if let Some(rest) = u.strip_prefix("//") {
            format!("https://{rest}")
        } else {
            u.to_string()
        };
        (u.starts_with("https://") || u.starts_with("http://")).then_some(u)
    };
    match v {
        Value::String(u) => ok(u),
        Value::Array(a) => a.iter().take(4).find_map(|x| image_of(x, depth)),
        Value::Object(o) if depth > 0 => [
            "url",
            "src",
            "MEDIUM",
            "SMALL",
            "medium",
            "small",
            "thumbnail",
            "original",
            "main",
            "default",
        ]
        .iter()
        .find_map(|k| o.get(*k).and_then(|x| image_of(x, depth - 1)))
        .or_else(|| o.values().take(6).find_map(|x| image_of(x, depth - 1))),
        _ => None,
    }
}

/// Unix seconds → "YYYY-MM-DD HH:MM:SS" (UTC), Howard Hinnant's civil-from-days.
pub fn iso_utc(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// A listing date as ISO text: ISO strings are kept, epoch seconds / milliseconds converted.
fn date_of(v: &Value) -> Option<String> {
    let iso = |secs: i64| Some(iso_utc(secs));
    match v {
        Value::Number(n) => {
            let n = n.as_f64()? as i64;
            match n {
                1_000_000_000..=9_999_999_999 => iso(n),
                1_000_000_000_000..=9_999_999_999_999 => iso(n / 1000),
                _ => None,
            }
        }
        Value::String(s) => {
            let s = s.trim();
            let looks_iso = s.len() >= 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-';
            if looks_iso {
                Some(s.replace('T', " ").chars().take(19).collect())
            } else {
                s.parse::<i64>().ok().and_then(|n| date_of(&Value::from(n)))
            }
        }
        _ => None,
    }
}

/// Every "product with a price" found anywhere in a JSON document.
pub fn offers_in(json: &Value, platform: &str, rial: bool) -> Vec<Offer> {
    let mut out = vec![];
    walk(json, platform, rial, &mut out, 0);
    let mut seen = std::collections::HashSet::new();
    out.retain(|o| seen.insert((o.title.clone(), o.seller.clone(), o.price)));
    out
}

fn walk(v: &Value, platform: &str, rial: bool, out: &mut Vec<Offer>, depth: usize) {
    if depth > 14 {
        return;
    }
    match v {
        Value::Array(a) => a.iter().for_each(|x| walk(x, platform, rial, out, depth + 1)),
        Value::Object(o) => {
            let title = TITLE_KEYS
                .iter()
                .find_map(|k| o.get(*k).and_then(Value::as_str))
                .map(str::trim)
                .filter(|t| t.chars().count() >= 3);
            if let (Some(title), Some(price)) = (title, price_in(o)) {
                let price = if rial { price / 10.0 } else { price } as i64;
                if price >= 1_000 {
                    let rating = find_num(o, RATING_KEYS, 2)
                        .map(|r| if r > 5.0 { r / 20.0 } else { r })
                        .filter(|r| *r > 0.0 && *r <= 5.0);
                    // the product's own page (built from the platform's ids), else a link field
                    // of this object; never a nested photo / vendor link
                    let (mut url, seller_url) = crate::links::from_json(platform, o);
                    if url.is_empty() {
                        url = URL_KEYS.iter().find_map(|k| o.get(*k).and_then(Value::as_str)).unwrap_or("").to_string();
                    }
                    out.push(Offer {
                        platform: platform.into(),
                        title: title.to_string(),
                        brand: find(o, BRAND_KEYS, 2).and_then(text).unwrap_or_default(),
                        price,
                        seller: find(o, SELLER_KEYS, 1).and_then(text).unwrap_or_default(),
                        rating,
                        reviews: find_num(o, REVIEW_KEYS, 2).unwrap_or(0.0) as i64,
                        url,
                        shops: o
                            .get("shop_text")
                            .and_then(Value::as_str)
                            .and_then(|s| {
                                crate::logic::ascii_digits(s)
                                    .split_whitespace()
                                    .find_map(|w| w.parse().ok())
                            })
                            .unwrap_or(0),
                        image: IMAGE_KEYS
                            .iter()
                            .find_map(|k| o.get(*k))
                            .and_then(|v| image_of(v, 3))
                            .unwrap_or_default(),
                        sales: find_num(o, SALES_KEYS, 1).unwrap_or(0.0) as i64,
                        updated: find(o, DATE_KEYS, 1).and_then(date_of).unwrap_or_default(),
                        location: find(o, LOCATION_KEYS, 1).and_then(text).unwrap_or_default(),
                        seller_url,
                        ..Default::default()
                    });
                    return; // the product object itself: don't count its nested variants twice
                }
            }
            o.values().for_each(|x| walk(x, platform, rial, out, depth + 1));
        }
        _ => {}
    }
}

// ---------------- sizes, brands, relevance ----------------

/// Grams (or millilitres) stated in a title: «۵۰۰ گرمی», «۱ کیلویی», «۲.۵ کیلوگرم», «۱ لیتری».
pub fn size_grams(title: &str) -> Option<f64> {
    let t = crate::logic::ascii_digits(title).replace(['٫', '/'], ".");
    let words: Vec<&str> = t
        .split(|c: char| c.is_whitespace() || c == '-' || c == '،' || c == ',')
        .filter(|w| !w.is_empty())
        .collect();
    for (i, w) in words.iter().enumerate() {
        let (n, unit) = match w.find(|c: char| !(c.is_ascii_digit() || c == '.')) {
            Some(0) => continue,
            Some(p) => (&w[..p], &w[p..]),
            None => (*w, words.get(i + 1).copied().unwrap_or("")),
        };
        let Ok(n) = n.parse::<f64>() else { continue };
        let factor = if unit.starts_with("کیلو")
            || unit.starts_with("kg")
            || unit.starts_with("لیتر")
            || unit.starts_with("ليتر")
        {
            1000.0
        } else if unit.starts_with("گرم")
            || unit.starts_with("میلی")
            || unit.starts_with("سی")
            || unit.starts_with("g")
            || unit.starts_with("ml")
        {
            1.0
        } else if unit.starts_with("مثقال") {
            4.6875
        } else {
            continue;
        };
        if n > 0.0 && n * factor <= 200_000.0 {
            return Some(n * factor);
        }
    }
    None
}

/// One spelling for matching: Arabic letters → Persian, Persian digits → ASCII, no diacritics,
/// half-space (ZWNJ) → space, lower case.
pub fn norm(s: &str) -> String {
    crate::logic::ascii_digits(s)
        .chars()
        .filter(|c| !('\u{064B}'..='\u{065F}').contains(c) && *c != '\u{0670}' && *c != 'ـ')
        .map(|c| match c {
            'ي' | 'ى' | 'ئ' => 'ی',
            'ك' => 'ک',
            'ة' => 'ه',
            'ؤ' => 'و',
            'أ' | 'إ' | 'ٱ' => 'ا',
            '\u{200c}' | '\u{200d}' => ' ',
            c => c,
        })
        .collect::<String>()
        .to_lowercase()
}

/// Lower-case Persian/Latin words of a text, for matching («پینگ‌پنگ» → «پینگ», «پنگ»).
pub fn words(s: &str) -> Vec<String> {
    norm(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

fn edit1(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let (mut i, mut j, mut diff) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        diff += 1;
        if diff > 1 {
            return false;
        }
        match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Greater => i += 1,
            std::cmp::Ordering::Less => j += 1,
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    diff + (a.len() - i) + (b.len() - j) <= 1
}

/// A title word stands for a query word: same word, a suffixed form («کیسه‌ها», «فریزری»), a
/// shortened spelling («فریز» for «فریزر») or a one-letter typo in a long word.
pub fn tok_eq(t: &str, q: &str) -> bool {
    if t == q {
        return true;
    }
    let (tl, ql) = (t.chars().count(), q.chars().count());
    if ql >= 3 && tl >= 3 && ((t.starts_with(q) && tl <= ql + 3) || (q.starts_with(t) && ql - tl <= 1)) {
        return true;
    }
    ql >= 5 && tl >= 5 && edit1(t, q)
}

/// Shop words before the product name that don't change what it is.
pub(crate) const FILLERS: &[&str] = &[
    "خرید", "فروش", "قیمت", "پک", "بسته", "ست", "اصل", "اورجینال", "جدید", "ویژه", "عمده", "تک",
];

/// The offer is about the searched product. Every query word must appear (spelling-tolerant).
/// `exact` also requires the product to be named near the start of the title, so «میز پینگ پنگ
/// با دو چوب» is not counted as «چوب پینگ پنگ».
pub fn matches(title: &str, q: &[String], exact: bool) -> bool {
    if q.is_empty() {
        return true;
    }
    let t = words(title);
    let lead = t.iter().take_while(|w| FILLERS.contains(&w.as_str())).count();
    let mut compact: Option<String> = None;
    let mut first = usize::MAX;
    
    for (i, w) in q.iter().enumerate() {
        let mut at = t.iter().position(|x| tok_eq(x, w));
        if at.is_none() && w.chars().count() >= 4 {
            let c = compact.get_or_insert_with(|| t.join(""));
            if c.contains(w.as_str()) {
                at = Some(usize::MAX - 1);
            }
        }
        match at {
            None => return false,
            Some(p) if i == 0 => first = p,
            _ => {}
        }
    }
    !exact || first.saturating_sub(lead) <= 2
}

/// Exact-mode relevance (kept for callers that don't choose a mode).
#[cfg(test)]
pub fn relevant(title: &str, query: &str) -> bool {
    matches(title, &words(query), true)
}

/// Persian alphabetical order (Unicode order puts پ چ ژ گ after ی).
pub fn collate(s: &str) -> Vec<u32> {
    const ALPHA: &str = "آاأبپتثجچحخدذرزژسشصضطظعغفقکگلمنوهی";
    norm(s)
        .trim()
        .chars()
        .map(|c| ALPHA.chars().position(|a| a == c).map_or(1000 + c as u32, |i| i as u32))
        .collect()
}

/// Words after a product name that are descriptions, varieties, adjectives or quantities — NEVER brands.
pub(crate) const NOT_BRANDS: &[&str] = &[
    "طبیعی", "اصل", "اصلی", "ارگانیک", "خالص", "ممتاز", "درجه", "یک", "دو", "سه", "چهار", "پنج", "چند", "چهل", "هزار", "گیاه", "گل",
    "استاندارد", "کوهستان", "کوهی", "وحشی", "گون", "کنار", "آویشن", "بهار", "نارنج", "بهارنارنج", "گشنیز", "سیاهدانه", "جعفری", "مرکبات",
    "یونجه", "اسطوخودوس", "خارشتر", "زول", "موم", "مومدار", "ساکارز", "رژیمی", "دیابتی", "تغذیه", "زنبور", "دارویی", "درمانی", "مقوی",
    "تازه", "امسالی", "اعلا", "سوپر", "نگین", "سرگل", "پوشال", "دسته", "ممتاز", "صادراتی", "محلی", "سنتی", "خانگی", "روستایی", "ییلاقی",
    "مدل", "رنگ", "سایز", "بسته", "عددی", "عدد", "گرمی", "گرم", "کیلویی", "کیلو", "کیلوگرم", "لیتری", "لیتر", "سی", "میلی", "مثقال", "مثقالی",
    "بزرگ", "کوچک", "متوسط", "جدید", "ویژه", "ایرانی", "خارجی", "وارداتی", "با", "و", "در", "از", "به", "برای", "همراه", "بدون", "دار",
    "مخصوص", "حرفه", "ای", "سری", "پک", "ست", "عمده", "تک", "فله", "کد", "طرح", "نوع", "برند", "شرکت", "فروشگاه", "غرفه", "تولیدی", "بازرگانی",
    "پخش", "ارسال", "رایگان", "فوری", "تضمینی", "ضمانت", "آزمایش", "برگه", "دارای", "ظرف", "شیشه", "پلاستیکی", "فلزی", "کادویی", "تخفیف",
    "فروش", "خرید", "قیمت", "ارزان", "کیفیت", "عالی", "فوق", "العاده", "خوشمزه", "معطر", "خام", "رسیده", "صاف", "شده", "عصاره", "ژل", "رویال", "گرده",
];

/// Cleans a shop / booth / company name so it can be used as a verified merchant/brand entity.
pub fn clean_merchant_name(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    for p in [
        "فروشگاه اینترنتی ",
        "فروشگاه ",
        "غرفه ",
        "شرکت ",
        "بازرگانی ",
        "پخش ",
        "تولیدی ",
        "مجموعه ",
        "گروه ",
        "برند ",
    ] {
        if let Some(rest) = s.strip_prefix(p) {
            s = rest.trim().to_string();
        }
    }
    if s.chars().count() < 2
        || s.chars().count() > 35
        || s.contains("تومان")
        || s.contains("ریال")
        || s.contains("فروشنده")
    {
        return String::new();
    }
    let w = words(&s);
    if w.is_empty() || w.iter().all(|x| NOT_BRANDS.contains(&x.as_str())) {
        return String::new();
    }
    s
}

/// Brands named in titles but not given by the platform: the word right after the product
/// name, when several offers share it («کیسه فریزر آغوش …» → «آغوش»), strictly excluding descriptive words.
pub fn mine_brands(offers: &[Offer], query: &str) -> Vec<String> {
    let q = words(query);
    let mut seen: HashMap<String, usize> = HashMap::new();
    for o in offers {
        let t = words(&o.title);
        for (i, w) in t.iter().enumerate() {
            if matches!(w.as_str(), "برند" | "مارک") {
                if let Some(next) = t.get(i + 1) {
                    if next.chars().count() >= 2
                        && !next.chars().any(|c| c.is_ascii_digit())
                        && !NOT_BRANDS.contains(&next.as_str())
                    {
                        *seen.entry(next.clone()).or_default() += 2;
                    }
                }
            }
        }
        let last = q.iter().filter_map(|w| t.iter().position(|x| tok_eq(x, w))).max();
        let Some(next) = last.and_then(|i| t.get(i + 1)) else { continue };
        let ok = next.chars().count() >= 3
            && !next.chars().any(|c| c.is_ascii_digit())
            && !NOT_BRANDS.contains(&next.as_str())
            && !q.iter().any(|w| tok_eq(next, w));
        if ok {
            *seen.entry(next.clone()).or_default() += 1;
        }
    }
    let mut v: Vec<(String, usize)> = seen.into_iter().filter(|(_, n)| *n >= 2).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.into_iter().map(|(b, _)| b).collect()
}

/// One brand as the brand menu shows it: spelling variants merged, with its channels.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct BrandEntry {
    pub name: String,
    pub count: usize,
    pub platforms: Vec<String>,
    pub median: i64,
    pub min: i64,
}

/// The key that merges «خوانسار», «برند خوانسار», «عسل‌خوانسار ‌» … into one brand.
pub fn brand_key(b: &str) -> String {
    words(b)
        .into_iter()
        .filter(|w| {
            ![
                "برند",
                "شرکت",
                "فروشگاه",
                "غرفه",
                "محصولات",
                "گروه",
                "صنایع",
                "تولیدی",
                "بازرگانی",
            ]
            .contains(&w.as_str())
        })
        .collect::<Vec<_>>()
        .concat()
}

// ---------------- built-in taxonomy (sub-categories & known brands) ----------------

pub struct Category {
    pub keys: &'static [&'static str],
    pub subs: &'static [&'static str],
    pub brands: &'static [&'static str],
}

pub const TAXONOMY: &[Category] = &[
    Category {
        keys: &["عسل"],
        subs: &[
            "عسل طبیعی",
            "عسل گون",
            "عسل کنار",
            "عسل چهل گیاه",
            "عسل آویشن",
            "عسل وحشی",
            "عسل موم دار",
            "ژل رویال",
            "گرده گل",
        ],
        brands: &[
            "خوانسار",
            "سبلان",
            "شهدینه",
            "اورازان",
            "طبیعت",
            "گل‌نوش",
            "زرین‌گل",
            "فرانک",
            "پگاه",
            "کندو",
            "ملکه",
        ],
    },
    Category {
        keys: &["زعفران"],
        subs: &[
            "زعفران سرگل",
            "زعفران نگین",
            "زعفران پوشال",
            "زعفران دسته",
            "زعفران قائنات",
            "زعفران تربت حیدریه",
        ],
        brands: &[
            "سحرخیز",
            "نوین",
            "قائنات",
            "اکبر",
            "گلستان",
            "یک و یک",
            "تارونه",
            "ویستا",
            "بهرامن",
            "مصطفوی",
        ],
    },
    Category {
        keys: &["موبایل", "گوشی"],
        subs: &[
            "گوشی سامسونگ",
            "گوشی شیائومی",
            "گوشی آیفون",
            "گوشی نوکیا",
            "گوشی ساده",
            "گوشی ۵G",
        ],
        brands: &[
            "سامسونگ",
            "شیائومی",
            "اپل",
            "آنر",
            "نوکیا",
            "هواوی",
            "ریلمی",
            "موتورولا",
            "وان‌پلاس",
            "پوکو",
        ],
    },
    Category {
        keys: &["پوشاک", "لباس", "تیشرت", "شلوار", "مانتو"],
        subs: &[
            "تیشرت مردانه",
            "شلوار جین",
            "مانتو زنانه",
            "لباس ورزشی",
            "لباس بچگانه",
            "هودی",
        ],
        brands: &[
            "ال‌سی‌من",
            "کوتون",
            "دفکتو",
            "ماکسی‌ماکس",
            "جین‌وست",
            "آدیداس",
            "نایکی",
            "هاکوپیان",
            "سارک",
            "برشکا",
        ],
    },
    Category {
        keys: &["مبل", "مبلمان"],
        subs: &[
            "مبل راحتی",
            "مبل کلاسیک",
            "مبل ال",
            "کاناپه",
            "مبل تختخواب‌شو",
            "صندلی راحتی",
        ],
        brands: &["هانسون", "نظری", "ساج", "آرین", "داوین", "نیلپر", "لیو", "رست"],
    },
    Category {
        keys: &["خشکبار", "آجیل", "پسته", "بادام", "گردو"],
        subs: &[
            "پسته اکبری",
            "پسته احمدآقایی",
            "بادام درختی",
            "گردو",
            "آجیل چهارمغز",
            "کشمش",
            "خرما",
        ],
        brands: &[
            "گلستان",
            "سرزمین",
            "تواضع",
            "برتر",
            "آجیل حسینی",
            "تابان",
            "مزمز",
            "دایتی",
        ],
    },
    Category {
        keys: &["لوازم خانگی", "یخچال", "ماشین لباسشویی", "اجاق", "جاروبرقی"],
        subs: &[
            "یخچال فریزر",
            "ماشین لباسشویی",
            "ماشین ظرفشویی",
            "جاروبرقی",
            "اجاق گاز",
            "مایکروویو",
        ],
        brands: &[
            "اسنوا",
            "ال‌جی",
            "سامسونگ",
            "پاکشوما",
            "بوش",
            "دوو",
            "پارس خزر",
            "تفال",
            "فیلیپس",
            "امرسان",
            "جی‌پلاس",
        ],
    },
    Category {
        keys: &["قطعات خودرو", "لنت", "روغن موتور", "باتری خودرو", "لاستیک"],
        subs: &[
            "لنت ترمز",
            "روغن موتور",
            "فیلتر روغن",
            "باتری خودرو",
            "لاستیک خودرو",
            "شمع",
        ],
        brands: &[
            "ایساکو",
            "بهران",
            "سپاهان",
            "صبا باتری",
            "کویر تایر",
            "بارز",
            "توتال",
            "کاسترول",
            "ایرانول",
            "عظام",
        ],
    },
    Category {
        keys: &["قهوه", "تجهیزات کافه", "اسپرسو"],
        subs: &[
            "دانه قهوه",
            "قهوه اسپرسو",
            "دستگاه اسپرسو",
            "آسیاب قهوه",
            "قهوه ترک",
            "شیر جوش",
        ],
        brands: &[
            "لاوازا",
            "ایلی",
            "مولتی‌کافه",
            "جیمی",
            "گاستروبک",
            "دلونگی",
            "تکنو",
            "بن‌مانو",
            "ریو",
            "مباشی",
        ],
    },
    Category {
        keys: &["چای"],
        subs: &[
            "چای سیاه",
            "چای سبز",
            "چای ایرانی",
            "چای کیسه‌ای",
            "دمنوش",
        ],
        brands: &[
            "گلستان",
            "احمد",
            "دبش",
            "شهرزاد",
            "محمود",
            "سحرخیز",
            "دوغزال",
            "لاهیجان",
            "نیوشا",
        ],
    },
    Category {
        keys: &["برنج"],
        subs: &[
            "برنج طارم",
            "برنج هاشمی",
            "برنج دم‌سیاه",
            "برنج پاکستانی",
            "برنج هندی",
        ],
        brands: &["مهتاب", "خاطره", "گلستان", "طبیعت", "فجر", "محسن", "مژده", "آوازه"],
    },
    Category {
        keys: &["روغن", "روغن خوراکی"],
        subs: &[
            "روغن مایع",
            "روغن سرخ‌کردنی",
            "روغن زیتون",
            "روغن کنجد",
            "روغن جامد",
        ],
        brands: &[
            "لادن",
            "بهار",
            "اویلا",
            "فامیلا",
            "طبیعت",
            "غنچه",
            "ورامین",
            "اتکا",
        ],
    },
];

pub fn category_for(query: &str) -> Option<&'static Category> {
    let q = words(query).join(" ");
    TAXONOMY
        .iter()
        .find(|c| c.keys.iter().any(|k| q.contains(&words(k).join(" "))))
}

/// The brand of an offer: the platform's field, else a known brand named in the title,
/// else a verified shop / booth name (`seller`).
pub fn brand_of(o: &Offer, known: &[String]) -> String {
    let b = o.brand.trim();
    if !b.is_empty()
        && b != "متفرقه"
        && !b.eq_ignore_ascii_case("other")
        && !NOT_BRANDS.contains(&norm(b).as_str())
    {
        return b.to_string();
    }
    let t = words(&o.title);
    if let Some(found) = known.iter().find(|k| {
        let bw = words(k);
        !bw.is_empty()
            && bw
                .iter()
                .all(|w| t.iter().any(|x| x == w || (w.chars().count() >= 4 && tok_eq(x, w))))
    }) {
        return found.clone();
    }
    let shop = clean_merchant_name(&o.seller);
    if !shop.is_empty() && !PLATFORMS.iter().any(|p| p.name == shop) {
        return shop;
    }
    String::new()
}

// ---------------- analysis ----------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Stats {
    pub count: usize,
    pub min: i64,
    pub max: i64,
    pub median: i64,
    pub p25: i64,
    pub p75: i64,
    /// Weighted mean of the offers that are not outliers (IQR rule), weighted by trust.
    pub weighted: i64,
    pub outliers: usize,
    /// "per_kg" when most offers state a size, so prices are compared per kilogram / litre.
    pub basis: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Share {
    pub name: String,
    pub count: usize,
    pub pct: f64,
    pub median: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Advice {
    pub buy_from: String, // seller · platform
    pub buy_price: i64,
    pub buy_url: String,
    pub sell_on: String,        // platform with the best price level for selling
    pub sell_price: i64,        // = suggested retail price
    pub margin_pct: f64,        // at the suggested price
    pub market_margin_pct: f64, // if sold at the market median
    pub notes: Vec<String>,
}

pub type History = (String, i64, i64); // (date, median, min)

/// One product in «بازار این ماه»: from the user's own searches of the last 30 days.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Trend {
    #[serde(skip_serializing)]
    pub key: String,
    pub query: String,
    pub searches: i64,
    pub first_median: i64,
    pub last_median: i64,
    pub last_at: String,
    pub last_id: i64,
    pub channels: Vec<Share>,
}
pub type RunRow = (i64, String, String, i64, i64, String); // (id, query, brand, offers, median, at)

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Report {
    pub id: i64,
    pub query: String,
    pub brand: String,
    pub created_at: String,
    pub target_margin: f64,
    pub stats: Stats,
    pub srp: i64,
    pub offers: Vec<Offer>, // sorted by value score
    pub brands: Vec<Share>,
    pub channels: Vec<Share>,
    pub tiers: Vec<Share>, // اقتصادی / رقابتی / میان‌رده / پریمیوم
    pub subcategories: Vec<String>,
    pub brand_chips: Vec<String>,
    pub advice: Advice,
    pub history: Vec<History>,                   // earlier searches of the same product
    pub platforms: Vec<(String, usize, String)>, // (platform, offers found, error or "")
    /// Every brand in the results, spelling variants merged, sorted by market presence.
    pub brand_list: Vec<BrandEntry>,
    /// The platforms' own related / popular searches for this product.
    pub related: Vec<String>,
    pub dropped: usize,      // offers about another product (e.g. ping-pong tables for «چوب پینگ پنگ»)
    pub out_of_range: usize, // outside the price range the user gave
    pub req: MarketReq,      // the search as it was asked (to repeat it from history)
    /// The same product listed by several sellers / platforms, side by side (see identity.rs).
    pub groups: Vec<crate::identity::Group>,
    /// Earlier searches per source: (date, platform name, median).
    pub source_history: Vec<(String, String, i64)>,
}

pub(crate) fn pct(sorted: &[i64], p: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    let pos = p * (sorted.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    (sorted[lo] as f64 + (sorted[hi] - sorted[lo]) as f64 * (pos - lo as f64)).round() as i64
}

fn trust(o: &Offer) -> f64 {
    // 0..1: rating (unknown = neutral 3.5) and review volume
    let r = o.rating.unwrap_or(3.5) / 5.0;
    let v = ((o.reviews as f64 + 1.0).log10() / 3.0).min(1.0);
    0.75 * r + 0.25 * v
}

/// Rounds a Toman price to a "shelf" price: up to the next 1,000 (or 10,000 above 1,000,000).
pub fn shelf(p: f64) -> i64 {
    let step = if p >= 1_000_000.0 { 10_000.0 } else { 1_000.0 };
    ((p / step).ceil() * step) as i64
}

/// Everything the product page shows, from the collected offers (exact-phrase matching).
#[cfg(test)]
pub fn analyze(query: &str, brand_filter: &str, offers: Vec<Offer>, target_margin: f64) -> Report {
    analyze_opts(query, brand_filter, offers, target_margin, true)
}

pub fn analyze_opts(
    query: &str,
    brand_filter: &str,
    mut offers: Vec<Offer>,
    target_margin: f64,
    exact: bool,
) -> Report {
    let cat = category_for(query);
    let qw = words(query);
    let before = offers.len();
    offers.retain(|o| matches(&o.title, &qw, exact));
    let dropped = before - offers.len();
    // brands: the taxonomy's, every brand a platform named, and brands mined from titles
    let mut known: Vec<String> = cat
        .map(|c| c.brands.iter().map(|b| b.to_string()).collect())
        .unwrap_or_default();
    for o in &offers {
        let b = o.brand.trim();
        if !b.is_empty()
            && b != "متفرقه"
            && !NOT_BRANDS.contains(&norm(b).as_str())
            && !known.iter().any(|k| brand_key(k) == brand_key(b))
        {
            known.push(b.to_string());
        }
    }
    for b in mine_brands(&offers, query) {
        if !known.iter().any(|k| brand_key(k) == brand_key(&b)) {
            known.push(b);
        }
    }
    for o in offers.iter_mut() {
        o.brand = brand_of(o, &known);
        o.unit_price = size_grams(&o.title)
            .filter(|g| *g >= 20.0)
            .map(|g| (o.price as f64 * 1000.0 / g).round() as i64);
    }
    if !brand_filter.trim().is_empty() {
        let bk = brand_key(brand_filter);
        let bw = words(brand_filter);
        offers.retain(|o| {
            let ok_brand =
                !o.brand.is_empty() && (brand_key(&o.brand) == bk || brand_key(&o.brand).contains(&bk));
            let ok_seller = !o.seller.is_empty() && brand_key(&o.seller).contains(&bk);
            let t = words(&o.title);
            ok_brand || ok_seller || bw.iter().all(|w| t.iter().any(|x| tok_eq(x, w)))
        });
    }

    // compare per kilogram when most offers state a size (honey in 500 g and 1 kg jars …)
    let sized = offers.iter().filter(|o| o.unit_price.is_some()).count();
    let per_kg = offers.len() >= 4 && sized * 10 >= offers.len() * 6;
    let basis = |o: &Offer| if per_kg { o.unit_price.unwrap_or(0) } else { o.price };
    let pool: Vec<&Offer> = offers.iter().filter(|o| basis(o) > 0).collect();
    let mut prices: Vec<i64> = pool.iter().map(|o| basis(o)).collect();
    prices.sort_unstable();
    let (p25, p75) = (pct(&prices, 0.25), pct(&prices, 0.75));
    let iqr = (p75 - p25) as f64;
    let (lo, hi) = (p25 as f64 - 1.5 * iqr, p75 as f64 + 1.5 * iqr);
    let inliers: Vec<&&Offer> = pool
        .iter()
        .filter(|o| (basis(o) as f64) >= lo && (basis(o) as f64) <= hi)
        .collect();
    let wsum: f64 = inliers.iter().map(|o| 0.5 + trust(o)).sum();
    let weighted = if wsum > 0.0 {
        (inliers
            .iter()
            .map(|o| basis(o) as f64 * (0.5 + trust(o)))
            .sum::<f64>()
            / wsum)
            .round() as i64
    } else {
        0
    };
    let stats = Stats {
        count: prices.len(),
        min: inliers.iter().map(|o| basis(o)).min().unwrap_or(0),
        max: inliers.iter().map(|o| basis(o)).max().unwrap_or(0),
        median: pct(&prices, 0.5),
        p25,
        p75,
        weighted,
        outliers: pool.len() - inliers.len(),
        basis: if per_kg { "per_kg".into() } else { "unit".into() },
    };

    // value score: price advantage 55 % · seller trust 35 % · review volume 10 %
    let span = (stats.max - stats.min).max(1) as f64;
    for o in offers.iter_mut() {
        let b = if per_kg { o.unit_price.unwrap_or(0) } else { o.price };
        let adv = if b == 0 {
            0.0
        } else {
            ((stats.max - b) as f64 / span).clamp(0.0, 1.0)
        };
        let outlier = (b as f64) < lo || (b as f64) > hi;
        let rating = o.rating.unwrap_or(3.5) / 5.0;
        let volume = ((o.reviews as f64 + 1.0).log10() / 3.0).min(1.0);
        o.value = ((55.0 * adv + 35.0 * rating + 10.0 * volume) * if outlier { 0.5 } else { 1.0 }).round();
    }
    offers.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));

    let share = |key: &dyn Fn(&Offer) -> String| -> Vec<Share> {
        let mut m: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        for o in &offers {
            m.entry(key(o)).or_default().push(basis(o));
        }
        let total = offers.len().max(1) as f64;
        let mut v: Vec<Share> = m
            .into_iter()
            .map(|(name, mut ps)| {
                ps.sort_unstable();
                Share {
                    count: ps.len(),
                    pct: (ps.len() as f64 * 1000.0 / total).round() / 10.0,
                    median: pct(&ps, 0.5),
                    name,
                }
            })
            .collect();
        v.sort_by_key(|x| std::cmp::Reverse(x.count));
        v
    };
    let brands = share(&|o| {
        if o.brand.is_empty() {
            "سایر / بدون برند".into()
        } else {
            o.brand.clone()
        }
    });
    let channels = share(&|o| {
        platform(&o.platform).map_or(o.platform.clone(), |p| p.name.to_string())
    });
    let tier_names = ["اقتصادی", "رقابتی", "میان‌رده", "پریمیوم"];
    let q50 = stats.median;
    let mut tiers: Vec<Share> = tier_names
        .iter()
        .map(|n| Share {
            name: n.to_string(),
            ..Default::default()
        })
        .collect();
    for p in &prices {
        let i = if *p <= p25 {
            0
        } else if *p <= q50 {
            1
        } else if *p <= p75 {
            2
        } else {
            3
        };
        tiers[i].count += 1;
    }
    let n = prices.len().max(1) as f64;
    for (i, t) in tiers.iter_mut().enumerate() {
        t.pct = (t.count as f64 * 1000.0 / n).round() / 10.0;
        t.median = [stats.min, p25, q50, p75][i];
    }

    // recommended selling price: cost × (1 + margin), kept inside the market's p25–p75 band
    let best = offers
        .iter()
        .find(|o| basis(o) > 0 && !((basis(o) as f64) < lo || (basis(o) as f64) > hi));
    let cost = best.map_or(stats.p25, |o| o.price);
    let cost_basis = best.map_or(stats.p25, basis);
    let want = cost_basis as f64 * (1.0 + target_margin / 100.0);
    let srp_basis = want.clamp(p25 as f64, (p75 as f64).max(p25 as f64));
    // shown in the unit the user sells: per kg when comparing per kg, else per item
    let srp = shelf(srp_basis);
    let mut notes = vec![];
    if want > p75 as f64 && p75 > 0 {
        notes.push(format!(
            "حاشیه سود {}٪ در این بازار جا نمی‌شود؛ بالاتر از ۷۵٪ عرضه‌ها قیمت می‌خورد. قیمت پیشنهادی روی سقف رقابتی بازار گذاشته شد.",
            target_margin.round()
        ));
    }
    if want < p25 as f64 {
        notes.push("با این حاشیه، قیمت شما زیر ۲۵٪ ارزان‌ترین عرضه‌ها می‌افتد؛ بدون از دست دادن رقابت می‌توانید گران‌تر بفروشید.".into());
    }
    if stats.outliers > 0 {
        notes.push(format!(
            "{} قیمت غیرعادی (خیلی ارزان یا خیلی گران) از محاسبه میانگین کنار گذاشته شد.",
            stats.outliers
        ));
    }
    if per_kg {
        notes.push(
            "قیمت‌ها بر اساس هر کیلوگرم (یا لیتر) مقایسه شده‌اند، چون بسته‌ها اندازه‌های مختلف دارند."
                .into(),
        );
    }
    let sell_on = channels
        .iter()
        .filter(|c| c.count >= 2)
        .max_by_key(|c| c.median)
        .or(channels.first())
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let margin = |sell: i64| {
        if cost_basis > 0 {
            ((sell - cost_basis) as f64 * 1000.0 / cost_basis as f64).round() / 10.0
        } else {
            0.0
        }
    };
    let advice = Advice {
        buy_from: best
            .map(|o| {
                [
                    o.seller.as_str(),
                    platform(&o.platform).map_or(o.platform.as_str(), |p| p.name),
                ]
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" · ")
            })
            .unwrap_or_default(),
        buy_price: cost,
        buy_url: best.map(|o| o.url.clone()).unwrap_or_default(),
        sell_on,
        sell_price: srp,
        margin_pct: margin(srp),
        market_margin_pct: margin(stats.median),
        notes,
    };

    // sub-categories: the taxonomy's kinds of this product, then «query + next word» phrases that
    // several titles share (brands excluded — they get their own menu)
    let head = qw.first().cloned().unwrap_or_default();
    let mut subcategories: Vec<String> = cat
        .map(|c| {
            c.subs
                .iter()
                .filter(|s| words(s).iter().any(|w| tok_eq(w, &head)))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    let brand_keys: Vec<String> = known.iter().map(|b| brand_key(b)).collect();
    let mut phrases: HashMap<String, usize> = HashMap::new();
    for o in &offers {
        let t = words(&o.title);
        let last = qw
            .iter()
            .filter_map(|w| t.iter().position(|x| tok_eq(x, w)))
            .max();
        if let Some(next) = last.and_then(|i| t.get(i + 1)) {
            let useful = next.chars().count() > 2
                && !next.chars().any(|c| c.is_ascii_digit())
                && !FILLERS.contains(&next.as_str())
                && !brand_keys.contains(&brand_key(next));
            if useful {
                *phrases
                    .entry(format!("{} {next}", qw.join(" ")))
                    .or_default() += 1;
            }
        }
    }
    let mut phrases: Vec<(String, usize)> = phrases.into_iter().filter(|(_, n)| *n >= 2).collect();
    phrases.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (p, _) in phrases.into_iter().take(10) {
        if !subcategories.iter().any(|x| words(x) == words(&p)) {
            subcategories.push(p);
        }
    }
    let mut brand_chips: Vec<String> = brands
        .iter()
        .filter(|b| b.name != "سایر / بدون برند")
        .map(|b| b.name.clone())
        .collect();
    for b in cat.map(|c| c.brands).unwrap_or(&[]) {
        if !brand_chips.iter().any(|x| x == b) {
            brand_chips.push(b.to_string());
        }
    }
    // the brand menu: one row per brand / company / verified shop, with its channels and price level
    let mut groups: BTreeMap<String, Vec<&Offer>> = BTreeMap::new();
    for o in offers
        .iter()
        .filter(|o| !o.brand.is_empty() && !NOT_BRANDS.contains(&norm(&o.brand).as_str()))
    {
        let k = brand_key(&o.brand);
        if !k.is_empty() {
            groups.entry(k).or_default().push(o);
        }
    }
    let mut brand_list: Vec<BrandEntry> = groups
        .into_values()
        .map(|g| {
            let mut names: HashMap<&str, usize> = HashMap::new();
            g.iter()
                .for_each(|o| *names.entry(o.brand.as_str()).or_default() += 1);
            let name = names
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
                .map(|x| x.0.to_string())
                .unwrap_or_default();
            let mut ps: Vec<i64> = g.iter().map(|o| basis(o)).filter(|p| *p > 0).collect();
            ps.sort_unstable();
            let mut platforms: Vec<String> = g.iter().map(|o| o.platform.clone()).collect();
            platforms.sort();
            platforms.dedup();
            BrandEntry {
                name,
                count: g.len(),
                platforms,
                median: pct(&ps, 0.5),
                min: ps.first().copied().unwrap_or(0),
            }
        })
        .collect();
    for b in cat.map(|c| c.brands).unwrap_or(&[]) {
        if !brand_list.iter().any(|e| brand_key(&e.name) == brand_key(b)) {
            brand_list.push(BrandEntry {
                name: b.to_string(),
                ..Default::default()
            });
        }
    }
    brand_list.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| collate(&a.name).cmp(&collate(&b.name)))
    });
    let groups = crate::identity::group(&offers);

    Report {
        query: query.trim().into(),
        brand: brand_filter.trim().into(),
        target_margin,
        stats,
        srp,
        offers,
        brands,
        channels,
        tiers,
        subcategories,
        brand_chips,
        advice,
        brand_list,
        dropped,
        groups,
        ..Default::default()
    }
}

// ---------------- collecting ----------------

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36")
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(25))
        .build()
        .unwrap_or_default()
}

struct Url<'a> {
    q: &'a str,
    page: usize,
    per_page: usize,
    sort: &'a str,
    city: &'a str,
}

fn url_for(tpl: &str, u: &Url) -> String {
    tpl.replace("{q}", &crate::logic::pct_encode(u.q))
        .replace("{page}", &u.page.to_string())
        .replace("{page1}", &(u.page + 1).to_string())
        .replace("{from}", &(u.page * u.per_page).to_string())
        .replace("{sort}", u.sort)
        .replace("{city}", u.city)
}

/// Admin-published fixes («سناریو»): `market.<id>.api` / `.page` / `.suggest` replace the built-in URLs.
fn scenario_url(db: &Db, id: &str, which: &str) -> Option<String> {
    let v: Value = serde_json::from_str(&db.kv_get("scenario").unwrap_or_default()).ok()?;
    v.get(format!("market.{id}.{which}"))
        .and_then(Value::as_str)
        .filter(|s| s.starts_with("https://"))
        .map(String::from)
}

const PAGE_JS: &str = r#"
const out = [];
const nd = document.getElementById('__NEXT_DATA__');
if (nd) { try { out.push(JSON.parse(nd.textContent)); } catch (e) {} }
if (window.__PRELOADED_STATE__) { try { out.push(window.__PRELOADED_STATE__); } catch (e) {} }
for (const s of document.querySelectorAll('script[type="application/ld+json"]')) { try { out.push(JSON.parse(s.textContent)); } catch (e) {} }
// product / ad cards that show a «… تومان» price, with photo, seller, city and «… پیش» time when shown
const cards = [];
const price = /([\d۰-۹٬,]{4,})\s*(تومان|ریال)/;
const fa = (x) => Number(String(x).replace(/[۰-۹]/g, (d) => '۰۱۲۳۴۵۶۷۸۹'.indexOf(d)));
for (const el of document.querySelectorAll('a[href]')) {
  const t = (el.innerText || '').trim();
  const m = t.match(price);
  if (!m || t.length > 450) continue;
  const lines = t.split('\n').map(s => s.trim()).filter(Boolean);
  const title = lines.filter(s => s.length > 4 && !price.test(s))[0] || '';
  if (!title) continue;
  const sellerLine = lines.find(s => s !== title && (s.includes('فروشگاه') || s.includes('غرفه') || s.includes('توسط') || s.includes('بازرگانی') || s.includes('پخش'))) || '';
  const img = el.querySelector('img');
  const image = img ? (img.currentSrc || img.src || img.getAttribute('data-src') || '') : '';
  let updated = '';
  const ago = t.match(/(\d+|[۰-۹]+|یک|نیم)?\s*(لحظاتی|دقیقه|ساعت|روز|هفته|ماه)\s*(پیش|قبل)/);
  if (ago) {
    const n = !ago[1] ? 1 : ago[1] === 'یک' ? 1 : ago[1] === 'نیم' ? 0.5 : fa(ago[1]);
    const min = { 'لحظاتی': 0, 'دقیقه': 1, 'ساعت': 60, 'روز': 1440, 'هفته': 10080, 'ماه': 43200 }[ago[2]] * n;
    updated = Math.floor(Date.now() / 1000 - min * 60);
  }
  const loc = (t.match(/(?:در|،)\s*([^\n،]{2,24})$/m) || [])[1] || '';
  cards.push({ title, price: m[1], unit: m[2], url: el.href, seller: sellerLine.replace(/^در\s+/, ''), image, updated, location: ago ? loc : '' });
}
out.push({ cards });
const body = document.body ? document.body.innerText.slice(0, 3000) : '';
return {
  blocked: /captcha|challenge|verify you are human|ربات نیستم|ربات هستید|فلش پازل|تأیید هویت|arcaptcha/i.test(body),
  login: /\/(login|signin|sign-in|auth|session)(\/|$|\?)/i.test(location.pathname + location.search),
  cards: cards.length,
  data: out,
};
"#;

#[derive(Deserialize, Default, Debug, PartialEq)]
#[serde(default)]
struct PageData {
    blocked: bool,
    login: bool,
    cards: usize,
    data: Vec<Value>,
}

fn page_offers(d: &PageData, p: &Platform) -> Vec<Offer> {
    let mut out = vec![];
    for v in &d.data {
        if let Some(cards) = v.get("cards").and_then(Value::as_array) {
            for c in cards {
                let rial = c.get("unit").and_then(Value::as_str) == Some("ریال");
                out.extend(offers_in(c, p.id, rial));
            }
        } else {
            out.extend(offers_in(v, p.id, p.rial));
        }
    }
    out
}

/// Scrolls an endless-scroll results page until it shows `want` cards or stops growing.
async fn scroll_for(page: &chromiumoxide::Page, want: usize, token: &CancellationToken) -> PageData {
    let mut d: PageData = scanner::js(page, PAGE_JS).await.unwrap_or_default();
    for _ in 0..12 {
        if d.cards >= want || d.blocked || d.login || token.is_cancelled() {
            break;
        }
        let _ = scanner::js::<Value>(page, "window.scrollTo(0, document.body.scrollHeight); return 1;").await;
        tokio::time::sleep(Duration::from_millis(1600)).await;
        let next: PageData = scanner::js(page, PAGE_JS).await.unwrap_or_default();
        let grew = next.cards > d.cards;
        d = next;
        if !grew {
            break;
        }
    }
    d
}

const VERIFY_WAIT: &str = "یک پنجره مرورگر باز شد: «{p}» تأیید «ربات نیستم» یا ورود به حساب خواست. آن را در همان پنجره انجام دهید؛ جستجو خودکار ادامه پیدا می‌کند. برای رد کردن این منبع، پنجره را ببندید یا «رد کردن و ادامه» را بزنید.";

/// How a source that needed the user ended (shown per source in the report).
#[derive(Debug, PartialEq)]
enum Attended {
    /// The check was passed and the results page was read.
    Read(PageData),
    /// The user skipped the source (closed the window / «رد کردن و ادامه») or Stop was pressed.
    Skipped,
    /// Nobody passed the check in time.
    TimedOut,
}

/// The page needs the user (captcha or sign-in): a visible window on the same profile. The caller
/// must have closed its own headless browser first (one profile = one browser).
/// Never gets stuck: closing the window, «رد کردن و ادامه», Stop or the time limit (also while a
/// sign-in page is shown) all end the wait and the search goes on with the next platform.
async fn attended(
    s: &Settings,
    profile: &Path,
    p: &Platform,
    url: &str,
    want: usize,
    token: &CancellationToken,
    emit: Emit<'_>,
) -> Result<Attended, String> {
    scanner::clear_wait_skip();
    emit(ScanEvent::Captcha {
        message: VERIFY_WAIT.replace("{p}", p.name),
    });
    let mut eng = scanner::launch(s, profile, false, 0).await?;
    scanner::goto(&eng.list, url).await;
    let deadline = std::time::Instant::now() + Duration::from_secs(5 * 60);
    let mut reloaded_once = false;
    let result = loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(1500)) => {}
            _ = token.cancelled() => break Attended::Skipped,
        }
        if scanner::take_wait_skip() || eng.proc.exited() || !eng.alive.load(std::sync::atomic::Ordering::Relaxed) {
            break Attended::Skipped;
        }
        if std::time::Instant::now() > deadline {
            break Attended::TimedOut;
        }
        let d: PageData = scanner::js(&eng.list, PAGE_JS).await.unwrap_or_default();
        if d.blocked || d.login {
            continue;
        }
        if d.cards == 0 && !reloaded_once {
            // the check was passed on another page (e.g. the home page): open the search again
            reloaded_once = true;
            scanner::goto(&eng.list, url).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        break Attended::Read(scroll_for(&eng.list, want, token).await);
    };
    scanner::shut(Some(eng)).await;
    scanner::status(emit, format!("ادامه جستجو پس از {}…", p.name));
    Ok(result)
}

#[allow(clippy::too_many_arguments)] // one call site; a context struct would only rename these
async fn collect_one(
    db: &Db,
    s: &Settings,
    profile: &Path,
    p: &Platform,
    req: &MarketReq,
    query: &str,
    eng: &mut Option<scanner::Engine>,
    token: &CancellationToken,
    emit: Emit<'_>,
) -> Result<Vec<Offer>, String> {
    let want = req.limit.clamp(10, 600) as usize;
    let sort = p
        .sorts
        .iter()
        .find(|(k, _)| *k == req.sort)
        .or(p.sorts.first())
        .map_or("", |x| x.1);
    let city = city_slug(&req.province, &req.city);
    let qw = words(req.query.as_str()); // extracted words out of loop
    
    // 1) the public JSON search API, page after page until enough offers about this product
    let api = crate::logic::test_seam("HAMISTA_MARKET_BASE")
        .map(|base| format!("{base}/{}?q={{q}}&page={{page}}", p.id))
        .or_else(|| scenario_url(db, p.id, "api"))
        .unwrap_or_else(|| p.api.to_string());
    if !api.is_empty() {
        let mut got: Vec<Offer> = vec![];
        let mut seen = std::collections::HashSet::new();
        let mut api_answered = false;
        let pages = want.div_ceil(p.per_page.max(1)).clamp(1, 25) + 2; // a little extra: some results are about other products
        for page in 0..pages {
            if token.is_cancelled() {
                break;
            }
            let url = url_for(
                &api,
                &Url {
                    q: query,
                    page,
                    per_page: p.per_page,
                    sort,
                    city,
                },
            );
            let Ok(Ok(r)) = tokio::time::timeout(
                Duration::from_secs(25),
                http().get(&url).header("Accept", "application/json").send(),
            )
            .await
            else {
                break;
            };
            if !r.status().is_success() {
                break;
            }
            let Ok(v) = r.json::<Value>().await else { break };
            // If the JSON API returned a valid search payload (even with 0 products for a niche brand),
            // mark `api_answered = true` so we don't open a browser and trigger a false captcha!
            if v.get("results").is_some()
                || v.get("data").is_some()
                || v.get("products").is_some()
            {
                api_answered = true;
            }
            let fresh: Vec<Offer> = offers_in(&v, p.id, p.rial)
                .into_iter()
                .filter(|o| seen.insert((o.title.clone(), o.seller.clone(), o.price)))
                .collect();
            if fresh.is_empty() {
                break;
            }
            got.extend(fresh);
            if got
                .iter()
                .filter(|o| matches(&o.title, &qw, req.exact))
                .count()
                >= want
            {
                break;
            }
            if page + 1 < pages {
                scanner::status(
                    emit,
                    format!("{}: صفحه {} — {} پیشنهاد", p.name, page + 2, got.len()),
                );
                tokio::time::sleep(Duration::from_millis(350)).await; // polite pacing
            }
        }
        if !got.is_empty() || api_answered {
            return Ok(got);
        }
    }
    if token.is_cancelled() {
        return Ok(vec![]);
    }
    // 2) the public search page in the (headless) browser: embedded data, then visible cards
    if eng.is_none() {
        *eng = Some(scanner::launch(s, profile, s.headless, 0).await?);
    }
    let e = eng.as_ref().ok_or("مرورگر آماده نشد")?;
    let page = crate::logic::test_seam("HAMISTA_MARKET_PAGE")
        .map(|base| format!("{base}/{}?q={{q}}", p.id))
        .or_else(|| scenario_url(db, p.id, "page"))
        .unwrap_or_else(|| p.page.to_string());
    let url = url_for(
        &page,
        &Url {
            q: query,
            page: 0,
            per_page: p.per_page,
            sort,
            city,
        },
    );
    scanner::goto(&e.list, &url).await;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    let d = scroll_for(&e.list, want, token).await;
    if !(d.blocked || d.login) {
        return Ok(page_offers(&d, p));
    }
    // the visible window needs the profile this headless browser holds: close it first (the next
    // platform starts a fresh one)
    scanner::shut(eng.take()).await;
    let why = if d.login { "ورود به حساب لازم است" } else { "تأیید «ربات نیستم» لازم است" };
    match attended(s, profile, p, &url, want, token, emit).await? {
        Attended::Read(d) => Ok(page_offers(&d, p)),
        Attended::Skipped if token.is_cancelled() => Ok(vec![]),
        Attended::Skipped => Err(format!("{why}؛ رد شد")),
        Attended::TimedOut => Err(format!("{why}؛ در ۵ دقیقه انجام نشد")),
    }
}

/// The platforms' own autocomplete: what people search around this product.
async fn suggestions(db: &Db, chosen: &[&Platform], query: &str) -> Vec<String> {
    let head = words(query).into_iter().next().unwrap_or_default();
    let mut out: Vec<String> = vec![];
    for p in chosen {
        let tpl = scenario_url(db, p.id, "suggest").unwrap_or_else(|| p.suggest.to_string());
        if tpl.is_empty() || crate::logic::test_seam("HAMISTA_MARKET_BASE").is_some() {
            continue;
        }
        let url = url_for(
            &tpl,
            &Url {
                q: query,
                page: 0,
                per_page: 0,
                sort: "",
                city: "",
            },
        );
        let Ok(Ok(r)) = tokio::time::timeout(Duration::from_secs(6), http().get(&url).send()).await
        else {
            continue;
        };
        let Ok(v) = r.json::<Value>().await else { continue };
        let mut texts = vec![];
        collect_texts(&v, &mut texts, 0);
        for t in texts {
            let t = t.trim().to_string();
            let w = words(&t);
            if (2..=60).contains(&t.chars().count())
                && w.iter().any(|x| tok_eq(x, &head))
                && words(query) != w
                && !out.iter().any(|o| words(o) == w)
            {
                out.push(t);
            }
        }
    }
    out.truncate(16);
    out
}

fn collect_texts(v: &Value, out: &mut Vec<String>, depth: usize) {
    if depth > 8 || out.len() > 200 {
        return;
    }
    match v {
        Value::String(s) if !s.starts_with("http") && !s.contains('/') => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| collect_texts(x, out, depth + 1)),
        Value::Object(o) => o
            .iter()
            .filter(|(k, _)| !k.contains("url") && !k.contains("image"))
            .for_each(|(_, x)| collect_texts(x, out, depth + 1)),
        _ => {}
    }
}

/// Signs in to a platform in a visible window (the browser profile the searches use keeps the
/// session). Returns when the user closes the window.
pub async fn login(s: &Settings, profile: &Path, id: &str, token: &CancellationToken) -> Res<()> {
    let p = platform(id).ok_or("پلتفرم ناشناخته")?;
    let url = if p.login.is_empty() {
        p.page.replace("{q}", "").replace("{city}", "iran")
    } else {
        p.login.to_string()
    };
    let mut eng = scanner::launch(s, profile, false, 0).await?;
    scanner::goto(&eng.list, &url).await;
    let deadline = std::time::Instant::now() + Duration::from_secs(20 * 60);
    while !eng.proc.exited()
        && eng.alive.load(std::sync::atomic::Ordering::Relaxed)
        && std::time::Instant::now() < deadline
        && !token.is_cancelled()
    {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    scanner::shut(Some(eng)).await;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct MarketReq {
    pub query: String,
    pub brand: String,
    pub platforms: Vec<String>,
    pub target_margin: f64,
    /// How many offers to read from each platform.
    pub limit: i64,
    /// One of `SORTS`.
    pub sort: String,
    /// Exact phrase (default) or every word anywhere.
    pub exact: bool,
    /// The market's plausible price range (Toman, 0 = no limit): stale or wrong prices are left out.
    pub min_price: i64,
    pub max_price: i64,
    /// Region: classifieds search this city; `local_only` keeps only sellers there.
    pub province: String,
    pub city: String,
    /// Neighbourhood / area inside the city (matched against the ad's location text).
    pub district: String,
    pub local_only: bool,
    /// "seller" (pricing my product) or "buyer" (where to buy) — how the results are presented.
    pub mode: String,
}

impl Default for MarketReq {
    fn default() -> Self {
        MarketReq {
            query: String::new(),
            brand: String::new(),
            platforms: vec![],
            target_margin: 25.0,
            limit: 60,
            sort: "relevance".into(),
            exact: true,
            min_price: 0,
            max_price: 0,
            province: String::new(),
            city: String::new(),
            district: String::new(),
            local_only: false,
            mode: "seller".into(),
        }
    }
}

/// Collects from every chosen platform, analyses, stores the snapshot (price history) and returns it.
pub async fn search(
    db: &Db,
    s: &Settings,
    profile: &Path,
    req: &MarketReq,
    token: &CancellationToken,
    emit: Emit<'_>,
) -> Res<Report> {
    let query = req.query.trim();
    if query.chars().count() < 2 {
        return Err("نام کالا را بنویسید؛ مثلاً «عسل» یا «زعفران سرگل».".into());
    }
    if req.max_price > 0 && req.min_price > req.max_price {
        return Err("در محدوده قیمت، «از» بزرگ‌تر از «تا» است.".into());
    }
    let chosen: Vec<&Platform> = PLATFORMS
        .iter()
        .filter(|p| req.platforms.is_empty() || req.platforms.iter().any(|x| x == p.id))
        .collect();
    let full_query = if req.brand.trim().is_empty() {
        query.to_string()
    } else {
        format!("{query} {}", req.brand.trim())
    };
    let qw = words(query); // base query words for filtering logic
    let mut all = vec![];
    let mut status = vec![];
    let mut eng: Option<scanner::Engine> = None;
    for (i, p) in chosen.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }
        scanner::status(
            emit,
            format!(
                "جستجوی «{full_query}» در {} ({} از {})",
                p.name,
                i + 1,
                chosen.len()
            ),
        );
        let r = tokio::select! {
            r = collect_one(db, s, profile, p, req, &full_query, &mut eng, token, emit) => r,
            _ = token.cancelled() => Ok(vec![]),
        };
        match r {
            Ok(mut found) => {
                for o in &mut found {
                    verify_links(o);
                }
                let n = found
                    .iter()
                    .filter(|o| matches(&o.title, &qw, req.exact))
                    .count();
                scanner::log(
                    emit,
                    if n > 0 { "success" } else { "warning" },
                    format!("{}: {} پیشنهاد مرتبط از {} نتیجه", p.name, n, found.len()),
                );
                status.push((p.id.to_string(), n, String::new()));
                all.extend(found);
            }
            Err(e) => {
                scanner::log(emit, "warning", format!("{}: {e}", p.name));
                status.push((p.id.to_string(), 0, e));
            }
        }
        emit(ScanEvent::Progress {
            keyword: full_query.clone(),
            collected: all.len() as i64,
            checked: (i + 1) as i64,
            found: all.len() as i64,
            target: chosen.len() as i64,
        });
    }
    scanner::shut(eng).await;
    // the user's price range and region, before any statistics
    let before = all.len();
    all.retain(|o| {
        (req.min_price <= 0 || o.price >= req.min_price)
            && (req.max_price <= 0 || o.price <= req.max_price)
    });
    let out_of_range = before - all.len();
    let place = if req.district.trim().is_empty() {
        &req.city
    } else {
        &req.district
    };
    if req.local_only && !place.trim().is_empty() {
        let c = words(place);
        all.retain(|o| {
            let l = words(&o.location);
            c.iter().all(|w| l.iter().any(|x| tok_eq(x, w)))
        });
    }
    let mut report = analyze_opts(
        query,
        &req.brand,
        all,
        if req.target_margin > 0.0 {
            req.target_margin
        } else {
            25.0
        },
        req.exact,
    );
    report.platforms = status;
    report.out_of_range = out_of_range;
    if report.offers.is_empty() {
        let hint = if out_of_range > 0 {
            format!(" {out_of_range} قیمت خارج از محدوده‌ای بود که دادید؛ محدوده را بازتر کنید.")
        } else {
            String::new()
        };
        return Err(format!("هیچ پیشنهاد مرتبطی پیدا نشد.{hint} اگر نام محصول دقیق است، حالت «همه کلمات» را امتحان کنید یا املای دیگری بنویسید (مثلاً «کیسه فریز»)."));
    }
    scanner::status(emit, "جستجوهای مرتبط…");
    let mut related = suggestions(db, &chosen, query).await;
    related.retain(|r| !report.subcategories.iter().any(|x| words(x) == words(r)));
    report.related = related;
    report.req = MarketReq {
        query: query.into(),
        ..req.clone()
    };
    report.id = db.market_save(&report)?;
    report.history = db.market_history(query, &report.brand)?;
    report.source_history = db.market_source_history(query, &report.brand)?;
    db.log(
        "market",
        &format!(
            "تحلیل محصول «{}»: {} پیشنهاد از {} پلتفرم",
            report.query,
            report.offers.len(),
            report.channels.len()
        ),
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    #[test]
    fn smart_matching() {
        use super::{matches, mine_brands, Offer, words};
        // exact product, not the accessory of another one
        assert!(matches("چوب پینگ پنگ دونیک مدل ۳۰۰", &words("چوب پینگ پنگ"), true));
        assert!(matches("خرید چوب پینگ‌پنگ باترفلای", &words("چوب پینگ پنگ"), true));
        assert!(!matches("میز پینگ پنگ تاشو با دو عدد چوب", &words("چوب پینگ پنگ"), true));
        assert!(!matches("میز پینگ پنگ", &words("چوب پینگ پنگ"), true));
        assert!(
            matches("میز پینگ پنگ تاشو با دو عدد چوب", &words("چوب پینگ پنگ"), false),
            "broad mode: any order"
        );
        // spelling variants: «فریز» / «فریزر», Arabic ی and ک, half-spaces
        assert!(matches("کیسه فریز آغوش ۱۰۰ عددی", &words("کیسه فریزر آغوش"), true));
        assert!(matches("كيسه‌هاي فريزر آغوش", &words("کیسه فریزر"), true));
        assert!(!matches("کیسه زباله آغوش", &words("کیسه فریزر"), true));
        // brands named only in titles are found, while descriptive words («چند», «استاندارد») are rejected
        let o = |t: &str| Offer {
            title: t.into(),
            price: 50_000,
            ..Default::default()
        };
        let mined = mine_brands(
            &[
                o("کیسه فریزر آغوش ۳۰ عددی"),
                o("کیسه فریزر آغوش رول"),
                o("کیسه فریزر گلرنگ"),
                o("کیسه فریزر ۵۰ عددی"),
                o("کیسه فریزر گلرنگ بزرگ"),
            ],
            "کیسه فریزر",
        );
        assert_eq!(mined, vec!["آغوش".to_string(), "گلرنگ".to_string()]);
        let bad = mine_brands(
            &[
                o("عسل چند گیاه ۱ کیلویی"),
                o("عسل چند گیاه طبیعی"),
                o("عسل استاندارد ۵۰۰ گرم"),
                o("عسل استاندارد ۱ کیلو"),
            ],
            "عسل",
        );
        assert!(bad.is_empty(), "{bad:?}");
        let r = super::analyze(
            "کیسه فریزر",
            "آغوش",
            vec![
                o("کیسه فریزر آغوش ۳۰ عددی"),
                o("کیسه فریز آغوش رول"),
                o("کیسه فریزر گلرنگ"),
            ],
            20.0,
        );
        assert_eq!(r.offers.len(), 2, "brand filter keeps both «آغوش» offers");
        assert!(r.brand_list.iter().any(|b| b.name == "آغوش" && b.count == 2));
    }

    #[test]
    fn helpers() {
        assert_eq!(super::iso_utc(0), "1970-01-01 00:00:00");
        assert_eq!(super::iso_utc(1_760_000_000), "2025-10-09 08:53:20");
        assert_eq!(super::city_slug("فارس", ""), "shiraz");
        assert_eq!(super::city_slug("تهران", "کرج"), "karaj");
        assert_eq!(super::city_slug("", "بروجن"), "iran");
        let mut v = vec!["گلرنگ", "آغوش", "پاکشو", "بهداد", "Zara"];
        v.sort_by_key(|x| super::collate(x));
        assert_eq!(v, vec!["آغوش", "بهداد", "پاکشو", "گلرنگ", "Zara"]);
        assert_eq!(super::brand_key("برند خوانسار"), super::brand_key("خوانسار"));
        let j = serde_json::json!({"images": {"main": {"url": ["https://dkstatics.ir/a.jpg"]}}});
        assert_eq!(
            super::image_of(&j["images"], 3).as_deref(),
            Some("https://dkstatics.ir/a.jpg")
        );
        assert_eq!(
            super::date_of(&serde_json::json!(1_760_000_000_000_i64)).as_deref(),
            Some("2025-10-09 08:53:20")
        );
    }

    #[test]
    fn links_are_exact_or_dropped() {
        let mut o = Offer { platform: "basalam".into(), url: "/shahd/product/12".into(), seller_url: "/shahd".into(), ..Default::default() };
        super::verify_links(&mut o);
        assert_eq!((o.url.as_str(), o.link.as_str(), o.seller_url.as_str()), ("https://basalam.com/shahd/product/12", "exact", "https://basalam.com/shahd"));
        let mut home = Offer { platform: "basalam".into(), url: "https://basalam.com/".into(), ..Default::default() };
        super::verify_links(&mut home);
        assert_eq!((home.url.as_str(), home.link.as_str()), ("", ""), "the home page is never a product link");
        let mut search = Offer { platform: "digikala".into(), url: "https://www.digikala.com/search/?q=x".into(), ..Default::default() };
        super::verify_links(&mut search);
        assert!(search.url.is_empty());
        assert!(super::platform("divar").is_some());
    }

    use super::*;
    use serde_json::json;

    fn offer(
        platform: &str,
        title: &str,
        price: i64,
        seller: &str,
        rating: Option<f64>,
        reviews: i64,
    ) -> Offer {
        Offer {
            platform: platform.into(),
            title: title.into(),
            price,
            seller: seller.into(),
            rating,
            reviews,
            ..Default::default()
        }
    }

    #[test]
    fn walks_any_json() {
        // digikala-like: nested price object in Rial, brand object
        let dk = json!({"data": {"products": [
            {"title_fa": "عسل گون خوانسار ۵۰۰ گرمی", "data_layer": {"brand": "خوانسار"}, "default_variant": {"price": {"selling_price": 2_450_000, "rrp_price": 2_900_000}, "seller": {"title": "فروشگاه الف", "rating": {"total_rate": 92}}}, "rating": {"rate": 4.4, "count": 120}},
            {"title_fa": "x", "price": 10}
        ]}});
        let o = offers_in(&dk, "digikala", true);
        assert_eq!(o.len(), 1);
        assert_eq!(
            (o[0].price, o[0].brand.as_str(), o[0].seller.as_str()),
            (245_000, "خوانسار", "فروشگاه الف")
        );
        assert!(o[0].rating.unwrap() > 4.0);
        // torob-like: plain fields, shop count text
        let tb = json!({"results": [{"name1": "عسل کنار طبیعی ۱ کیلویی", "price": 610000, "shop_text": "در ۱۴ فروشگاه", "web_client_absolute_url": "/p/abc/"}]});
        let o = offers_in(&tb, "torob", false);
        assert_eq!(
            (o[0].price, o[0].shops, o[0].url.as_str()),
            (610_000, 14, "https://torob.com/p/abc/")
        );
        // DOM cards with Persian digits
        let card =
            json!({"title": "زعفران سرگل یک مثقالی", "price": "۸۹۰٬۰۰۰", "url": "https://emalls.ir/x"});
        assert_eq!(offers_in(&card, "emalls", false)[0].price, 890_000);
    }

    #[test]
    fn sizes() {
        assert_eq!(size_grams("عسل گون ۵۰۰ گرمی"), Some(500.0));
        assert_eq!(size_grams("عسل کنار 1 کیلویی"), Some(1000.0));
        assert_eq!(size_grams("روغن ۱.۵ لیتری"), Some(1500.0));
        assert_eq!(size_grams("زعفران ۲ مثقال"), Some(9.375));
        assert_eq!(size_grams("گوشی سامسونگ A55"), None);
    }

    #[test]
    fn analysis() {
        let offers = vec![
            offer("torob", "عسل گون خوانسار ۵۰۰ گرمی", 300_000, "الف", Some(4.6), 300),
            offer("torob", "عسل گون خوانسار ۱ کیلویی", 560_000, "ب", Some(4.2), 40),
            offer("digikala", "عسل طبیعی سبلان ۱ کیلویی", 640_000, "ج", Some(4.0), 900),
            offer("digikala", "عسل کنار شهدینه ۹۶۰ گرمی", 700_000, "د", None, 0),
            offer("basalam", "عسل چهل گیاه ۱ کیلویی", 480_000, "زنبوردار", Some(4.9), 60),
            offer("basalam", "عسل وحشی ۱ کیلویی", 5_900_000, "گران", Some(3.0), 2), // outlier
            offer("basalam", "شامپو عسل", 90_000, "نامرتبط", None, 0),              // not about honey? (contains عسل) → kept
            offer("torob", "مربا هویج", 50_000, "نامرتبط", None, 0),                // irrelevant → dropped
        ];
        let r = analyze("عسل", "", offers, 25.0);
        assert!(r.offers.iter().all(|o| o.title.contains("عسل")));
        assert_eq!(r.stats.basis, "per_kg", "most titles state a size");
        assert!(r.stats.outliers >= 1, "{:?}", r.stats);
        assert!(r.stats.max < 5_900_000, "outlier left out of min/max");
        assert!(r.stats.min <= r.stats.median && r.stats.median <= r.stats.max);
        assert!(r.srp >= r.stats.p25 && r.srp <= shelf(r.stats.p75 as f64));
        assert_eq!(r.srp % 1000, 0);
        assert!(r.brands.iter().any(|b| b.name == "خوانسار" && b.count == 2));
        let total: f64 = r.channels.iter().map(|c| c.pct).sum();
        assert!((total - 100.0).abs() < 0.5);
        assert_eq!(r.tiers.iter().map(|t| t.count).sum::<usize>(), r.stats.count);
        assert!(
            r.offers.windows(2).all(|w| w[0].value >= w[1].value),
            "sorted by value score"
        );
        assert!(r.subcategories.iter().any(|s| s == "عسل گون"));
        assert!(r.brand_chips.iter().any(|b| b == "خوانسار"));
        assert!(!r.advice.buy_from.is_empty() && r.advice.sell_price == r.srp);
        // brand filter
        let r2 = analyze(
            "عسل",
            "خوانسار",
            vec![
                offer("torob", "عسل خوانسار", 300_000, "", None, 0),
                offer("torob", "عسل سبلان", 300_000, "", None, 0),
            ],
            20.0,
        );
        assert_eq!(r2.offers.len(), 1);
    }

    #[test]
    fn relevance_and_shelf() {
        assert!(relevant("زعفران سرگل قائنات", "زعفران سرگل"));
        assert!(!relevant("زعفران نگین", "زعفران سرگل"));
        assert_eq!(shelf(123_400.0), 124_000);
        assert_eq!(shelf(1_234_000.0), 1_240_000);
    }
}