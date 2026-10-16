//! Shared server state and configuration (all via env vars, with safe defaults).
use crate::crypto::Signer0;
use crate::db::{self, Pool};
use crate::error::AppResult;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
    pub signer: Signer0,
    pub cfg: Config,
}

#[derive(Clone)]
pub struct Config {
    pub bind: String,
    pub data_dir: PathBuf,
    pub token_ttl_hours: i64, // licence token lifetime = max offline grace
    pub agent_min_version: String,
}

impl Config {
    pub fn from_env() -> Self {
        let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        Config {
            bind: env("HAMISTA_BIND", "0.0.0.0:8080"),
            data_dir: PathBuf::from(env("HAMISTA_DATA_DIR", "./data")),
            token_ttl_hours: env("HAMISTA_TOKEN_TTL_HOURS", "72").parse().unwrap_or(72),
            agent_min_version: env("HAMISTA_AGENT_MIN_VERSION", "1.0.0"),
        }
    }
}

impl AppState {
    pub fn new(cfg: Config) -> AppResult<Self> {
        std::fs::create_dir_all(&cfg.data_dir).ok();
        let pool = db::open(&cfg.data_dir.join("hamista-cloud.db"))?;
        let signer = load_or_make_signer(&pool, &cfg.data_dir)?;
        Ok(AppState { pool, signer, cfg })
    }
}

/// The signing key lives in kv (sealed by nothing but file perms — keep data_dir private on the VPS).
/// On first run it is generated and also written to `signing-key.txt` so the admin can back it up and
/// paste the public half into the desktop build's `license.pub`.
fn load_or_make_signer(pool: &Pool, data_dir: &std::path::Path) -> AppResult<Signer0> {
    let c = pool.get()?;
    // the admin desktop app's key (one key for offline HS2 and online HC1 licences, so one license.pub)
    // kept only in the environment file (not in the database, so a DB backup can't mint licences)
    if let Ok(seed) = std::env::var("HAMISTA_SIGNING_SEED") {
        return Signer0::from_seed_hex(&seed);
    }
    if let Some(seed) = db::kv_get(&c, "signing_seed") {
        return Signer0::from_seed_hex(&seed);
    }
    let (signer, seed) = Signer0::generate();
    db::kv_set(&c, "signing_seed", &seed)?;
    let pk = signer.public_hex();
    let note = format!(
        "HAMISTA CLOUD — signing key (keep secret, back up off-server)\n\nprivate_seed: {seed}\npublic_key:   {pk}\n\n\
         Put public_key into the desktop build's src-tauri/license.pub, then rebuild the customer app.\n"
    );
    let _ = std::fs::write(data_dir.join("signing-key.txt"), note);
    tracing::warn!("generated a new signing key; backup saved to data_dir/signing-key.txt");
    Ok(signer)
}
