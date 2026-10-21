//! Buy-box position and the repricer: «am I the cheapest listing of this product?», and the price that
//! would win it back without selling at a loss. Derived from the listed prices of the offers read —
//! not from a marketplace's own (private) buy-box algorithm.
use crate::market::Offer;
use serde::{Deserialize, Serialize};

pub const WON: &str = "won";
pub const LOST: &str = "lost";
pub const NOT_LISTED: &str = "not_listed";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct BuyBox {
    /// won | lost | not_listed | "" (not checked)
    pub status: String,
    pub my_price: i64,
    pub winner_seller: String,
    pub winner_price: i64,
    pub winner_platform: String,
    pub winner_url: String,
    /// The lowest price that still earns `min_margin_pct` after the platform commission (0 = cost unknown).
    pub floor: i64,
    /// Price to set to take the buy box; 0 = none recommended.
    pub suggested: i64,
    pub advice: String,
}

pub struct Mine<'a> {
    pub seller: &'a str,
    pub url: &'a str,
    pub cost: i64,
    pub min_margin_pct: f64,
    pub category: &'a str,
}

/// 1 000 – 5 000 Toman under the winner (about 0.5 % of its price, in whole thousands).
pub fn undercut(winner: i64) -> i64 {
    ((winner / 200) / 1000 * 1000).clamp(1000, 5000)
}

fn same_url(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().split(['?', '#']).next().unwrap_or("").trim_end_matches('/').to_lowercase();
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && a == b
}

fn is_mine(o: &Offer, m: &Mine) -> bool {
    let seller = m.seller.trim().to_lowercase();
    same_url(&o.url, m.url) || (!seller.is_empty() && o.seller.to_lowercase().contains(&seller))
}

fn pack(o: Option<&Offer>) -> (String, i64, String, String) {
    o.map(|o| (o.seller.clone(), o.price, o.platform.clone(), o.url.clone())).unwrap_or_default()
}

