//! Daily price watchlist («پایش روزانه») and the seller's own catalogue («کالاهای من»).
//!
//! A watched product is re-analysed once a day (the scheduler in background.rs, or «بررسی همه»):
//! every run is a normal market snapshot, so the price history and charts grow by themselves, and
//! the change since the previous run becomes an alert when it passes the item's threshold.
//! Catalogue items are searched with a short query and then identified among the results by
//! `identity::match_item`, so only listings of exactly that product set its market price.
use crate::db::{err, Db, Res};
use crate::identity;
use crate::market::{self, History, MarketReq, Offer};
use crate::scanner::{self, Emit, Settings, Summary};
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio_util::sync::CancellationToken;

// ---------------- price swings (pure) ----------------

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Swing {
    pub points: usize,
    /// Median change since the previous snapshot, %.
    pub change_pct: f64,
    pub change7: Option<f64>,
    pub change30: Option<f64>,
    pub low: i64,
    pub high: i64,
    /// Coefficient of variation of the daily medians, %: how jumpy the price is.
    pub volatility: f64,
    /// "up" | "down" | "flat": the fitted trend over the whole history.
    pub direction: String,
}

/// Days since 1970-01-01 for "YYYY-MM-DD" (inverse of `market::iso_utc`'s civil-from-days).
fn day_no(d: &str) -> Option<i64> {
    let mut it = d.get(..10)?.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, dd) = (it.next()??, it.next()??, it.next()??);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn change(from: i64, to: i64) -> f64 {
    if from <= 0 {
        0.0
    } else {
        ((to - from) as f64 * 1000.0 / from as f64).round() / 10.0
    }
}

pub fn swing(h: &[History]) -> Swing {
    let pts: Vec<(i64, i64, i64)> = h.iter().filter(|p| p.1 > 0).filter_map(|p| Some((day_no(&p.0)?, p.1, p.2))).collect();
    let Some(&(last_d, last, _)) = pts.last() else {
        return Swing {
            direction: "flat".into(),
            ..Default::default()
        };
    };
    let back = |days: i64| pts.iter().rev().find(|p| p.0 <= last_d - days).map(|p| change(p.1, last));
    let n = pts.len() as f64;
    let mean = pts.iter().map(|p| p.1 as f64).sum::<f64>() / n;
    let sd = (pts.iter().map(|p| (p.1 as f64 - mean).powi(2)).sum::<f64>() / n).sqrt();
    // least squares slope (Toman per day) → change over the span, as % of the mean
    let xm = pts.iter().map(|p| p.0 as f64).sum::<f64>() / n;
    let sxx: f64 = pts.iter().map(|p| (p.0 as f64 - xm).powi(2)).sum();
    let slope = if sxx > 0.0 {
        pts.iter().map(|p| (p.0 as f64 - xm) * (p.1 as f64 - mean)).sum::<f64>() / sxx
    } else {
        0.0
    };
    let span = (last_d - pts[0].0) as f64;
    let total = if mean > 0.0 { slope * span / mean * 100.0 } else { 0.0 };
    Swing {
        points: pts.len(),
        change_pct: if pts.len() >= 2 { change(pts[pts.len() - 2].1, last) } else { 0.0 },
        change7: back(7),
        change30: back(30),
        low: pts.iter().map(|p| if p.2 > 0 { p.2 } else { p.1 }).min().unwrap_or(0),
        high: pts.iter().map(|p| p.1).max().unwrap_or(0),
        volatility: if mean > 0.0 { (sd / mean * 1000.0).round() / 10.0 } else { 0.0 },
        direction: if total > 3.0 {
            "up"
        } else if total < -3.0 {
            "down"
        } else {
            "flat"
        }
        .into(),
    }
}

