//! Talks to the admin's «هامیستا کلود» server. Everything here is optional: with no server the app
//! runs on offline licences only (hybrid model).
//!
//! * customer copy: login → `HC1.` token (verified offline by `license`), heartbeat, quota,
//!   mega-bank (transparent upload of PUBLIC business fields only + optional shared search),
//!   notifications, agent version, remote scanner fixes («سناریو»).
//! * admin panel:   login with the admin account, then a thin authenticated proxy to `/api/v1/admin/*`.
use crate::db::{err, Db, Place, Res};
use crate::license;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "/api/v1";
const SYNC_BATCH: i64 = 500;

/// `https://api.example.ir/` → `https://api.example.ir`; rejects anything that is not http(s).
pub fn normalize_url(raw: &str) -> Res<String> {
    let u = raw.trim().trim_end_matches('/').to_string();
    // plain http only for this PC (tests, trying the server locally): elsewhere passwords would travel in clear
    // exact host only: "http://127.0.0.1@evil.com" or "localhost.evil.com" must not pass
    let local = || reqwest::Url::parse(&u).is_ok_and(|p| p.username().is_empty() && matches!(p.host_str(), Some("127.0.0.1" | "localhost")));
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://").filter(|_| local()));
    match rest {
        Some(host) if !host.is_empty() && !host.contains(char::is_whitespace) => Ok(u),
        _ => Err("آدرس سرور باید با https:// شروع شود؛ مثل https://api.hamista.ir".into()),
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("HamistaSeller/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

/// Why a call failed: the server said no (`Denied`, with its Persian message) or we never reached it.
#[derive(Debug)]
pub enum CallErr {
    Denied(u16, String),
    Network(String),
}
impl CallErr {
    pub fn text(self) -> String {
        match self {
            CallErr::Denied(_, m) => m,
            CallErr::Network(m) => m,
        }
    }
}

/// One JSON request. `auth` is ("authorization", "Bearer …") or ("x-admin-token", …).
async fn call(url: &str, method: &str, path: &str, auth: Option<(&str, &str)>, body: Option<&Value>) -> Result<Value, CallErr> {
    let m = reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| CallErr::Network("متد نامعتبر".into()))?;
    let mut rq = client().request(m, format!("{url}{path}"));
    if let Some((h, v)) = auth {
        rq = rq.header(h, v);
    }
    if let Some(b) = body {
        rq = rq.json(b);
    }
    let resp = rq.send().await.map_err(|e| {
        crate::log_line("WARN", &format!("cloud {path}: {e}"));
        CallErr::Network("اتصال به سرور برقرار نشد؛ اینترنت یا آدرس سرور را بررسی کنید.".into())
    })?;
    let status = resp.status().as_u16();
    let is_json = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    let text = resp.text().await.map_err(|e| CallErr::Network(e.to_string()))?;
    let v: Value = if is_json {
        serde_json::from_str(&text).unwrap_or(Value::Null)
    } else {
        json!({ "ok": status < 400, "text": text }) // e.g. the mega-bank CSV export
    };
    if status >= 400 {
        // only the app's own JSON error is a verdict; an HTML page from a CDN / firewall / proxy is not
        return Err(match v["error"].as_str() {
            Some(msg) if is_json => CallErr::Denied(status, msg.to_string()),
            _ => CallErr::Network(format!("سرور در دسترس نیست (کد {status}). کمی بعد دوباره امتحان کنید.")),
        });
    }
    Ok(v)
}

fn kv(db: &Db, k: &str) -> String {
    db.kv_get(k).unwrap_or_default()
}

fn kv_del(db: &Db, k: &str) {
    let _ = db.c().execute("DELETE FROM kv WHERE k=?1", [k]);
}

fn seal(db: &Db, k: &str, v: &str) -> Res<()> {
    db.kv_set(k, &crate::logic::b64_encode(&crate::secrets::seal_str(v)?))
}

fn unseal(db: &Db, k: &str) -> Option<String> {
    crate::secrets::open_str(&crate::logic::b64_decode(&db.kv_get(k)?)?).ok()
}

// ======================= customer copy =======================

/// Drops the online session (logout, or an offline code was activated).
pub fn forget_session(db: &Db) {
    // pending quota stays: it is settled at the next heartbeat of any session on this PC
    for k in ["cloud_token", "cloud_error", "cloud_synced"] {
        kv_del(db, k);
    }
}

fn store_token(db: &Db, token: &str) -> Res<()> {
    license::verify_cloud(token, &license::machine())?; // never keep a token this build can't verify
    db.kv_set("cloud_token", token)?;
    db.kv_set("cloud_synced", &license::now_secs().to_string())?;
    for k in ["cloud_error", "quota_left_scan", "quota_left_send"] {
        kv_del(db, k); // the new token carries the current figures
    }
    Ok(())
}

fn bearer(db: &Db) -> Res<(String, String)> {
    let url = kv(db, "cloud_url");
    let token = db.kv_get("cloud_token").ok_or("برنامه با حساب آنلاین فعال نشده است.")?;
    Ok((url, format!("Bearer {token}")))
}

pub async fn login(db: &Db, url: &str, user: &str, pass: &str) -> Res<license::Status> {
    let url = normalize_url(url)?;
    let body = json!({
        "username": user.trim(), "password": pass, "machineId": license::machine(),
        "machineName": std::env::var("COMPUTERNAME").unwrap_or_default(), "appVersion": env!("CARGO_PKG_VERSION"),
    });
    let v = call(&url, "POST", &format!("{API}/auth/login"), None, Some(&body))
        .await
        .map_err(CallErr::text)?;
    let token = v["token"].as_str().ok_or("پاسخ سرور نامعتبر است.")?;
    db.kv_set("cloud_url", &url)?;
    db.kv_set("cloud_user", user.trim())?;
    store_token(db, token)?;
    db.log("license", &format!("ورود به حساب آنلاین «{}»", user.trim()));
    Ok(license::status(db))
}

/// Whether the server offers a free trial, and for how many days.
pub async fn trial_info(url: &str) -> Res<Value> {
    let url = normalize_url(url)?;
    call(&url, "GET", &format!("{API}/trial"), None, None).await.map_err(CallErr::text)
}

/// One free trial per PC: the server creates a short account and signs this PC in.
pub async fn start_trial(db: &Db, url: &str, name: &str, phone: &str) -> Res<license::Status> {
    let url = normalize_url(url)?;
    let body = json!({
        "machineId": license::machine(), "machineName": std::env::var("COMPUTERNAME").unwrap_or_default(),
        "appVersion": env!("CARGO_PKG_VERSION"), "name": name.trim(), "phone": phone.trim(),
    });
    let v = call(&url, "POST", &format!("{API}/auth/trial"), None, Some(&body))
        .await
        .map_err(CallErr::text)?;
    let token = v["token"].as_str().ok_or("پاسخ سرور نامعتبر است.")?;
    db.kv_set("cloud_url", &url)?;
    db.kv_set("cloud_user", v["username"].as_str().unwrap_or("trial"))?;
    store_token(db, token)?;
    db.log("license", "دوره آزمایشی فعال شد");
    Ok(license::status(db))
}

pub fn logout(db: &Db) -> license::Status {
    forget_session(db);
    db.log("license", "خروج از حساب آنلاین");
    license::status(db)
}

/// Refreshes the token. A refusal (suspended, expired, device removed) ends the session;
/// a network failure keeps the current token (offline grace until it expires).
pub async fn heartbeat(db: &Db) -> Res<license::Status> {
    let (url, auth) = bearer(db)?;
    let body = json!({ "appVersion": env!("CARGO_PKG_VERSION") });
    match call(&url, "POST", &format!("{API}/license/heartbeat"), Some(("authorization", &auth)), Some(&body)).await {
        Ok(v) => {
            store_token(db, v["token"].as_str().ok_or("پاسخ سرور نامعتبر است.")?)?;
            // use made offline is counted with the fresh token, then the token is refreshed again
            if flush_pending(db, &url).await {
                let (_, auth) = bearer(db)?;
                if let Ok(v) = call(&url, "POST", &format!("{API}/license/heartbeat"), Some(("authorization", &auth)), Some(&body)).await {
                    if let Some(t) = v["token"].as_str() {
                        store_token(db, t)?;
                    }
                }
            }
        }
        Err(CallErr::Denied(code, msg)) if code == 401 || code == 403 => {
            kv_del(db, "cloud_token");
            db.kv_set("cloud_error", &msg)?;
            db.log("license", &format!("سرور اشتراک را متوقف کرد: {msg}"));
        }
        Err(e) => return Err(e.text()),
    }
    Ok(license::status(db))
}

/// Server-side quota (atomic there). Offline licences and the master copy have none.
/// Without internet the use is allowed while the last token still shows quota left, and is
/// counted at the next heartbeat (`flush_pending`).
pub async fn consume(db: &Db, kind: &str, n: i64) -> Res<()> {
    if license::MASTER || db.kv_get("cloud_token").is_none() || n <= 0 {
        return Ok(());
    }
    let (url, auth) = bearer(db)?; // records use that already happened: callers run `precheck` before starting
    let left_key = format!("quota_left_{kind}");
    match call(
        &url,
        "POST",
        &format!("{API}/quota/consume"),
        Some(("authorization", &auth)),
        Some(&json!({ "kind": kind, "n": n })),
    )
    .await
    {
        Ok(v) => {
            if let Some(left) = v[if kind == "scan" { "scanLeft" } else { "sendLeft" }].as_i64() {
                db.kv_set(&left_key, &left.to_string())?;
            }
            Ok(())
        }
        Err(CallErr::Denied(429, msg)) => {
            db.kv_set(&left_key, "0")?;
            Err(msg)
        }
        Err(CallErr::Network(_)) => {
            let key = format!("quota_pending_{kind}");
            let pending: i64 = kv(db, &key).parse().unwrap_or(0);
            db.kv_set(&key, &(pending + n).to_string())
        }
        Err(e) => Err(e.text()),
    }
}

/// Fast local check from the last token: is any of this month's quota left?
pub fn precheck(db: &Db, kind: &str) -> Res<()> {
    let st = license::status(db);
    if st.mode != "online" {
        return Ok(());
    }
    let token_left = if kind == "scan" { st.scan_left } else { st.send_left };
    if token_left < 0 {
        return Ok(()); // unlimited plan
    }
    // newest figure: the last consume answer if any (the token only changes at a heartbeat), minus use not yet counted
    let left = kv(db, &format!("quota_left_{kind}")).parse().unwrap_or(token_left) - kv(db, &format!("quota_pending_{kind}")).parse().unwrap_or(0);
    if left <= 0 {
        return Err(format!(
            "سهمیه {} این ماه تمام شده است. ماه بعد صفر می‌شود یا اشتراک را ارتقا دهید.",
            if kind == "scan" { "استخراج" } else { "ارسال" }
        ));
    }
    Ok(())
}

/// Returns true when something was settled (the token's quota figures are then stale).
async fn flush_pending(db: &Db, url: &str) -> bool {
    let Ok((_, auth)) = bearer(db) else { return false };
    let mut settled = false;
    for kind in ["scan", "send"] {
        let key = format!("quota_pending_{kind}");
        let n: i64 = kv(db, &key).parse().unwrap_or(0);
        if n <= 0 {
            continue;
        }
        let r = call(
            url,
            "POST",
            &format!("{API}/quota/consume"),
            Some(("authorization", &auth)),
            Some(&json!({ "kind": kind, "n": n })),
        )
        .await;
        // counted, or over the cap (the server then fills the quota to the cap): settled either way
        if r.is_ok() || matches!(r, Err(CallErr::Denied(429, _))) {
            kv_del(db, &key);
            settled = true;
        }
    }
    settled
}

/// Public business fields of one place, as the server's mega-bank takes them.
/// Private data (notes, follow-up status, tags, campaign history) is never part of this.
#[derive(Serialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PublicPlace {
    pub place_key: String,
    pub name: String,
    pub mobiles: Vec<String>,
    pub landlines: Vec<String>,
    pub emails: Vec<String>,
    pub website: String,
    pub instagram: String,
    pub telegram: String,
    pub whatsapp: String,
    pub eitaa: String,
    pub bale: String,
    pub rubika: String,
    pub address: String,
    pub province: String,
    pub city: String,
    pub category: String,
    pub source: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

/// Places saved after `after_id` (only ones with a contact), with their public contacts.
pub fn public_batch(db: &Db, after_id: i64, limit: i64) -> Res<(Vec<PublicPlace>, i64)> {
    let c = db.c();
    // only listings the engines found in public sources: never manual entries, file imports
    // (the user's own customer lists) or rows that came from the mega-bank itself
    let mut st = c
        .prepare(&format!(
            "SELECT p.id,p.place_key,p.name,p.website,p.address,p.province,p.city,p.category,p.source,p.lat,p.lng
             FROM places p WHERE p.id>?1 AND p.source IN ({}) AND EXISTS(SELECT 1 FROM contacts c WHERE c.place_id=p.id)
             ORDER BY p.id LIMIT ?2",
            crate::sources::public_sql_list()
        ))
        .map_err(err)?;
    let rows: Vec<(i64, PublicPlace)> = st
        .query_map(params![after_id, limit], |r| {
            Ok((
                r.get(0)?,
                PublicPlace {
                    place_key: r.get(1)?,
                    name: r.get(2)?,
                    website: r.get(3)?,
                    address: r.get(4)?,
                    province: r.get(5)?,
                    city: r.get(6)?,
                    category: r.get(7)?,
                    source: r.get(8)?,
                    lat: r.get(9)?,
                    lng: r.get(10)?,
                    ..Default::default()
                },
            ))
        })
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    let last = rows.last().map(|(id, _)| *id).unwrap_or(after_id);
    // only what the engines read from public pages: never numbers the user typed in by hand
    let mut cst = c
        .prepare("SELECT kind,value FROM contacts WHERE place_id=?1 AND origin NOT IN ('manual','import','megabank','')")
        .map_err(err)?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, mut p) in rows {
        let contacts: Vec<(String, String)> = cst
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?
            .collect::<Result<_, _>>()
            .map_err(err)?;
        for (kind, value) in contacts {
            match kind.as_str() {
                "mobile" => p.mobiles.push(value),
                "landline" => p.landlines.push(value),
                "email" => p.emails.push(value),
                "instagram" if p.instagram.is_empty() => p.instagram = value,
                "telegram" if p.telegram.is_empty() => p.telegram = value,
                "whatsapp" if p.whatsapp.is_empty() => p.whatsapp = value,
                "eitaa" if p.eitaa.is_empty() => p.eitaa = value,
                "bale" if p.bale.is_empty() => p.bale = value,
                "rubika" if p.rubika.is_empty() => p.rubika = value,
                _ => {}
            }
        }
        out.push(p);
    }
    Ok((out, last))
}

/// Uploads new public business data to the mega-bank (stated in the licence terms and in Settings).
/// Returns how many places were sent.
pub async fn mega_sync(db: &Db) -> Res<i64> {
    if db.kv_get("cloud_token").is_none() {
        return Ok(0);
    }
    let (url, auth) = bearer(db)?;
    let mut sent = 0;
    for _ in 0..20 {
        let after: i64 = kv(db, "mega_synced").parse().unwrap_or(0);
        let (batch, last) = public_batch(db, after, SYNC_BATCH)?;
        if batch.is_empty() {
            break;
        }
        let n = batch.len() as i64;
        call(
            &url,
            "POST",
            &format!("{API}/megabank/ingest"),
            Some(("authorization", &auth)),
            Some(&json!({ "places": batch })),
        )
        .await
        .map_err(CallErr::text)?;
        db.kv_set("mega_synced", &last.to_string())?;
        sent += n;
        if n < SYNC_BATCH {
            break;
        }
    }
    Ok(sent)
}

#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct MegaSearch {
    pub q: String,
    pub province: String,
    pub city: String,
    pub category: String,
    pub source: String,
    pub offset: i64,
    pub limit: i64,
}

