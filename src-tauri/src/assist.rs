//! Chat assistant: the messenger's own web client, in a window of this app.
//! You log in once (the session stays in `HamistaSeller-Data\sessions\<app>`); the app opens the
//! next person's chat and prepares the text — **you** press Send. Nothing is sent automatically
//! from a personal account. These windows load remote pages and have no access to the app's commands.
use crate::db::{err, Res};
use crate::logic::chat_link;
use std::path::Path;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const APPS: [(&str, &str); 4] = [
    ("whatsapp", "واتساپ"),
    ("telegram", "تلگرام"),
    ("eitaa", "ایتا"),
    ("bale", "بله"),
];

fn name(which: &str) -> Res<&'static str> {
    APPS.iter().find(|(k, _)| *k == which).map(|(_, n)| *n).ok_or_else(|| "پیام‌رسان نامعتبر است.".into())
}

fn home_url(which: &str) -> &'static str {
    match which {
        "whatsapp" => "https://web.whatsapp.com/",
        "telegram" => "https://web.telegram.org/k/",
        "eitaa" => "https://web.eitaa.com/",
        _ => "https://web.bale.ai/",
    }
}

/// Shows the messenger's own home page in its window: log in, log out or switch account there.
pub fn home(app: &AppHandle, sessions: &Path, which: &str) -> Res<()> {
    name(which)?;
    let url: tauri::Url = home_url(which).parse().map_err(err)?;
    match app.get_webview_window(&format!("assist-{which}")) {
        Some(w) => {
            w.navigate(url).map_err(err)?;
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
            Ok(())
        }
        None => build(app, sessions, which, url),
    }
}

/// Signs out for good: closes the window and deletes the saved login of this messenger.
pub async fn reset(app: &AppHandle, sessions: &Path, which: &str) -> Res<()> {
    name(which)?;
    if let Some(w) = app.get_webview_window(&format!("assist-{which}")) {
        let _ = w.destroy();
    }
    let dir = sessions.join(which);
    // the browser engine lets go of its files a moment after the window closes
    for _ in 0..20 {
        if !dir.exists() || std::fs::remove_dir_all(&dir).is_ok() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Err("خروج کامل انجام نشد؛ برنامه را ببندید و دوباره باز کنید و دوباره امتحان کنید.".into())
}

fn build(app: &AppHandle, sessions: &Path, which: &str, url: tauri::Url) -> Res<()> {
    let dir = sessions.join(which);
    std::fs::create_dir_all(&dir).map_err(err)?;
    WebviewWindowBuilder::new(app, format!("assist-{which}"), WebviewUrl::External(url))
        .title(format!("{} — دستیار گفتگوی هامیستا سلر", name(which)?))
        .inner_size(1100.0, 800.0)
        .min_inner_size(700.0, 500.0)
        .data_directory(dir) // own login per messenger, kept between runs
        .build()
        .map_err(err)?;
    Ok(())
}

/// Opens (or reuses) the messenger window on `to`'s chat. Returns true when the text is
/// already typed in the chat; otherwise the UI copies it for the user to paste.
pub fn open(app: &AppHandle, sessions: &Path, which: &str, to: &str, text: &str) -> Res<bool> {
    name(which)?;
    let (link, prefilled) = chat_link(which, to, text).ok_or("واتساپ فقط با شماره موبایل باز می‌شود.")?;
    let url: tauri::Url = link.parse().map_err(err)?;
    let label = format!("assist-{which}");
    match app.get_webview_window(&label) {
        Some(w) => {
            // the client home is already loaded: don't reload it for every person
            let home = url.path() == "/" && url.query().is_none() && url.fragment().is_none();
            if !home {
                w.navigate(url).map_err(err)?;
            }
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
        None => build(app, sessions, which, url)?,
    }
    Ok(prefilled)
}