/// What changed enough to tell the seller (empty = nothing).
pub fn alert(prev_median: i64, prev_min: i64, median: i64, min: i64, own: i64, pct: f64, per_kg: bool) -> String {
    let pct = if pct > 0.0 { pct } else { 5.0 };
    let fa = crate::logic::fa_num;
    let mut out = vec![];
    if prev_median > 0 && median > 0 {
        let c = change(prev_median, median);
        if c.abs() >= pct {
            out.push(format!(
                "میانه قیمت {}٪ {} (از {} به {} تومان)",
                fa(c.abs()),
                if c > 0.0 { "بالا رفت" } else { "پایین آمد" },
                fa(prev_median as f64),
                fa(median as f64)
            ));
        }
    }
    if prev_min > 0 && min > 0 && (min as f64) < prev_min as f64 * (1.0 - pct / 100.0) {
        out.push(format!("کمترین قیمت بازار به {} تومان رسید (قبلاً {})", fa(min as f64), fa(prev_min as f64)));
    }
    // the seller's own price is per item; compare only when the market is too
    if own > 0 && median > 0 && !per_kg {
        if own as f64 > median as f64 * (1.0 + pct / 100.0) {
            out.push(format!("قیمت شما {}٪ بالاتر از میانه بازار است", fa(change(median, own))));
        } else if min > 0 && own < min {
            out.push("قیمت شما از ارزان‌ترین رقیب هم کمتر است؛ جای افزایش قیمت دارید".into());
        }
    }
    out.join(" · ")
}

// ---------------- watchlist ----------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Watch {
    pub id: i64,
    pub name: String,
    pub req: MarketReq,
    pub own_price: i64,
    pub alert_pct: f64,
    pub enabled: bool,
    pub last_run: String,
    pub last_id: i64,
    pub median: i64,
    pub min: i64,
    pub prev_median: i64,
    pub prev_min: i64,
    pub basis: String,
    pub alert: String,
    pub error: String,
    pub created_at: String,
    // view only
    pub history: Vec<History>,
    pub swing: Swing,
}

const WCOLS: &str = "id,name,req,own_price,alert_pct,enabled,last_run,last_id,median,min,prev_median,prev_min,basis,alert,error,created_at";

fn to_watch(r: &Row) -> rusqlite::Result<Watch> {
    Ok(Watch {
        id: r.get(0)?,
        name: r.get(1)?,
        req: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default(),
        own_price: r.get(3)?,
        alert_pct: r.get(4)?,
        enabled: r.get::<_, i64>(5)? != 0,
        last_run: r.get(6)?,
        last_id: r.get(7)?,
        median: r.get(8)?,
        min: r.get(9)?,
        prev_median: r.get(10)?,
        prev_min: r.get(11)?,
        basis: r.get(12)?,
        alert: r.get(13)?,
        error: r.get(14)?,
        created_at: r.get(15)?,
        ..Default::default()
    })
}

fn rows(db: &Db, where_: &str) -> Res<Vec<Watch>> {
    let c = db.c();
    let mut st = c
        .prepare(&format!("SELECT {WCOLS} FROM watch {where_} ORDER BY enabled DESC, id DESC"))
        .map_err(err)?;
    let v = st.query_map([], to_watch).map_err(err)?.collect::<Result<Vec<_>, _>>().map_err(err)?;
    Ok(v)
}

/// Every watched product, with its price history and swings.
pub fn list(db: &Db) -> Res<Vec<Watch>> {
    let mut v = rows(db, "")?;
    for w in &mut v {
        w.history = db.market_history(&w.req.query, &w.req.brand)?;
        w.swing = swing(&w.history);
    }
    Ok(v)
}

pub fn save(db: &Db, w: &Watch) -> Res<i64> {
    let q = w.req.query.trim();
    if q.chars().count() < 2 {
        return Err("نام کالا را بنویسید.".into());
    }
    let name = if w.name.trim().is_empty() {
        [q, w.req.brand.trim()]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" — ")
    } else {
        w.name.trim().to_string()
    };
    if name.chars().count() > 80 {
        return Err("نام پایش حداکثر ۸۰ حرف است.".into());
    }
    if !(0.0..=100.0).contains(&w.alert_pct) || w.own_price < 0 {
        return Err("آستانه هشدار یا قیمت شما نامعتبر است.".into());
    }
    let req = serde_json::to_string(&MarketReq {
        query: q.into(),
        ..w.req.clone()
    })
    .map_err(err)?;
    let pct = if w.alert_pct > 0.0 { w.alert_pct } else { 5.0 };
    let c = db.c();
    if w.id == 0 {
        // the same product and brand is watched once
        let key = market::words(q).join(" ");
        let dup: Option<i64> = {
            let mut st = c.prepare("SELECT id, req FROM watch").map_err(err)?;
            let all: Vec<(i64, String)> = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(err)?
                .collect::<Result<_, _>>()
                .map_err(err)?;
            all.into_iter()
                .find(|(_, r)| {
                    serde_json::from_str::<MarketReq>(r)
                        .is_ok_and(|r| market::words(&r.query).join(" ") == key && market::brand_key(&r.brand) == market::brand_key(&w.req.brand))
                })
                .map(|x| x.0)
        };
        if let Some(id) = dup {
            c.execute(
                "UPDATE watch SET own_price=?1, alert_pct=?2, enabled=1 WHERE id=?3",
                params![w.own_price, pct, id],
            )
            .map_err(err)?;
            return Ok(id);
        }
        c.execute(
            "INSERT INTO watch(name,req,own_price,alert_pct) VALUES(?1,?2,?3,?4)",
            params![name, req, w.own_price, pct],
        )
        .map_err(err)?;
        Ok(c.last_insert_rowid())
    } else {
        c.execute(
            "UPDATE watch SET name=?1,req=?2,own_price=?3,alert_pct=?4,enabled=?5 WHERE id=?6",
            params![name, req, w.own_price, pct, w.enabled as i64, w.id],
        )
        .map_err(err)?;
        Ok(w.id)
    }
}

