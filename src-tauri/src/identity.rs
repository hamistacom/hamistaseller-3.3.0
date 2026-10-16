//! Product identity: «is this listing the same product?» across shops that name it differently.
//!
//! A title is reduced to a `Spec`: brand, pack size, pack count, model codes (A55, 256GB, 15),
//! variant words (Pro, Max, Mini …) and the remaining content words. Hard facts must agree —
//! a different size, count, model code or variant is never the same product, however similar the
//! words are — and only then do the words decide. Used to merge the same product across platforms
//! (`group`) and to find a seller's own catalogue items in the market (`match_item`).
use crate::market::{brand_key, norm, size_grams, tok_eq, Offer, FILLERS, NOT_BRANDS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const COUNT_UNITS: &[&str] = &["عدد", "عددی", "تایی", "تا", "عدد\u{200c}ی", "pcs", "pc", "بسته\u{200c}ای", "برگ", "ورق", "رول"];
const SIZE_UNITS: &[&str] = &["کیلو", "کیلویی", "کیلوگرم", "کیلوگرمی", "گرم", "گرمی", "لیتر", "لیتری", "ليتر", "میلی", "میلی\u{200c}لیتر", "سی", "سی\u{200c}سی", "مثقال", "مثقالی", "kg", "gr", "ml", "lit"];
/// Technical units whose number is part of the model: (spelling, canonical).
const TECH_UNITS: &[(&str, &str)] = &[
    ("گیگ", "gb"), ("گیگابایت", "gb"), ("gb", "gb"), ("ترابایت", "tb"), ("ترا", "tb"), ("tb", "tb"),
    ("مگاپیکسل", "mp"), ("mp", "mp"), ("میلی\u{200c}آمپر", "mah"), ("mah", "mah"), ("وات", "w"), ("w", "w"),
    ("اینچ", "in"), ("اینچی", "in"), ("inch", "in"), ("هرتز", "hz"), ("hz", "hz"), ("ولت", "v"), ("v", "v"), ("متر", "m"), ("متری", "m"), ("سانت", "cm"), ("سانتی", "cm"), ("cm", "cm"),
];
/// Words that make a different model of the same line: (spelling, canonical).
const VARIANTS: &[(&str, &str)] = &[
    ("pro", "pro"), ("پرو", "pro"), ("max", "max"), ("مکس", "max"), ("plus", "plus"), ("پلاس", "plus"), ("ultra", "ultra"), ("الترا", "ultra"), ("اولترا", "ultra"),
    ("mini", "mini"), ("مینی", "mini"), ("lite", "lite"), ("لایت", "lite"), ("fe", "fe"), ("neo", "neo"), ("نئو", "neo"),
];
/// Words that say nothing about which product it is.
const STOP: &[&str] = &[
    "مدل", "رنگ", "نیم", "سایز", "ظرفیت", "حافظه", "رم", "داخلی", "نسخه", "سری", "کد", "اصلی", "اصل", "اورجینال", "گارانتی", "ضمانت", "همراه", "با", "و", "از", "در", "به", "برای",
    "مشکی", "سفید", "آبی", "قرمز", "سبز", "زرد", "طوسی", "خاکستری", "نقره", "نقره\u{200c}ای", "طلایی", "صورتی", "بنفش", "سرمه", "سرمه\u{200c}ای", "black", "white", "blue",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spec {
    pub brand: String,       // brand_key, "" = unknown
    pub size: Option<f64>,   // grams / millilitres
    pub count: Option<u32>,  // pieces in the pack
    pub codes: Vec<String>,  // letter+digit model codes: a55, 5g, s24
    pub numbers: Vec<String>, // bare numbers: «آیفون 15», «سری 7»
    pub units: Vec<(String, String)>, // (unit, value): (gb, 256), (in, 55)
    pub variants: Vec<String>,
    pub words: Vec<String>,
}

/// Tokens of a title: one spelling (`norm`), decimals kept together («1.5»).
fn tokens(s: &str) -> Vec<String> {
    let n = norm(s).replace(['٫', '/'], ".");
    let mut out: Vec<String> = n.split(|c: char| !(c.is_alphanumeric() || c == '.')).map(|t| t.trim_matches('.').to_string()).filter(|t| !t.is_empty()).collect();
    // «4G»/«5G» stay codes; a number glued to a unit («256GB», «500گرمی») splits into number + unit
    let mut split = vec![];
    for t in out.drain(..) {
        let digits = t.chars().take_while(|c| c.is_ascii_digit() || *c == '.').count();
        let unit: String = t.chars().skip(digits).collect();
        let known = SIZE_UNITS.contains(&unit.as_str()) || COUNT_UNITS.contains(&unit.as_str()) || TECH_UNITS.iter().any(|(u, _)| *u == unit);
        if digits > 0 && !unit.is_empty() && known {
            split.push(t.chars().take(digits).collect());
            split.push(unit);
        } else {
            split.push(t);
        }
    }
    split
}

fn is_num(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '.') && t.chars().any(|c| c.is_ascii_digit())
}

pub fn spec(title: &str, brand: &str) -> Spec {
    let t = tokens(title);
    let mut s = Spec { brand: brand_key(brand), size: size_grams(title), ..Default::default() };
    let n = norm(title);
    if s.size.is_none() && (n.contains("نیم کیلو") || n.contains("نیم کیلوگرم")) {
        s.size = Some(500.0);
    }
    let brand_words: Vec<String> = crate::market::words(brand);
    let mut i = 0;
    while i < t.len() {
        let w = t[i].as_str();
        let next = t.get(i + 1).map(String::as_str).unwrap_or("");
        if is_num(w) {
            if SIZE_UNITS.contains(&next) || SIZE_UNITS.iter().any(|u| next.starts_with(u) && u.chars().count() >= 3) {
                i += 2; // the size itself: compared through `size`
                continue;
            }
            if COUNT_UNITS.contains(&next) {
                s.count = w.parse::<f64>().ok().map(|c| c as u32);
                i += 2;
                continue;
            }
            if let Some((_, u)) = TECH_UNITS.iter().find(|(u, _)| *u == next) {
                s.units.push((u.to_string(), w.to_string()));
                i += 2;
                continue;
            }
            if i > 0 && ["پک", "بسته"].contains(&t[i - 1].as_str()) {
                s.count = w.parse::<f64>().ok().map(|c| c as u32);
            } else if w.parse::<f64>().is_ok_and(|v| v < 100_000.0) {
                s.numbers.push(w.to_string());
            }
            i += 1;
            continue;
        }
        if let Some((_, v)) = VARIANTS.iter().find(|(x, _)| *x == w) {
            s.variants.push(v.to_string());
        } else if w.chars().any(|c| c.is_ascii_digit()) && w.chars().any(|c| c.is_alphabetic()) {
            s.codes.push(w.to_string());
        } else if w.chars().count() >= 2
            && !STOP.contains(&w)
            && !FILLERS.contains(&w)
            && !NOT_BRANDS.contains(&w)
            && !SIZE_UNITS.contains(&w)
            && !COUNT_UNITS.contains(&w)
            && !brand_words.iter().any(|b| b == w)
        {
            s.words.push(w.to_string());
        }
        i += 1;
    }
    for v in [&mut s.codes, &mut s.numbers, &mut s.variants, &mut s.words] {
        v.sort();
        v.dedup();
    }
    s.units.sort();
    s.units.dedup();
    s
}

fn subset<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    a.iter().all(|x| b.contains(x))
}