pub async fn mega_search(db: &Db, s: &MegaSearch) -> Res<Value> {
    let (url, auth) = bearer(db)?;
    let mut s = s.clone();
    s.limit = s.limit.clamp(1, 200);
    call(&url, "POST", &format!("{API}/megabank/search"), Some(("authorization", &auth)), Some(&json!(s)))
        .await
        .map_err(CallErr::text)
}

/// One mega-bank row (as the server lists it) → a place in a local bank.
pub fn mega_row_to_place(list_id: i64, r: &Value) -> Place {
    let s = |k: &str| r[k].as_str().unwrap_or_default().trim().to_string();
    let mut contacts = vec![];
    for (field, kind) in [
        ("mobiles", "mobile"),
        ("landlines", "landline"),
        ("emails", "email"),
        ("instagram", "instagram"),
        ("telegram", "telegram"),
        ("whatsapp", "whatsapp"),
        ("eitaa", "eitaa"),
        ("bale", "bale"),
        ("rubika", "rubika"),
    ] {
        for v in s(field).split(',').map(str::trim).filter(|v| !v.is_empty()) {
            contacts.push(crate::db::Contact {
                kind: kind.into(),
                value: v.into(),
                origin: "megabank".into(),
                ..Default::default()
            });
        }
    }
    Place {
        list_id,
        place_key: format!("mega:{}", r["id"].as_i64().unwrap_or_default()),
        name: s("name"),
        website: s("website"),
        address: s("address"),
        province: s("province"),
        city: s("city"),
        category: s("category"),
        source: "megabank".into(),
        contacts,
        ..Default::default()
    }
}

