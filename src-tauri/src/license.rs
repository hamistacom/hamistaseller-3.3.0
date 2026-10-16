//! Offline licences: Ed25519-signed, bound to one PC, with an expiry date.
//! Three editions from one codebase:
//!   * customer (default)            — needs a licence; can only verify.
//!   * master   (`--features master`) — your own full copy: no licence, no issuing.
//!   * issuer   (`--features issuer`) — the admin panel: offline licence issuing + online user management.
//!
//! A customer copy is activated one of two ways (hybrid):
//!   * offline: an `HS2.` code signed by the admin app for this PC (no internet needed);
//!   * online:  an account on the admin's server; the server returns a short-lived `HC1.` token signed with
//!     the SAME key (the admin pastes it into the server once), refreshed by a heartbeat. Remote suspend,
//!     device control, quotas and the shared bank only exist in online mode.
use crate::db::{err, Db, Res};
use crate::logic::{licence_code, licence_payload, machine_code, parse_licence, reg_value, valid_date, LicenceInfo};
use crate::secrets;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(all(feature = "master", feature = "issuer"))]
compile_error!("build one edition at a time: --features master  OR  --features issuer");

pub const MASTER: bool = cfg!(feature = "master");
pub const ISSUER: bool = cfg!(feature = "issuer");
pub const EDITION: &str = if MASTER {
    "master"
} else if ISSUER {
    "issuer"
} else {
    "customer"
};

/// Public half of your signing key, pasted here once (see README «لایسنس»).
#[cfg(not(feature = "testkey"))]
const PUBLIC_KEY_HEX: &str = include_str!("../license.pub");
/// Test harness only: a throwaway key whose private half the tests hold.
#[cfg(feature = "testkey")]
const PUBLIC_KEY_HEX: &str = include_str!("../../tests-harness/test-key.pub");

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub edition: &'static str, // master | customer | issuer
    pub valid: bool,
    pub machine: String,
    pub customer: String,
    pub plan: String,
    pub expires: String,
    pub days_left: i64,
    pub message: String,
    pub mode: &'static str, // "" | offline | online
    // online mode only
    pub server: String,
    pub user: String,
    pub sources: Vec<String>, // [] = every source
    pub megabank: bool,
    pub scan_left: i64, // -1 = unlimited
    pub send_left: i64,
    pub synced_at: i64, // unix secs of the last heartbeat
}

/// Claims inside an online `HC1.` token (same shape the server signs).
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct CloudClaims {
    pub user: String,
    pub customer: String,
    pub machine: String,
    pub plan: String,
    pub iat: i64,
    pub exp: i64,
    pub sub_expires: String,
    pub sources: Vec<String>,
    pub megabank: bool,
    pub scan_left: i64,
    pub send_left: i64,
}

/// Server URL baked into the build (src-tauri/server.url; empty = the customer types it).
pub const DEFAULT_SERVER: &str = include_str!("../server.url");

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The machine id the server binds tokens to: its `machine_code()` of our local code.
pub fn cloud_machine(local: &str) -> String {
    let digest = Sha256::digest(format!("hamista-cloud|{}", local.trim().to_lowercase()).as_bytes());
    machine_code(&hex(&digest[..8]))
}

/// Signature + machine check of an online token. Expiry is judged by the caller (it needs the clock).
pub fn verify_cloud(token: &str, machine: &str) -> Result<CloudClaims, String> {
    let key = hex32(PUBLIC_KEY_HEX).ok_or("کلید عمومی لایسنس در این نسخه تنظیم نشده است.")?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| "کلید عمومی لایسنس نامعتبر است.")?;
    let mut parts = token.trim().splitn(3, '.');
    let (Some("HC1"), Some(p), Some(sg)) = (parts.next(), parts.next(), parts.next()) else {
        return Err("توکن سرور نامعتبر است.".into());
    };
    let payload = crate::logic::b64_decode(p).ok_or("توکن سرور نامعتبر است.")?;
    let sig: [u8; 64] = crate::logic::b64_decode(sg).and_then(|v| v.try_into().ok()).ok_or("توکن سرور نامعتبر است.")?;
    key.verify(&payload, &Signature::from_bytes(&sig))
        .map_err(|_| "امضای سرور با کلید این نسخه نمی‌خواند. کلید خصوصی پنل مدیر باید روی سرور تنظیم شده باشد (راهنمای سرور).".to_string())?;
    let c: CloudClaims = serde_json::from_slice(&payload).map_err(|_| "توکن سرور نامعتبر است.")?;
    if c.machine != cloud_machine(machine) {
        return Err("این توکن برای سیستم دیگری صادر شده است.".into());
    }
    Ok(c)
}