pub fn delete(db: &Db, id: i64) -> Res<()> {
    db.c().execute("DELETE FROM watch WHERE id=?1", [id]).map(|_| ()).map_err(err)
}

/// The daily check time, "HH:MM" (PC local time).
pub fn time(db: &Db) -> String {
    db.kv_get("watch_at").filter(|t| t.len() == 5).unwrap_or_else(|| "09:00".into())
}

pub fn set_time(db: &Db, at: &str) -> Res<()> {
    let ok = at.len() == 5 && at.as_bytes()[2] == b':' && at[..2].parse::<u32>().is_ok_and(|h| h < 24) && at[3..].parse::<u32>().is_ok_and(|m| m < 60);
    if !ok {
        return Err("ساعت را به شکل ۲۴ساعته (مثلاً 09:30) وارد کنید.".into());
    }
    db.kv_set("watch_at", at)
}

/// Enabled items not yet checked today, once today's check time has come.
pub fn due(db: &Db) -> Vec<i64> {
    let at = time(db);
    db.c()
        .prepare(
            "SELECT id FROM watch WHERE enabled=1 AND (last_run='' OR date(last_run) < date('now','localtime'))
               AND strftime('%H:%M','now','localtime') >= ?1 ORDER BY id",
        )
        .and_then(|mut st| st.query_map([at], |r| r.get(0))?.collect())
        .unwrap_or_default()
}

/// Re-analyses the given items (all enabled ones when `ids` is empty), one after another.
pub async fn run(db: &Db, s: &Settings, profile: &Path, ids: &[i64], token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    let items: Vec<Watch> = rows(db, "WHERE enabled=1")?
        .into_iter()
        .filter(|w| ids.is_empty() || ids.contains(&w.id))
        .collect();
    if items.is_empty() {
        return Err("کالایی برای پایش نیست؛ از «تحلیل محصول» یک کالا را به پایش اضافه کنید.".into());
    }
    let (mut ok, mut alerts) = (0, 0);
    for (i, w) in items.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }
        scanner::status(emit, format!("پایش {} از {}: «{}»", i + 1, items.len(), w.name));
        let res = market::search(db, s, profile, &w.req, token, emit).await;
        let c = db.c();
        let done = match res {
            Ok(r) => {
                ok += 1;
                let per_kg = r.stats.basis == "per_kg";
                let a = alert(w.median, w.min, r.stats.median, r.stats.min, w.own_price, w.alert_pct, per_kg);
                if !a.is_empty() {
                    alerts += 1;
                    scanner::log(emit, "warning", format!("{}: {a}", w.name));
                }
                c.execute(
                    "UPDATE watch SET last_run=datetime('now','localtime'), last_id=?1, prev_median=median, prev_min=min, median=?2, min=?3, basis=?4, alert=?5, error='' WHERE id=?6",
                    params![r.id, r.stats.median, r.stats.min, r.stats.basis, a, w.id],
                )
                .map(|_| a)
            }
            Err(e) => {
                scanner::log(emit, "warning", format!("{}: {e}", w.name));
                c.execute("UPDATE watch SET last_run=datetime('now','localtime'), error=?1 WHERE id=?2", params![e, w.id])
                    .map(|_| String::new())
            }
        };
        drop(c);
        if let Ok(a) = done.map_err(err) {
            if !a.is_empty() {
                db.log("watch", &format!("پایش «{}»: {a}", w.name));
            }
        }
        emit(scanner::ScanEvent::Progress {
            keyword: w.name.clone(),
            collected: ok,
            checked: (i + 1) as i64,
            found: alerts,
            target: items.len() as i64,
        });
    }
    let cancelled = token.is_cancelled();
    let message = format!(
        "پایش قیمت: {} از {} کالا بررسی شد{}.",
        crate::logic::fa_num(ok as f64),
        crate::logic::fa_num(items.len() as f64),
        if alerts > 0 {
            format!("؛ {} هشدار", crate::logic::fa_num(alerts as f64))
        } else {
            String::new()
        }
    );
    Ok(Summary {
        checked: items.len() as i64,
        found: alerts,
        cancelled,
        error: ok == 0,
        message,
    })
}