/// Hard facts that make two listings different products.
pub fn conflict(a: &Spec, b: &Spec) -> bool {
    if !a.brand.is_empty() && !b.brand.is_empty() && a.brand != b.brand && !a.brand.contains(&b.brand) && !b.brand.contains(&a.brand) {
        return true;
    }
    if let (Some(x), Some(y)) = (a.size, b.size) {
        if (x - y).abs() > x.max(y) * 0.03 {
            return true;
        }
    }
    if let (Some(x), Some(y)) = (a.count, b.count) {
        if x != y {
            return true;
        }
    }
    if a.variants != b.variants {
        return true;
    }
    if !a.codes.is_empty() && !b.codes.is_empty() && !subset(&a.codes, &b.codes) && !subset(&b.codes, &a.codes) {
        return true;
    }
    if !a.numbers.is_empty() && !b.numbers.is_empty() && !subset(&a.numbers, &b.numbers) && !subset(&b.numbers, &a.numbers) {
        return true;
    }
    // per unit (storage, screen size …): one value must be among the other's
    let units: Vec<&String> = a.units.iter().map(|(u, _)| u).filter(|u| b.units.iter().any(|(v, _)| v == *u)).collect();
    units.into_iter().any(|u| {
        let va: Vec<&String> = a.units.iter().filter(|(x, _)| x == u).map(|(_, v)| v).collect();
        let vb: Vec<&String> = b.units.iter().filter(|(x, _)| x == u).map(|(_, v)| v).collect();
        !subset(&va, &vb) && !subset(&vb, &va)
    })
}

