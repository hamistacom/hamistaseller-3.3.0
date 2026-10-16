//! Admin API (the web panel in phase 5-ب consumes this). Auth = `x-admin-token` from /admin/login.
use crate::error::{invalid, not_found, unauthorized, AppResult};
use crate::state::AppState;
use crate::{crypto, db, megabank};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/overview", get(overview))
        .route("/plans", get(plans_list).post(plan_save))
        .route("/plans/:id", axum::routing::delete(plan_delete))
        .route("/users", get(users_list).post(user_create))
        .route("/users/:id", put(user_update))
        .route("/users/:id/devices", get(user_devices))
        .route("/devices/:id/revoke", post(device_revoke))
        .route("/logs", get(logs))
        .route("/megabank/search", post(mega_search))
        .route("/megabank/facets", post(mega_facets))
        .route("/megabank/export", post(mega_export))
        .route("/notifications", get(notif_list).post(notif_create))
        .route("/version", get(version_get).put(version_set))
        .route("/scenarios", get(scenarios_list))
        .route("/scenarios/:source", put(scenario_set))
        .route("/optouts", get(optouts_get).post(optouts_set))
        .route("/trial", get(trial_get).put(trial_set))
}

fn admin(_state: &AppState, headers: &HeaderMap, c: &db::Conn) -> AppResult<i64> {
    let token = headers.get("x-admin-token").and_then(|v| v.to_str().ok()).ok_or_else(|| unauthorized("وارد پنل مدیر نشده‌اید."))?;
    c.query_row(
        "SELECT s.user_id FROM admin_sessions s JOIN users u ON u.id=s.user_id
         WHERE s.token=?1 AND s.expires_at>datetime('now') AND u.role='admin'",
        [token],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| unauthorized("نشست مدیر منقضی شده؛ دوباره وارد شوید."))
}

