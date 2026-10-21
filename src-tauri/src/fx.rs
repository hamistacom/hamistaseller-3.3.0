//! Exchange rates for the price trend: the free-market dollar and 18-carat gold (Toman), one row a day.
//! Rates come from a public quote page (best effort), or are typed in by hand; a day without a rate takes
//! the closest earlier one. With them a Toman price history can be read in dollars or in grams of gold,
//! which shows whether a product really got dearer or only the currency fell.
use crate::db::{err, Db, Res};
use rusqlite::params;
use serde::{Deserialize, Serialize};

const SOURCE: &str = "https://call5.tgju.org/ajax.json";
/// Retry a failed fetch no sooner than this (seconds).
const RETRY_SECS: i64 = 6 * 3600;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Rate {
    pub date: String,
    /// Toman per US dollar (0 = unknown).
    pub usd_toman: i64,
    /// Toman per gram of 18-carat gold (0 = unknown).
    pub gold18_toman: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Adjusted {
    /// Median price per point, in dollars / grams of gold (0 where no rate is known).
    pub usd: Vec<f64>,
    pub gold: Vec<f64>,
    /// Real change first → last point, %, per mode; None = not enough rates.
    pub nominal_pct: Option<f64>,
    pub usd_pct: Option<f64>,
    pub gold_pct: Option<f64>,
    /// Days with a stored rate — with fewer than two the adjusted lines only rescale the Toman line.
    pub rate_days: usize,
    pub latest: Option<Rate>,
    /// Why today's rate could not be fetched ("" = fine).
    pub fetch_error: String,
}

/// `p` of the quote `key` under `current`, "1,234,000" Rial → Toman.
fn quote(v: &serde_json::Value, key: &str) -> i64 {
    let p = &v["current"][key]["p"];
    let rial = match p {
        serde_json::Value::String(s) => crate::logic::ascii_digits(s).chars().filter(char::is_ascii_digit).collect::<String>().parse::<i64>().unwrap_or(0),
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) as i64,
        _ => 0,
    };
    rial / 10
}

/// The two rates of one quote page (pure). Err when neither is there.
pub fn parse(body: &str) -> Res<(i64, i64)> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| "پاسخ منبع نرخ ارز خوانده نشد.")?;
    let (usd, gold) = (quote(&v, "price_dollar_rl"), quote(&v, "geram18"));
    let sane = |x: i64, lo: i64, hi: i64| if (lo..=hi).contains(&x) { x } else { 0 };
    let (usd, gold) = (sane(usd, 1_000, 10_000_000), sane(gold, 10_000, 100_000_000));
    if usd == 0 && gold == 0 {
        return Err("نرخ دلار و طلا در پاسخ منبع پیدا نشد.".into());
    }
    Ok((usd, gold))
}

fn today(db: &Db) -> String {
    db.c().query_row("SELECT date('now','localtime')", [], |r| r.get(0)).unwrap_or_default()
}

fn valid_date(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && d[..4].parse::<u32>().is_ok_and(|y| (2000..=2100).contains(&y)) && d[5..7].parse::<u32>().is_ok_and(|m| (1..=12).contains(&m)) && d[8..].parse::<u32>().is_ok_and(|x| (1..=31).contains(&x))
}

/// Saves (or replaces) a day's rate. A zero keeps the stored value of that column.
pub fn set(db: &Db, r: &Rate) -> Res<()> {
    if !valid_date(&r.date) {
        return Err("تاریخ را به شکل 2026-10-21 بنویسید.".into());
    }
    if !(0..=10_000_000).contains(&r.usd_toman) || !(0..=100_000_000).contains(&r.gold18_toman) || (r.usd_toman == 0 && r.gold18_toman == 0) {
        return Err("نرخ دلار یا طلا نامعتبر است (به تومان).".into());
    }
    db.c()
        .execute(
            "INSERT INTO fx_rates(date,usd_toman,gold18_toman) VALUES(?1,?2,?3)
             ON CONFLICT(date) DO UPDATE SET usd_toman=CASE WHEN ?2>0 THEN ?2 ELSE usd_toman END, gold18_toman=CASE WHEN ?3>0 THEN ?3 ELSE gold18_toman END",
            params![r.date, r.usd_toman, r.gold18_toman],
        )
        .map(|_| ())
        .map_err(err)
}

pub fn list(db: &Db, limit: i64) -> Res<Vec<Rate>> {
    let c = db.c();
    let mut st = c.prepare("SELECT date,usd_toman,gold18_toman FROM fx_rates ORDER BY date DESC LIMIT ?1").map_err(err)?;
    let rows = st.query_map([limit], |r| Ok(Rate { date: r.get(0)?, usd_toman: r.get(1)?, gold18_toman: r.get(2)? })).map_err(err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(err)
}

/// Fetches today's rates once (a failure is retried after a few hours). Returns the error text, "" when fine.
pub async fn refresh(db: &Db) -> String {
    let day = today(db);
    if list(db, 1).ok().and_then(|v| v.into_iter().next()).is_some_and(|r| r.date == day && r.usd_toman > 0) {
        return String::new();
    }
    let now = crate::license::now_secs();
    let last: i64 = db.kv_get("fx_tried").and_then(|v| v.parse().ok()).unwrap_or(0);
    if now - last < RETRY_SECS {
        return db.kv_get("fx_error").unwrap_or_default();
    }
    let _ = db.kv_set("fx_tried", &now.to_string());
    let res = async {
        let body = crate::enrich::client().get(SOURCE).send().await.map_err(|e| format!("اتصال به منبع نرخ ارز برقرار نشد: {}", e.without_url()))?.text().await.map_err(|e| e.to_string())?;
        let (usd, gold) = parse(&body)?;
        set(db, &Rate { date: day.clone(), usd_toman: usd, gold18_toman: gold })
    }
    .await;
    let msg = res.err().map(|e| format!("{e} — نرخ را دستی وارد کنید.")).unwrap_or_default();
    let _ = db.kv_set("fx_error", &msg);
    msg
}

/// The rate in force on `date`: the latest one on or before it, else the earliest one (rates sorted by date).
fn at<'a>(rates: &'a [Rate], date: &str, pick: fn(&Rate) -> i64) -> Option<&'a Rate> {
    let known = || rates.iter().filter(|r| pick(r) > 0);
    known().rfind(|r| r.date.as_str() <= date).or_else(|| known().next())
}