/// Adds chosen mega-bank rows to a local bank. Returns (places added, contacts added).
pub fn mega_import(db: &Db, list_id: i64, rows: &[Value]) -> Res<(usize, usize)> {
    let (mut places, mut contacts) = (0, 0);
    for r in rows {
        let (new, n) = db.insert_place(&mega_row_to_place(list_id, r))?;
        places += new as usize;
        contacts += n;
    }
    Ok((places, contacts))
}

pub async fn notifications(db: &Db) -> Res<Value> {
    let (url, auth) = bearer(db)?;
    call(&url, "GET", &format!("{API}/notifications"), Some(("authorization", &auth)), None)
        .await
        .map_err(CallErr::text)
}

pub async fn notification_read(db: &Db, id: i64) -> Res<()> {
    let (url, auth) = bearer(db)?;
    call(
        &url,
        "POST",
        &format!("{API}/notifications/read"),
        Some(("authorization", &auth)),
        Some(&json!({ "id": id })),
    )
    .await
    .map(|_| ())
    .map_err(CallErr::text)
}

/// Latest app version the admin published (public endpoint).
pub async fn agent_version(db: &Db) -> Res<Value> {
    let url = kv(db, "cloud_url");
    if url.is_empty() {
        return Ok(Value::Null);
    }
    call(&url, "GET", &format!("{API}/agent/version"), None, None).await.map_err(CallErr::text)
}