#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}
async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<Login>,
) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let from = crate::routes::client::ip(&headers, peer);
    // brute-force brake: 10 failures per IP in 15 minutes
    let fails: i64 = c.query_row(
        "SELECT count(*) FROM activity_log WHERE kind='admin_fail' AND ip=?1 AND at>=datetime('now','-15 minutes')",
        [&from],
        |r| r.get(0),
    )?;
    if fails >= 10 {
        return Err(crate::error::client(axum::http::StatusCode::TOO_MANY_REQUESTS, "تلاش ناموفق زیاد بود؛ ۱۵ دقیقه بعد دوباره امتحان کنید."));
    }
    let row: Option<(i64, String, String)> = c
        .query_row(
            "SELECT id,pass_hash,role FROM users WHERE username=?1 COLLATE NOCASE",
            [req.username.trim()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (id, hash, role) = match row {
        Some(v) => v,
        None => {
            crypto::verify_dummy(&req.password);
            db::log(&c, None, None, "admin_fail", req.username.trim(), &from);
            return Err(unauthorized("نام کاربری یا رمز نادرست است."));
        }
    };
    if role != "admin" || !crypto::verify_password(&req.password, &hash) {
        db::log(&c, None, None, "admin_fail", req.username.trim(), &from);
        return Err(unauthorized("نام کاربری یا رمز نادرست است."));
    }
    let token = crypto::random_token();
    let expires = (Utc::now() + Duration::hours(12)).format("%Y-%m-%d %H:%M:%S").to_string();
    c.execute("INSERT INTO admin_sessions(token,user_id,expires_at) VALUES(?1,?2,?3)", rusqlite::params![token, id, expires])?;
    c.execute("DELETE FROM admin_sessions WHERE expires_at<datetime('now')", [])?;
    db::log(&c, Some(id), None, "admin", "login", "");
    Ok(Json(json!({ "ok": true, "token": token, "username": req.username.trim() })))
}

async fn overview(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let n = |sql: &str| -> AppResult<i64> { Ok(c.query_row(sql, [], |r| r.get(0))?) };
    Ok(Json(json!({
        "ok": true,
        "users": n("SELECT count(*) FROM users WHERE role='customer'")?,
        "activeUsers": n("SELECT count(*) FROM users WHERE role='customer' AND status='active' AND (expires_at='' OR expires_at>=date('now'))")?,
        "devices": n("SELECT count(*) FROM devices WHERE revoked=0")?,
        "megabank": n("SELECT count(*) FROM megabank")?,
        "loginsToday": n("SELECT count(*) FROM activity_log WHERE kind='login' AND at>=date('now')")?,
        "newMegaToday": n("SELECT count(*) FROM megabank WHERE first_seen>=date('now')")?,
        "publicKey": state.signer.public_hex(),
        "serverVersion": env!("CARGO_PKG_VERSION")
    })))
}

// ---------------- plans ----------------

async fn plans_list(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let mut st = c.prepare(
        "SELECT id,name,max_devices,monthly_scan_quota,monthly_send_quota,allowed_sources,megabank_access,note,
           (SELECT count(*) FROM users WHERE users.plan_id=plans.id) FROM plans ORDER BY id",
    )?;
    let rows: Vec<Value> = st
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_,i64>(0)?, "name": r.get::<_,String>(1)?, "maxDevices": r.get::<_,i64>(2)?,
                "scanQuota": r.get::<_,i64>(3)?, "sendQuota": r.get::<_,i64>(4)?, "sources": r.get::<_,String>(5)?,
                "megabank": r.get::<_,i64>(6)? != 0, "note": r.get::<_,String>(7)?, "users": r.get::<_,i64>(8)?
            }))
        })?
        .collect::<Result<_, _>>()?;
    Ok(Json(json!({ "ok": true, "items": rows })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanIn {
    #[serde(default)]
    id: i64,
    name: String,
    #[serde(default = "one")]
    max_devices: i64,
    #[serde(default)]
    scan_quota: i64,
    #[serde(default)]
    send_quota: i64,
    #[serde(default)]
    sources: String,
    #[serde(default)]
    megabank: bool,
    #[serde(default)]
    note: String,
}
fn one() -> i64 {
    1
}
async fn plan_save(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<PlanIn>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    if p.name.trim().is_empty() {
        return Err(invalid("نام پلن را وارد کنید."));
    }
    let mega = p.megabank as i64;
    if p.id == 0 {
        c.execute(
            "INSERT INTO plans(name,max_devices,monthly_scan_quota,monthly_send_quota,allowed_sources,megabank_access,note)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![p.name.trim(), p.max_devices, p.scan_quota, p.send_quota, p.sources.trim(), mega, p.note.trim()],
        )?;
        Ok(Json(json!({ "ok": true, "id": c.last_insert_rowid() })))
    } else {
        c.execute(
            "UPDATE plans SET name=?1,max_devices=?2,monthly_scan_quota=?3,monthly_send_quota=?4,allowed_sources=?5,megabank_access=?6,note=?7 WHERE id=?8",
            rusqlite::params![p.name.trim(), p.max_devices, p.scan_quota, p.send_quota, p.sources.trim(), mega, p.note.trim(), p.id],
        )?;
        Ok(Json(json!({ "ok": true, "id": p.id })))
    }
}
async fn plan_delete(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let used: i64 = c.query_row("SELECT count(*) FROM users WHERE plan_id=?1", [id], |r| r.get(0))?;
    if used > 0 {
        return Err(invalid(format!("{used} کاربر روی این پلن هستند؛ ابتدا پلنشان را عوض کنید.")));
    }
    c.execute("DELETE FROM plans WHERE id=?1", [id])?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------- users ----------------

async fn users_list(State(state): State<AppState>, headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let search = q.get("q").cloned().unwrap_or_default();
    let like = format!("%{}%", search.trim());
    let mut st = c.prepare(
        "SELECT u.id,u.username,u.customer_name,u.status,u.expires_at,u.note,u.created_at,p.name,u.plan_id,
           (SELECT count(*) FROM devices d WHERE d.user_id=u.id AND d.revoked=0)
         FROM users u LEFT JOIN plans p ON p.id=u.plan_id
         WHERE u.role='customer' AND (?1='%%' OR u.username LIKE ?1 OR u.customer_name LIKE ?1)
         ORDER BY u.id DESC LIMIT 500",
    )?;
    let rows: Vec<Value> = st
        .query_map([&like], |r| {
            Ok(json!({
                "id": r.get::<_,i64>(0)?, "username": r.get::<_,String>(1)?, "customerName": r.get::<_,String>(2)?,
                "status": r.get::<_,String>(3)?, "expires": r.get::<_,String>(4)?, "note": r.get::<_,String>(5)?,
                "createdAt": r.get::<_,String>(6)?, "plan": r.get::<_,Option<String>>(7)?, "planId": r.get::<_,Option<i64>>(8)?,
                "devices": r.get::<_,i64>(9)?
            }))
        })?
        .collect::<Result<_, _>>()?;
    Ok(Json(json!({ "ok": true, "items": rows })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserIn {
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    customer_name: String,
    #[serde(default)]
    plan_id: Option<i64>,
    #[serde(default)]
    expires: String,
    #[serde(default)]
    note: String,
}
async fn user_create(State(state): State<AppState>, headers: HeaderMap, Json(u): Json<UserIn>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    if u.username.trim().len() < 3 || u.password.len() < 6 {
        return Err(invalid("نام کاربری حداقل ۳ و رمز حداقل ۶ کاراکتر باشد."));
    }
    let hash = crypto::hash_password(&u.password)?;
    let r = c.execute(
        "INSERT INTO users(username,pass_hash,role,customer_name,plan_id,expires_at,note) VALUES(?1,?2,'customer',?3,?4,?5,?6)",
        rusqlite::params![u.username.trim(), hash, u.customer_name.trim(), u.plan_id, u.expires.trim(), u.note.trim()],
    );
    match r {
        Ok(_) => Ok(Json(json!({ "ok": true, "id": c.last_insert_rowid() }))),
        Err(e) if e.to_string().contains("UNIQUE") => Err(invalid("این نام کاربری قبلاً ثبت شده است.")),
        Err(e) => Err(e.into()),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserPatch {
    customer_name: Option<String>,
    #[serde(default, deserialize_with = "some_or_null")]
    plan_id: Option<Option<i64>>, // absent = keep, null = no plan, n = plan n
    status: Option<String>,
    expires: Option<String>,
    note: Option<String>,
    new_password: Option<String>,
}
fn some_or_null<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<i64>>, D::Error> {
    Option::<i64>::deserialize(d).map(Some)
}

async fn user_update(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<i64>, Json(p): Json<UserPatch>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    let admin_id = admin(&state, &headers, &c)?;
    let exists: bool = c.query_row("SELECT 1 FROM users WHERE id=?1 AND role='customer'", [id], |_| Ok(true)).optional()?.is_some();
    if !exists {
        return Err(not_found("کاربر یافت نشد."));
    }
    if let Some(v) = &p.customer_name {
        c.execute("UPDATE users SET customer_name=?1 WHERE id=?2", rusqlite::params![v.trim(), id])?;
    }
    if let Some(v) = &p.plan_id {
        c.execute("UPDATE users SET plan_id=?1 WHERE id=?2", rusqlite::params![v, id])?;
    }
    if let Some(v) = &p.status {
        if !["active", "suspended"].contains(&v.as_str()) {
            return Err(invalid("وضعیت نامعتبر است."));
        }
        c.execute("UPDATE users SET status=?1 WHERE id=?2", rusqlite::params![v, id])?;
    }
    if let Some(v) = &p.expires {
        c.execute("UPDATE users SET expires_at=?1 WHERE id=?2", rusqlite::params![v.trim(), id])?;
    }
    if let Some(v) = &p.note {
        c.execute("UPDATE users SET note=?1 WHERE id=?2", rusqlite::params![v.trim(), id])?;
    }
    if let Some(pw) = &p.new_password {
        if pw.len() < 6 {
            return Err(invalid("رمز جدید حداقل ۶ کاراکتر باشد."));
        }
        c.execute("UPDATE users SET pass_hash=?1 WHERE id=?2", rusqlite::params![crypto::hash_password(pw)?, id])?;
    }
    db::log(&c, Some(admin_id), None, "admin", &format!("edit user {id}"), "");
    Ok(Json(json!({ "ok": true })))
}

async fn user_devices(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let mut st = c.prepare(
        "SELECT id,machine_id,name,app_version,last_ip,revoked,first_seen,last_seen FROM devices WHERE user_id=?1 ORDER BY last_seen DESC",
    )?;
    let rows: Vec<Value> = st
        .query_map([id], |r| {
            Ok(json!({
                "id": r.get::<_,i64>(0)?, "machineId": r.get::<_,String>(1)?, "name": r.get::<_,String>(2)?,
                "appVersion": r.get::<_,String>(3)?, "lastIp": r.get::<_,String>(4)?, "revoked": r.get::<_,i64>(5)? != 0,
                "firstSeen": r.get::<_,String>(6)?, "lastSeen": r.get::<_,String>(7)?
            }))
        })?
        .collect::<Result<_, _>>()?;
    Ok(Json(json!({ "ok": true, "items": rows })))
}

async fn device_revoke(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    c.execute("UPDATE devices SET revoked=1 WHERE id=?1", [id])?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------- logs ----------------

async fn logs(State(state): State<AppState>, headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let kind = q.get("kind").cloned().unwrap_or_default();
    let user = q.get("user").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    let limit: i64 = q.get("limit").and_then(|v| v.parse().ok()).unwrap_or(200).clamp(1, 2000);
    let mut st = c.prepare(
        "SELECT a.id,a.user_id,u.username,a.device_id,a.kind,a.detail,a.ip,a.at FROM activity_log a LEFT JOIN users u ON u.id=a.user_id
         WHERE (?1='' OR a.kind=?1) AND (?2=0 OR a.user_id=?2) ORDER BY a.id DESC LIMIT ?3",
    )?;
    let rows: Vec<Value> = st
        .query_map(rusqlite::params![kind, user, limit], |r| {
            Ok(json!({
                "id": r.get::<_,i64>(0)?, "userId": r.get::<_,Option<i64>>(1)?, "username": r.get::<_,Option<String>>(2)?,
                "deviceId": r.get::<_,Option<i64>>(3)?, "kind": r.get::<_,String>(4)?, "detail": r.get::<_,String>(5)?,
                "ip": r.get::<_,String>(6)?, "at": r.get::<_,String>(7)?
            }))
        })?
        .collect::<Result<_, _>>()?;
    Ok(Json(json!({ "ok": true, "items": rows })))
}

// ---------------- megabank ----------------

async fn mega_search(State(state): State<AppState>, headers: HeaderMap, Json(s): Json<megabank::Search>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let (rows, total) = megabank::search(&c, &s)?;
    Ok(Json(json!({ "ok": true, "rows": rows, "total": total })))
}

async fn mega_facets(State(state): State<AppState>, headers: HeaderMap, Json(s): Json<megabank::Search>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    Ok(Json(json!({
        "ok": true,
        "provinces": megabank::facets(&c, "province", &s)?,
        "cities": megabank::facets(&c, "city", &s)?,
        "categories": megabank::facets(&c, "category", &s)?,
        "sources": megabank::facets(&c, "source", &s)?
    })))
}

fn csv_cell(s: &str) -> String {
    let needs = s.starts_with(['=', '+', '-', '@']);
    let s = if needs { format!("'{s}") } else { s.to_string() };
    format!("\"{}\"", s.replace('"', "\"\""))
}

async fn mega_export(State(state): State<AppState>, headers: HeaderMap, Json(mut s): Json<megabank::Search>) -> AppResult<Response> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    s.limit = 100_000;
    s.offset = 0;
    let (rows, _total) = megabank::search(&c, &s)?;
    let head = ["نام", "موبایل", "تلفن ثابت", "ایمیل", "وب‌سایت", "اینستاگرام", "تلگرام", "واتساپ", "ایتا", "بله", "روبیکا", "آدرس", "استان", "شهر", "حوزه کاری", "منبع", "تعداد گزارش", "آخرین مشاهده"];
    let mut out = String::from("\u{feff}");
    out += &head.iter().map(|h| csv_cell(h)).collect::<Vec<_>>().join(",");
    out.push_str("\r\n");
    for r in &rows {
        let cells = [
            &r.name, &r.mobiles, &r.landlines, &r.emails, &r.website, &r.instagram, &r.telegram, &r.whatsapp, &r.eitaa, &r.bale, &r.rubika,
            &r.address, &r.province, &r.city, &r.category, &r.source,
        ];
        out += &cells.iter().map(|v| csv_cell(v)).collect::<Vec<_>>().join(",");
        out += &format!(",{},{}\r\n", csv_cell(&r.report_count.to_string()), csv_cell(&r.last_seen));
    }
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (axum::http::header::CONTENT_DISPOSITION, "attachment; filename=\"megabank.csv\""),
        ],
        out,
    )
        .into_response())
}

// ---------------- notifications ----------------

async fn notif_list(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let mut st = c.prepare("SELECT id,title,body,tag,audience,created_at FROM notifications ORDER BY id DESC LIMIT 200")?;
    let rows: Vec<Value> = st
        .query_map([], |r| {
            Ok(json!({ "id": r.get::<_,i64>(0)?, "title": r.get::<_,String>(1)?, "body": r.get::<_,String>(2)?,
                "tag": r.get::<_,String>(3)?, "audience": r.get::<_,String>(4)?, "createdAt": r.get::<_,String>(5)? }))
        })?
        .collect::<Result<_, _>>()?;
    Ok(Json(json!({ "ok": true, "items": rows })))
}

#[derive(Deserialize)]
struct NotifIn {
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default = "sys")]
    tag: String,
    #[serde(default = "all")]
    audience: String,
}
fn sys() -> String {
    "system".into()
}
fn all() -> String {
    "all".into()
}
async fn notif_create(State(state): State<AppState>, headers: HeaderMap, Json(n): Json<NotifIn>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    if n.title.trim().is_empty() {
        return Err(invalid("عنوان اعلان را وارد کنید."));
    }
    c.execute(
        "INSERT INTO notifications(title,body,tag,audience) VALUES(?1,?2,?3,?4)",
        rusqlite::params![n.title.trim(), n.body.trim(), n.tag.trim(), n.audience.trim()],
    )?;
    Ok(Json(json!({ "ok": true, "id": c.last_insert_rowid() })))
}

// ---------------- version & scenarios ----------------

async fn version_get(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let row: Option<(String, String, String, i64)> = c
        .query_row("SELECT version,url,notes,mandatory FROM versions WHERE channel='agent'", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()?;
    let (version, url, notes, mandatory) = row.unwrap_or_default();
    Ok(Json(json!({ "ok": true, "version": version, "url": url, "notes": notes, "mandatory": mandatory != 0 })))
}

#[derive(Deserialize)]
struct VersionIn {
    version: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    mandatory: bool,
}
async fn version_set(State(state): State<AppState>, headers: HeaderMap, Json(v): Json<VersionIn>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    c.execute(
        "INSERT INTO versions(channel,version,url,notes,mandatory,updated_at) VALUES('agent',?1,?2,?3,?4,datetime('now'))
         ON CONFLICT(channel) DO UPDATE SET version=?1,url=?2,notes=?3,mandatory=?4,updated_at=datetime('now')",
        rusqlite::params![v.version.trim(), v.url.trim(), v.notes.trim(), v.mandatory as i64],
    )?;
    Ok(Json(json!({ "ok": true })))
}

async fn scenarios_list(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let mut st = c.prepare("SELECT source,version,body,updated_at FROM scenarios ORDER BY source")?;
    let rows: Vec<Value> = st
        .query_map([], |r| {
            Ok(json!({ "source": r.get::<_,String>(0)?, "version": r.get::<_,i64>(1)?,
                "body": serde_json::from_str::<Value>(&r.get::<_,String>(2)?).unwrap_or(Value::Null), "updatedAt": r.get::<_,String>(3)? }))
        })?
        .collect::<Result<_, _>>()?;
    Ok(Json(json!({ "ok": true, "items": rows })))
}

#[derive(Deserialize)]
struct ScenarioIn {
    body: Value,
}
async fn scenario_set(State(state): State<AppState>, headers: HeaderMap, Path(source): Path<String>, Json(s): Json<ScenarioIn>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let body = serde_json::to_string(&s.body).map_err(|e| invalid(e.to_string()))?;
    c.execute(
        "INSERT INTO scenarios(source,version,body,updated_at) VALUES(?1,1,?2,datetime('now'))
         ON CONFLICT(source) DO UPDATE SET version=version+1,body=?2,updated_at=datetime('now')",
        rusqlite::params![source.trim(), body],
    )?;
    let version: i64 = c.query_row("SELECT version FROM scenarios WHERE source=?1", [source.trim()], |r| r.get(0))?;
    Ok(Json(json!({ "ok": true, "version": version })))
}

// ---------------- free trial settings ----------------

async fn trial_get(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let (on, plan, days) = crate::routes::client::trial_settings(&c);
    let used: i64 = c.query_row("SELECT count(*) FROM trials", [], |r| r.get(0))?;
    Ok(Json(json!({ "ok": true, "enabled": on, "planId": plan, "days": days, "used": used })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrialIn {
    enabled: bool,
    plan_id: Option<i64>,
    days: i64,
}
async fn trial_set(State(state): State<AppState>, headers: HeaderMap, Json(t): Json<TrialIn>) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    if t.enabled && t.plan_id.is_none() {
        return Err(invalid("برای دوره آزمایشی یک پلن انتخاب کنید."));
    }
    if let Some(id) = t.plan_id {
        // trial accounts are anonymous: never hand them the shared mega-bank
        let mega: Option<i64> = c.query_row("SELECT megabank_access FROM plans WHERE id=?1", [id], |r| r.get(0)).optional()?;
        match mega {
            None => return Err(invalid("پلن انتخاب‌شده وجود ندارد.")),
            Some(m) if m != 0 => return Err(invalid("پلن دوره آزمایشی نباید دسترسی بانک جامع داشته باشد.")),
            _ => {}
        }
    }
    let v = json!({ "enabled": t.enabled, "planId": t.plan_id, "days": t.days.clamp(1, 60) });
    db::kv_set(&c, "trial", &v.to_string())?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------- opt-outs (shared do-not-contact) ----------------

async fn optouts_get(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let mut st = c.prepare("SELECT value FROM optouts ORDER BY value")?;
    let rows: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(Json(json!({ "ok": true, "items": rows })))
}
#[derive(Deserialize)]
struct Optouts {
    values: Vec<String>,
}
async fn optouts_set(State(state): State<AppState>, headers: HeaderMap, Json(o): Json<Optouts>) -> AppResult<Json<Value>> {
    let mut c = state.pool.get()?;
    admin(&state, &headers, &c)?;
    let tx = c.transaction()?;
    for v in &o.values {
        let v = v.trim();
        if !v.is_empty() {
            tx.execute("INSERT OR IGNORE INTO optouts(value) VALUES(?1)", [v])?;
        }
    }
    tx.commit()?;
    Ok(Json(json!({ "ok": true, "added": o.values.len() })))
}