// ---------------- the seller's catalogue ----------------

/// One market listing identified as the catalogue item (shown so the seller can verify it).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Match {
    pub title: String,
    pub platform: String,
    pub seller: String,
    pub price: i64,
    pub url: String,
    pub image: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Item {
    pub id: i64,
    pub name: String,
    pub sku: String,
    pub brand: String,
    pub own_price: i64,
    pub cost: i64,
    pub matched: i64,
    pub confidence: String, // high | medium | none | "" (not checked yet)
    pub min: i64,
    pub median: i64,
    pub max: i64,
    pub srp: i64,
    /// Share of matched listings cheaper than the seller's price, % (−1 = no price / no market).
    pub position: f64,
    pub matches: Vec<Match>,
    pub run_id: i64,
    pub note: String,
    pub checked_at: String,
    /// View only: the search text used for this item (also what a watch of it searches).
    pub query: String,
}

/// The market picture of one item from the listings identified as that item (pure).
pub fn evaluate(item: &Item, offers: &[&Offer], margin: f64) -> Item {
    let mut prices: Vec<i64> = offers.iter().map(|o| o.price).filter(|p| *p > 0).collect();
    prices.sort_unstable();
    let mut out = Item {
        matched: prices.len() as i64,
        position: -1.0,
        ..item.clone()
    };
    if prices.is_empty() {
        out.note = "این کالا در نتایج بازار پیدا نشد؛ نام را کامل‌تر (با برند، مدل یا وزن) بنویسید.".into();
        return out;
    }
    // stale or wrong prices: the IQR rule, as in the market analysis
    let (q1, q3) = (market::pct(&prices, 0.25), market::pct(&prices, 0.75));
    let iqr = (q3 - q1) as f64;
    let valid: Vec<i64> = prices
        .iter()
        .copied()
        .filter(|p| prices.len() < 4 || (*p as f64 >= q1 as f64 - 1.5 * iqr && *p as f64 <= q3 as f64 + 1.5 * iqr))
        .collect();
    let (p25, p75) = (market::pct(&valid, 0.25), market::pct(&valid, 0.75));
    out.min = valid[0];
    out.max = valid[valid.len() - 1];
    out.median = market::pct(&valid, 0.5);
    let m = if margin > 0.0 { margin } else { 25.0 };
    out.srp = if item.cost > 0 {
        market::shelf((item.cost as f64 * (1.0 + m / 100.0)).clamp(p25 as f64, p75.max(p25) as f64))
    } else {
        market::shelf(out.median as f64)
    };
    let mut notes = vec![];
    if item.own_price > 0 {
        out.position = (valid.iter().filter(|p| **p < item.own_price).count() as f64 * 1000.0 / valid.len() as f64).round() / 10.0;
        notes.push(match out.position {
            p if p < 25.0 => "ارزان‌تر از بیشتر بازار؛ جای افزایش قیمت دارید.",
            p if p <= 75.0 => "در محدوده رقابتی بازار.",
            _ => "گران‌تر از بیشتر عرضه‌ها؛ احتمال فروش کمتر است.",
        });
    }
    if item.cost > 0 && p75 > 0 && (item.cost as f64) * (1.0 + m / 100.0) > p75 as f64 {
        notes.push("با حاشیه سود هدف، قیمت از ۷۵٪ بازار بالاتر می‌رود؛ قیمت پیشنهادی روی سقف رقابتی گذاشته شد.");
    }
    if item.cost > 0 && out.min > 0 && item.cost >= out.min {
        notes.push("بهای خرید شما از ارزان‌ترین قیمت بازار بیشتر است؛ تأمین‌کننده ارزان‌تری لازم است.");
    }
    out.note = notes.join(" ");
    out
}

