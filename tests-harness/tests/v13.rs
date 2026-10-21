//! Schema v13 and what lives on it: the upgrade from v12, the B2B timeline, report pruning, buy-box
//! tracking of the catalogue, FX rates and the catalogue upsert.
use harness::db::{Db, Filter, SCHEMA_VERSION};
use harness::{export, fx, market, watch};
use rusqlite::Connection;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("hs-v13-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn version(p: &std::path::Path) -> i64 {
    Connection::open(p).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
}

fn one_place(db: &Db) -> i64 {
    let csv = "نام کسب‌وکار,موبایل\nکافه لمیز,09123456789\n";
    export::import(db, 1, "a.csv", csv.as_bytes().to_vec()).unwrap();
    db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap().0[0].id
}

#[test]
fn upgrade_from_v12_keeps_the_data_and_adds_v13() {
    let dir = tmp("up");
    let p = dir.join("t.db");
    {
        let db = Db::open(&p).unwrap();
        one_place(&db);
        watch::save(&db, &{ let mut w = watch::Watch::default(); w.req.query = "عسل".into(); w }).unwrap();
    }
    assert_eq!(version(&p), SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, 13);
    // put the file back the way a v12 database looks
    {
        let c = Connection::open(&p).unwrap();
        c.execute_batch(
            "DROP INDEX places_follow; DROP TABLE place_logs; DROP TABLE fx_rates;
             ALTER TABLE places DROP COLUMN follow_up_at;
             ALTER TABLE watch DROP COLUMN my_seller_name; ALTER TABLE watch DROP COLUMN my_product_url;
             ALTER TABLE watch DROP COLUMN min_margin_pct; ALTER TABLE watch DROP COLUMN alert_channel;
             ALTER TABLE watch DROP COLUMN cost; ALTER TABLE watch DROP COLUMN buybox;
             ALTER TABLE catalog DROP COLUMN my_seller_name; ALTER TABLE catalog DROP COLUMN my_product_url;
             ALTER TABLE catalog DROP COLUMN min_margin_pct; ALTER TABLE catalog DROP COLUMN alert_channel;
             ALTER TABLE catalog DROP COLUMN buybox; PRAGMA user_version=12;",
        )
        .unwrap();
    }
    let db = Db::open(&p).unwrap();
    assert_eq!(version(&p), 13);
    assert_eq!(db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap().1, 1, "rows survive");
    assert_eq!(watch::list(&db).unwrap().len(), 1);
    let id = one_place_id(&db);
    db.place_log_add(id, "call", "تماس اول", Some("2026-10-22 10:00")).unwrap();
    // a second open is a no-op
    drop(db);
    Db::open(&p).unwrap();
    assert_eq!(version(&p), 13);
}

fn one_place_id(db: &Db) -> i64 {
    db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap().0[0].id
}

#[test]
fn crm_timeline_and_follow_ups() {
    let db = Db::open(&tmp("crm").join("t.db")).unwrap();
    let id = one_place(&db);
    assert!(db.place_log_add(id, "x", "n", None).is_err(), "unknown kind");
    assert!(db.place_log_add(id, "call", "  ", None).is_err(), "empty note");
    assert!(db.place_log_add(id, "call", "n", Some("فردا")).is_err(), "bad time");
    assert!(db.place_log_add(999, "call", "n", None).is_err(), "no such business");
    db.place_log_add(id, "call", "اولین تماس", Some("2000-01-01 09:00")).unwrap();
    let second = db.place_log_add(id, "meeting", "جلسه حضوری", None).unwrap();
    let logs = db.place_logs(id).unwrap();
    assert_eq!(logs.iter().map(|l| l.kind.as_str()).collect::<Vec<_>>(), ["meeting", "call"], "newest first");
    let due = db.follow_ups_due(10).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!((due[0].last.as_str(), due[0].phone.as_str()), ("جلسه حضوری", "09123456789"));
    assert!(due[0].overdue_days > 0);
    // told once only
    assert_eq!(db.follow_ups_to_notify().unwrap().len(), 1);
    assert_eq!(db.follow_ups_to_notify().unwrap().len(), 0);
    // a future time is not due; clearing removes it
    db.follow_up_set(id, "2099-01-01 09:00").unwrap();
    assert!(db.follow_ups_due(10).unwrap().is_empty());
    db.follow_up_set(id, "").unwrap();
    db.place_log_delete(second).unwrap();
    assert_eq!(db.place_logs(id).unwrap().len(), 1);
}