/// Remote fixes for a source («سناریو»): stored locally and applied to the scanner.
pub async fn scenario_refresh(db: &Db, source: &str) -> Res<i64> {
    let url = kv(db, "cloud_url");
    if url.is_empty() || db.kv_get("cloud_token").is_none() {
        return Ok(0);
    }
    match call(&url, "GET", &format!("{API}/scenarios/{source}"), None, None).await {
        Ok(v) => {
            let ver = v["version"].as_i64().unwrap_or(0);
            db.kv_set(&format!("scenario_{source}"), &v["body"].to_string())?;
            crate::scanner::set_scenario(&v["body"]); // maps + balad + web-search scripts share one body
            Ok(ver)
        }
        Err(CallErr::Denied(404, _)) => {
            kv_del(db, &format!("scenario_{source}"));
            crate::scanner::set_scenario(&Value::Null);
            Ok(0)
        }
        Err(e) => Err(e.text()),
    }
}

/// At start-up: the last scenario received, so fixes work offline too.
pub fn scenario_load(db: &Db) {
    if db.kv_get("cloud_token").is_some() {
        if let Some(body) = db.kv_get("scenario_maps").and_then(|b| serde_json::from_str::<Value>(&b).ok()) {
            crate::scanner::set_scenario(&body);
        }
    }
}

