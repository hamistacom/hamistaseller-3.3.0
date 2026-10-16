//! SQLite store (WAL) with an r2d2 pool. Schema lives in `PRAGMA user_version`; `migrate` upgrades in place.
//! SQLite is deliberate: one file, nothing to install on the VPS, trivial to back up. WAL + a pool
//! handle the load of a licence server (periodic heartbeats, modest ingest) comfortably.
use crate::error::{bad, AppResult};
use chrono::Utc;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::Path;

pub type Pool = r2d2::Pool<SqliteConnectionManager>;
pub type Conn = r2d2::PooledConnection<SqliteConnectionManager>;

pub const SCHEMA_VERSION: i64 = 2;

/// v2: one free trial per PC.
const SCHEMA_V2: &str = "CREATE TABLE IF NOT EXISTS trials(machine_id TEXT PRIMARY KEY, user_id INTEGER, phone TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT (datetime('now')));";

const SCHEMA: &str = r#"
CREATE TABLE users(
  id INTEGER PRIMARY KEY,
  username TEXT NOT NULL UNIQUE COLLATE NOCASE,
  pass_hash TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT 'customer',          -- customer | admin
  customer_name TEXT NOT NULL DEFAULT '',
  plan_id INTEGER REFERENCES plans(id),
  status TEXT NOT NULL DEFAULT 'active',           -- active | suspended
  expires_at TEXT NOT NULL DEFAULT '',             -- subscription end, 'YYYY-MM-DD'
  note TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE plans(
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  max_devices INTEGER NOT NULL DEFAULT 1,
  monthly_scan_quota INTEGER NOT NULL DEFAULT 0,   -- 0 = unlimited
  monthly_send_quota INTEGER NOT NULL DEFAULT 0,
  allowed_sources TEXT NOT NULL DEFAULT '',        -- csv; empty = all
  megabank_access INTEGER NOT NULL DEFAULT 0,
  note TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE devices(
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  machine_id TEXT NOT NULL,
  name TEXT NOT NULL DEFAULT '',
  app_version TEXT NOT NULL DEFAULT '',
  last_ip TEXT NOT NULL DEFAULT '',
  revoked INTEGER NOT NULL DEFAULT 0,
  first_seen TEXT NOT NULL DEFAULT (datetime('now')),
  last_seen TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(user_id, machine_id)
);
CREATE INDEX devices_user ON devices(user_id);
CREATE TABLE quota_usage(
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  period TEXT NOT NULL,                            -- 'YYYY-MM' (UTC)
  scans INTEGER NOT NULL DEFAULT 0,
  sends INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(user_id, period)
);
CREATE TABLE activity_log(
  id INTEGER PRIMARY KEY,
  user_id INTEGER,
  device_id INTEGER,
  kind TEXT NOT NULL,                              -- login | scan | campaign | ingest | error | admin
  detail TEXT NOT NULL DEFAULT '',
  ip TEXT NOT NULL DEFAULT '',
  at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX activity_user ON activity_log(user_id, id);
CREATE INDEX activity_kind ON activity_log(kind, id);
CREATE TABLE megabank(
  id INTEGER PRIMARY KEY,
  place_key TEXT NOT NULL UNIQUE,                  -- stable id: maps feature id, else mobile/name hash
  name TEXT NOT NULL DEFAULT '',
  mobiles TEXT NOT NULL DEFAULT '',                -- csv, deduped
  landlines TEXT NOT NULL DEFAULT '',
  emails TEXT NOT NULL DEFAULT '',
  website TEXT NOT NULL DEFAULT '',
  instagram TEXT NOT NULL DEFAULT '',
  telegram TEXT NOT NULL DEFAULT '',
  whatsapp TEXT NOT NULL DEFAULT '',
  eitaa TEXT NOT NULL DEFAULT '',
  bale TEXT NOT NULL DEFAULT '',
  rubika TEXT NOT NULL DEFAULT '',
  address TEXT NOT NULL DEFAULT '',
  province TEXT NOT NULL DEFAULT '',
  city TEXT NOT NULL DEFAULT '',
  category TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL DEFAULT '',                 -- maps | neshan | balad | instagram | telegram | site | import | ...
  lat REAL, lng REAL,
  report_count INTEGER NOT NULL DEFAULT 1,         -- how many users reported it (confidence)
  first_seen TEXT NOT NULL DEFAULT (datetime('now')),
  last_seen TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX mega_province ON megabank(province);
CREATE INDEX mega_city ON megabank(city);
CREATE INDEX mega_category ON megabank(category);
CREATE INDEX mega_source ON megabank(source);
CREATE INDEX mega_seen ON megabank(last_seen);
CREATE TABLE notifications(
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  body TEXT NOT NULL DEFAULT '',
  tag TEXT NOT NULL DEFAULT 'system',              -- system | update | support
  audience TEXT NOT NULL DEFAULT 'all',            -- all | plan:<id> | user:<id>
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE notification_reads(
  user_id INTEGER NOT NULL,
  notification_id INTEGER NOT NULL,
  PRIMARY KEY(user_id, notification_id)
);
CREATE TABLE versions(
  channel TEXT PRIMARY KEY,                        -- 'agent'
  version TEXT NOT NULL,
  url TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  mandatory INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE scenarios(
  source TEXT PRIMARY KEY,                         -- modular: one row per extraction source
  version INTEGER NOT NULL DEFAULT 1,
  body TEXT NOT NULL DEFAULT '{}',                 -- JSON steps/selectors the desktop engine runs
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE admin_sessions(
  token TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  expires_at TEXT NOT NULL
);
CREATE TABLE optouts(value TEXT PRIMARY KEY, created_at TEXT NOT NULL DEFAULT (datetime('now')));
CREATE TABLE kv(k TEXT PRIMARY KEY, v TEXT NOT NULL);
"#;

pub fn open(path: &Path) -> AppResult<Pool> {
    let manager = SqliteConnectionManager::file(path).with_init(|c| {
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;")
    });
    let pool = r2d2::Pool::builder().max_size(8).build(manager).map_err(|e| bad(format!("db pool: {e}")))?;
    let mut c = pool.get().map_err(|e| bad(format!("db: {e}")))?;
    migrate(&mut c)?;
    Ok(pool)
}

fn migrate(c: &mut Connection) -> AppResult<()> {
    let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| bad(e.to_string()))?;
    if v < SCHEMA_VERSION {
        let tx = c.transaction().map_err(|e| bad(e.to_string()))?;
        if v == 0 {
            tx.execute_batch(SCHEMA).map_err(|e| bad(format!("schema: {e}")))?;
        }
        if v < 2 {
            tx.execute_batch(SCHEMA_V2).map_err(|e| bad(format!("schema v2: {e}")))?;
        }
        tx.execute_batch(&format!("PRAGMA user_version={SCHEMA_VERSION}")).map_err(|e| bad(e.to_string()))?;
        tx.commit().map_err(|e| bad(e.to_string()))?;
    }
    Ok(())
}

pub fn kv_get(c: &Conn, k: &str) -> Option<String> {
    c.query_row("SELECT v FROM kv WHERE k=?1", [k], |r| r.get(0)).ok()
}
pub fn kv_set(c: &Conn, k: &str, v: &str) -> AppResult<()> {
    c.execute("INSERT INTO kv(k,v) VALUES(?1,?2) ON CONFLICT(k) DO UPDATE SET v=?2", [k, v])
        .map(|_| ())
        .map_err(|e| bad(e.to_string()))
}

pub fn log(c: &Conn, user_id: Option<i64>, device_id: Option<i64>, kind: &str, detail: &str, ip: &str) {
    let _ = c.execute(
        "INSERT INTO activity_log(user_id,device_id,kind,detail,ip) VALUES(?1,?2,?3,?4,?5)",
        rusqlite::params![user_id, device_id, kind, detail, ip],
    );
}

pub fn period_now() -> String {
    Utc::now().format("%Y-%m").to_string()
}