fn online_status(db: &Db, token: &str, mut st: Status) -> Status {
    st.mode = "online";
    st.server = db.kv_get("cloud_url").unwrap_or_default();
    st.user = db.kv_get("cloud_user").unwrap_or_default();
    st.synced_at = db.kv_get("cloud_synced").and_then(|v| v.parse().ok()).unwrap_or(0);
    let c = match verify_cloud(token, &st.machine) {
        Ok(c) => c,
        Err(e) => {
            st.message = e;
            return st;
        }
    };
    let now = now_secs();
    // the server sets the time: a clock a day behind the newest time seen (token or PC) was turned back
    let seen = db.kv_get("cloud_seen").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0).max(c.iat);
    if now > seen {
        let _ = db.kv_set("cloud_seen", &now.to_string());
    }
    if now < seen - 86_400 {
        st.message = "تاریخ سیستم به عقب برگشته است؛ ساعت و تاریخ ویندوز را درست کنید.".into();
        return st;
    }
    (st.customer, st.plan, st.expires) = (c.customer.clone(), c.plan.clone(), c.sub_expires.clone());
    (st.sources, st.megabank, st.scan_left, st.send_left) = (c.sources, c.megabank, c.scan_left, c.send_left);
    if now > c.exp {
        st.message = "مدت کار بدون اینترنت تمام شده است. به اینترنت وصل شوید و «همگام‌سازی» را بزنید.".into();
        return st;
    }
    let today = db.today();
    if !c.sub_expires.is_empty() && c.sub_expires < today {
        st.message = format!("اعتبار اشتراک در تاریخ {} تمام شده است.", c.sub_expires);
        return st;
    }
    st.valid = true;
    st.days_left = if c.sub_expires.is_empty() {
        9999
    } else {
        db.days_between(&today, &c.sub_expires)
    };
    if st.days_left <= 7 {
        st.message = format!("{} روز تا پایان اشتراک", st.days_left);
    }
    st
}

/// Online: the source must be in the plan (an empty list = all). Offline/master: everything.
pub fn source_allowed(st: &Status, source: &str) -> Res<()> {
    if st.mode != "online" || st.sources.is_empty() || st.sources.iter().any(|s| s == source) {
        Ok(())
    } else {
        Err("این منبع در اشتراک شما فعال نیست.".into())
    }
}