/// How many of `a`'s words appear in `b` (spelling-tolerant).
fn found(a: &[String], b: &[String]) -> usize {
    a.iter().filter(|w| b.iter().any(|x| tok_eq(x, w) || tok_eq(w, x))).count()
}

/// 0..1: the same product? 0 when a hard fact differs.
pub fn similarity(a: &Spec, b: &Spec) -> f64 {
    if conflict(a, b) {
        return 0.0;
    }
    let (short, long) = if a.words.len() <= b.words.len() { (a, b) } else { (b, a) };
    let hit = found(&short.words, &long.words) as f64;
    let cov = if short.words.is_empty() { 0.0 } else { hit / short.words.len() as f64 };
    let dice = if a.words.len() + b.words.len() == 0 { 0.0 } else { 2.0 * hit / (a.words.len() + b.words.len()) as f64 };
    let mut score = 0.5 * cov + 0.5 * dice;
    // the same model code on both sides is strong evidence
    if !a.codes.is_empty() && a.codes == b.codes && cov >= 0.5 {
        score = score.max(0.85);
    }
    // a brand on one side only: probably a different (or unbranded) product
    if a.brand.is_empty() != b.brand.is_empty() {
        score *= 0.85;
    }
    score
}

/// Merge threshold: below it two listings are kept apart (precision over recall).
pub const SAME: f64 = 0.7;

/// The same product on several listings / platforms.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Group {
    pub title: String,
    pub brand: String,
    pub image: String,
    pub offers: Vec<usize>, // indexes into the report's offers, cheapest first
    pub platforms: Vec<String>,
    pub min: i64,
    pub median: i64,
    pub max: i64,
    /// (max − min) / min, %: how much the same product's price differs between sellers.
    pub spread_pct: f64,
}