/// Everything the background loop does: heartbeat, mega-bank upload, scenario. Errors are logged only.
pub async fn sync_all(db: &Db) -> Res<license::Status> {
    let st = heartbeat(db).await?;
    if st.valid {
        if let Err(e) = mega_sync(db).await {
            crate::log_line("WARN", &format!("mega sync: {e}"));
        }
        if let Err(e) = scenario_refresh(db, "maps").await {
            crate::log_line("WARN", &format!("scenario: {e}"));
        }
    }
    Ok(st)
}

/// Heartbeat is due when the last one is older than 6 h (token lifetime is 72 h).
pub fn sync_due(db: &Db) -> bool {
    db.kv_get("cloud_token").is_some() && license::now_secs() - kv(db, "cloud_synced").parse::<i64>().unwrap_or(0) > 6 * 3600
}

// ======================= admin panel =======================

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminConn {
    pub url: String,
    pub user: String,
    pub connected: bool,
    pub key_matches: Option<bool>, // server signs with the admin app's key (one license.pub for both)
    pub server_public_key: String,
    pub server_version: String,
    pub overview: Value,
}

pub fn admin_conn_info(db: &Db) -> AdminConn {
    AdminConn {
        url: kv(db, "admin_url"),
        user: kv(db, "admin_user"),
        connected: db.kv_get("admin_token").is_some(),
        key_matches: None,
        server_public_key: String::new(),
        server_version: String::new(),
        overview: Value::Null,
    }
}