const ICOLS: &str = "id,name,sku,brand,own_price,cost,matched,confidence,min,median,max,srp,position,matches,run_id,note,checked_at";

fn to_item(r: &Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        name: r.get(1)?,
        sku: r.get(2)?,
        brand: r.get(3)?,
        own_price: r.get(4)?,
        cost: r.get(5)?,
        matched: r.get(6)?,
        confidence: r.get(7)?,
        min: r.get(8)?,
        median: r.get(9)?,
        max: r.get(10)?,
        srp: r.get(11)?,
        position: r.get(12)?,
        matches: serde_json::from_str(&r.get::<_, String>(13)?).unwrap_or_default(),
        run_id: r.get(14)?,
        note: r.get(15)?,
        checked_at: r.get(16)?,
        query: String::new(),
    })
}

pub fn items(db: &Db) -> Res<Vec<Item>> {
    let c = db.c();
    let mut st = c.prepare(&format!("SELECT {ICOLS} FROM catalog ORDER BY id")).map_err(err)?;
    let mut v = st.query_map([], to_item).map_err(err)?.collect::<Result<Vec<Item>, _>>().map_err(err)?;
    v.iter_mut().for_each(|i| i.query = identity::search_query(&i.name, &i.brand));
    Ok(v)
}

pub fn delete_items(db: &Db, ids: &[i64]) -> Res<()> {
    let c = db.c();
    for id in ids {
        c.execute("DELETE FROM catalog WHERE id=?1", [id]).map_err(err)?;
    }
    Ok(())
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CatalogImport {
    pub added: usize,
    pub updated: usize,
    pub skipped: usize,
    pub columns: Vec<String>, // "name", "price" … in file order, "" = ignored
}

/// Which catalogue field a column header is (most specific first).
pub fn column(header: &str) -> &'static str {
    let h = market::words(header).join(" ");
    let any = |ks: &[&str]| ks.iter().any(|k| h.contains(k));
    if any(&["قیمت خرید", "بهای", "هزینه", "تمام شده", "cost"]) {
        "cost"
    } else if any(&["برند", "مارک", "brand"]) {
        "brand"
    } else if any(&["قیمت", "price", "فروش"]) {
        "price"
    } else if any(&["کد", "sku", "شناسه", "بارکد", "barcode"]) {
        "sku"
    } else if any(&["نام", "کالا", "محصول", "عنوان", "شرح", "title", "name", "product"]) {
        "name"
    } else {
        ""
    }
}