#[test]
fn old_reports_are_slimmed_and_the_trend_survives() {
    let dir = tmp("prune");
    let p = dir.join("t.db");
    let db = Db::open(&p).unwrap();
    let mut rep = market::Report::default();
    rep.query = "عسل".into();
    rep.srp = 500_000;
    for i in 0..300 {
        rep.offers.push(market::Offer { platform: "digikala".into(), title: format!("عسل شماره {i} با توضیح بلند برای سنگین شدن گزارش"), price: 400_000 + i * 1000, ..Default::default() });
    }
    let json = serde_json::to_string(&rep).unwrap();
    assert!(json.len() > 12_000);
    {
        let c = Connection::open(&p).unwrap();
        for (q, age) in [("old", "-60 days"), ("fresh", "-2 days")] {
            c.execute(
                "INSERT INTO market_runs(query,qkey,offers,median,min,report,created_at) VALUES(?1,?1,300,450000,400000,?2,datetime('now','localtime',?3))",
                rusqlite::params![q, json, age],
            )
            .unwrap();
        }
    }
    assert_eq!(db.prune_old_reports().unwrap(), 1);
    assert_eq!(db.prune_old_reports().unwrap(), 0, "idempotent");
    let c = Connection::open(&p).unwrap();
    let get = |q: &str| -> (market::Report, i64, i64) {
        c.query_row("SELECT report, offers, median FROM market_runs WHERE query=?1", [q], |r| Ok((serde_json::from_str(&r.get::<_, String>(0)?).unwrap(), r.get(1)?, r.get(2)?))).unwrap()
    };
    let (old, n, med) = get("old");
    assert_eq!((old.offers.len(), old.offers_total), (market::KEEP_OLD_OFFERS, 300));
    assert_eq!((n, med, old.srp), (300, 450_000, 500_000), "the numbers the trend rests on stay");
    assert_eq!(get("fresh").0.offers.len(), 300);
}

#[test]
fn catalogue_tracking_and_upsert() {
    let db = Db::open(&tmp("cat").join("t.db")).unwrap();
    let rep = watch::upsert(&db, vec![("عسل گون".into(), "H1".into(), "هامیستا".into(), 450_000, 300_000)]).unwrap();
    assert_eq!((rep.added, rep.updated), (1, 0));
    let rep = watch::upsert(&db, vec![("عسل گون".into(), "H1".into(), "هامیستا".into(), 470_000, 310_000)]).unwrap();
    assert_eq!((rep.added, rep.updated), (0, 1));
    let id = watch::items(&db).unwrap()[0].id;
    assert!(watch::track_item(&db, id, "فروشگاه هامیستا", "not a url", 12.0, "telegram").is_err());
    watch::track_item(&db, id, "فروشگاه هامیستا", "https://www.digikala.com/product/dkp-1/", 12.0, "telegram").unwrap();
    let it = &watch::items(&db).unwrap()[0];
    assert_eq!((it.own_price, it.cost, it.min_margin_pct, it.alert_channel.as_str(), it.my_seller_name.as_str()), (470_000, 310_000, 12.0, "telegram", "فروشگاه هامیستا"));
}

#[test]
fn fx_rates_roundtrip_and_validation() {
    let db = Db::open(&tmp("fx").join("t.db")).unwrap();
    fx::set(&db, &fx::Rate { date: "2026-10-20".into(), usd_toman: 100_000, gold18_toman: 7_000_000 }).unwrap();
    fx::set(&db, &fx::Rate { date: "2026-10-21".into(), usd_toman: 105_000, gold18_toman: 0 }).unwrap();
    fx::set(&db, &fx::Rate { date: "2026-10-21".into(), usd_toman: 106_000, gold18_toman: 0 }).unwrap(); // same day replaces
    assert!(fx::set(&db, &fx::Rate { date: "21/10".into(), usd_toman: 1, gold18_toman: 0 }).is_err());
    assert!(fx::set(&db, &fx::Rate { date: "2026-10-22".into(), usd_toman: 0, gold18_toman: 0 }).is_err());
    let v = fx::list(&db, 10).unwrap();
    assert_eq!(v.iter().map(|r| (r.date.as_str(), r.usd_toman)).collect::<Vec<_>>(), [("2026-10-21", 106_000), ("2026-10-20", 100_000)]);
}