/// Where the seller stands among `offers` (the listings of the same product), and what to do about it.
pub fn buybox(offers: &[&Offer], m: &Mine) -> BuyBox {
    let priced: Vec<&Offer> = offers.iter().copied().filter(|o| o.price > 0).collect();
    if priced.is_empty() {
        return BuyBox::default();
    }
    let mine = priced.iter().copied().filter(|o| is_mine(o, m)).min_by_key(|o| o.price);
    let rivals_all: Vec<&Offer> = priced.iter().copied().filter(|o| !is_mine(o, m)).collect();
    // the buy box is per platform: compare on the platform my listing is on
    let rivals: Vec<&Offer> = match mine {
        Some(me) if rivals_all.iter().any(|o| o.platform == me.platform) => rivals_all.iter().copied().filter(|o| o.platform == me.platform).collect(),
        _ => rivals_all,
    };
    let best = rivals.iter().copied().min_by_key(|o| o.price);
    let (winner_seller, winner_price, winner_platform, winner_url) = pack(best);
    let mut out = BuyBox {
        my_price: mine.map_or(0, |o| o.price),
        winner_seller,
        winner_price,
        winner_platform,
        winner_url,
        ..Default::default()
    };
    let Some(best) = best else {
        out.status = if mine.is_some() { WON } else { NOT_LISTED }.into();
        out.advice = if mine.is_some() { "رقیبی با همین کالا پیدا نشد؛ شما تنها فروشنده‌اید.".into() } else { "کالای شما در نتایج پیدا نشد.".into() };
        return out;
    };
    let rate = crate::fees::usual_rate(&best.platform, crate::fees::group_of(m.category)).filter(|r| *r > 0.0).unwrap_or(10.0);
    if m.cost > 0 {
        out.floor = crate::fees::price_for_margin(m.cost, 0, 0, rate, 10.0, m.min_margin_pct.clamp(0.0, 90.0)).unwrap_or(0);
    }
    let fa = crate::logic::fa_num;
    match mine {
        Some(me) if me.price <= best.price => {
            out.status = WON.into();
            // a gap to the runner-up means the price can go up without losing the box
            let room = best.price - me.price - undercut(best.price);
            out.advice = if room >= 1000 {
                out.suggested = best.price - undercut(best.price);
                format!("بای‌باکس با شماست؛ تا {} تومان می‌توانید قیمت را بالا ببرید.", fa(out.suggested as f64))
            } else {
                "بای‌باکس با شماست.".into()
            };
        }
        _ => {
            out.status = if mine.is_some() { LOST } else { NOT_LISTED }.into();
            let target = (best.price - undercut(best.price)).max(1);
            if out.floor > 0 && target < out.floor {
                out.advice = "رقابت زیان‌ده — کاهش قیمت توصیه نمی‌شود".into();
            } else {
                out.suggested = target;
                out.advice = format!("برای گرفتن بای‌باکس قیمت را روی {} تومان بگذارید", fa(target as f64));
                if out.floor == 0 {
                    out.advice.push_str(" (بهای خرید ثبت نشده؛ کفِ سود بررسی نشد)");
                }
                out.advice.push('.');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(seller: &str, platform: &str, price: i64) -> Offer {
        Offer {
            seller: seller.into(),
            platform: platform.into(),
            price,
            url: format!("https://x.ir/{seller}"),
            ..Default::default()
        }
    }
    fn run(list: &[Offer], m: &Mine) -> BuyBox {
        buybox(&list.iter().collect::<Vec<_>>(), m)
    }
    fn me(cost: i64) -> Mine<'static> {
        Mine {
            seller: "هامیستا",
            url: "",
            cost,
            min_margin_pct: 10.0,
            category: "عسل طبیعی",
        }
    }

    #[test]
    fn undercut_is_one_to_five_thousand() {
        assert_eq!((undercut(50_000), undercut(400_000), undercut(900_000), undercut(5_000_000)), (1000, 2000, 4000, 5000));
    }

    #[test]
    fn lost_with_room_suggests_a_price_under_the_winner() {
        let l = [o("هامیستا", "digikala", 420_000), o("رقیب", "digikala", 400_000), o("دیگر", "digikala", 450_000)];
        let b = run(&l, &me(250_000));
        assert_eq!((b.status.as_str(), b.winner_seller.as_str(), b.winner_price), (LOST, "رقیب", 400_000));
        assert_eq!(b.suggested, 398_000);
        assert!(b.floor > 250_000 && b.floor < 398_000, "{}", b.floor);
        assert!(b.advice.contains("بای‌باکس"));
    }

    #[test]
    fn losing_below_the_floor_is_not_recommended() {
        let l = [o("هامیستا", "digikala", 420_000), o("رقیب", "digikala", 300_000)];
        let b = run(&l, &me(290_000));
        assert_eq!(b.status, LOST);
        assert_eq!(b.suggested, 0);
        assert_eq!(b.advice, "رقابت زیان‌ده — کاهش قیمت توصیه نمی‌شود");
    }

    #[test]
    fn won_ties_and_room_to_raise() {
        let tie = run(&[o("هامیستا", "digikala", 400_000), o("رقیب", "digikala", 400_000)], &me(0));
        assert_eq!((tie.status.as_str(), tie.suggested), (WON, 0));
        let room = run(&[o("هامیستا", "digikala", 300_000), o("رقیب", "digikala", 400_000)], &me(0));
        assert_eq!((room.status.as_str(), room.suggested), (WON, 398_000));
        assert_eq!(run(&[o("هامیستا", "basalam", 300_000)], &me(0)).status, WON, "alone");
    }

    #[test]
    fn not_listed_and_my_url_match() {
        let l = [o("رقیب", "digikala", 400_000)];
        let b = run(&l, &me(0));
        assert_eq!((b.status.as_str(), b.my_price), (NOT_LISTED, 0));
        let by_url = Mine {
            seller: "",
            url: "https://x.ir/رقیب/?utm=1",
            ..me(0)
        };
        assert_eq!(run(&l, &by_url).status, WON, "my listing found by its link");
        assert_eq!(run(&[], &me(0)), BuyBox::default());
    }

    #[test]
    fn compares_on_the_platform_of_my_listing() {
        let l = [o("هامیستا", "basalam", 410_000), o("رقیب", "digikala", 300_000), o("رقیب۲", "basalam", 420_000)];
        let b = run(&l, &me(0));
        assert_eq!((b.status.as_str(), b.winner_platform.as_str()), (WON, "basalam"));
    }
}