async fn admin_login_raw(url: &str, user: &str, pass: &str) -> Result<String, CallErr> {
    let v = call(
        url,
        "POST",
        &format!("{API}/admin/login"),
        None,
        Some(&json!({ "username": user, "password": pass })),
    )
    .await?;
    v["token"].as_str().map(String::from).ok_or(CallErr::Network("پاسخ سرور نامعتبر است.".into()))
}

/// Logs in; the session token and password are kept DPAPI-sealed so the 12-hour session renews itself.
pub async fn admin_connect(db: &Db, url: &str, user: &str, pass: &str, local_public_key: &str) -> Res<AdminConn> {
    let url = normalize_url(url)?;
    let token = admin_login_raw(&url, user.trim(), pass).await.map_err(CallErr::text)?;
    db.kv_set("admin_url", &url)?;
    db.kv_set("admin_user", user.trim())?;
    seal(db, "admin_pass", pass)?;
    seal(db, "admin_token", &token)?;
    db.log("cloud", &format!("اتصال به سرور {url}"));
    admin_status(db, local_public_key).await
}

pub fn admin_disconnect(db: &Db) {
    for k in ["admin_token", "admin_pass"] {
        kv_del(db, k);
    }
    db.log("cloud", "قطع اتصال از سرور");
}

/// Connection check + overview + whether the server signs with our key.
pub async fn admin_status(db: &Db, local_public_key: &str) -> Res<AdminConn> {
    let mut info = admin_conn_info(db);
    if !info.connected {
        return Ok(info);
    }
    let ov = admin_call(db, "GET", "/overview", None).await?;
    info.server_public_key = ov["publicKey"].as_str().unwrap_or_default().to_string();
    info.server_version = ov["serverVersion"].as_str().unwrap_or_default().to_string();
    info.key_matches = Some(info.server_public_key.eq_ignore_ascii_case(local_public_key.trim()));
    info.overview = ov;
    Ok(info)
}

