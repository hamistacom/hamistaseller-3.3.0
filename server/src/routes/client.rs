//! Desktop-client API: login, licence heartbeat, quota, mega-bank ingest/search, notifications, agent info.
use crate::crypto::Claims;
use crate::error::{client, forbidden, invalid, not_found, unauthorized, AppError, AppResult};
use crate::state::AppState;
use crate::{crypto, db, megabank};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/trial", post(trial))
        .route("/trial", get(trial_info))
        .route("/license/heartbeat", post(heartbeat))
        .route("/quota/consume", post(quota_consume))
        .route("/megabank/ingest", post(ingest))
        .route("/megabank/search", post(mega_search))
        .route("/notifications", get(notifications))
        .route("/notifications/read", post(notifications_read))
        .route("/agent/version", get(agent_version))
        .route("/scenarios/:source", get(scenario))
}

/// Client IP for the logs. `X-Forwarded-For` is trusted only from a reverse proxy on the same machine (Nginx).
pub fn ip(headers: &HeaderMap, peer: SocketAddr) -> String {
    if !peer.ip().is_loopback() {
        return peer.ip().to_string();
    }
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| peer.ip().to_string())
}

fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

struct UserRow {
    id: i64,
    username: String,
    customer_name: String,
    status: String,
    expires_at: String,
    plan_id: Option<i64>,
}
struct PlanRow {
    name: String,
    max_devices: i64,
    scan_quota: i64,
    send_quota: i64,
    sources: Vec<String>,
    megabank: bool,
}

fn load_user(c: &db::Conn, username: &str) -> AppResult<Option<(UserRow, String, String)>> {
    // returns (user, pass_hash, role)
    c.query_row(
        "SELECT id,username,customer_name,status,expires_at,plan_id,pass_hash,role FROM users WHERE username=?1 COLLATE NOCASE",
        [username],
        |r| {
            Ok((
                UserRow { id: r.get(0)?, username: r.get(1)?, customer_name: r.get(2)?, status: r.get(3)?, expires_at: r.get(4)?, plan_id: r.get(5)? },
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
            ))
        },
    )
    .optional()
    .map_err(Into::into)
}

fn load_plan(c: &db::Conn, plan_id: Option<i64>) -> PlanRow {
    plan_id
        .and_then(|id| {
            c.query_row(
                "SELECT name,max_devices,monthly_scan_quota,monthly_send_quota,allowed_sources,megabank_access FROM plans WHERE id=?1",
                [id],
                |r| {
                    Ok(PlanRow {
                        name: r.get(0)?,
                        max_devices: r.get(1)?,
                        scan_quota: r.get(2)?,
                        send_quota: r.get(3)?,
                        sources: r.get::<_, String>(4)?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                        megabank: r.get::<_, i64>(5)? != 0,
                    })
                },
            )
            .ok()
        })
        .unwrap_or(PlanRow { name: "—".into(), max_devices: 1, scan_quota: 0, send_quota: 0, sources: vec![], megabank: false })
}