/// Stable per-PC code shown to the customer, e.g. `3F25-04E0-4F89-11D3`.
pub fn machine() -> String {
    let guid = machine_guid().unwrap_or_else(|| "unknown-machine".into());
    let digest = Sha256::digest(format!("hamista-seller|{}", guid.trim().to_lowercase()).as_bytes());
    machine_code(&digest.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

fn machine_guid() -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let out = std::process::Command::new("reg")
            .args(["query", r"HKLM\SOFTWARE\Microsoft\Cryptography", "/v", "MachineGuid", "/reg:64"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        reg_value(&String::from_utf8_lossy(&out.stdout), "MachineGuid")
    }
    #[cfg(not(windows))]
    {
        let _ = reg_value;
        std::fs::read_to_string("/etc/machine-id").ok()
    }
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v: Vec<u8> = (0..32).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect::<Option<_>>()?;
    v.try_into().ok()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Checks a licence code against this PC, the embedded public key and today's date (`YYYY-MM-DD`).
pub fn verify(code: &str, machine: &str, today: &str) -> Result<LicenceInfo, String> {
    let key = hex32(PUBLIC_KEY_HEX).ok_or("کلید عمومی لایسنس در این نسخه تنظیم نشده است.")?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| "کلید عمومی لایسنس نامعتبر است.")?;
    let (payload, sig, info) = parse_licence(code).ok_or("کد لایسنس نامعتبر است.")?;
    key.verify(&payload, &Signature::from_bytes(&sig))
        .map_err(|_| "کد لایسنس معتبر نیست یا دست‌کاری شده است.")?;
    if info.machine != machine {
        return Err(format!("این لایسنس برای سیستم دیگری ({}) صادر شده است.", info.machine));
    }
    if info.expires.as_str() < today {
        return Err(format!("اعتبار لایسنس در تاریخ {} تمام شده است.", info.expires));
    }
    Ok(info)
}

/// Current status; also guards against the clock being turned back.
pub fn status(db: &Db) -> Status {
    let machine = machine();
    if MASTER || ISSUER {
        return Status {
            edition: EDITION,
            valid: MASTER, // the issuer app never scans or sends, so `ensure` keeps refusing there
            machine,
            message: if MASTER {
                "نسخه کامل مدیر — بدون نیاز به لایسنس"
            } else {
                "نسخه صدور لایسنس"
            }
            .into(),
            ..Default::default()
        };
    }
    let st = Status {
        edition: "customer",
        machine: machine.clone(),
        ..Default::default()
    };
    // online account first; if it is not usable, a valid offline code the admin issued still works
    let online = if let Some(token) = db.kv_get("cloud_token") {
        Some(online_status(db, &token, st.clone()))
    } else {
        db.kv_get("cloud_error").map(|e| Status {
            mode: "online",
            message: e, // the server ended the session (suspended / expired / device removed)
            ..st.clone()
        })
    };
    if let Some(o) = &online {
        if o.valid || db.kv_get("license_code").is_none() {
            return o.clone();
        }
    }
    let off = offline_status(db, st);
    match online {
        Some(o) if !off.valid => o,
        _ => off,
    }
}

fn offline_status(db: &Db, mut st: Status) -> Status {
    st.mode = "offline";
    let Some(code) = db.kv_get("license_code") else {
        st.message = "برنامه فعال نشده است.".into();
        return st;
    };
    let today = db.today();
    let seen = db.kv_get("license_seen").unwrap_or_default();
    if !seen.is_empty() && today < db.date_shift(&seen, -1) {
        st.message = "تاریخ سیستم به عقب برگشته است؛ ساعت و تاریخ ویندوز را درست کنید.".into();
        return st;
    }
    if today > seen {
        let _ = db.kv_set("license_seen", &today);
    }
    match verify(&code, &st.machine, &today) {
        Ok(info) => {
            st.valid = true;
            st.days_left = db.days_between(&today, &info.expires);
            st.message = if st.days_left <= 7 {
                format!("{} روز تا پایان اعتبار لایسنس", st.days_left)
            } else {
                String::new()
            };
            (st.customer, st.plan, st.expires) = (info.customer, info.plan, info.expires);
        }
        Err(e) => st.message = e,
    }
    st
}

pub fn activate(db: &Db, code: &str) -> Res<Status> {
    verify(code, &machine(), &db.today())?;
    db.kv_set("license_code", code.trim())?;
    crate::cloud::forget_session(db); // an offline code replaces an online account
    Ok(status(db))
}

/// Backend gate for scans, campaigns and exports (the UI gate alone could be bypassed).
pub fn ensure(db: &Db) -> Res<()> {
    let st = status(db);
    if st.valid {
        Ok(())
    } else {
        Err(format!("{} برای استفاده از این بخش، لایسنس معتبر لازم است.", st.message))
    }
}

// ---------------- issuer edition: key management and issuing ----------------

fn signing_key(db: &Db) -> Res<SigningKey> {
    if let Some(sealed) = db.kv_get("license_signing_key") {
        let raw = crate::logic::b64_decode(&sealed).ok_or("کلید خصوصی خراب است.")?;
        let key = hex32(&secrets::open_str(&raw)?).ok_or("کلید خصوصی خراب است.")?;
        return Ok(SigningKey::from_bytes(&key));
    }
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(err)?;
    store_key(db, &seed)?;
    Ok(SigningKey::from_bytes(&seed))
}

fn store_key(db: &Db, seed: &[u8; 32]) -> Res<()> {
    let sealed = secrets::seal_str(&hex(seed))?;
    db.kv_set("license_signing_key", &crate::logic::b64_encode(&sealed))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub public_key: String,
    pub embedded_matches: bool, // the public key compiled into this build is the same one
}

pub fn key_info(db: &Db) -> Res<KeyInfo> {
    let public_key = hex(&signing_key(db)?.verifying_key().to_bytes());
    Ok(KeyInfo {
        embedded_matches: PUBLIC_KEY_HEX.trim().eq_ignore_ascii_case(&public_key),
        public_key,
    })
}

pub fn issue(db: &Db, machine: &str, customer: &str, plan: &str, expires: &str) -> Res<String> {
    let machine = machine.trim().to_uppercase();
    if machine.len() != 19 || machine_code(&machine.replace('-', "")) != machine {
        return Err("کد سیستم باید به شکل XXXX-XXXX-XXXX-XXXX باشد.".into());
    }
    if !valid_date(expires) || expires <= db.today().as_str() {
        return Err("تاریخ انقضا باید یک تاریخ میلادی در آینده باشد (YYYY-MM-DD).".into());
    }
    let info = LicenceInfo {
        machine,
        expires: expires.into(),
        issued: db.today(),
        customer: customer.trim().into(),
        plan: plan.trim().into(),
    };
    let payload = licence_payload(&info);
    let sig = signing_key(db)?.sign(payload.as_bytes()).to_bytes();
    let code = licence_code(&payload, &sig);
    db.log("license", &format!("لایسنس برای «{}» تا {} صادر شد", info.customer, info.expires));
    Ok(code)
}

/// Hex seed for safekeeping; whoever has it can issue licences.
pub fn export_key(db: &Db) -> Res<String> {
    Ok(hex(&signing_key(db)?.to_bytes()))
}

pub fn import_key(db: &Db, seed_hex: &str) -> Res<KeyInfo> {
    let seed = hex32(seed_hex).ok_or("کلید خصوصی باید ۶۴ کاراکتر هگز باشد.")?;
    store_key(db, &seed)?;
    key_info(db)
}