/// Admin-only paths: `/users`, `/plans/3`, … (relative to `/api/v1/admin`).
fn admin_path_ok(path: &str) -> bool {
    // traversal checks on the path itself; the query string may carry any (percent-encoded) search text
    let route = path.split('?').next().unwrap_or_default().to_ascii_lowercase();
    route.starts_with('/')
        && !route.contains("..")
        && !route.contains("//")
        && !route.contains('%')
        && path.chars().all(|c| c.is_ascii_alphanumeric() || "/-_?=&%.!'()*~".contains(c))
}

/// Authenticated admin request; renews an expired session once with the stored password.
pub async fn admin_call(db: &Db, method: &str, path: &str, body: Option<Value>) -> Res<Value> {
    if !["GET", "POST", "PUT", "DELETE"].contains(&method) || !admin_path_ok(path) {
        return Err("درخواست نامعتبر است.".into());
    }
    let url = kv(db, "admin_url");
    let token = unseal(db, "admin_token").ok_or("ابتدا به سرور وصل شوید (اتصال سرور).")?;
    let full = format!("{API}/admin{path}");
    match call(&url, method, &full, Some(("x-admin-token", &token)), body.as_ref()).await {
        Err(CallErr::Denied(401, msg)) => {
            let (Some(pass), user) = (unseal(db, "admin_pass"), kv(db, "admin_user")) else {
                kv_del(db, "admin_token");
                return Err(msg);
            };
            let token = match admin_login_raw(&url, &user, &pass).await {
                Ok(t) => t,
                Err(e) => {
                    kv_del(db, "admin_token");
                    return Err(e.text());
                }
            };
            seal(db, "admin_token", &token)?;
            call(&url, method, &full, Some(("x-admin-token", &token)), body.as_ref())
                .await
                .map_err(CallErr::text)
        }
        r => r.map_err(CallErr::text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_and_paths() {
        assert_eq!(normalize_url(" https://api.x.ir/ ").unwrap(), "https://api.x.ir");
        assert!(normalize_url("api.x.ir").is_err());
        assert!(normalize_url("ftp://x").is_err());
        assert!(normalize_url("https://").is_err());
        assert!(admin_path_ok("/users/3/devices"));
        assert!(admin_path_ok("/logs?kind=login&limit=50"));
        assert!(!admin_path_ok("users"));
        assert!(!admin_path_ok("/../auth/login"));
        assert!(!admin_path_ok("//evil.com/x"));
        assert!(!admin_path_ok("/users\r\nX: y"));
        assert!(!admin_path_ok("/%2e%2e/auth/login"));
        assert!(!admin_path_ok("/%2E%2E%2Fauth"));
        assert!(admin_path_ok("/users?q=%D8%A7"));
        assert!(admin_path_ok("/users?q=a%2Fb%2E%2E"), "encoded text in the query is fine");
        assert!(!admin_path_ok("/users%2F..%2Fx"));
        assert!(normalize_url("http://api.x.ir").is_err(), "no plain http on the internet");
        assert!(normalize_url("http://127.0.0.1@evil.com").is_err() && normalize_url("http://localhost.evil.com").is_err());
        assert!(normalize_url("http://127.0.0.1:8080").is_ok());
    }
}
