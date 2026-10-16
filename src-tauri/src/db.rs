//! SQLite store (WAL). Schema version lives in `PRAGMA user_version`; `migrate()` upgrades in place.
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

pub type Res<T> = Result<T, String>;
pub fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

pub const SCHEMA_VERSION: i64 = 10;

// ---------------- types ----------------

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Contact {
    pub id: i64,
    pub kind: String, // mobile | landline | email | instagram | telegram | whatsapp | eitaa | bale
    pub value: String,
    pub label: String,
    pub origin: String, // maps | website | manual | import
    pub delivery: String, // last official send: sent | failed | invalid | no_account
    /// Mobile only: messengers seen for this number, "whatsapp:1,telegram:0" (1 = has an account, 0 = none).
    pub apps: String,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Place {
    pub id: i64,
    pub list_id: i64,
    pub place_key: String,
    pub name: String,
    pub contact_person: String,
    pub contact_role: String,
    pub category: String,
    pub address: String,
    pub website: String,
    pub rating: Option<f64>,
    pub reviews: Option<i64>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub maps_url: String,
    pub keyword: String,
    pub province: String,
    pub city: String,
    pub status: String,
    pub note: String,
    pub tags: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
    /// Picture of the business from its public page — shown on the card only, never exported.
    pub photo: String,
    pub contacts: Vec<Contact>,
}

impl Place {
    /// First value of a contact kind (e.g. the first mobile).
    pub fn first(&self, kind: &str) -> Option<&Contact> {
        self.contacts.iter().find(|c| c.kind == kind)
    }
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Filter {
    pub list_id: i64, // 0 = every bank
    pub q: String,
    pub channel: String,
    pub keyword: String,
    pub province: String,
    pub city: String,
    pub category: String, // حوزه کاری
    pub status: String,
    pub tag: String,
    pub ids: Vec<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInfo {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub note: String,
    pub count: i64,
    pub created_at: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub total: i64,
    pub mobile: i64,
    pub landline: i64,
    pub email: i64,
    pub social: i64,
    pub website: i64,
    pub person: i64,
    pub today: i64,
    pub week: i64,
    pub prev_week: i64,
    pub contacted: i64,
    pub interested: i64,
    pub customers: i64,
    pub interested_waiting: i64, // interested but not touched for 3+ days
    pub sent_week: i64,
    pub daily: Vec<(String, i64)>, // last 30 days, oldest first
    pub top_keywords: Vec<(String, i64)>,
    pub top_cities: Vec<(String, i64)>,
    pub month: i64,      // added in the last 30 days
    pub prev_month: i64, // the 30 days before that
    pub sent_prev_week: i64,
    pub contacts: i64, // ways to reach (numbers, emails, pages) in all banks
    pub weekly: Vec<(String, i64)>, // last 12 weeks (week start date), oldest first
    pub sources: Vec<(String, i64, i64)>, // source → (all time, last 30 days)
}

#[derive(Serialize)]
pub struct BankStats {
    pub total: i64,
    pub provinces: Vec<(String, i64)>,
    pub cities: Vec<(String, String, i64)>, // (province, county, count)
    pub categories: Vec<(String, i64)>,    // line of business
    pub keywords: Vec<(String, i64)>,
    pub statuses: Vec<(String, i64)>,
    pub channels: Vec<(String, i64)>, // kind → places reachable on it (+ website, person)
}

#[derive(Serialize)]
pub struct Facets {
    pub keywords: Vec<(String, i64)>,
    pub categories: Vec<(String, i64)>,
    pub provinces: Vec<(String, i64)>,
    pub cities: Vec<(String, i64)>,
    pub tags: Vec<(String, i64)>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: i64,
    pub list_id: i64,
    pub list_name: String,
    pub keyword: String,
    pub province: String,
    pub city: String,
    pub requested: i64,
    pub checked: i64,
    pub found: i64,
    pub status: String,
    pub error: String,
    pub resume: String, // engine checkpoint (JSON) for «ادامه استخراج»
    pub started_at: String,
    pub finished_at: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub kind: String, // sms
    pub name: String,
    pub config: Value, // non-secret settings
    pub per_minute: i64,
    pub has_secret: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Template {
    pub id: i64,
    pub channel: String,
    pub name: String,
    pub subject: String,
    pub body: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Campaign {
    pub id: i64,
    pub channel: String,
    pub list_name: String,
    pub account_name: String,
    pub subject: String,
    pub message: String,
    pub total: i64,
    pub sent: i64,
    pub failed: i64,
    pub skipped: i64,
    pub status: String,
    pub error: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct Activity {
    pub at: String,
    pub kind: String,
    pub text: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub lists: i64,
    pub places: i64,
    pub contacts: i64,
    pub runs: i64,
    pub campaigns: i64,
    #[serde(skip)]
    pub accounts_json: Option<String>, // v3 kept secrets in plain JSON; caller re-encrypts them
}

// ---------------- schema ----------------

const SCHEMA: &str = "
CREATE TABLE lists(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, color TEXT NOT NULL DEFAULT '', note TEXT NOT NULL DEFAULT '',
  archived INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
CREATE TABLE places(
  id INTEGER PRIMARY KEY, list_id INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE, place_key TEXT NOT NULL,
  name TEXT NOT NULL DEFAULT '', contact_person TEXT NOT NULL DEFAULT '', contact_role TEXT NOT NULL DEFAULT '',
  category TEXT NOT NULL DEFAULT '', address TEXT NOT NULL DEFAULT '', website TEXT NOT NULL DEFAULT '',
  rating REAL, reviews INTEGER, lat REAL, lng REAL, maps_url TEXT NOT NULL DEFAULT '',
  keyword TEXT NOT NULL DEFAULT '', province TEXT NOT NULL DEFAULT '', city TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'new', note TEXT NOT NULL DEFAULT '', tags TEXT NOT NULL DEFAULT '', source TEXT NOT NULL DEFAULT 'maps',
  created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')), updated_at TEXT NOT NULL DEFAULT '',
  UNIQUE(list_id, place_key));
CREATE INDEX places_list ON places(list_id, id);
CREATE INDEX places_created ON places(created_at);
CREATE TABLE contacts(
  id INTEGER PRIMARY KEY, place_id INTEGER NOT NULL REFERENCES places(id) ON DELETE CASCADE, list_id INTEGER NOT NULL,
  kind TEXT NOT NULL, value TEXT NOT NULL, label TEXT NOT NULL DEFAULT '', origin TEXT NOT NULL DEFAULT '',
  delivery TEXT NOT NULL DEFAULT '',
  UNIQUE(list_id, kind, value));
CREATE INDEX contacts_place ON contacts(place_id);
CREATE INDEX contacts_value ON contacts(value);
CREATE TABLE runs(
  id INTEGER PRIMARY KEY, list_id INTEGER, keyword TEXT NOT NULL DEFAULT '', province TEXT NOT NULL DEFAULT '', city TEXT NOT NULL DEFAULT '',
  requested INTEGER NOT NULL DEFAULT 0, checked INTEGER NOT NULL DEFAULT 0, found INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'running', error TEXT NOT NULL DEFAULT '', resume TEXT NOT NULL DEFAULT '',
  started_at TEXT NOT NULL DEFAULT (datetime('now','localtime')), finished_at TEXT NOT NULL DEFAULT '');
CREATE TABLE accounts(
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL, config TEXT NOT NULL DEFAULT '{}',
  secret BLOB, per_minute INTEGER NOT NULL DEFAULT 60, created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
CREATE TABLE templates(
  id INTEGER PRIMARY KEY, channel TEXT NOT NULL, name TEXT NOT NULL, subject TEXT NOT NULL DEFAULT '', body TEXT NOT NULL);
CREATE TABLE campaigns(
  id INTEGER PRIMARY KEY, channel TEXT NOT NULL, list_id INTEGER, account_id TEXT NOT NULL DEFAULT '', account_name TEXT NOT NULL DEFAULT '',
  template_id INTEGER, subject TEXT NOT NULL DEFAULT '', message TEXT NOT NULL DEFAULT '',
  total INTEGER NOT NULL DEFAULT 0, sent INTEGER NOT NULL DEFAULT 0, failed INTEGER NOT NULL DEFAULT 0, skipped INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'running', error TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')), finished_at TEXT NOT NULL DEFAULT '');
CREATE TABLE messages(
  campaign_id INTEGER NOT NULL, place_id INTEGER, contact_id INTEGER, recipient TEXT NOT NULL, channel TEXT NOT NULL,
  status TEXT NOT NULL, error TEXT NOT NULL DEFAULT '', provider_id TEXT NOT NULL DEFAULT '',
  sent_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
CREATE INDEX messages_recipient ON messages(recipient, channel, sent_at);
CREATE INDEX messages_campaign ON messages(campaign_id);
CREATE TABLE optouts(value TEXT PRIMARY KEY, reason TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
CREATE TABLE activity(id INTEGER PRIMARY KEY, at TEXT NOT NULL DEFAULT (datetime('now','localtime')), kind TEXT NOT NULL, text TEXT NOT NULL);
CREATE TABLE kv(k TEXT PRIMARY KEY, v TEXT NOT NULL);
INSERT INTO lists(name) VALUES('بانک اصلی');
";

/// v5: the admin edition's own customer registry and licence history (empty tables in other editions).
const SCHEMA_V5: &str = "
CREATE TABLE IF NOT EXISTS adm_customers(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, phone TEXT NOT NULL DEFAULT '', email TEXT NOT NULL DEFAULT '',
  machine TEXT NOT NULL DEFAULT '', plan TEXT NOT NULL DEFAULT '', expires TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active', note TEXT NOT NULL DEFAULT '', cloud_user TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')), updated_at TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS adm_licenses(
  id INTEGER PRIMARY KEY, customer_id INTEGER REFERENCES adm_customers(id) ON DELETE SET NULL,
  customer TEXT NOT NULL DEFAULT '', machine TEXT NOT NULL, plan TEXT NOT NULL DEFAULT '', expires TEXT NOT NULL,
  code TEXT NOT NULL, issued_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
CREATE INDEX IF NOT EXISTS adm_licenses_customer ON adm_licenses(customer_id, id);
";

/// v6: scheduled extractions.
const SCHEMA_V6: &str = "
CREATE TABLE IF NOT EXISTS schedules(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, req TEXT NOT NULL, every TEXT NOT NULL DEFAULT 'daily',
  at TEXT NOT NULL DEFAULT '09:00', weekday INTEGER NOT NULL DEFAULT 6, on_date TEXT NOT NULL DEFAULT '',
  enabled INTEGER NOT NULL DEFAULT 1, next_run TEXT NOT NULL DEFAULT '', last_run TEXT NOT NULL DEFAULT '',
  last_result TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
";

/// v7: business photo (card only) and which messengers a mobile number has.
const SCHEMA_V7: &str = "
ALTER TABLE places ADD COLUMN photo TEXT NOT NULL DEFAULT '';
ALTER TABLE contacts ADD COLUMN apps TEXT NOT NULL DEFAULT '';
";

/// v8: product & price intelligence snapshots and brand analyses.
// v9: every brand analysis is kept (history and score over time)
const SCHEMA_V9: &str = "
CREATE TABLE IF NOT EXISTS brand_runs (
  id INTEGER PRIMARY KEY,
  brand_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  overall INTEGER,
  report TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);
CREATE INDEX IF NOT EXISTS brand_runs_b ON brand_runs(brand_id, id);
";

/// v10: the daily price watchlist and the seller's own catalogue (matched against the market).
const SCHEMA_V10: &str = "
CREATE TABLE IF NOT EXISTS watch(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, req TEXT NOT NULL, own_price INTEGER NOT NULL DEFAULT 0,
  alert_pct REAL NOT NULL DEFAULT 5, enabled INTEGER NOT NULL DEFAULT 1, last_run TEXT NOT NULL DEFAULT '',
  last_id INTEGER NOT NULL DEFAULT 0, median INTEGER NOT NULL DEFAULT 0, min INTEGER NOT NULL DEFAULT 0,
  prev_median INTEGER NOT NULL DEFAULT 0, prev_min INTEGER NOT NULL DEFAULT 0, basis TEXT NOT NULL DEFAULT 'unit', alert TEXT NOT NULL DEFAULT '',
  error TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
CREATE TABLE IF NOT EXISTS catalog(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, sku TEXT NOT NULL DEFAULT '', brand TEXT NOT NULL DEFAULT '',
  own_price INTEGER NOT NULL DEFAULT 0, cost INTEGER NOT NULL DEFAULT 0, matched INTEGER NOT NULL DEFAULT 0,
  confidence TEXT NOT NULL DEFAULT '', min INTEGER NOT NULL DEFAULT 0, median INTEGER NOT NULL DEFAULT 0,
  max INTEGER NOT NULL DEFAULT 0, srp INTEGER NOT NULL DEFAULT 0, position REAL NOT NULL DEFAULT -1,
  matches TEXT NOT NULL DEFAULT '[]', run_id INTEGER NOT NULL DEFAULT 0, note TEXT NOT NULL DEFAULT '',
  checked_at TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
CREATE UNIQUE INDEX IF NOT EXISTS catalog_key ON catalog(name, sku);
";

const SCHEMA_V8: &str = "
CREATE TABLE IF NOT EXISTS market_runs(
  id INTEGER PRIMARY KEY, query TEXT NOT NULL, qkey TEXT NOT NULL, brand TEXT NOT NULL DEFAULT '',
  offers INTEGER NOT NULL DEFAULT 0, median INTEGER NOT NULL DEFAULT 0, min INTEGER NOT NULL DEFAULT 0,
  report TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
CREATE INDEX IF NOT EXISTS market_runs_q ON market_runs(qkey, id);
CREATE TABLE IF NOT EXISTS brands(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, own INTEGER NOT NULL DEFAULT 0, website TEXT NOT NULL DEFAULT '',
  instagram TEXT NOT NULL DEFAULT '', category TEXT NOT NULL DEFAULT '', note TEXT NOT NULL DEFAULT '',
  report TEXT NOT NULL DEFAULT '', analyzed_at TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
";

/// Places reachable on at least one channel (the only ones the UI lists).
const HAS: &str = "EXISTS(SELECT 1 FROM contacts c WHERE c.place_id=p.id)";
const PCOLS: &str = "p.id,p.list_id,p.place_key,p.name,p.contact_person,p.contact_role,p.category,p.address,p.website,p.rating,p.reviews,p.lat,p.lng,p.maps_url,p.keyword,p.province,p.city,p.status,p.note,p.tags,p.source,p.created_at,p.updated_at,p.photo";
const SOCIAL: &[&str] = &["instagram", "telegram", "whatsapp", "eitaa", "bale"];
pub const KINDS: &[&str] = &["mobile", "landline", "email", "instagram", "telegram", "whatsapp", "eitaa", "bale"];

fn to_place(r: &Row) -> rusqlite::Result<Place> {
    Ok(Place {
        id: r.get(0)?,
        list_id: r.get(1)?,
        place_key: r.get(2)?,
        name: r.get(3)?,
        contact_person: r.get(4)?,
        contact_role: r.get(5)?,
        category: r.get(6)?,
        address: r.get(7)?,
        website: r.get(8)?,
        rating: r.get(9)?,
        reviews: r.get(10)?,
        lat: r.get(11)?,
        lng: r.get(12)?,
        maps_url: r.get(13)?,
        keyword: r.get(14)?,
        province: r.get(15)?,
        city: r.get(16)?,
        status: r.get(17)?,
        note: r.get(18)?,
        tags: r.get(19)?,
        source: r.get(20)?,
        created_at: r.get(21)?,
        updated_at: r.get(22)?,
        photo: r.get(23)?,
        contacts: vec![],
    })
}

fn ids_sql(ids: &[i64]) -> String {
    ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
}

fn collect<T>(rows: rusqlite::Result<rusqlite::MappedRows<'_, impl FnMut(&Row) -> rusqlite::Result<T>>>) -> Res<Vec<T>> {
    rows.map_err(err)?.collect::<Result<Vec<_>, _>>().map_err(err)
}

fn open_conn(path: &Path) -> rusqlite::Result<Connection> {
    let c = Connection::open(path)?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;")?;
    Ok(c)
}

fn migrate(c: &mut Connection) -> rusqlite::Result<()> {
    let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if v < SCHEMA_VERSION {
        let tx = c.transaction()?;
        if v == 0 {
            tx.execute_batch(SCHEMA)?;
        }
        if v < 5 {
            tx.execute_batch(SCHEMA_V5)?;
        }
        if v < 6 {
            tx.execute_batch(SCHEMA_V6)?;
        }
        if v < 7 {
            tx.execute_batch(SCHEMA_V7)?;
        }
        if v < 8 {
            tx.execute_batch(SCHEMA_V8)?;
        }
        if v < 9 {
            tx.execute_batch(SCHEMA_V9)?;
        }
        if v < 10 {
            tx.execute_batch(SCHEMA_V10)?;
        }
        tx.execute_batch(&format!("PRAGMA user_version={SCHEMA_VERSION}"))?;
        tx.commit()?;
    }
    // future: `if v < 6 { … }` blocks go above, each one idempotent
    c.execute_batch(
        "UPDATE runs SET status='interrupted' WHERE status='running';
         UPDATE campaigns SET status='interrupted' WHERE status='running';",
    )
}

pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Db {
    pub fn open(path: &Path) -> Res<Db> {
        let mut c = open_conn(path).map_err(err)?;
        migrate(&mut c).map_err(err)?;
        Ok(Db {
            conn: Mutex::new(c),
            path: path.to_path_buf(),
        })
    }

    pub(crate) fn c(&self) -> MutexGuard<'_, Connection> {
        // a panic elsewhere must not brick the database
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn today(&self) -> String {
        self.c().query_row("SELECT date('now','localtime')", [], |r| r.get(0)).unwrap_or_default()
    }

    /// `date(d, '±N days')`
    pub fn date_shift(&self, d: &str, days: i64) -> String {
        self.c()
            .query_row("SELECT date(?1, ?2)", params![d, format!("{days:+} days")], |r| r.get(0))
            .unwrap_or_default()
    }

    pub fn days_between(&self, from: &str, to: &str) -> i64 {
        self.c()
            .query_row("SELECT CAST(julianday(?2) - julianday(?1) AS INTEGER)", [from, to], |r| r.get(0))
            .unwrap_or(0)
    }

    pub fn stamp(&self) -> String {
        self.c()
            .query_row("SELECT strftime('%Y%m%d-%H%M%S','now','localtime')", [], |r| r.get(0))
            .unwrap_or_else(|_| "export".into())
    }

    // ---------------- banks ----------------

    pub fn lists(&self) -> Res<Vec<ListInfo>> {
        let c = self.c();
        let mut st = c
            .prepare(&format!(
                "SELECT l.id, l.name, l.color, l.note, (SELECT count(*) FROM places p WHERE p.list_id=l.id AND {HAS}), l.created_at
                 FROM lists l WHERE l.archived=0 ORDER BY l.id"
            ))
            .map_err(err)?;
        let rows = st.query_map([], |r| {
            Ok(ListInfo {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                note: r.get(3)?,
                count: r.get(4)?,
                created_at: r.get(5)?,
            })
        });
        collect(rows)
    }

    pub fn list_exists(&self, id: i64) -> bool {
        self.c().query_row("SELECT 1 FROM lists WHERE id=?1 AND archived=0", [id], |_| Ok(())).is_ok()
    }

    pub fn list_name(&self, id: i64) -> String {
        self.c().query_row("SELECT name FROM lists WHERE id=?1", [id], |r| r.get(0)).unwrap_or_default()
    }

    pub fn list_create(&self, name: &str, color: &str) -> Res<i64> {
        let c = self.c();
        c.execute("INSERT INTO lists(name,color) VALUES(?1,?2)", [name, color]).map_err(err)?;
        Ok(c.last_insert_rowid())
    }

    pub fn list_update(&self, id: i64, name: &str, color: &str, note: &str) -> Res<()> {
        self.c()
            .execute("UPDATE lists SET name=?1, color=?2, note=?3 WHERE id=?4", params![name, color, note, id])
            .map(|_| ())
            .map_err(err)
    }

    pub fn list_delete(&self, id: i64) -> Res<()> {
        let c = self.c();
        let n: i64 = c.query_row("SELECT count(*) FROM lists WHERE archived=0", [], |r| r.get(0)).map_err(err)?;
        if n <= 1 {
            return Err("حداقل یک بانک داده باید باقی بماند.".into());
        }
        c.execute("DELETE FROM lists WHERE id=?1", [id]).map(|_| ()).map_err(err)
        // places + contacts cascade
    }

    /// Copies (or moves) places into another bank. A place the target already has absorbs the new contacts.
    pub fn transfer(&self, ids: &[i64], to: i64, copy: bool) -> Res<usize> {
        let mut c = self.c();
        let tx = c.transaction().map_err(err)?;
        let cols = PCOLS.replace("p.", "").trim_start_matches("id,list_id,").to_string();
        let mut done = 0;
        for id in ids {
            let key: Option<(String, i64)> = tx
                .query_row("SELECT place_key, list_id FROM places WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()
                .map_err(err)?;
            let Some((key, from)) = key else { continue };
            if from == to {
                continue;
            }
            tx.execute(
                &format!("INSERT OR IGNORE INTO places(list_id,{cols}) SELECT ?1,{cols} FROM places WHERE id=?2"),
                params![to, id],
            )
            .map_err(err)?;
            let target: i64 = tx
                .query_row("SELECT id FROM places WHERE list_id=?1 AND place_key=?2", params![to, key], |r| r.get(0))
                .map_err(err)?;
            tx.execute(
                "INSERT OR IGNORE INTO contacts(place_id,list_id,kind,value,label,origin) SELECT ?1,?2,kind,value,label,origin FROM contacts WHERE place_id=?3",
                params![target, to, id],
            )
            .map_err(err)?;
            if !copy {
                tx.execute("DELETE FROM places WHERE id=?1", [id]).map_err(err)?;
            }
            done += 1;
        }
        tx.commit().map_err(err)?;
        Ok(done)
    }

    // ---------------- places ----------------

    pub fn known(&self, list_id: i64, key: &str) -> bool {
        self.c()
            .query_row("SELECT 1 FROM places WHERE list_id=?1 AND place_key=?2", params![list_id, key], |_| Ok(()))
            .is_ok()
    }

    /// Saves a place found by the engine or an import. Contacts the bank already has are skipped.
    /// Returns (place was new, contacts added).
    pub fn insert_place(&self, p: &Place) -> Res<(bool, usize)> {
        let mut c = self.c();
        let tx = c.transaction().map_err(err)?;
        let n = tx
            .execute(
                "INSERT OR IGNORE INTO places(list_id,place_key,name,contact_person,contact_role,category,address,website,rating,reviews,lat,lng,
                   maps_url,keyword,province,city,note,tags,source,photo)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
                params![
                    p.list_id,
                    p.place_key,
                    p.name,
                    p.contact_person,
                    p.contact_role,
                    p.category,
                    p.address,
                    p.website,
                    p.rating,
                    p.reviews,
                    p.lat,
                    p.lng,
                    p.maps_url,
                    p.keyword,
                    p.province,
                    p.city,
                    p.note,
                    p.tags,
                    p.source,
                    p.photo
                ],
            )
            .map_err(err)?;
        if n == 0 {
            return Ok((false, 0));
        }
        let id = tx.last_insert_rowid();
        let mut added = 0;
        for ct in &p.contacts {
            added += tx
                .execute(
                    "INSERT OR IGNORE INTO contacts(place_id,list_id,kind,value,label,origin) VALUES(?1,?2,?3,?4,?5,?6)",
                    params![id, p.list_id, ct.kind, ct.value, ct.label, ct.origin],
                )
                .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok((true, added))
    }

    /// Manual add / edit from the contact card. Returns the id and values another place in the bank already owns.
    pub fn save_place(&self, p: &Place) -> Res<(i64, Vec<String>)> {
        let mut c = self.c();
        let tx = c.transaction().map_err(err)?;
        let id = if p.id == 0 {
            let key = format!(
                "manual:{}",
                tx.query_row("SELECT lower(hex(randomblob(8)))", [], |r| r.get::<_, String>(0)).map_err(err)?
            );
            tx.execute(
                "INSERT INTO places(list_id,place_key,name,contact_person,contact_role,category,address,website,keyword,province,city,status,note,tags,source)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'manual')",
                params![
                    p.list_id,
                    key,
                    p.name,
                    p.contact_person,
                    p.contact_role,
                    p.category,
                    p.address,
                    p.website,
                    p.keyword,
                    p.province,
                    p.city,
                    p.status,
                    p.note,
                    p.tags
                ],
            )
            .map_err(err)?;
            tx.last_insert_rowid()
        } else {
            tx.execute(
                "UPDATE places SET name=?1, contact_person=?2, contact_role=?3, category=?4, address=?5, website=?6, keyword=?7, province=?8, city=?9,
                   status=?10, note=?11, tags=?12, updated_at=datetime('now','localtime') WHERE id=?13",
                params![
                    p.name,
                    p.contact_person,
                    p.contact_role,
                    p.category,
                    p.address,
                    p.website,
                    p.keyword,
                    p.province,
                    p.city,
                    p.status,
                    p.note,
                    p.tags,
                    p.id
                ],
            )
            .map_err(err)?;
            // keep rows that stay (their ids and delivery state matter to campaigns); drop the removed ones
            let keep: Vec<(&str, &str)> = p.contacts.iter().map(|c| (c.kind.as_str(), c.value.as_str())).collect();
            let existing: Vec<(i64, String, String)> = {
                let mut st = tx.prepare("SELECT id, kind, value FROM contacts WHERE place_id=?1").map_err(err)?;
                collect(st.query_map([p.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))?
            };
            for (cid, k, v) in existing {
                if !keep.contains(&(k.as_str(), v.as_str())) {
                    tx.execute("DELETE FROM contacts WHERE id=?1", [cid]).map_err(err)?;
                }
            }
            p.id
        };
        let list_id: i64 = tx.query_row("SELECT list_id FROM places WHERE id=?1", [id], |r| r.get(0)).map_err(err)?;
        let mut taken = vec![];
        for ct in &p.contacts {
            let n = tx
                .execute(
                    "INSERT OR IGNORE INTO contacts(place_id,list_id,kind,value,label,origin) VALUES(?1,?2,?3,?4,?5,?6)",
                    params![
                        id,
                        list_id,
                        ct.kind,
                        ct.value,
                        ct.label,
                        if ct.origin.is_empty() { "manual" } else { &ct.origin }
                    ],
                )
                .map_err(err)?;
            if n == 0 {
                let owner: i64 = tx
                    .query_row(
                        "SELECT place_id FROM contacts WHERE list_id=?1 AND kind=?2 AND value=?3",
                        params![list_id, ct.kind, ct.value],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                if owner != id {
                    taken.push(ct.value.clone());
                }
            }
        }
        tx.commit().map_err(err)?;
        Ok((id, taken))
    }

    fn filter_sql(f: &Filter) -> (String, Vec<String>) {
        let mut sql = format!(" FROM places p WHERE {HAS}");
        let mut args: Vec<String> = vec![];
        let mut arg = |sql: &mut String, v: String, tpl: &str| {
            args.push(v);
            *sql += &tpl.replace('#', &format!("?{}", args.len()));
        };
        if f.list_id > 0 {
            sql += &format!(" AND p.list_id={}", f.list_id);
        }
        for (col, val) in [("keyword", &f.keyword), ("province", &f.province), ("city", &f.city), ("category", &f.category), ("status", &f.status)] {
            if !val.is_empty() {
                arg(&mut sql, val.clone(), &format!(" AND p.{col}=#"));
            }
        }
        if !f.tag.is_empty() {
            arg(&mut sql, format!(",{},", f.tag), " AND instr(','||p.tags||',', #)>0");
        }
        let kinds = |ks: &[&str]| {
            format!(
                " AND EXISTS(SELECT 1 FROM contacts c WHERE c.place_id=p.id AND c.kind IN ('{}'))",
                ks.join("','")
            )
        };
        sql += &match f.channel.as_str() {
            "website" => " AND p.website<>''".to_string(),
            "person" => " AND p.contact_person<>''".to_string(),
            "social" => kinds(SOCIAL),
            "messenger" => kinds(&["eitaa", "bale"]),
            k if KINDS.contains(&k) => kinds(&[k]),
            _ => String::new(),
        };
        if !f.q.trim().is_empty() {
            arg(
                &mut sql,
                format!("%{}%", f.q.trim()),
                " AND (p.name LIKE # OR p.contact_person LIKE # OR p.address LIKE # OR p.note LIKE # OR p.category LIKE # OR p.tags LIKE #
                   OR EXISTS(SELECT 1 FROM contacts c WHERE c.place_id=p.id AND c.value LIKE #))",
            );
        }
        if !f.ids.is_empty() {
            sql += &format!(" AND p.id IN ({})", ids_sql(&f.ids));
        }
        (sql, args)
    }

    fn attach_contacts(c: &Connection, rows: &mut [Place]) -> Res<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let ids: Vec<i64> = rows.iter().map(|p| p.id).collect();
        let mut st = c
            .prepare(&format!(
                "SELECT place_id,id,kind,value,label,origin,delivery,apps FROM contacts WHERE place_id IN ({}) ORDER BY
                   CASE kind WHEN 'mobile' THEN 0 WHEN 'landline' THEN 1 WHEN 'email' THEN 2 ELSE 3 END, id",
                ids_sql(&ids)
            ))
            .map_err(err)?;
        let found = collect(st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                Contact {
                    id: r.get(1)?,
                    kind: r.get(2)?,
                    value: r.get(3)?,
                    label: r.get(4)?,
                    origin: r.get(5)?,
                    delivery: r.get(6)?,
                    apps: r.get(7)?,
                },
            ))
        }))?;
        let mut by: HashMap<i64, Vec<Contact>> = HashMap::new();
        for (pid, ct) in found {
            by.entry(pid).or_default().push(ct);
        }
        for p in rows.iter_mut() {
            p.contacts = by.remove(&p.id).unwrap_or_default();
        }
        Ok(())
    }

    /// `limit == 0` returns every matching row (exports, campaigns).
    pub fn list(&self, f: &Filter, offset: i64, limit: i64) -> Res<(Vec<Place>, i64)> {
        let (w, args) = Self::filter_sql(f);
        let c = self.c();
        let total: i64 = c
            .query_row(&format!("SELECT count(*){w}"), params_from_iter(args.iter()), |r| r.get(0))
            .map_err(err)?;
        let page = if limit > 0 {
            format!(" LIMIT {limit} OFFSET {offset}")
        } else {
            String::new()
        };
        let mut st = c.prepare(&format!("SELECT {PCOLS}{w} ORDER BY p.id DESC{page}")).map_err(err)?;
        let mut rows = collect(st.query_map(params_from_iter(args.iter()), to_place))?;
        Self::attach_contacts(&c, &mut rows)?;
        Ok((rows, total))
    }

    pub fn get(&self, id: i64) -> Res<Place> {
        let c = self.c();
        let mut p = [c
            .query_row(&format!("SELECT {PCOLS} FROM places p WHERE p.id=?1"), [id], to_place)
            .map_err(err)?];
        Self::attach_contacts(&c, &mut p)?;
        let [p] = p;
        Ok(p)
    }

    pub fn get_by_key(&self, list_id: i64, key: &str) -> Res<Place> {
        let id: i64 = self
            .c()
            .query_row("SELECT id FROM places WHERE list_id=?1 AND place_key=?2", params![list_id, key], |r| r.get(0))
            .map_err(err)?;
        self.get(id)
    }

    /// Breakdown of the places matching `f`: by province, county, line of business, keyword,
    /// status and channel coverage (the «تحلیل» view of a bank).
    pub fn bank_stats(&self, f: &Filter) -> Res<BankStats> {
        let (w, args) = Self::filter_sql(f);
        let c = self.c();
        let pairs = |sql: String| -> Res<Vec<(String, i64)>> {
            let mut st = c.prepare(&sql).map_err(err)?;
            collect(st.query_map(params_from_iter(args.iter()), |r| Ok((r.get(0)?, r.get(1)?))))
        };
        let by = |col: &str| pairs(format!("SELECT p.{col}, count(*){w} AND p.{col}<>'' GROUP BY 1 ORDER BY 2 DESC, 1 LIMIT 200"));
        let total: i64 = c.query_row(&format!("SELECT count(*){w}"), params_from_iter(args.iter()), |r| r.get(0)).map_err(err)?;
        let mut st = c
            .prepare(&format!("SELECT p.province, p.city, count(*){w} AND p.city<>'' GROUP BY 1, 2 ORDER BY 3 DESC, 2 LIMIT 300"))
            .map_err(err)?;
        let cities = collect(st.query_map(params_from_iter(args.iter()), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))?;
        let mut channels = pairs(format!(
            "SELECT c.kind, count(DISTINCT c.place_id) FROM contacts c WHERE c.place_id IN (SELECT p.id{w}) GROUP BY 1 ORDER BY 2 DESC"
        ))?;
        let (website, person): (i64, i64) = c
            .query_row(
                &format!("SELECT coalesce(sum(p.website<>''),0), coalesce(sum(p.contact_person<>''),0){w}"),
                params_from_iter(args.iter()),
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;
        channels.push(("website".into(), website));
        channels.push(("person".into(), person));
        Ok(BankStats {
            total,
            provinces: by("province")?,
            cities,
            categories: by("category")?,
            keywords: by("keyword")?,
            statuses: by("status")?,
            channels,
        })
    }

    pub fn facets(&self, list_id: i64) -> Res<Facets> {
        let c = self.c();
        let scope = if list_id > 0 { format!(" AND p.list_id={list_id}") } else { String::new() };
        let group = |col: &str| -> Res<Vec<(String, i64)>> {
            let mut st = c
                .prepare(&format!(
                    "SELECT p.{col}, count(*) FROM places p WHERE {HAS}{scope} AND p.{col}<>'' GROUP BY p.{col} ORDER BY 2 DESC LIMIT 300"
                ))
                .map_err(err)?;
            collect(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))))
        };
        let mut tags: HashMap<String, i64> = HashMap::new();
        for t in group("tags")? {
            for one in t.0.split(',').filter(|s| !s.is_empty()) {
                *tags.entry(one.to_string()).or_default() += t.1;
            }
        }
        let mut tags: Vec<(String, i64)> = tags.into_iter().collect();
        tags.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(Facets {
            keywords: group("keyword")?,
            categories: group("category")?,
            provinces: group("province")?,
            cities: group("city")?,
            tags,
        })
    }

    pub fn dashboard(&self) -> Res<Dashboard> {
        let c = self.c();
        let one = |sql: &str| -> i64 { c.query_row(sql, [], |r| r.get::<_, Option<i64>>(0)).ok().flatten().unwrap_or(0) };
        let with_kind = |ks: &[&str]| one(&format!("SELECT count(DISTINCT place_id) FROM contacts WHERE kind IN ('{}')", ks.join("','")));
        let mut d = Dashboard {
            total: one(&format!("SELECT count(*) FROM places p WHERE {HAS}")),
            mobile: with_kind(&["mobile"]),
            landline: with_kind(&["landline"]),
            email: with_kind(&["email"]),
            social: with_kind(SOCIAL),
            website: one(&format!("SELECT count(*) FROM places p WHERE {HAS} AND website<>''")),
            person: one(&format!("SELECT count(*) FROM places p WHERE {HAS} AND contact_person<>''")),
            today: one(&format!("SELECT count(*) FROM places p WHERE {HAS} AND date(created_at)=date('now','localtime')")),
            week: one(&format!("SELECT count(*) FROM places p WHERE {HAS} AND created_at >= datetime('now','localtime','-7 days')")),
            prev_week: one(&format!(
                "SELECT count(*) FROM places p WHERE {HAS} AND created_at >= datetime('now','localtime','-14 days') AND created_at < datetime('now','localtime','-7 days')"
            )),
            contacted: one(&format!("SELECT count(*) FROM places p WHERE {HAS} AND status<>'new'")),
            interested: one(&format!("SELECT count(*) FROM places p WHERE {HAS} AND status='interested'")),
            customers: one(&format!("SELECT count(*) FROM places p WHERE {HAS} AND status='customer'")),
            interested_waiting: one(&format!(
                "SELECT count(*) FROM places p WHERE {HAS} AND status='interested' AND coalesce(nullif(updated_at,''),created_at) < datetime('now','localtime','-3 days')"
            )),
            sent_week: one("SELECT count(*) FROM messages WHERE status='sent' AND sent_at >= datetime('now','localtime','-7 days')"),
            sent_prev_week: one(
                "SELECT count(*) FROM messages WHERE status='sent' AND sent_at >= datetime('now','localtime','-14 days') AND sent_at < datetime('now','localtime','-7 days')",
            ),
            month: one(&format!("SELECT count(*) FROM places p WHERE {HAS} AND created_at >= datetime('now','localtime','-30 days')")),
            prev_month: one(&format!(
                "SELECT count(*) FROM places p WHERE {HAS} AND created_at >= datetime('now','localtime','-60 days') AND created_at < datetime('now','localtime','-30 days')"
            )),
            contacts: one("SELECT count(*) FROM contacts"),
            ..Default::default()
        };
        // 12 weeks, each ending today-7k (week 11 = the current 7 days)
        let mut wk = c
            .prepare(&format!(
                "SELECT date('now','localtime', ?1), count(*) FROM places p WHERE {HAS}
                   AND created_at >= date('now','localtime', ?1) AND created_at < date('now','localtime', ?2)"
            ))
            .map_err(err)?;
        for back in (0..12).rev() {
            let from = format!("-{} days", 7 * back + 6);
            let to = if back == 0 { "+1 days".to_string() } else { format!("-{} days", 7 * back - 1) };
            let row: (String, i64) = wk.query_row([&from, &to], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?;
            d.weekly.push(row);
        }
        let mut src = c
            .prepare(&format!(
                "SELECT source, count(*), sum(created_at >= datetime('now','localtime','-30 days')) FROM places p WHERE {HAS} GROUP BY 1 ORDER BY 2 DESC"
            ))
            .map_err(err)?;
        d.sources = collect(src.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, Option<i64>>(2)?.unwrap_or(0)))))?;
        let mut st = c
            .prepare(&format!(
                "SELECT date(created_at), count(*) FROM places p WHERE {HAS} AND created_at >= date('now','localtime','-29 days') GROUP BY 1"
            ))
            .map_err(err)?;
        let per_day: HashMap<String, i64> = collect(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))))?.into_iter().collect();
        let mut days = c.prepare("SELECT date('now','localtime', ?1)").map_err(err)?;
        for back in (0..30).rev() {
            let day: String = days.query_row([format!("-{back} days")], |r| r.get(0)).map_err(err)?;
            let n = per_day.get(&day).copied().unwrap_or(0);
            d.daily.push((day, n));
        }
        let top = |col: &str| -> Res<Vec<(String, i64)>> {
            let mut st = c
                .prepare(&format!(
                    "SELECT {col}, count(*) FROM places p WHERE {HAS} AND {col}<>'' GROUP BY 1 ORDER BY 2 DESC LIMIT 6"
                ))
                .map_err(err)?;
            collect(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))))
        };
        d.top_keywords = top("keyword")?;
        d.top_cities = top("CASE WHEN city<>'' THEN city ELSE province END")?;
        Ok(d)
    }

    pub fn update_status(&self, id: i64, status: &str, note: &str) -> Res<()> {
        self.c()
            .execute(
                "UPDATE places SET status=?1, note=?2, updated_at=datetime('now','localtime') WHERE id=?3",
                params![status, note, id],
            )
            .map(|_| ())
            .map_err(err)
    }

    pub fn set_status(&self, ids: &[i64], status: &str) -> Res<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        self.c()
            .execute(
                &format!(
                    "UPDATE places SET status=?1, updated_at=datetime('now','localtime') WHERE id IN ({})",
                    ids_sql(ids)
                ),
                [status],
            )
            .map_err(err)
    }

    /// Adds `tag` to every place (tags stay unique, comma-joined).
    pub fn add_tag(&self, ids: &[i64], tag: &str) -> Res<usize> {
        if ids.is_empty() || tag.is_empty() {
            return Ok(0);
        }
        self.c()
            .execute(
                &format!(
                    "UPDATE places SET tags = CASE WHEN tags='' THEN ?1 ELSE tags||','||?1 END, updated_at=datetime('now','localtime')
                     WHERE id IN ({}) AND instr(','||tags||',', ','||?1||',')=0",
                    ids_sql(ids)
                ),
                [tag],
            )
            .map_err(err)
    }

    pub fn mark_contacted(&self, id: i64) {
        let _ = self.c().execute(
            "UPDATE places SET status='contacted', updated_at=datetime('now','localtime') WHERE id=?1 AND status='new'",
            [id],
        );
    }

    // ---------------- product & price intelligence ----------------

    pub fn market_save(&self, r: &crate::market::Report) -> Res<i64> {
        let c = self.c();
        c.execute(
            "INSERT INTO market_runs(query,qkey,brand,offers,median,min,report) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![r.query, crate::market::words(&r.query).join(" "), r.brand, r.offers.len() as i64, r.stats.median, r.stats.min, serde_json::to_string(r).map_err(err)?],
        )
        .map_err(err)?;
        Ok(c.last_insert_rowid())
    }

    /// Earlier searches of the same product and brand (oldest first, last of each day): (date, median, min).
    pub fn market_history(&self, query: &str, brand: &str) -> Res<Vec<crate::market::History>> {
        let c = self.c();
        let mut st = c
            .prepare(
                "SELECT d, median, min FROM (SELECT date(created_at) d, median, min FROM market_runs WHERE id IN
                   (SELECT max(id) FROM market_runs WHERE qkey=?1 AND brand=?2 AND offers>0 GROUP BY date(created_at))
                 ORDER BY d DESC LIMIT 180) ORDER BY d",
            )
            .map_err(err)?;
        collect(st.query_map(params![crate::market::words(query).join(" "), brand.trim()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))
    }

    /// The same history per source: (date, platform name, median) from each day's last snapshot.
    pub fn market_source_history(&self, query: &str, brand: &str) -> Res<Vec<(String, String, i64)>> {
        let c = self.c();
        let mut st = c
            .prepare(
                "SELECT date(created_at), json_extract(report,'$.channels') FROM market_runs WHERE id IN
                   (SELECT max(id) FROM market_runs WHERE qkey=?1 AND brand=?2 AND offers>0 GROUP BY date(created_at))
                 ORDER BY id DESC LIMIT 180",
            )
            .map_err(err)?;
        let rows: Vec<(String, Option<String>)> = collect(st.query_map(params![crate::market::words(query).join(" "), brand.trim()], |r| Ok((r.get(0)?, r.get(1)?))))?;
        let mut out = vec![];
        for (d, ch) in rows.into_iter().rev() {
            let shares: Vec<crate::market::Share> = ch.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
            out.extend(shares.into_iter().filter(|s| s.median > 0).map(|s| (d.clone(), s.name, s.median)));
        }
        Ok(out)
    }

    /// Recent searches: (id, query, brand, offers, median, created_at).
    pub fn market_runs(&self, limit: i64) -> Res<Vec<crate::market::RunRow>> {
        let c = self.c();
        let mut st = c.prepare("SELECT id,query,brand,offers,median,created_at FROM market_runs ORDER BY id DESC LIMIT ?1").map_err(err)?;
        collect(st.query_map([limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))))
    }

    pub fn market_get(&self, id: i64) -> Res<crate::market::Report> {
        let raw: String = self.c().query_row("SELECT report FROM market_runs WHERE id=?1", [id], |r| r.get(0)).map_err(err)?;
        let mut r: crate::market::Report = serde_json::from_str(&raw).map_err(err)?;
        r.id = id;
        r.history = self.market_history(&r.query, &r.brand)?;
        r.source_history = self.market_source_history(&r.query, &r.brand)?;
        Ok(r)
    }

    /// This month's product searches, one row per product: how often it was searched, its
    /// latest and first median price in the month, and each platform's latest median.
    pub fn market_trends(&self, days: i64) -> Res<Vec<crate::market::Trend>> {
        let c = self.c();
        let mut st = c
            .prepare("SELECT id,query,qkey,median,report,created_at FROM market_runs WHERE brand='' AND created_at >= datetime('now','localtime', ?1) ORDER BY id")
            .map_err(err)?;
        let rows: Vec<(i64, String, String, i64, String, String)> =
            collect(st.query_map([format!("-{days} days")], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))))?;
        let mut by: Vec<crate::market::Trend> = vec![];
        for (id, query, qkey, median, report, at) in rows {
            let channels: Vec<crate::market::Share> = serde_json::from_str::<Value>(&report)
                .ok()
                .and_then(|v| v.get("channels").cloned())
                .and_then(|c| serde_json::from_value(c).ok())
                .unwrap_or_default();
            match by.iter_mut().find(|t| t.key == qkey) {
                Some(t) => {
                    t.searches += 1;
                    t.last_median = median;
                    t.last_at = at;
                    t.last_id = id;
                    t.channels = channels;
                }
                None => by.push(crate::market::Trend { key: qkey, query, searches: 1, first_median: median, last_median: median, last_at: at, last_id: id, channels }),
            }
        }
        by.sort_by(|a, b| b.searches.cmp(&a.searches).then_with(|| b.last_at.cmp(&a.last_at)));
        by.truncate(24);
        Ok(by)
    }

    pub fn market_delete(&self, id: i64) -> Res<()> {
        self.c().execute("DELETE FROM market_runs WHERE id=?1", [id]).map_err(err)?;
        Ok(())
    }

    /// Records whether this mobile number has an account on `app` (seen in the chat assistant).
    pub fn set_app(&self, contact_id: i64, app: &str, has: bool) -> Res<()> {
        let c = self.c();
        let cur: String = c.query_row("SELECT apps FROM contacts WHERE id=?1", [contact_id], |r| r.get(0)).map_err(err)?;
        let mut parts: Vec<String> = cur.split(',').filter(|x| !x.is_empty() && x.split(':').next() != Some(app)).map(String::from).collect();
        parts.push(format!("{app}:{}", u8::from(has)));
        c.execute("UPDATE contacts SET apps=?1 WHERE id=?2", params![parts.join(","), contact_id]).map_err(err)?;
        Ok(())
    }

    pub fn set_delivery(&self, contact_id: i64, state: &str) {
        let _ = self.c().execute("UPDATE contacts SET delivery=?1 WHERE id=?2", params![state, contact_id]);
    }

    pub fn delete(&self, ids: &[i64]) -> Res<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        self.c().execute(&format!("DELETE FROM places WHERE id IN ({})", ids_sql(ids)), []).map_err(err)
    }

    // ---------------- scan history ----------------

    pub fn run_start(&self, list_id: i64, keyword: &str, province: &str, city: &str, requested: i64) -> Res<i64> {
        let c = self.c();
        c.execute(
            "INSERT INTO runs(list_id,keyword,province,city,requested) VALUES(?1,?2,?3,?4,?5)",
            params![list_id, keyword, province, city, requested],
        )
        .map_err(err)?;
        Ok(c.last_insert_rowid())
    }

    pub fn run_checkpoint(&self, id: i64, checked: i64, found: i64, resume: &str) {
        let _ = self.c().execute(
            "UPDATE runs SET checked=?1, found=?2, resume=?3 WHERE id=?4",
            params![checked, found, resume, id],
        );
    }

    pub fn run_finish(&self, id: i64, checked: i64, found: i64, status: &str, error: &str) -> Res<()> {
        self.c()
            .execute(
                // a finished keyword has nothing left to resume
                "UPDATE runs SET checked=?1, found=?2, status=?3, error=?4, finished_at=datetime('now','localtime'),
                   resume=CASE WHEN ?3='done' THEN '' ELSE resume END WHERE id=?5",
                params![checked, found, status, error, id],
            )
            .map(|_| ())
            .map_err(err)
    }

    /// «ادامه استخراج» was started from this row: its checkpoint now lives on the new run.
    pub fn run_resumed(&self, id: i64) {
        let _ = self.c().execute("UPDATE runs SET resume='' WHERE id=?1", [id]);
    }

    fn run_rows(&self, filter: &str) -> Res<Vec<Run>> {
        let c = self.c();
        let mut st = c
            .prepare(&format!(
                "SELECT r.id, coalesce(r.list_id,0), coalesce(l.name,''), r.keyword, r.province, r.city, r.requested, r.checked, r.found, r.status, r.error, r.resume,
                   r.started_at, r.finished_at FROM runs r LEFT JOIN lists l ON l.id=r.list_id {filter} ORDER BY r.id DESC LIMIT 100"
            ))
            .map_err(err)?;
        collect(st.query_map([], |r| {
            Ok(Run {
                id: r.get(0)?,
                list_id: r.get(1)?,
                list_name: r.get(2)?,
                keyword: r.get(3)?,
                province: r.get(4)?,
                city: r.get(5)?,
                requested: r.get(6)?,
                checked: r.get(7)?,
                found: r.get(8)?,
                status: r.get(9)?,
                error: r.get(10)?,
                resume: r.get(11)?,
                started_at: r.get(12)?,
                finished_at: r.get(13)?,
            })
        }))
    }

    pub fn runs(&self) -> Res<Vec<Run>> {
        self.run_rows("")
    }

    pub fn run(&self, id: i64) -> Res<Run> {
        self.run_rows(&format!("WHERE r.id={id}"))?.pop().ok_or_else(|| "اجرا پیدا نشد.".into())
    }

    // ---------------- accounts (secrets stay encrypted; see secrets.rs) ----------------

    pub fn accounts(&self) -> Res<Vec<Account>> {
        let c = self.c();
        let mut st = c
            .prepare("SELECT id,kind,name,config,per_minute,secret IS NOT NULL FROM accounts ORDER BY created_at")
            .map_err(err)?;
        collect(st.query_map([], |r| {
            Ok(Account {
                id: r.get(0)?,
                kind: r.get(1)?,
                name: r.get(2)?,
                config: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or(Value::Null),
                per_minute: r.get(4)?,
                has_secret: r.get(5)?,
            })
        }))
    }

    pub fn account_secret(&self, id: &str) -> Res<Option<Vec<u8>>> {
        self.c()
            .query_row("SELECT secret FROM accounts WHERE id=?1", [id], |r| r.get(0))
            .map_err(|_| "حساب پیدا نشد.".to_string())
    }

    /// `secret: None` keeps the stored secret (the UI never receives it back).
    pub fn account_save(&self, a: &Account, secret: Option<Vec<u8>>) -> Res<()> {
        let c = self.c();
        let config = serde_json::to_string(&a.config).map_err(err)?;
        c.execute(
            "INSERT INTO accounts(id,kind,name,config,per_minute) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET kind=excluded.kind, name=excluded.name, config=excluded.config, per_minute=excluded.per_minute",
            params![a.id, a.kind, a.name, config, a.per_minute],
        )
        .map_err(err)?;
        if let Some(s) = secret {
            c.execute("UPDATE accounts SET secret=?1 WHERE id=?2", params![s, a.id]).map_err(err)?;
        }
        Ok(())
    }

    pub fn account_delete(&self, id: &str) -> Res<()> {
        self.c().execute("DELETE FROM accounts WHERE id=?1", [id]).map(|_| ()).map_err(err)
    }

    // ---------------- templates ----------------

    pub fn templates(&self) -> Res<Vec<Template>> {
        let c = self.c();
        let mut st = c
            .prepare("SELECT id,channel,name,subject,body FROM templates ORDER BY channel, name")
            .map_err(err)?;
        collect(st.query_map([], |r| {
            Ok(Template {
                id: r.get(0)?,
                channel: r.get(1)?,
                name: r.get(2)?,
                subject: r.get(3)?,
                body: r.get(4)?,
            })
        }))
    }

    pub fn template_save(&self, t: &Template) -> Res<i64> {
        let c = self.c();
        if t.id > 0 {
            c.execute(
                "UPDATE templates SET channel=?1,name=?2,subject=?3,body=?4 WHERE id=?5",
                params![t.channel, t.name, t.subject, t.body, t.id],
            )
            .map_err(err)?;
            return Ok(t.id);
        }
        c.execute(
            "INSERT INTO templates(channel,name,subject,body) VALUES(?1,?2,?3,?4)",
            params![t.channel, t.name, t.subject, t.body],
        )
        .map_err(err)?;
        Ok(c.last_insert_rowid())
    }

    pub fn template_delete(&self, id: i64) -> Res<()> {
        self.c().execute("DELETE FROM templates WHERE id=?1", [id]).map(|_| ()).map_err(err)
    }

    // ---------------- campaigns ----------------

    #[allow(clippy::too_many_arguments)]
    pub fn campaign_start(&self, channel: &str, list_id: i64, account_id: &str, account_name: &str, subject: &str, message: &str, total: i64) -> Res<i64> {
        let c = self.c();
        c.execute(
            "INSERT INTO campaigns(channel,list_id,account_id,account_name,subject,message,total) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![channel, list_id, account_id, account_name, subject, message, total],
        )
        .map_err(err)?;
        Ok(c.last_insert_rowid())
    }

    pub fn campaign_finish(&self, id: i64, sent: i64, failed: i64, skipped: i64, status: &str, error: &str) -> Res<()> {
        self.c()
            .execute(
                "UPDATE campaigns SET sent=?1, failed=?2, skipped=?3, status=?4, error=?5, finished_at=datetime('now','localtime') WHERE id=?6",
                params![sent, failed, skipped, status, error, id],
            )
            .map(|_| ())
            .map_err(err)
    }

    pub fn campaigns(&self) -> Res<Vec<Campaign>> {
        let c = self.c();
        let mut st = c
            .prepare(
                "SELECT c.id, c.channel, coalesce(l.name,''), c.account_name, c.subject, c.message, c.total, c.sent, c.failed, c.skipped, c.status, c.error, c.created_at
                 FROM campaigns c LEFT JOIN lists l ON l.id=c.list_id ORDER BY c.id DESC LIMIT 100",
            )
            .map_err(err)?;
        collect(st.query_map([], |r| {
            Ok(Campaign {
                id: r.get(0)?,
                channel: r.get(1)?,
                list_name: r.get(2)?,
                account_name: r.get(3)?,
                subject: r.get(4)?,
                message: r.get(5)?,
                total: r.get(6)?,
                sent: r.get(7)?,
                failed: r.get(8)?,
                skipped: r.get(9)?,
                status: r.get(10)?,
                error: r.get(11)?,
                created_at: r.get(12)?,
            })
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn log_message(&self, campaign_id: i64, place_id: i64, contact_id: i64, recipient: &str, channel: &str, status: &str, error: &str, provider_id: &str) {
        let _ = self.c().execute(
            "INSERT INTO messages(campaign_id,place_id,contact_id,recipient,channel,status,error,provider_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![campaign_id, place_id, contact_id, recipient, channel, status, error, provider_id],
        );
    }

    pub fn messaged_within(&self, recipient: &str, channel: &str, days: i64) -> bool {
        self.c()
            .query_row(
                "SELECT 1 FROM messages WHERE recipient=?1 AND channel=?2 AND status='sent' AND sent_at > datetime('now','localtime',?3)",
                params![recipient, channel, format!("-{days} days")],
                |_| Ok(()),
            )
            .is_ok()
    }

    // ---------------- do-not-contact ----------------

    pub fn optouts(&self) -> Res<Vec<String>> {
        let c = self.c();
        let mut st = c.prepare("SELECT value FROM optouts ORDER BY created_at DESC").map_err(err)?;
        collect(st.query_map([], |r| r.get(0)))
    }

    pub fn set_optouts(&self, values: &[String]) -> Res<usize> {
        let mut c = self.c();
        let tx = c.transaction().map_err(err)?;
        tx.execute("DELETE FROM optouts", []).map_err(err)?;
        for v in values {
            tx.execute("INSERT OR IGNORE INTO optouts(value) VALUES(?1)", [v]).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(values.len())
    }

    pub fn opted_out(&self, value: &str) -> bool {
        self.c().query_row("SELECT 1 FROM optouts WHERE value=?1", [value], |_| Ok(())).is_ok()
    }

    // ---------------- activity feed ----------------

    pub fn log(&self, kind: &str, text: &str) {
        let c = self.c();
        let _ = c.execute("INSERT INTO activity(kind,text) VALUES(?1,?2)", [kind, text]);
        let _ = c.execute("DELETE FROM activity WHERE id <= (SELECT max(id) - 500 FROM activity)", []);
    }

    pub fn activity(&self, n: i64) -> Res<Vec<Activity>> {
        let c = self.c();
        let mut st = c.prepare("SELECT at,kind,text FROM activity ORDER BY id DESC LIMIT ?1").map_err(err)?;
        collect(st.query_map([n], |r| {
            Ok(Activity {
                at: r.get(0)?,
                kind: r.get(1)?,
                text: r.get(2)?,
            })
        }))
    }

    // ---------------- key/value ----------------

    pub fn kv_get(&self, k: &str) -> Option<String> {
        self.c().query_row("SELECT v FROM kv WHERE k=?1", [k], |r| r.get(0)).ok()
    }

    pub fn kv_set(&self, k: &str, v: &str) -> Res<()> {
        self.c()
            .execute("INSERT OR REPLACE INTO kv(k,v) VALUES(?1,?2)", [k, v])
            .map(|_| ())
            .map_err(err)
    }

    // ---------------- backup / restore ----------------

    /// Consistent snapshot while the app keeps running.
    pub fn backup_to(&self, dest: &Path) -> Res<()> {
        if dest.exists() {
            std::fs::remove_file(dest).map_err(err)?;
        }
        self.c().execute("VACUUM INTO ?1", [dest.to_string_lossy()]).map(|_| ()).map_err(err)
    }

    /// Replaces the live database with a backup (the current one is saved next to it first).
    pub fn restore_from(&self, src: &Path) -> Res<()> {
        if !src.is_file() {
            return Err("فایل پشتیبان پیدا نشد.".into());
        }
        let probe = Connection::open(src).map_err(err)?;
        let ok: String = probe
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|_| "فایل پشتیبان معتبر نیست.".to_string())?;
        let v: i64 = probe.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(err)?;
        drop(probe);
        if ok != "ok" || v == 0 || v > SCHEMA_VERSION {
            return Err("فایل پشتیبان معتبر نیست یا مربوط به نسخه جدیدتر برنامه است.".into());
        }
        let mut c = self.c();
        let before = self.path.with_extension("before-restore.db");
        let _ = std::fs::remove_file(&before);
        c.execute("VACUUM INTO ?1", [before.to_string_lossy()]).map_err(err)?;
        *c = Connection::open_in_memory().map_err(err)?; // releases the file handle
        for ext in ["db-wal", "db-shm"] {
            let _ = std::fs::remove_file(self.path.with_extension(ext));
        }
        let copied = std::fs::copy(src, &self.path);
        if copied.is_err() {
            let _ = std::fs::copy(&before, &self.path);
        }
        match open_conn(&self.path).and_then(|mut f| migrate(&mut f).map(|_| f)) {
            Ok(f) => *c = f,
            Err(e) => {
                // never leave the app on the empty in-memory database
                let _ = std::fs::copy(&before, &self.path);
                *c = open_conn(&self.path).map_err(err)?;
                return Err(err(e));
            }
        }
        copied.map(|_| ()).map_err(err)
    }

    // ---------------- migration from SellerMap v3 ----------------

    pub fn is_empty(&self) -> bool {
        self.c().query_row("SELECT count(*) FROM places", [], |r| r.get::<_, i64>(0)).unwrap_or(0) == 0
    }

    /// Copies banks, places, contacts, history, opt-outs and settings from `sellermap3.db` (ids preserved).
    pub fn import_v3(&self, old: &Path) -> Res<ImportReport> {
        let mut c = self.c();
        c.execute("ATTACH DATABASE ?1 AS v3", [old.to_string_lossy()]).map_err(err)?;
        let result = (|| -> Res<ImportReport> {
            let tx = c.transaction().map_err(err)?;
            let mut r = ImportReport::default();
            tx.execute("DELETE FROM lists", []).map_err(err)?; // drops the default bank of the fresh schema
            r.lists = tx
                .execute("INSERT INTO lists(id,name,created_at) SELECT id,name,created_at FROM v3.lists", [])
                .map_err(err)? as i64;
            r.places = tx
                .execute(
                    "INSERT INTO places(id,list_id,place_key,name,category,address,website,rating,reviews,lat,lng,maps_url,keyword,province,city,status,note,created_at)
                     SELECT id,list_id,place_key,name,category,address,website,rating,reviews,lat,lng,url,keyword,province,city,status,note,created_at FROM v3.places
                     WHERE list_id IN (SELECT id FROM lists)",
                    [],
                )
                .map_err(err)? as i64;
            let rows: Vec<(i64, i64, Vec<String>)> = {
                let mut st = tx
                    .prepare("SELECT id,list_id,mobiles,landlines,emails,instagram,telegram,whatsapp,eitaa,bale,rubika FROM v3.places")
                    .map_err(err)?;
                collect(st.query_map([], |row| {
                    let mut v = vec![];
                    for i in 2..11 {
                        v.push(row.get::<_, String>(i)?);
                    }
                    Ok((row.get(0)?, row.get(1)?, v))
                }))?
            };
            let kinds = ["mobile", "landline", "email", "instagram", "telegram", "whatsapp", "eitaa", "bale", ""]; // Rubika: not kept
            for (pid, lid, vals) in rows {
                for (kind, csv) in kinds.iter().zip(vals).filter(|(k, _)| !k.is_empty()) {
                    for value in csv.split(',').filter(|s| !s.is_empty()) {
                        r.contacts += tx
                            .execute(
                                "INSERT OR IGNORE INTO contacts(place_id,list_id,kind,value,origin) SELECT ?1,?2,?3,?4,'import' WHERE EXISTS(SELECT 1 FROM places WHERE id=?1)",
                                params![pid, lid, kind, value],
                            )
                            .map_err(err)? as i64;
                    }
                }
            }
            r.runs = tx
                .execute(
                    "INSERT INTO runs(id,list_id,keyword,province,city,requested,checked,found,status,started_at,finished_at)
                     SELECT id,list_id,keyword,province,city,requested,checked,found,status,started_at,finished_at FROM v3.runs",
                    [],
                )
                .map_err(err)? as i64;
            r.campaigns = tx
                .execute(
                    "INSERT INTO campaigns(id,channel,list_id,account_name,subject,message,total,sent,failed,skipped,status,created_at,finished_at)
                     SELECT id,channel,list_id,account,subject,message,total,sent,failed,skipped,status,created_at,finished_at FROM v3.campaigns",
                    [],
                )
                .map_err(err)? as i64;
            tx.execute(
                "INSERT INTO messages(campaign_id,place_id,recipient,channel,status,error,sent_at) SELECT campaign_id,place_id,recipient,channel,status,error,sent_at FROM v3.messages",
                [],
            )
            .map_err(err)?;
            tx.execute("INSERT OR IGNORE INTO optouts(value,created_at) SELECT value,created_at FROM v3.optouts", [])
                .map_err(err)?;
            tx.execute("INSERT OR REPLACE INTO kv(k,v) SELECT k,v FROM v3.kv WHERE k='settings'", [])
                .map_err(err)?;
            r.accounts_json = tx
                .query_row("SELECT v FROM v3.kv WHERE k='accounts'", [], |row| row.get(0))
                .optional()
                .map_err(err)?;
            tx.commit().map_err(err)?;
            Ok(r)
        })();
        let _ = c.execute("DETACH DATABASE v3", []);
        result
    }
}
