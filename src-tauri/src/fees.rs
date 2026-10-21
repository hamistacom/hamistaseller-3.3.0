//! Net profit per sales channel: the marketplace's commission (by product category), VAT on that
//! commission, packaging and delivery, an optional per-unit ad cost — and from them the net profit, the real
//! margin and the lowest price that still pays. Used by the price analysis («قیمت پیشنهادی»), the
//! seller's catalogue and the BuyBox repricer.
//!
//! The commission rates below are **estimates of the usual ranges** of each marketplace by product group,
//! not the platforms' contracts (those differ per seller and change over time). Every rate can be
//! overwritten per platform by the seller (`overrides`), and the page says so.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// (platform id, name, commission % per product group in the order of `GROUPS`).
/// Torob / Emalls charge per click, not per sale: their commission is 0 and the cost goes in `ad_cost`.
const TARIFF: &[(&str, &str, [f64; 9])] = &[
    //             food  fashion beauty home  digital auto  craft health other
    ("digikala", "دیجی‌کالا", [8.0, 14.0, 12.0, 10.0, 6.0, 9.0, 12.0, 10.0, 10.0]),
    ("basalam", "باسلام", [9.0, 10.0, 10.0, 9.0, 8.0, 9.0, 8.0, 9.0, 9.0]),
    ("snappshop", "اسنپ‌شاپ", [9.0, 13.0, 11.0, 10.0, 6.0, 9.0, 12.0, 10.0, 10.0]),
    ("torob", "ترب / ایمالز (هزینه کلیک)", [0.0; 9]),
    ("site", "فروش مستقیم (سایت اختصاصی)", [1.5; 9]), // payment gateway only
];

/// Product groups and the words that put a product in them (first match wins).
const GROUPS: &[(&str, &[&str])] = &[
    ("food", &["عسل", "زعفران", "خشکبار", "آجیل", "پسته", "بادام", "گردو", "چای", "قهوه", "برنج", "روغن", "ادویه", "خرما", "شیرینی", "لبنیات", "مربا", "ترشی", "شکلات", "خوراکی", "ارگانیک", "نان", "کشمش"]),
    ("fashion", &["پوشاک", "لباس", "مانتو", "کیف", "کفش", "شال", "روسری", "تیشرت", "شلوار", "پیراهن", "کت", "هودی", "اکسسوری", "ساعت", "عینک"]),
    ("beauty", &["آرایشی", "بهداشتی", "کرم", "شامپو", "عطر", "ادکلن", "رژ", "لاک", "ضدآفتاب", "پوست", "مو"]),
    ("home", &["مبل", "مبلمان", "یخچال", "ماشین لباسشویی", "جاروبرقی", "اجاق", "فرش", "پرده", "ظرف", "قابلمه", "دکوراسیون", "لوازم خانگی", "میز", "صندلی"]),
    ("digital", &["موبایل", "گوشی", "لپ‌تاپ", "لپ تاپ", "تبلت", "هدفون", "کنسول", "دوربین", "شارژر", "کابل", "مانیتور", "کامپیوتر", "دیجیتال"]),
    ("auto", &["قطعات خودرو", "لنت", "روغن موتور", "باتری خودرو", "لاستیک", "خودرو", "موتورسیکلت"]),
    ("craft", &["صنایع دستی", "سفال", "نقره", "قلمکار", "گلیم", "خاتم", "هدیه", "سوغات"]),
    ("health", &["مکمل", "ویتامین", "گیاهی", "دمنوش", "طب سنتی", "ماسک", "دستگاه فشار", "سلامت"]),
];
const GROUP_KEYS: [&str; 9] = ["food", "fashion", "beauty", "home", "digital", "auto", "craft", "health", "other"];

/// Whether `keyword` is a word of `text` (a longer word that starts with a keyword of 4+ letters counts
/// too: «عسل‌ها»). Whole words only: «نان» is not in «زنانه».
pub fn has_word(text: &str, keyword: &str) -> bool {
    let (t, k) = (crate::market::words(text), crate::market::words(keyword));
    match k.len() {
        0 => false,
        1 => t.iter().any(|w| *w == k[0] || (k[0].chars().count() >= 4 && w.starts_with(&k[0]))),
        _ => format!(" {} ", t.join(" ")).contains(&format!(" {} ", k.join(" "))),
    }
}

/// The product group of a category / product name ("other" when none of the words is in it).
pub fn group_of(text: &str) -> &'static str {
    GROUPS.iter().find(|(_, ks)| ks.iter().any(|k| has_word(text, k))).map_or("other", |g| g.0)
}