fn quota_left(c: &db::Conn, user_id: i64, plan: &PlanRow) -> (i64, i64) {
    let (scans, sends): (i64, i64) = c
        .query_row(
            "SELECT scans,sends FROM quota_usage WHERE user_id=?1 AND period=?2",
            rusqlite::params![user_id, db::period_now()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    let left = |quota: i64, used: i64| if quota == 0 { -1 } else { (quota - used).max(0) };
    (left(plan.scan_quota, scans), left(plan.send_quota, sends))
}

fn mint(state: &AppState, c: &db::Conn, u: &UserRow, plan: &PlanRow, machine: &str) -> AppResult<String> {
    let now = Utc::now().timestamp();
    let (scan_left, send_left) = quota_left(c, u.id, plan);
    state.signer.sign_token(&Claims {
        user: u.username.clone(),
        customer: u.customer_name.clone(),
        machine: machine.to_string(),
        plan: plan.name.clone(),
        iat: now,
        exp: now + state.cfg.token_ttl_hours * 3600,
        sub_expires: u.expires_at.clone(),
        sources: plan.sources.clone(),
        megabank: plan.megabank,
        scan_left,
        send_left,
    })
}

fn license_json(u: &UserRow, plan: &PlanRow, scan_left: i64, send_left: i64) -> Value {
    json!({
        "customer": u.customer_name, "plan": plan.name, "status": u.status, "expires": u.expires_at,
        "sources": plan.sources, "megabank": plan.megabank, "scanLeft": scan_left, "sendLeft": send_left,
        "maxDevices": plan.max_devices
    })
}

fn ensure_usable(u: &UserRow) -> AppResult<()> {
    if u.status != "active" {
        return Err(forbidden("حساب شما موقتاً غیرفعال شده است. با پشتیبانی هامیستا در تماس باشید."));
    }
    if !u.expires_at.is_empty() && u.expires_at.as_str() < today().as_str() {
        return Err(forbidden(format!("اعتبار اشتراک شما در {} تمام شده است. برای تمدید اقدام کنید.", u.expires_at)));
    }
    Ok(())
}

// ---------------- login ----------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginReq {
    username: String,
    password: String,
    machine_id: String,
    #[serde(default)]
    machine_name: String,
    #[serde(default)]
    app_version: String,
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginReq>,
) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let ipaddr = ip(&headers, peer);
    let machine = crypto::machine_code(&req.machine_id);
    let (u, pass_hash, role) = match load_user(&c, req.username.trim())? {
        Some(v) => v,
        None => {
            crypto::verify_dummy(&req.password); // constant-ish time: don't reveal which usernames exist
            return Err(unauthorized("نام کاربری یا رمز عبور نادرست است."));
        }
    };
    if !crypto::verify_password(&req.password, &pass_hash) {
        db::log(&c, Some(u.id), None, "login", "failed password", &ipaddr);
        return Err(unauthorized("نام کاربری یا رمز عبور نادرست است."));
    }
    if role != "customer" {
        return Err(forbidden("این حساب فقط از پنل مدیر وارد می‌شود."));
    }
    ensure_usable(&u)?;
    let plan = load_plan(&c, u.plan_id);

    // register / update device, enforcing the plan's device cap
    let existing: Option<(i64, bool)> = c
        .query_row(
            "SELECT id,revoked FROM devices WHERE user_id=?1 AND machine_id=?2",
            rusqlite::params![u.id, machine],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)),
        )
        .optional()?;
    let device_id = match existing {
        Some((_, true)) => return Err(forbidden("این دستگاه مسدود شده است.")),
        Some((id, false)) => {
            c.execute(
                "UPDATE devices SET name=?1,app_version=?2,last_ip=?3,last_seen=datetime('now') WHERE id=?4",
                rusqlite::params![req.machine_name.trim(), req.app_version.trim(), ipaddr, id],
            )?;
            id
        }
        None => {
            let active: i64 = c.query_row("SELECT count(*) FROM devices WHERE user_id=?1 AND revoked=0", [u.id], |r| r.get(0))?;
            if active >= plan.max_devices {
                return Err(forbidden(format!(
                    "سقف تعداد دستگاه این اشتراک ({}) پر شده است. یکی از دستگاه‌ها را از پشتیبانی حذف کنید.",
                    plan.max_devices
                )));
            }
            c.execute(
                "INSERT INTO devices(user_id,machine_id,name,app_version,last_ip) VALUES(?1,?2,?3,?4,?5)",
                rusqlite::params![u.id, machine, req.machine_name.trim(), req.app_version.trim(), ipaddr],
            )?;
            c.last_insert_rowid()
        }
    };
    db::log(&c, Some(u.id), Some(device_id), "login", &format!("v{}", req.app_version.trim()), &ipaddr);
    let token = mint(&state, &c, &u, &plan, &machine)?;
    let (sl, dl) = quota_left(&c, u.id, &plan);
    Ok(Json(json!({
        "ok": true, "token": token, "publicKey": state.signer.public_hex(),
        "machine": machine, "serverTime": Utc::now().timestamp(),
        "license": license_json(&u, &plan, sl, dl)
    })))
}

// ---------------- free trial (one per PC) ----------------

/// Trial settings the admin chose: (enabled, plan id, days).
pub fn trial_settings(c: &db::Conn) -> (bool, Option<i64>, i64) {
    let v: Value = db::kv_get(c, "trial").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    (v["enabled"].as_bool().unwrap_or(false), v["planId"].as_i64(), v["days"].as_i64().unwrap_or(3).clamp(1, 60))
}