/// Groups listings of the same product. Only groups of two or more are returned, those sold on
/// most platforms first.
pub fn group(offers: &[Offer]) -> Vec<Group> {
    let specs: Vec<Spec> = offers.iter().map(|o| spec(&o.title, &o.brand)).collect();
    let mut groups: Vec<Vec<usize>> = vec![];
    for (i, s) in specs.iter().enumerate() {
        if offers[i].price <= 0 || s.words.is_empty() && s.codes.is_empty() {
            continue;
        }
        // best group whose every member is the same product
        let best = groups
            .iter()
            .enumerate()
            .filter_map(|(g, m)| {
                let worst = m.iter().map(|&j| similarity(s, &specs[j])).fold(f64::MAX, f64::min);
                (worst >= SAME).then_some((g, worst))
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        match best {
            Some((g, _)) => groups[g].push(i),
            None => groups.push(vec![i]),
        }
    }
    let mut out: Vec<Group> = groups
        .into_iter()
        .filter(|m| m.len() >= 2)
        .map(|mut m| {
            m.sort_by_key(|&i| offers[i].price);
            let mut prices: Vec<i64> = m.iter().map(|&i| offers[i].price).collect();
            prices.sort_unstable();
            let mut platforms: Vec<String> = m.iter().map(|&i| offers[i].platform.clone()).collect();
            platforms.sort();
            platforms.dedup();
            // the name: the most specific (most codes), then the shortest full title
            let rep = *m.iter().max_by_key(|&&i| (specs[i].codes.len() + specs[i].units.len(), std::cmp::Reverse(offers[i].title.chars().count()))).unwrap_or(&m[0]);
            let mut brands: BTreeMap<&str, usize> = BTreeMap::new();
            m.iter().filter(|&&i| !offers[i].brand.is_empty()).for_each(|&i| *brands.entry(offers[i].brand.as_str()).or_default() += 1);
            let (min, max) = (prices[0], prices[prices.len() - 1]);
            Group {
                title: offers[rep].title.clone(),
                brand: brands.into_iter().max_by_key(|x| x.1).map(|x| x.0.to_string()).unwrap_or_default(),
                image: m.iter().map(|&i| offers[i].image.as_str()).find(|x| !x.is_empty()).unwrap_or("").to_string(),
                platforms,
                min,
                median: crate::market::pct(&prices, 0.5),
                max,
                spread_pct: if min > 0 { ((max - min) as f64 * 1000.0 / min as f64).round() / 10.0 } else { 0.0 },
                offers: m,
            }
        })
        .collect();
    out.sort_by(|a, b| b.platforms.len().cmp(&a.platforms.len()).then(b.offers.len().cmp(&a.offers.len())).then(b.spread_pct.total_cmp(&a.spread_pct)));
    out
}

/// A seller's own item found among market listings: (offer indexes, confidence).
/// "high" = every word, code and size of the item is on the listing; "medium" = most words and no
/// conflicting fact; "none". When high matches exist only those are returned.
pub fn match_item(name: &str, brand: &str, offers: &[Offer]) -> (Vec<usize>, &'static str) {
    let mut item = spec(name, brand);
    if item.brand.is_empty() {
        // a known brand of these results named in the item («عسل خوانسار …»)
        let w = crate::market::words(name);
        if let Some(b) = offers.iter().map(|o| o.brand.as_str()).filter(|b| !b.is_empty()).find(|b| {
            let bw = crate::market::words(b);
            !bw.is_empty() && bw.iter().all(|x| w.contains(x))
        }) {
            item = spec(name, b);
        }
    }
    let (mut high, mut medium) = (vec![], vec![]);
    for (i, o) in offers.iter().enumerate() {
        if o.price <= 0 {
            continue;
        }
        let s = spec(&o.title, &o.brand);
        if conflict(&item, &s) {
            continue;
        }
        let need = item.words.len();
        let hit = found(&item.words, &s.words) + item.words.iter().filter(|w| crate::market::words(&o.brand).contains(w)).count();
        let cov = if need == 0 { 1.0 } else { (hit.min(need)) as f64 / need as f64 };
        let codes_ok = subset(&item.codes, &s.codes) && subset(&item.numbers, &s.numbers) && subset(&item.units, &s.units);
        let size_ok = item.size.is_none() || s.size.is_some();
        let brand_ok = item.brand.is_empty() || !s.brand.is_empty() || crate::market::words(&o.title).concat().contains(&item.brand);
        if cov >= 1.0 && codes_ok && size_ok && brand_ok && (need > 0 || !item.codes.is_empty()) {
            high.push(i);
        } else if cov >= if need <= 2 { 1.0 } else { 0.75 } && (item.codes.is_empty() || subset(&item.codes, &s.codes)) {
            medium.push(i);
        }
    }
    match (high.is_empty(), medium.is_empty()) {
        (false, _) => (high, "high"),
        (true, false) => (medium, "medium"),
        _ => (vec![], "none"),
    }
}

/// A short search text for a long catalogue name: brand, main words and model codes. Shops' search
/// engines do badly with long names; the precise check is `match_item` afterwards.
pub fn search_query(name: &str, brand: &str) -> String {
    let s = spec(name, brand);
    let mut parts: Vec<String> = vec![];
    if !brand.trim().is_empty() {
        parts.push(brand.trim().to_string());
    }
    // the words in the order the name has them
    let t = tokens(name);
    let keep: Vec<&String> = t.iter().filter(|w| s.words.contains(w) || s.codes.contains(w) || s.variants.iter().any(|v| VARIANTS.iter().any(|(x, c)| x == w && c == v)) || s.numbers.contains(w)).collect();
    for w in keep {
        if parts.len() >= 6 {
            break;
        }
        if !parts.iter().any(|p| p == w) {
            parts.push(w.clone());
        }
    }
    if parts.is_empty() {
        name.trim().to_string()
    } else {
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(platform: &str, title: &str, brand: &str, price: i64) -> Offer {
        Offer { platform: platform.into(), title: title.into(), brand: brand.into(), price, ..Default::default() }
    }

    #[test]
    fn specs() {
        let s = spec("گوشی موبایل سامسونگ مدل Galaxy A55 5G ظرفیت ۲۵۶ گیگابایت رم ۸ گیگابایت", "سامسونگ");
        assert_eq!(s.codes, vec!["5g", "a55"]);
        assert_eq!(s.units, vec![("gb".to_string(), "256".to_string()), ("gb".to_string(), "8".to_string())]);
        assert!(s.words.contains(&"galaxy".to_string()) && !s.words.contains(&"سامسونگ".to_string()));
        let h = spec("عسل گون خوانسار ۵۰۰ گرمی", "");
        assert_eq!((h.size, h.numbers.len()), (Some(500.0), 0));
        assert_eq!(spec("دستمال کاغذی ۱۰۰ برگ پک ۶ عددی", "").count, Some(6));
        assert_eq!(spec("روغن ۱.۵ لیتری", "").size, Some(1500.0));
        assert_eq!(spec("آیفون 15 Pro Max", "اپل").variants, vec!["max", "pro"]);
    }

    #[test]
    fn same_product_or_not() {
        let a = spec("گوشی موبایل سامسونگ مدل Galaxy A55 5G ظرفیت 256 گیگابایت رم 8 گیگابایت", "سامسونگ");
        let b = spec("سامسونگ Galaxy A55 5G 256GB", "Samsung سامسونگ");
        let c = spec("سامسونگ Galaxy A55 5G 128GB", "سامسونگ");
        let d = spec("سامسونگ Galaxy A35 5G 256GB", "سامسونگ");
        assert!(similarity(&a, &b) >= SAME, "{}", similarity(&a, &b));
        assert_eq!(similarity(&a, &c), 0.0, "different storage");
        assert_eq!(similarity(&b, &d), 0.0, "different model");
        let p = spec("گوشی اپل آیفون 15 پرو 128 گیگ", "اپل");
        let q = spec("Apple iPhone 15 128GB آیفون", "اپل");
        assert_eq!(similarity(&p, &q), 0.0, "Pro is another model");
        let x = spec("عسل گون خوانسار ۵۰۰ گرمی", "خوانسار");
        let y = spec("عسل طبیعی گون خوانسار نیم کیلویی", "خوانسار");
        let z = spec("عسل گون خوانسار ۱ کیلویی", "خوانسار");
        let w = spec("عسل گون سبلان ۵۰۰ گرمی", "سبلان");
        assert!(similarity(&x, &y) >= SAME);
        assert_eq!(similarity(&x, &z), 0.0, "different size");
        assert_eq!(similarity(&x, &w), 0.0, "different brand");
    }

    #[test]
    fn groups_across_platforms() {
        let offers = vec![
            o("torob", "عسل گون خوانسار ۵۰۰ گرمی", "خوانسار", 310_000),
            o("digikala", "عسل گون طبیعی خوانسار - 500 گرم", "خوانسار", 345_000),
            o("basalam", "خرید عسل گون خوانسار ۵۰۰ گرمی", "خوانسار", 298_000),
            o("digikala", "عسل گون خوانسار ۱ کیلویی", "خوانسار", 590_000),
            o("torob", "عسل چهل گیاه سبلان ۹۰۰ گرمی", "سبلان", 420_000),
            o("basalam", "عسل چهل گیاه سبلان 900 گرم", "سبلان", 405_000),
        ];
        let g = group(&offers);
        assert_eq!(g.len(), 2, "{g:?}");
        assert_eq!(g[0].platforms, vec!["basalam", "digikala", "torob"]);
        assert_eq!((g[0].min, g[0].max), (298_000, 345_000));
        assert_eq!(offers[g[0].offers[0]].price, 298_000, "cheapest first");
        assert!(g[0].spread_pct > 15.0 && g[0].spread_pct < 16.0);
        assert!(!g.iter().any(|x| x.offers.contains(&3)), "the 1 kg jar is another product");
    }

    #[test]
    fn catalogue_items() {
        let offers = vec![
            o("torob", "کیسه فریزر آغوش ۱۰۰ عددی سایز متوسط", "", 85_000),
            o("digikala", "کیسه فریزر رولی آغوش 100 عددی", "آغوش", 92_000),
            o("basalam", "کیسه فریزر آغوش ۵۰ عددی", "آغوش", 48_000),
            o("torob", "کیسه زباله آغوش ۱۰۰ عددی", "آغوش", 70_000),
            o("digikala", "کیسه فریزر نایلکس ۱۰۰ عددی", "نایلکس", 80_000),
        ];
        let (m, c) = match_item("کیسه فریزر آغوش 100 عددی", "", &offers);
        assert_eq!((m, c), (vec![0, 1], "high"));
        let (m, c) = match_item("کیسه فریزر ماهی ۱۰۰ عددی", "", &offers);
        assert!(m.is_empty() && c == "none", "{m:?}");
        assert_eq!(search_query("کیسه فریزر آغوش 100 عددی سایز متوسط درجه یک", ""), "کیسه فریزر آغوش");
        assert_eq!(search_query("گوشی سامسونگ Galaxy A55 5G ظرفیت 256 گیگ", "سامسونگ"), "سامسونگ گوشی galaxy a55 5g");
    }
}