fn change(first: f64, last: f64) -> Option<f64> {
    (first > 0.0 && last > 0.0).then(|| ((last / first - 1.0) * 1000.0).round() / 10.0)
}

/// A price history (date, median, min) in dollars and grams of gold (pure). `rates` sorted by date.
pub fn adjust(points: &[(String, i64, i64)], rates: &[Rate]) -> Adjusted {
    let conv = |pick: fn(&Rate) -> i64, scale: f64| -> Vec<f64> {
        points
            .iter()
            .map(|(d, median, _)| at(rates, d, pick).map_or(0.0, |r| (*median as f64 / pick(r) as f64 * scale).max(0.0)))
            .map(|x| (x * 1000.0).round() / 1000.0)
            .collect()
    };
    let usd = conv(|r| r.usd_toman, 1.0);
    let gold = conv(|r| r.gold18_toman, 1.0);
    let ends = |v: &[f64]| v.first().zip(v.last()).and_then(|(a, b)| change(*a, *b));
    Adjusted {
        nominal_pct: points.first().zip(points.last()).and_then(|(a, b)| change(a.1 as f64, b.1 as f64)),
        usd_pct: ends(&usd),
        gold_pct: ends(&gold),
        usd,
        gold,
        rate_days: rates.len(),
        latest: rates.last().cloned(),
        fetch_error: String::new(),
    }
}

/// The trend of a product in dollars and gold; also tries to fetch today's rates first.
pub async fn view(db: &Db, points: &[(String, i64, i64)]) -> Res<Adjusted> {
    let fetch_error = refresh(db).await;
    let mut rates = list(db, 2000)?;
    rates.reverse();
    Ok(Adjusted { fetch_error, ..adjust(points, &rates) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(d: &str, usd: i64, gold: i64) -> Rate {
        Rate { date: d.into(), usd_toman: usd, gold18_toman: gold }
    }

    #[test]
    fn parses_the_quote_page() {
        let body = r#"{"current":{"price_dollar_rl":{"p":"1,050,000"},"geram18":{"p":"۶٬۵۰۰٬۰۰۰"},"x":{"p":"1"}}}"#;
        assert_eq!(parse(body).unwrap(), (105_000, 650_000));
        assert_eq!(parse(r#"{"current":{"geram18":{"p":"6500000"}}}"#).unwrap(), (0, 650_000));
        assert!(parse(r#"{"current":{}}"#).is_err() && parse("نه json").is_err());
        assert!(parse(r#"{"current":{"price_dollar_rl":{"p":"12"}}}"#).is_err(), "an absurd rate is not kept");
    }

    #[test]
    fn dollar_rise_hides_a_real_fall() {
        // the Toman price rose 20 %, the dollar rose 50 %: in dollars the product got cheaper
        let pts = vec![("2026-01-01".into(), 100_000, 90_000), ("2026-06-01".into(), 120_000, 100_000)];
        let rates = vec![r("2026-01-01", 50_000, 400_000), r("2026-06-01", 75_000, 500_000)];
        let a = adjust(&pts, &rates);
        assert_eq!(a.nominal_pct, Some(20.0));
        assert_eq!(a.usd_pct, Some(-20.0));
        assert_eq!(a.gold_pct, Some(-4.0));
        assert_eq!(a.usd, vec![2.0, 1.6]);
        assert_eq!(a.rate_days, 2);
    }

    #[test]
    fn a_day_without_a_rate_takes_the_earlier_one() {
        let rates = vec![r("2026-03-01", 60_000, 0), r("2026-05-01", 70_000, 450_000)];
        assert_eq!(at(&rates, "2026-04-15", |x| x.usd_toman).unwrap().date, "2026-03-01");
        assert_eq!(at(&rates, "2026-02-01", |x| x.usd_toman).unwrap().date, "2026-03-01", "before the first: the first");
        assert_eq!(at(&rates, "2026-04-15", |x| x.gold18_toman).unwrap().date, "2026-05-01", "gold known only later");
        assert!(adjust(&[("2026-01-01".into(), 1000, 900)], &[]).usd == vec![0.0] && adjust(&[], &rates).usd_pct.is_none());
    }

    #[test]
    fn dates_and_rates_are_checked() {
        assert!(valid_date("2026-10-21") && !valid_date("21/10/2026") && !valid_date("2026-13-01"));
    }
}