fn money(s: &str) -> i64 {
    crate::logic::ascii_digits(s)
        .split('.')
        .next()
        .unwrap_or("")
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// Rows of an Excel / CSV file → (name, sku, brand, price, cost). The first row with a name column
/// is the header; without one, column 1 is the name and column 2 the price.
pub fn parse_catalog(rows: &[Vec<String>]) -> (Vec<(String, String, String, i64, i64)>, Vec<String>) {
    let head = rows.iter().take(10).position(|r| r.iter().any(|h| column(h) == "name"));
    let (cols, start, rial): (Vec<&str>, usize, Vec<bool>) = match head {
        Some(i) => (
            rows[i].iter().map(|h| column(h)).collect(),
            i + 1,
            rows[i].iter().map(|h| h.contains("ریال") || h.to_lowercase().contains("rial")).collect(),
        ),
        None => (vec!["name", "price"], 0, vec![false, false]),
    };
    let mut out = vec![];
    for r in rows.iter().skip(start).take(2000) {
        let get = |f: &str| cols.iter().position(|c| *c == f).and_then(|i| r.get(i).map(|v| (i, v.trim().to_string())));
        let price = |f: &str| {
            get(f)
                .map(|(i, v)| money(&v) / if rial.get(i).copied().unwrap_or(false) { 10 } else { 1 })
                .unwrap_or(0)
        };
        let name = get("name").map(|x| x.1).unwrap_or_default();
        if name.chars().count() < 2 || name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        out.push((
            name.chars().take(160).collect(),
            get("sku").map(|x| x.1).unwrap_or_default(),
            get("brand").map(|x| x.1).unwrap_or_default(),
            price("price"),
            price("cost"),
        ));
    }
    (out, cols.iter().map(|c| c.to_string()).collect())
}

pub fn import(db: &Db, file_name: &str, data: Vec<u8>) -> Res<CatalogImport> {
    let rows = crate::export::read_rows(file_name, data)?;
    let (items, columns) = parse_catalog(&rows);
    let mut rep = CatalogImport {
        columns,
        skipped: rows.len().saturating_sub(items.len() + 1),
        ..Default::default()
    };
    if items.is_empty() {
        return Err("کالایی در فایل پیدا نشد. ستون «نام کالا» لازم است؛ «قیمت فروش»، «قیمت خرید»، «برند» و «کد کالا» اختیاری‌اند.".into());
    }
    let mut c = db.c();
    let tx = c.transaction().map_err(err)?;
    for (name, sku, brand, price, cost) in items {
        let n = tx
            .execute(
                "UPDATE catalog SET brand=?3, own_price=?4, cost=?5 WHERE name=?1 AND sku=?2",
                params![name, sku, brand, price, cost],
            )
            .map_err(err)?;
        if n > 0 {
            rep.updated += 1;
        } else {
            tx.execute(
                "INSERT INTO catalog(name,sku,brand,own_price,cost) VALUES(?1,?2,?3,?4,?5)",
                params![name, sku, brand, price, cost],
            )
            .map_err(err)?;
            rep.added += 1;
        }
    }
    tx.commit().map_err(err)?;
    Ok(rep)
}

/// Looks every given item up in the market (all items when `ids` is empty) on `platforms`
/// (the four shops when empty).
#[allow(clippy::too_many_arguments)]
pub async fn check(
    db: &Db,
    s: &Settings,
    profile: &Path,
    ids: &[i64],
    platforms: &[String],
    margin: f64,
    token: &CancellationToken,
    emit: Emit<'_>,
) -> Res<Summary> {
    let list: Vec<Item> = items(db)?.into_iter().filter(|i| ids.is_empty() || ids.contains(&i.id)).collect();
    if list.is_empty() {
        return Err("کالایی انتخاب نشده است.".into());
    }
    let mut found = 0;
    for (i, it) in list.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }
        scanner::status(emit, format!("کالای {} از {}: «{}»", i + 1, list.len(), it.name));
        let req = MarketReq {
            query: identity::search_query(&it.name, &it.brand),
            platforms: if platforms.is_empty() {
                vec!["torob".into(), "digikala".into(), "basalam".into(), "emalls".into()]
            } else {
                platforms.to_vec()
            },
            exact: false,
            limit: 60,
            target_margin: margin,
            ..Default::default()
        };
        let out = match market::search(db, s, profile, &req, token, emit).await {
            Ok(r) => {
                let (idx, conf) = identity::match_item(&it.name, &it.brand, &r.offers);
                let picked: Vec<&Offer> = idx.iter().map(|&j| &r.offers[j]).collect();
                let mut e = evaluate(it, &picked, margin);
                e.confidence = conf.into();
                e.run_id = r.id;
                e.matches = picked
                    .iter()
                    .take(15)
                    .map(|o| Match {
                        title: o.title.clone(),
                        platform: o.platform.clone(),
                        seller: o.seller.clone(),
                        price: o.price,
                        url: o.url.clone(),
                        image: o.image.clone(),
                    })
                    .collect();
                if conf != "none" {
                    found += 1;
                }
                scanner::log(
                    emit,
                    if conf == "none" { "warning" } else { "success" },
                    format!("{}: {} پیشنهاد همین کالا", it.name, e.matched),
                );
                e
            }
            Err(e) => {
                scanner::log(emit, "warning", format!("{}: {e}", it.name));
                Item {
                    matched: 0,
                    confidence: "none".into(),
                    position: -1.0,
                    matches: vec![],
                    note: e,
                    min: 0,
                    median: 0,
                    max: 0,
                    srp: 0,
                    ..it.clone()
                }
            }
        };
        db.c()
            .execute(
                "UPDATE catalog SET matched=?1, confidence=?2, min=?3, median=?4, max=?5, srp=?6, position=?7, matches=?8, run_id=?9, note=?10, checked_at=datetime('now','localtime') WHERE id=?11",
                params![out.matched, out.confidence, out.min, out.median, out.max, out.srp, out.position, serde_json::to_string(&out.matches).map_err(err)?, out.run_id, out.note, it.id],
            )
            .map_err(err)?;
        emit(scanner::ScanEvent::Progress {
            keyword: it.name.clone(),
            collected: found,
            checked: (i + 1) as i64,
            found,
            target: list.len() as i64,
        });
    }
    let fa = crate::logic::fa_num;
    Ok(Summary {
        checked: list.len() as i64,
        found,
        cancelled: token.is_cancelled(),
        error: false,
        message: format!("{} از {} کالا در بازار شناسایی شد.", fa(found as f64), fa(list.len() as f64)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swings() {
        let h: Vec<History> = vec![
            ("2026-09-01".into(), 100_000, 90_000),
            ("2026-09-20".into(), 104_000, 95_000),
            ("2026-10-08".into(), 110_000, 99_000),
            ("2026-10-15".into(), 121_000, 100_000),
        ];
        let s = swing(&h);
        assert_eq!(s.points, 4);
        assert_eq!(s.change_pct, 10.0);
        assert_eq!(s.change7, Some(10.0), "vs 7 days before");
        assert_eq!(s.change30, Some(21.0), "vs the last point 30+ days before (09-01)");
        assert_eq!((s.low, s.high, s.direction.as_str()), (90_000, 121_000, "up"));
        assert!(s.volatility > 5.0 && s.volatility < 10.0, "{}", s.volatility);
        assert_eq!(swing(&[]).direction, "flat");
        assert_eq!(day_no("1970-01-02"), Some(1));
        assert_eq!(market::iso_utc(day_no("2026-10-15").unwrap() * 86_400)[..10], *"2026-10-15");
    }

    #[test]
    fn alerts() {
        assert_eq!(alert(0, 0, 100, 90, 0, 5.0, false), "", "first run");
        assert!(alert(100_000, 90_000, 108_000, 90_000, 0, 5.0, false).contains("بالا رفت"));
        assert!(alert(100_000, 90_000, 101_000, 80_000, 0, 5.0, false).contains("کمترین قیمت"));
        assert!(alert(100_000, 90_000, 100_000, 90_000, 120_000, 5.0, false).contains("بالاتر از میانه"));
        assert_eq!(
            alert(100_000, 90_000, 100_000, 90_000, 120_000, 5.0, true),
            "",
            "per-kg market vs per-item price"
        );
    }

    #[test]
    fn catalogue() {
        let rows: Vec<Vec<String>> = [
            vec!["ردیف", "کد کالا", "نام کالا", "برند", "قیمت فروش (ریال)", "قیمت خرید"],
            vec!["1", "A-1", "عسل گون خوانسار ۵۰۰ گرمی", "خوانسار", "3,500,000", "۲۸۰٬۰۰۰"],
            vec!["2", "", "", "", "", ""],
        ]
        .iter()
        .map(|r| r.iter().map(|s| s.to_string()).collect())
        .collect();
        let (items, cols) = parse_catalog(&rows);
        assert_eq!(cols, vec!["", "sku", "name", "brand", "price", "cost"]);
        assert_eq!(
            items,
            vec![("عسل گون خوانسار ۵۰۰ گرمی".into(), "A-1".into(), "خوانسار".into(), 350_000, 280_000)]
        );
        // no header: name, price
        let (items, _) = parse_catalog(&[vec!["کیسه فریزر آغوش".into(), "85000".into()]]);
        assert_eq!(items[0].3, 85_000);

        let offers: Vec<Offer> = [300_000, 310_000, 320_000, 340_000, 900_000]
            .iter()
            .map(|p| Offer {
                price: *p,
                ..Default::default()
            })
            .collect();
        let refs: Vec<&Offer> = offers.iter().collect();
        let it = Item {
            name: "x".into(),
            own_price: 335_000,
            cost: 250_000,
            ..Default::default()
        };
        let e = evaluate(&it, &refs, 25.0);
        assert_eq!(
            (e.matched, e.min, e.max, e.median),
            (5, 300_000, 340_000, 315_000),
            "the 900k price is left out"
        );
        assert_eq!(e.position, 75.0);
        assert!(e.srp >= 310_000 && e.srp <= 330_000, "{}", e.srp);
        assert_eq!(evaluate(&it, &[], 25.0).matched, 0);
    }
}
