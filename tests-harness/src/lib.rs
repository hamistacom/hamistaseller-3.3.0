//! Test harness: the app's engine modules without Tauri (Linux host has no WebKitGTK here).
#![allow(dead_code)]
#[path = "../../src-tauri/src/db.rs"] pub mod db;
#[path = "../../src-tauri/src/logic.rs"] pub mod logic;
#[path = "../../src-tauri/src/secrets.rs"] pub mod secrets;
#[path = "../../src-tauri/src/export.rs"] pub mod export;
#[path = "../../src-tauri/src/channels.rs"] pub mod channels;
#[path = "../../src-tauri/src/enrich.rs"] pub mod enrich;
#[path = "../../src-tauri/src/outreach.rs"] pub mod outreach;
#[path = "../../src-tauri/src/scanner.rs"] pub mod scanner;
#[path = "../../src-tauri/src/license.rs"] pub mod license;
#[path = "../../src-tauri/src/cloud.rs"] pub mod cloud;
#[path = "../../src-tauri/src/admin.rs"] pub mod admin;
#[path = "../../src-tauri/src/sources.rs"] pub mod sources;
#[path = "../../src-tauri/src/websearch.rs"] pub mod websearch;
#[path = "../../src-tauri/src/neshan.rs"] pub mod neshan;
#[path = "../../src-tauri/src/balad.rs"] pub mod balad;
#[path = "../../src-tauri/src/telegram.rs"] pub mod telegram;
#[path = "../../src-tauri/src/instagram.rs"] pub mod instagram;
#[path = "../../src-tauri/src/schedule.rs"] pub mod schedule;
#[path = "../../src-tauri/src/market.rs"] pub mod market;
#[path = "../../src-tauri/src/brand.rs"] pub mod brand;
#[path = "../../src-tauri/src/identity.rs"] pub mod identity;
#[path = "../../src-tauri/src/watch.rs"] pub mod watch;
#[path = "../../src-tauri/src/jobs.rs"] pub mod jobs;
#[path = "../../src-tauri/src/links.rs"] pub mod links;
#[path = "../../src-tauri/src/web.rs"] pub mod web;
#[path = "../../src-tauri/src/products.rs"] pub mod products;

pub fn log_line(_: &str, _: &str) {}

/// Same as the app's commands.rs pieces these modules use.
pub mod commands {
    use crate::db::{err, Db, Res};
    use serde_json::{Map, Value};
    pub const STATUSES: [(&str, &str); 5] = [("new", "جدید"), ("contacted", "پیگیری‌شده"), ("interested", "علاقه‌مند"), ("customer", "مشتری"), ("rejected", "عدم تمایل")];
    pub fn account_secret(db: &Db, id: &str) -> Res<Map<String, Value>> {
        match db.account_secret(id)? {
            Some(blob) => serde_json::from_str(&crate::secrets::open_str(&blob)?).map_err(err),
            None => Ok(Map::new()),
        }
    }
}
