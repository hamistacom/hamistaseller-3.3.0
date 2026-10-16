#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod admin;
mod assist;
mod background;
mod balad;
mod channels;
mod cloud;
mod commands;
mod db;
mod enrich;
mod export;
mod instagram;
mod license;
mod logic;
mod neshan;
mod outreach;
mod scanner;
mod market;
mod brand;
mod identity;
mod jobs;
mod links;
mod web;
mod watch;
mod schedule;
mod secrets;
mod sources;
mod telegram;
mod websearch;

use commands::AppState;
use db::Db;
use serde_json::{json, Map, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock};
use tauri::Manager;

// the issuer keeps its signing key apart from any seller data on the same PC
const DATA_DIR: &str = if license::ISSUER { "HamistaIssuer-Data" } else { "HamistaSeller-Data" };

// ---------------- diagnostics log (logs/hamista.log, rotated at 2 MB) ----------------

static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
#[cfg(windows)]
static INSTANCE_LOCK: OnceLock<std::fs::File> = OnceLock::new(); // held open (unshared) while the app runs

pub fn log_line(level: &str, text: &str) {
    let Some(dir) = LOG_DIR.get() else { return };
    let path = dir.join("hamista.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 2 * 1024 * 1024) {
        let _ = std::fs::rename(&path, dir.join("hamista.1.log"));
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} UTC [{level}] {text}", logic::utc_datetime(secs));
    }
}

/// Portable: data lives next to the exe; falls back to %APPDATA% when that folder is read-only.
fn data_dir(app: &tauri::App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(d) = std::env::current_exe()?.parent().map(|p| p.join(DATA_DIR)) {
        if std::fs::create_dir_all(&d).is_ok() && std::fs::write(d.join(".write-test"), b"ok").is_ok() {
            return Ok(d);
        }
    }
    let mut d = app.path().app_data_dir()?;
    if license::ISSUER {
        d.push("issuer");
    }
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

/// SellerMap v3 databases this PC may have (portable folder first, then its %APPDATA% fallback).
fn legacy_db(app: &tauri::App, dir: &Path) -> Option<PathBuf> {
    let beside_exe = dir.parent().map(|p| p.join("SellerMap-Data").join("sellermap3.db"));
    let appdata = app
        .path()
        .app_data_dir()
        .ok()
        .and_then(|d| d.parent().map(|p| p.join("ir.sellermap.desktop").join("sellermap3.db")));
    [beside_exe, appdata].into_iter().flatten().find(|p| p.is_file())
}

/// v3 kept sending accounts as plain JSON; split each into public config + DPAPI-sealed secret.
fn migrate_accounts(db: &Db, json: &str) -> usize {
    let Ok(Value::Object(all)) = serde_json::from_str::<Value>(json) else {
        return 0;
    };
    let mut n = 0;
    for (kind, secret_keys) in [("sms", &["apiKey", "username", "password"][..])] {
        for item in all.get(kind).and_then(Value::as_array).into_iter().flatten() {
            let Some(obj) = item.as_object() else { continue };
            let mut config = obj.clone();
            let mut secret = Map::new();
            for k in secret_keys {
                if let Some(v) = config.remove(*k) {
                    secret.insert(k.to_string(), v);
                }
            }
            let per_minute = config.remove("perMinute").and_then(|v| v.as_i64()).unwrap_or(60);
            let a = db::Account {
                id: config
                    .remove("id")
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| format!("{kind}-v3-{n}")),
                name: config.get("name").and_then(Value::as_str).unwrap_or(kind).to_string(),
                kind: kind.into(),
                config: Value::Object(config),
                per_minute,
                has_secret: true,
            };
            if let Ok(sealed) = secrets::seal_str(&Value::Object(secret).to_string()) {
                if db.account_save(&a, Some(sealed)).is_ok() {
                    n += 1;
                }
            }
        }
    }
    n
}

fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let dir = data_dir(app)?;
    for sub in ["logs", "backups"] {
        std::fs::create_dir_all(dir.join(sub))?;
    }
    let _ = LOG_DIR.set(dir.join("logs"));
    // one copy per data folder: a second start (e.g. while the first sits in the tray) would run
    // schedules twice against the same database
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        match std::fs::OpenOptions::new().create(true).truncate(false).write(true).share_mode(0).open(dir.join("app.lock")) {
            Ok(f) => {
                let _ = INSTANCE_LOCK.set(f);
            }
            Err(_) => return Err("هامیستا سلر از قبل باز است؛ آن را از آیکون کنار ساعت ویندوز (سینی) باز کنید.".into()),
        }
    }
    log_line("INFO", &format!("start v{} ({})", env!("CARGO_PKG_VERSION"), license::EDITION));
    let db = Db::open(&dir.join("hamista.db"))?;

    let mut migrated = None;
    if !license::ISSUER && db.is_empty() && db.kv_get("migrated_v3").is_none() {
        if let Some(old) = legacy_db(app, &dir) {
            match db.import_v3(&old) {
                Ok(r) => {
                    let accounts = r.accounts_json.as_deref().map_or(0, |j| migrate_accounts(&db, j));
                    let summary =
                        json!({ "lists": r.lists, "places": r.places, "contacts": r.contacts, "runs": r.runs, "campaigns": r.campaigns, "accounts": accounts });
                    let _ = db.kv_set("migrated_v3", &summary.to_string());
                    db.log("migrate", &format!("{} مخاطب و {} بانک از نسخه قبل منتقل شد", r.places, r.lists));
                    log_line("INFO", &format!("imported v3 from {}: {summary}", old.display()));
                    migrated = Some(summary);
                }
                Err(e) => log_line("ERROR", &format!("v3 import failed: {e}")),
            }
        }
    }

    app.manage(AppState {
        db,
        dir,
        jobs: jobs::Jobs::default(),
        quit: AtomicBool::new(false),
        migrated: Mutex::new(migrated),
    });
    // every change of an operation goes to the window («عملیات» panel)
    let h = app.handle().clone();
    app.state::<AppState>().jobs.on_change(move |job| {
        let _ = tauri::Emitter::emit(&h, "job-update", job);
    });

    if license::ISSUER {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.set_title("هامیستا سلر — پنل مدیر");
        }
    }

    if !license::ISSUER {
        let d = app.state::<AppState>().dir.clone();
        std::thread::spawn(move || scanner::cleanup(&d, false));
        if let Err(e) = background::tray(app) {
            log_line("WARN", &format!("tray: {e}"));
        }
        background::scheduler(app.handle().clone());
    }

    // online licence: heartbeat + mega-bank upload + scenario, every 6 h while the app runs
    if !license::MASTER && !license::ISSUER {
        let handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            let st = handle.state::<AppState>();
            cloud::scenario_load(&st.db);
            loop {
                if cloud::sync_due(&st.db) {
                    if let Err(e) = cloud::sync_all(&st.db).await {
                        log_line("WARN", &format!("sync: {e}"));
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(15 * 60)).await;
            }
        });
    }

    // daily automatic backup, off the UI thread
    let handle = app.handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let st = handle.state::<AppState>();
        let today = st.db.today();
        if st.db.kv_get("last_auto_backup").as_deref() != Some(today.as_str()) {
            match commands::make_backup(&st, "auto") {
                Ok(_) => {
                    let _ = st.db.kv_set("last_auto_backup", &today);
                }
                Err(e) => log_line("ERROR", &format!("auto backup failed: {e}")),
            }
        }
    });
    // safety net: show the window even if the page never reports "loaded"
    let h = app.handle().clone();
    std::thread::spawn(move || {
        if background::started_hidden() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(4));
        if let Some(w) = h.get_webview_window("main") {
            let _ = w.show();
        }
    });
    Ok(())
}

#[cfg(windows)]
fn fatal_box(text: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(hwnd: *mut std::ffi::c_void, text: *const u16, caption: *const u16, flags: u32) -> i32;
    }
    let w = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    const MB_ICONERROR_RTL: u32 = 0x10 | 0x0008_0000 | 0x0010_0000; // MB_ICONERROR | MB_RIGHT | MB_RTLREADING
                                                                    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the call.
    unsafe { MessageBoxW(std::ptr::null_mut(), w(text).as_ptr(), w("هامیستا سلر").as_ptr(), MB_ICONERROR_RTL) };
}

#[cfg(not(windows))]
fn fatal_box(text: &str) {
    eprintln!("{text}");
}