/// Public: is a trial on offer, and for how many days (shown before the user asks for one).
async fn trial_info(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let (on, plan, days) = trial_settings(&c);
    Ok(Json(json!({ "ok": true, "enabled": on && plan.is_some(), "days": days })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrialReq {
    machine_id: String,
    #[serde(default)]
    machine_name: String,
    #[serde(default)]
    app_version: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    phone: String,
}

/// Creates a short account for this PC (once ever) and signs it in like `login`.
async fn trial(State(state): State<AppState>, headers: HeaderMap, ConnectInfo(peer): ConnectInfo<SocketAddr>, Json(req): Json<TrialReq>) -> AppResult<Json<Value>> {
    let mut c = state.pool.get()?;
    let ipaddr = ip(&headers, peer);
    let (on, plan_id, days) = trial_settings(&c);
    let Some(plan_id) = plan_id.filter(|_| on) else {
        return Err(forbidden("دوره آزمایشی در حال حاضر ارائه نمی‌شود؛ با پشتیبانی هامیستا تماس بگیرید."));
    };
    let phone: String = req.phone.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if req.name.trim().chars().count() < 2 || !(10..=13).contains(&phone.len()) {
        return Err(invalid("نام و شماره موبایل را درست وارد کنید."));
    }
    if req.machine_id.trim().is_empty() {
        return Err(invalid("کد سیستم ارسال نشده است."));
    }
    // a few trial requests per address per day, so the free plan isn't farmed
    let today_n: i64 = c.query_row("SELECT count(*) FROM activity_log WHERE kind='trial' AND ip=?1 AND at>=date('now')", [&ipaddr], |r| r.get(0))?;
    if today_n >= 5 {
        return Err(client(axum::http::StatusCode::TOO_MANY_REQUESTS, "درخواست آزمایشی زیادی از این شبکه ثبت شده است؛ فردا دوباره امتحان کنید."));
    }
    let machine = crypto::machine_code(&req.machine_id);
    let username = format!("trial-{}", machine.replace('-', "").to_lowercase());
    let expires = (Utc::now() + chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let tx = c.transaction()?;
    let used = || forbidden("این سیستم یا این شماره قبلاً از دوره آزمایشی استفاده کرده است. برای خرید اشتراک با پشتیبانی هامیستا تماس بگیرید.");
    if tx.query_row("SELECT 1 FROM trials WHERE machine_id=?1 OR phone=?2", [&machine, &phone], |_| Ok(())).optional()?.is_some() {
        return Err(used());
    }
    let hash = crypto::hash_password(&crypto::random_token())?; // nobody signs in with it: the token is refreshed by heartbeat
    tx.execute(
        "INSERT INTO users(username,pass_hash,role,customer_name,plan_id,expires_at,note) VALUES(?1,?2,'customer',?3,?4,?5,?6)",
        rusqlite::params![username, hash, req.name.trim(), plan_id, expires, format!("آزمایشی · {phone}")],
    )
    .map_err(|_| used())?; // same machine twice at once, or a leftover trial user

    let uid = tx.last_insert_rowid();
    tx.execute("INSERT INTO trials(machine_id,user_id,phone) VALUES(?1,?2,?3)", rusqlite::params![machine, uid, phone]).map_err(|_| used())?;
    tx.execute(
        "INSERT INTO devices(user_id,machine_id,name,app_version,last_ip) VALUES(?1,?2,?3,?4,?5)",
        rusqlite::params![uid, machine, req.machine_name.trim(), req.app_version.trim(), ipaddr],
    )?;
    tx.commit()?;
    db::log(&c, Some(uid), None, "trial", &format!("{days} days"), &ipaddr);
    let (u, _, _) = load_user(&c, &username)?.ok_or_else(|| unauthorized("حساب ساخته نشد."))?;
    let plan = load_plan(&c, u.plan_id);
    let token = mint(&state, &c, &u, &plan, &machine)?;
    let (sl, dl) = quota_left(&c, u.id, &plan);
    Ok(Json(json!({
        "ok": true, "token": token, "publicKey": state.signer.public_hex(), "username": username,
        "machine": machine, "serverTime": Utc::now().timestamp(), "license": license_json(&u, &plan, sl, dl)
    })))
}

// ---------------- authenticated requests ----------------

/// Verifies the bearer token's signature and that the account/device are still good.
/// `require_fresh` also enforces the token's expiry (heartbeat refreshes before that).
fn auth(state: &AppState, headers: &HeaderMap, c: &db::Conn, require_fresh: bool) -> AppResult<(Claims, UserRow, i64, PlanRow)> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| unauthorized("توکن ارسال نشده است."))?;
    let claims = crypto::verify_token(&state.signer.public_hex(), token)?;
    let now = Utc::now().timestamp();
    if require_fresh && claims.exp < now {
        return Err(client(axum::http::StatusCode::UNAUTHORIZED, "اعتبار نشست تمام شده؛ دوباره همگام‌سازی کنید."));
    }
    // bound how old a token used for heartbeat may be (stops reuse of ancient tokens)
    if claims.iat < now - 90 * 24 * 3600 {
        return Err(unauthorized("توکن بسیار قدیمی است؛ دوباره وارد شوید."));
    }
    let (u, _hash, _role) = load_user(c, &claims.user)?.ok_or_else(|| unauthorized("حساب یافت نشد."))?;
    ensure_usable(&u)?;
    let device_id: i64 = c
        .query_row(
            "SELECT id FROM devices WHERE user_id=?1 AND machine_id=?2 AND revoked=0",
            rusqlite::params![u.id, claims.machine],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| forbidden("این دستگاه دیگر مجاز نیست."))?;
    let plan = load_plan(c, u.plan_id);
    Ok((claims, u, device_id, plan))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Heartbeat {
    #[serde(default)]
    app_version: String,
}

async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(req): Json<Heartbeat>,
) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let (_claims, u, device_id, plan) = auth(&state, &headers, &c, false)?;
    let machine: String = c.query_row("SELECT machine_id FROM devices WHERE id=?1", [device_id], |r| r.get(0))?;
    c.execute(
        "UPDATE devices SET last_seen=datetime('now'), last_ip=?1, app_version=COALESCE(NULLIF(?2,''),app_version) WHERE id=?3",
        rusqlite::params![ip(&headers, peer), req.app_version.trim(), device_id],
    )?;
    let token = mint(&state, &c, &u, &plan, &machine)?;
    let (sl, dl) = quota_left(&c, u.id, &plan);
    Ok(Json(json!({ "ok": true, "token": token, "serverTime": Utc::now().timestamp(), "license": license_json(&u, &plan, sl, dl) })))
}

// ---------------- quota ----------------

#[derive(Deserialize)]
struct QuotaReq {
    kind: String, // scan | send
    #[serde(default = "one")]
    n: i64,
}
fn one() -> i64 {
    1
}

async fn quota_consume(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<QuotaReq>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let (_claims, u, device_id, plan) = auth(&state, &headers, &c, true)?;
    let n = req.n.clamp(0, 1_000_000);
    let (col, quota) = match req.kind.as_str() {
        "scan" => ("scans", plan.scan_quota),
        "send" => ("sends", plan.send_quota),
        _ => return Err(invalid("نوع سهمیه نامعتبر است.")),
    };
    let period = db::period_now();
    c.execute(
        &format!("INSERT INTO quota_usage(user_id,period,{col}) VALUES(?1,?2,0) ON CONFLICT(user_id,period) DO NOTHING"),
        rusqlite::params![u.id, period],
    )?;
    // one atomic conditional UPDATE: ?4=0 means unlimited, else only succeeds while under cap.
    // A single statement on one connection can't be raced by concurrent requests (no TOCTOU).
    let changed = c.execute(
        &format!("UPDATE quota_usage SET {col}={col}+?1 WHERE user_id=?2 AND period=?3 AND (?4=0 OR {col}+?1<=?4)"),
        rusqlite::params![n, u.id, period, quota],
    )?;
    if changed == 0 {
        // over the cap: count up to the cap, so use reported after the fact is never lost
        c.execute(
            &format!("UPDATE quota_usage SET {col}=?1 WHERE user_id=?2 AND period=?3 AND {col}<?1"),
            rusqlite::params![quota, u.id, period],
        )?;
        db::log(&c, Some(u.id), Some(device_id), "quota", &format!("{} cap reached (+{n} refused)", req.kind), "");
        return Err(client(
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            format!("سهمیه این ماه ({quota}) پر شده است. ماه آینده صفر می‌شود یا اشتراک را ارتقا دهید."),
        ));
    }
    db::log(&c, Some(u.id), Some(device_id), if req.kind == "scan" { "scan" } else { "campaign" }, &format!("+{n}"), "");
    let (sl, dl) = quota_left(&c, u.id, &plan);
    Ok(Json(json!({ "ok": true, "scanLeft": sl, "sendLeft": dl })))
}

// ---------------- mega-bank ----------------

#[derive(Deserialize)]
struct IngestReq {
    places: Vec<megabank::InPlace>,
}

async fn ingest(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<IngestReq>) -> AppResult<Json<Value>> {
    let mut c = state.pool.get()?;
    let (_claims, u, device_id, _plan) = auth(&state, &headers, &c, false)?;
    if req.places.len() > 5000 {
        return Err(invalid("هر بار حداکثر ۵۰۰۰ مورد ارسال کنید."));
    }
    // ponytail: request count per day (≤ 200 × 5000 rows); per-row quotas if abuse shows up
    let today_n: i64 = c.query_row(
        "SELECT count(*) FROM activity_log WHERE user_id=?1 AND kind='ingest' AND at>=date('now')",
        [u.id],
        |r| r.get(0),
    )?;
    if today_n >= 200 {
        return Err(client(axum::http::StatusCode::TOO_MANY_REQUESTS, "سقف ارسال روزانه به بانک جامع پر شده است."));
    }
    let tx = c.transaction()?;
    let (mut added, mut merged) = (0i64, 0i64);
    {
        // tx derefs to Connection; upsert takes &Conn via a thin shim
        for p in &req.places {
            if megabank::upsert(&tx, p)? {
                added += 1;
            } else {
                merged += 1;
            }
        }
    }
    tx.commit()?;
    let c = state.pool.get()?;
    db::log(&c, Some(u.id), Some(device_id), "ingest", &format!("+{added} new / {merged} merged"), "");
    Ok(Json(json!({ "ok": true, "added": added, "merged": merged })))
}

async fn mega_search(State(state): State<AppState>, headers: HeaderMap, Json(s): Json<megabank::Search>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let (_claims, _u, _dev, plan) = auth(&state, &headers, &c, true)?;
    if !plan.megabank {
        return Err(forbidden("دسترسی به بانک جامع در اشتراک شما فعال نیست."));
    }
    let (rows, total) = megabank::search(&c, &s)?;
    Ok(Json(json!({ "ok": true, "rows": rows, "total": total })))
}

// ---------------- notifications ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Notif {
    id: i64,
    title: String,
    body: String,
    tag: String,
    created_at: String,
    read: bool,
}