/// The group's name for the seller («مواد غذایی»).
pub fn group_name(group: &str) -> &'static str {
    ["مواد غذایی", "پوشاک و اکسسوری", "آرایشی و بهداشتی", "لوازم خانه", "کالای دیجیتال", "خودرو", "صنایع دستی", "سلامت و طب سنتی", "سایر"][slot(group)]
}

fn slot(group: &str) -> usize {
    GROUP_KEYS.iter().position(|g| *g == group).unwrap_or(8)
}

/// The platforms the calculator knows: (id, name).
pub fn platforms() -> Vec<(&'static str, &'static str)> {
    TARIFF.iter().map(|t| (t.0, t.1)).collect()
}

/// The usual commission (%) of a platform for a product group; None = unknown platform.
pub fn usual_rate(platform: &str, group: &str) -> Option<f64> {
    TARIFF.iter().find(|t| t.0 == platform).map(|t| t.2[slot(group)])
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ProfitIn {
    /// Cost price of one unit, Toman.
    pub cost: i64,
    /// Selling price of one unit, Toman.
    pub price: i64,
    /// Category or product name, to find the product group.
    pub category: String,
    /// Packaging + delivery paid by the seller per unit.
    pub pack_cost: i64,
    /// Ad cost per unit sold (e.g. Torob clicks), Toman.
    pub ad_cost: i64,
    /// VAT on the platform's commission, % (10 = the legal rate); 0 for a seller outside VAT.
    pub vat_pct: Option<f64>,
    /// The seller's own commission per platform id (overrides the usual rate).
    pub overrides: HashMap<String, f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ProfitOut {
    pub platform: String,
    pub name: String,
    pub group: String,
    pub commission_pct: f64,
    /// True when the rate is the usual one, not the seller's own.
    pub estimated: bool,
    pub price: i64,
    pub commission: i64,
    pub vat: i64,
    pub pack: i64,
    pub ad: i64,
    pub net: i64,
    pub margin_pct: f64,
    /// The lowest price at which this channel breaks even.
    pub floor_price: i64,
}

fn round(x: f64) -> i64 {
    x.round() as i64
}

/// Share of the price left after commission and its VAT: `1 − c(1+v)`.
fn keep(commission_pct: f64, vat_pct: f64) -> f64 {
    1.0 - commission_pct / 100.0 * (1.0 + vat_pct / 100.0)
}

/// The price that leaves `margin_pct` of itself as net profit on a platform (0 = break-even), rounded up to
/// a shelf price. None when the commission leaves nothing to price against.
pub fn price_for_margin(cost: i64, pack: i64, ad: i64, commission_pct: f64, vat_pct: f64, margin_pct: f64) -> Option<i64> {
    let k = keep(commission_pct, vat_pct) - margin_pct / 100.0;
    (k > 0.01).then(|| crate::market::shelf((cost + pack + ad).max(0) as f64 / k))
}

/// Net profit of selling `price` on one platform.
pub fn calc_marketplace_profit(inp: &ProfitIn, platform: &str) -> Option<ProfitOut> {
    let (_, name, _) = TARIFF.iter().find(|t| t.0 == platform)?;
    let group = group_of(&inp.category);
    let own = inp.overrides.get(platform).copied().filter(|v| v.is_finite() && (0.0..=60.0).contains(v));
    let commission_pct = own.unwrap_or_else(|| usual_rate(platform, group).unwrap_or(10.0));
    let vat_pct = inp.vat_pct.filter(|v| v.is_finite()).map_or(10.0, |v| v.clamp(0.0, 30.0));
    let commission = round(inp.price as f64 * commission_pct / 100.0);
    let vat = round(commission as f64 * vat_pct / 100.0);
    let net = inp.price - inp.cost - commission - vat - inp.pack_cost - inp.ad_cost;
    Some(ProfitOut {
        platform: platform.into(),
        name: (*name).into(),
        group: group.into(),
        commission_pct,
        estimated: own.is_none(),
        price: inp.price,
        commission,
        vat,
        pack: inp.pack_cost,
        ad: inp.ad_cost,
        net,
        margin_pct: if inp.price > 0 { (net as f64 * 1000.0 / inp.price as f64).round() / 10.0 } else { 0.0 },
        floor_price: price_for_margin(inp.cost, inp.pack_cost, inp.ad_cost, commission_pct, vat_pct, 0.0).unwrap_or(0),
    })
}

/// The same sale on every platform, best net profit first.
pub fn compare(inp: &ProfitIn) -> Vec<ProfitOut> {
    let mut v: Vec<ProfitOut> = TARIFF.iter().filter_map(|t| calc_marketplace_profit(inp, t.0)).collect();
    v.sort_by_key(|p| std::cmp::Reverse(p.net));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp() -> ProfitIn {
        ProfitIn { cost: 300_000, price: 500_000, category: "عسل گون ۱ کیلویی".into(), pack_cost: 20_000, ..Default::default() }
    }

    #[test]
    fn groups_come_from_the_words() {
        assert_eq!(group_of("عسل گون ۱ کیلویی"), "food");
        assert_eq!(group_of("مانتو زنانه"), "fashion");
        assert_eq!(group_of("گوشی سامسونگ A15"), "digital");
        assert_eq!(group_of("چیزی ناشناخته"), "other");
        // whole words only: «نان» inside «زنانه» is not bread; plurals of long words still count
        assert!(!has_word("مانتو زنانه", "نان") && has_word("نان سنگک", "نان"));
        assert!(has_word("عسل‌های طبیعی", "عسل") && has_word("ماشین لباسشویی ال‌جی", "ماشین لباسشویی"));
    }

    #[test]
    fn profit_on_each_platform() {
        let p = calc_marketplace_profit(&inp(), "digikala").unwrap();
        // food on Digikala: 8% of 500k = 40k, VAT 10% of it = 4k
        assert_eq!((p.commission_pct, p.commission, p.vat), (8.0, 40_000, 4_000));
        assert_eq!(p.net, 500_000 - 300_000 - 40_000 - 4_000 - 20_000);
        assert_eq!(p.net, 136_000);
        assert_eq!(p.margin_pct, 27.2);
        assert!(p.estimated);
        // selling direct costs only the gateway: more is left
        let site = calc_marketplace_profit(&inp(), "site").unwrap();
        assert!(site.net > p.net && site.commission == 7_500);
        // a tariff the seller typed beats the usual one
        let own = calc_marketplace_profit(&ProfitIn { overrides: HashMap::from([("digikala".to_string(), 12.0)]), ..inp() }, "digikala").unwrap();
        assert_eq!((own.commission, own.estimated), (60_000, false));
        assert!(calc_marketplace_profit(&inp(), "nowhere").is_none());
    }

    #[test]
    fn floor_price_is_where_net_profit_is_zero() {
        let i = inp();
        let p = calc_marketplace_profit(&i, "digikala").unwrap();
        // at the floor the seller neither gains nor loses (within the rounding up to a shelf price)
        let at_floor = calc_marketplace_profit(&ProfitIn { price: p.floor_price, ..i.clone() }, "digikala").unwrap();
        assert!((0..=2_000).contains(&at_floor.net), "net at the floor: {}", at_floor.net);
        let below = calc_marketplace_profit(&ProfitIn { price: p.floor_price - 5_000, ..i.clone() }, "digikala").unwrap();
        assert!(below.net < 0);
        // a wanted margin lifts the price: 20% net margin
        let want = price_for_margin(300_000, 20_000, 0, 8.0, 10.0, 20.0).unwrap();
        let got = calc_marketplace_profit(&ProfitIn { price: want, ..i }, "digikala").unwrap();
        assert!(got.margin_pct >= 20.0 && got.margin_pct < 21.0, "{}", got.margin_pct);
        // a commission that eats the whole price leaves nothing to price against
        assert!(price_for_margin(100, 0, 0, 99.0, 10.0, 0.0).is_none());
    }

    #[test]
    fn compare_sorts_by_net_and_handles_odd_input() {
        let v = compare(&inp());
        assert_eq!(v.len(), TARIFF.len());
        assert!(v.windows(2).all(|w| w[0].net >= w[1].net));
        // VAT outside the legal range is bounded; a seller outside VAT pays none
        let none = calc_marketplace_profit(&ProfitIn { vat_pct: Some(0.0), ..inp() }, "digikala").unwrap();
        assert_eq!(none.vat, 0);
        let wild = calc_marketplace_profit(&ProfitIn { vat_pct: Some(500.0), ..inp() }, "digikala").unwrap();
        assert_eq!(wild.vat, 12_000, "30% of the commission at most");
        // no price → no margin, no division by zero
        assert_eq!(calc_marketplace_profit(&ProfitIn { price: 0, ..inp() }, "basalam").unwrap().margin_pct, 0.0);
    }
}