fn main() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log_line("PANIC", &info.to_string());
        default_hook(info);
    }));

    tauri::Builder::default()
        .setup(|app| {
            if let Err(e) = setup_app(app) {
                // Tauri would panic on a setup error; release builds have no console, so say it in a dialog
                log_line("FATAL", &format!("setup failed: {e}"));
                fatal_box(&format!("هامیستا سلر اجرا نشد:\n{e}\n\nجزئیات در پوشه {DATA_DIR}\\logs ثبت شده است."));
                std::process::exit(1);
            }
            Ok(())
        })
        // shown only once the UI has painted: no white flash at start-up
        .on_page_load(|webview, payload| {
            // started with Windows (--hidden): stay in the tray until the user opens it
            if payload.event() == tauri::webview::PageLoadEvent::Finished && webview.label() == "main" && !background::started_hidden() {
                let _ = webview.window().show();
            }
        })
        .on_window_event(|window, event| {
            // only the main window ends the app; messenger windows just close
            if let (tauri::WindowEvent::CloseRequested { api, .. }, "main") = (event, window.label()) {
                let st = window.state::<AppState>();
                // close-to-tray: the app (running job, schedules, licence sync) keeps going in the background
                if !license::ISSUER && background::close_to_tray(&st) && window.app_handle().tray_by_id("main").is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                    return;
                }
                if st.jobs.stop(None) {
                    // stop the jobs first (they close the browser); the last one quits the app when done
                    st.quit.store(true, Relaxed);
                    api.prevent_close();
                } else {
                    for (label, w) in window.app_handle().webview_windows() {
                        if label != "main" {
                            let _ = w.close();
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::license_status,
            commands::license_activate,
            commands::license_key_info,
            commands::license_issue,
            commands::license_export_key,
            commands::license_import_key,
            commands::job_stop,
            commands::scan_start,
            commands::scan_resume,
            commands::browser_detect,
            commands::campaign_run,
            commands::template_preview,
            commands::assist_open,
            commands::assist_done,
            commands::contact_app,
            commands::assist_session,
            commands::place_photo,
            commands::city_photo,
            commands::brand_runs,
            commands::brand_run_get,
            commands::market_trends,
            commands::market_login,
            commands::market_search,
            commands::market_rfq,
            commands::market_runs,
            commands::market_get,
            commands::market_delete,
            commands::watch_list,
            commands::watch_save,
            commands::watch_delete,
            commands::watch_time,
            commands::watch_run,
            commands::catalog_list,
            commands::catalog_import,
            commands::catalog_delete,
            commands::catalog_check,
            commands::brands,
            commands::brand_save,
            commands::brand_delete,
            commands::brand_analyze,
            commands::export_data,
            commands::import_data,
            commands::job_current,
            commands::jobs_list,
            commands::jobs_clear,
            commands::lists,
            commands::list_create,
            commands::list_update,
            commands::list_delete,
            commands::transfer,
            commands::places,
            commands::place,
            commands::place_save,
            commands::place_status,
            commands::places_status,
            commands::places_tag,
            commands::places_delete,
            commands::facets,
            commands::bank_stats,
            commands::dashboard,
            commands::runs,
            commands::campaigns,
            commands::activity,
            commands::settings_get,
            commands::settings_save,
            commands::accounts,
            commands::account_save,
            commands::account_delete,
            commands::templates,
            commands::template_save,
            commands::template_delete,
            commands::optouts,
            commands::optouts_save,
            commands::backups,
            commands::backup_now,
            commands::backup_restore,
            commands::reveal,
            commands::open_url,
            commands::logs_folder,
            commands::cloud_login,
            commands::cloud_logout,
            commands::cloud_sync,
            commands::cloud_notifications,
            commands::cloud_notification_read,
            commands::cloud_version,
            commands::mega_search,
            commands::mega_import,
            commands::sources_list,
            commands::source_config,
            commands::source_config_save,
            commands::source_toggle,
            commands::background_get,
            commands::background_set,
            commands::schedules,
            commands::schedule_save,
            commands::schedule_toggle,
            commands::schedule_delete,
            commands::support_report,
            commands::cloud_trial_info,
            commands::cloud_trial,
            commands::admin_stats,
            commands::admin_customers,
            commands::admin_customer_save,
            commands::admin_customer_delete,
            commands::admin_issue,
            commands::admin_licences,
            commands::admin_export,
            commands::admin_server,
            commands::admin_connect,
            commands::admin_disconnect,
            commands::admin_api
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log_line("FATAL", &format!("failed to start: {e}"));
            std::process::exit(1)
        })
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(st) = app.try_state::<AppState>() {
                    scanner::cleanup(&st.dir, true); // never leave a browser behind
                }
            }
        });
}