async fn notifications(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let (_claims, u, _dev, _plan) = auth(&state, &headers, &c, false)?;
    let plan_aud = u.plan_id.map(|id| format!("plan:{id}")).unwrap_or_default();
    let user_aud = format!("user:{}", u.id);
    let mut st = c.prepare(
        "SELECT n.id,n.title,n.body,n.tag,n.created_at, EXISTS(SELECT 1 FROM notification_reads r WHERE r.user_id=?1 AND r.notification_id=n.id)
         FROM notifications n WHERE n.audience='all' OR n.audience=?2 OR n.audience=?3 ORDER BY n.id DESC LIMIT 100",
    )?;
    let rows = st
        .query_map(rusqlite::params![u.id, plan_aud, user_aud], |r| {
            Ok(Notif { id: r.get(0)?, title: r.get(1)?, body: r.get(2)?, tag: r.get(3)?, created_at: r.get(4)?, read: r.get::<_, i64>(5)? != 0 })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!({ "ok": true, "items": rows })))
}

#[derive(Deserialize)]
struct ReadReq {
    id: i64,
}
async fn notifications_read(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<ReadReq>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let (_claims, u, _dev, _plan) = auth(&state, &headers, &c, false)?;
    c.execute(
        "INSERT OR IGNORE INTO notification_reads(user_id,notification_id) VALUES(?1,?2)",
        rusqlite::params![u.id, req.id],
    )?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------- agent version & scenarios (public) ----------------

async fn agent_version(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let row: Option<(String, String, String, i64)> = c
        .query_row("SELECT version,url,notes,mandatory FROM versions WHERE channel='agent'", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()?;
    let (version, url, notes, mandatory) = row.unwrap_or((state.cfg.agent_min_version.clone(), String::new(), String::new(), 0));
    Ok(Json(json!({ "ok": true, "version": version, "url": url, "notes": notes, "mandatory": mandatory != 0, "minVersion": state.cfg.agent_min_version })))
}

async fn scenario(State(state): State<AppState>, Path(source): Path<String>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let row: Option<(i64, String, String)> = c
        .query_row("SELECT version,body,updated_at FROM scenarios WHERE source=?1", [&source], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?;
    match row {
        Some((version, body, updated_at)) => Ok(Json(json!({
            "ok": true, "source": source, "version": version,
            "body": serde_json::from_str::<Value>(&body).unwrap_or(Value::Null), "updatedAt": updated_at
        }))),
        None => Err(not_found("برای این منبع سناریویی ثبت نشده است.")),
    }
}

// let AppError satisfy `?` on serde_json in a couple of spots
impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        invalid(format!("JSON نامعتبر: {e}"))
    }
}
