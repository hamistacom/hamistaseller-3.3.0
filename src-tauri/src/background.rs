//! Running in the background: Windows tray icon, «start with Windows», close-to-tray, and the
//! scheduler that starts due extractions (see schedule.rs) and the daily price watch (watch.rs).
//! Seller editions only.
use crate::commands::{self, AppState};
use crate::{license, log_line, schedule};
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

/// Started by Windows at log-on: stay in the tray instead of opening the window.
pub fn started_hidden() -> bool {
    std::env::args().any(|a| a == "--hidden")
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    pub tray: bool,      // closing the window keeps the app (and schedules) running in the tray
    pub autostart: bool, // start with Windows (in the tray)
}

pub fn prefs(st: &AppState) -> Prefs {
    Prefs { tray: close_to_tray(st), autostart: autostart_on() }
}

/// Closing the window hides it to the tray — only when the tray icon really exists.
pub fn close_to_tray(st: &AppState) -> bool {
    st.db.kv_get("close_to_tray").as_deref() != Some("0")
}

pub fn set_prefs(st: &AppState, p: Prefs) -> Result<Prefs, String> {
    set_autostart(p.autostart)?; // first: if Windows refuses it, nothing is half-saved
    st.db.kv_set("close_to_tray", if p.tray { "1" } else { "0" })?;
    Ok(prefs(st))
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_NAME: &str = "HamistaSeller";

#[cfg(windows)]
fn reg(args: &[&str]) -> std::io::Result<std::process::Output> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg").args(args).creation_flags(0x0800_0000).output()
}

fn autostart_on() -> bool {
    #[cfg(windows)]
    {
        reg(&["query", RUN_KEY, "/v", RUN_NAME]).is_ok_and(|o| o.status.success())
    }
    #[cfg(not(windows))]
    {
        let _ = (RUN_KEY, RUN_NAME);
        false
    }
}

fn set_autostart(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let ok = if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let cmd = format!("\"{}\" --hidden", exe.display());
            reg(&["add", RUN_KEY, "/v", RUN_NAME, "/t", "REG_SZ", "/d", &cmd, "/f"]).is_ok_and(|o| o.status.success())
        } else {
            !autostart_on() || reg(&["delete", RUN_KEY, "/v", RUN_NAME, "/f"]).is_ok_and(|o| o.status.success())
        };
        if ok {
            Ok(())
        } else {
            Err("تنظیم اجرای خودکار با ویندوز ممکن نشد.".into())
        }
    }
    #[cfg(not(windows))]
    {
        if on {
            Err("اجرای خودکار فقط در ویندوز در دسترس است.".into())
        } else {
            Ok(())
        }
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Quit for real (tray menu): a running job is stopped first and quits the app when it has shut down.
fn quit(app: &AppHandle) {
    let st = app.state::<AppState>();
    if st.jobs.stop(None) {
        st.quit.store(true, std::sync::atomic::Ordering::Relaxed);
    } else {
        app.exit(0);
    }
}

pub fn tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "نمایش هامیستا سلر", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "توقف عملیات در حال اجرا", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "quit", "خروج کامل", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &stop, &exit])?;
    let mut b = TrayIconBuilder::with_id("main")
        .tooltip("هامیستا سلر — در پس‌زمینه")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id().as_ref() {
            "show" => show_main(app),
            "stop" => {
                app.state::<AppState>().jobs.stop(None);
            }
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(app)?;
    Ok(())
}

/// Every 30 s: start the earliest due schedule if nothing else is running and the licence is valid.
pub fn scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let st = app.state::<AppState>();
            if !st.jobs.running().is_empty() || !license::status(&st.db).valid {
                continue;
            }
            let Some(s) = schedule::due(&st.db) else {
                // no extraction due: the daily price watch, once a day after its time
                let ids = crate::watch::due(&st.db);
                if !ids.is_empty() {
                    let _ = app.emit("job-start", serde_json::json!({ "kind": "watch", "title": "پایش روزانه قیمت" }));
                    let h = app.clone();
                    let emit = move |e: crate::scanner::ScanEvent| {
                        let _ = h.emit("job-event", e);
                    };
                    let res = commands::run_watch(&st, &ids, &emit, true).await;
                    let skipped = matches!(&res, Err(e) if e == commands::BUSY || e == crate::jobs::BUSY_MANY);
                    let text = match &res {
                        Ok(sum) => sum.message.clone(),
                        Err(e) => format!("پایش قیمت انجام نشد: {e}"),
                    };
                    let _ = app.emit("job-end", serde_json::json!({ "scheduled": true, "skipped": skipped, "message": text, "error": res.as_ref().map_or(true, |r| r.error) }));
                    if st.quit.load(std::sync::atomic::Ordering::Relaxed) {
                        app.exit(0);
                    }
                }
                continue;
            };
            log_line("INFO", &format!("schedule {} «{}» starts", s.id, s.name));
            let _ = app.emit("job-start", serde_json::json!({ "kind": "scan", "title": format!("زمان‌بندی: {}", s.name) }));
            let h = app.clone();
            let emit = move |e: crate::scanner::ScanEvent| {
                let _ = h.emit("job-event", e);
            };
            let res = commands::run_scan(Some(&app), &st, s.req.clone(), &emit, None, true).await;
            if matches!(&res, Err(e) if e == commands::BUSY || e == crate::jobs::BUSY_MANY) {
                // the user started a job in the meantime: try again on the next tick
                let _ = app.emit("job-end", serde_json::json!({ "scheduled": true, "skipped": true }));
                continue;
            }
            let text = match &res {
                Ok(sum) => sum.message.clone(),
                Err(e) => format!("انجام نشد: {e}"),
            };
            schedule::finished(&st.db, s.id, &text);
            st.db.log("schedule", &format!("زمان‌بندی «{}»: {text}", s.name));
            let _ = app.emit("job-end", serde_json::json!({ "scheduled": true, "message": text, "error": res.as_ref().map_or(true, |r| r.error) }));
            if st.quit.load(std::sync::atomic::Ordering::Relaxed) {
                app.exit(0); // quit was asked while the schedule ran
            }
            if let Some(t) = app.tray_by_id("main") {
                let _ = t.set_tooltip(Some(format!("هامیستا سلر — آخرین زمان‌بندی: {}", s.name)));
            }
        }
    });
}
