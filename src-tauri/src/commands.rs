//! IPC surface. Commands stay thin: validate input, call a module, return `Res<T>`.
//! Tauri converts snake_case argument names to camelCase for JS (`list_id` → `listId`).
use crate::db::{err, Account, Activity, Campaign, Contact, Dashboard, Db, Facets, Filter, ListInfo, Place, Res, Run, Template, KINDS};
use crate::jobs::{Job, Jobs};
use crate::outreach::CampaignReq;
use crate::scanner::{Emit, ScanEvent, ScanRequest, Summary};
use crate::{admin, assist, channels, cloud, export, license, logic, outreach, scanner, secrets, sources};
use futures::FutureExt;
use serde::Serialize;
use serde_json::{Map, Value};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Mutex;
use tauri::{ipc::Channel, AppHandle, Manager, State};

pub struct AppState {
    pub db: Db,
    pub dir: PathBuf,
    /// Every running (and the last few finished) operations — see jobs.rs.
    pub jobs: Jobs,
    pub quit: AtomicBool,               // window closed mid-job: exit once every job has shut down
    pub migrated: Mutex<Option<Value>>, // one-time notice after importing SellerMap v3
}

/// Kept for callers that only need to know a job could not start (the scheduler retries later).
pub const BUSY: &str = crate::jobs::BUSY_SAME;

/// A job's events go both to its live state in the operation manager and to `emit` (the page).
pub fn tee<'a>(jobs: &'a Jobs, id: u64, emit: Emit<'a>) -> impl Fn(ScanEvent) + Send + Sync + 'a {
    move |e: ScanEvent| {
        jobs.record(id, &e);
        emit(e);
    }
}

/// Runs `work` as `job`: `scanner::launch` then knows which job holds the browser, a panic ends
/// the job as failed instead of the app, and the job is closed with its outcome. When the window
/// was closed meanwhile, the app quits once the last job has shut its browser down.
pub async fn run_job<T>(
    app: Option<&AppHandle>,
    state: &AppState,
    job: Job,
    work: impl std::future::Future<Output = Res<T>>,
    outcome: impl Fn(&T) -> Result<String, String>,
) -> Res<T> {
    let res = AssertUnwindSafe(scanner::JOB_ID.scope(job.id, work)).catch_unwind().await.unwrap_or_else(|_| {
        crate::log_line("PANIC", "job panicked");
        Err("خطای داخلی رخ داد؛ داده‌های ذخیره‌شده حفظ شده‌اند.".into())
    });
    match res.as_ref().map(&outcome) {
        Ok(Ok(msg)) => job.finish(None, &msg),
        Ok(Err(e)) => job.finish(Some(&e), ""),
        Err(e) => job.finish(Some(e), ""),
    }
    if state.quit.load(Relaxed) && state.jobs.running().is_empty() {
        if let Some(app) = app {
            app.exit(0);
        }
    }
    res
}

fn summary_outcome(s: &Summary) -> Result<String, String> {
    if s.error {
        Err(s.message.clone())
    } else {
        Ok(s.message.clone())
    }
}

fn issuer_only() -> Res<()> {
    if license::ISSUER {
        Ok(())
    } else {
        Err("این بخش فقط در نسخه صدور لایسنس در دسترس است.".into())
    }
}

pub const STATUSES: [(&str, &str); 5] = [
    ("new", "جدید"),
    ("contacted", "پیگیری‌شده"),
    ("interested", "علاقه‌مند"),
    ("customer", "مشتری"),
    ("rejected", "عدم تمایل"),
];

fn check_status(s: &str) -> Res<()> {
    STATUSES.iter().any(|(k, _)| *k == s).then_some(()).ok_or_else(|| "وضعیت نامعتبر است.".into())
}

fn clean_text(s: &str, max: usize, what: &str) -> Res<String> {
    let t = s.trim();
    if t.chars().count() > max {
        return Err(format!("{what} نباید بیشتر از {max} حرف باشد."));
    }
    Ok(t.to_string())
}

// ---------------- app / licence ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: &'static str,
    edition: &'static str,
    data_dir: String,
    license: license::Status,
    migrated: Option<Value>,
    default_server: &'static str,
}

#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> Res<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        edition: license::EDITION,
        data_dir: state.dir.to_string_lossy().into_owned(),
        license: license::status(&state.db),
        migrated: state.migrated.lock().unwrap_or_else(|e| e.into_inner()).take(),
        default_server: license::DEFAULT_SERVER.trim(),
    })
}

#[tauri::command]
pub async fn license_status(state: State<'_, AppState>) -> Res<license::Status> {
    Ok(license::status(&state.db))
}

#[tauri::command]
pub async fn license_activate(state: State<'_, AppState>, code: String) -> Res<license::Status> {
    let st = license::activate(&state.db, &code)?;
    state.db.log("license", "برنامه فعال شد");
    Ok(st)
}

#[tauri::command]
pub async fn license_key_info(state: State<'_, AppState>) -> Res<license::KeyInfo> {
    issuer_only()?;
    license::key_info(&state.db)
}

#[tauri::command]
pub async fn license_issue(state: State<'_, AppState>, machine: String, customer: String, plan: String, expires: String) -> Res<String> {
    issuer_only()?;
    license::issue(
        &state.db,
        &machine,
        &clean_text(&customer, 80, "نام مشتری")?,
        &clean_text(&plan, 40, "نوع لایسنس")?,
        expires.trim(),
    )
}

#[tauri::command]
pub async fn license_export_key(state: State<'_, AppState>) -> Res<String> {
    issuer_only()?;
    license::export_key(&state.db)
}

#[tauri::command]
pub async fn license_import_key(state: State<'_, AppState>, seed: String) -> Res<license::KeyInfo> {
    issuer_only()?;
    license::import_key(&state.db, &seed)
}

// ---------------- jobs ----------------

/// Stops one operation (`id`) or, without an id, every running one.
#[tauri::command]
pub async fn job_stop(state: State<'_, AppState>, id: Option<u64>) -> Res<bool> {
    Ok(state.jobs.stop(id))
}

/// The kind of the first running operation (kept for older pages).
#[tauri::command]
pub async fn job_current(state: State<'_, AppState>) -> Res<Option<String>> {
    Ok(state.jobs.running().into_iter().next().map(|j| j.kind))
}

/// Running operations first, then the last finished ones (the «عملیات» panel).
#[tauri::command]
pub async fn jobs_list(state: State<'_, AppState>) -> Res<Vec<crate::jobs::JobInfo>> {
    Ok(state.jobs.list())
}

#[tauri::command]
pub async fn jobs_clear(state: State<'_, AppState>) -> Res<()> {
    state.jobs.clear_finished();
    Ok(())
}

fn emitter(on_event: Channel<ScanEvent>) -> impl Fn(ScanEvent) + Send + Sync {
    move |e| {
        let _ = on_event.send(e); // the window may be gone; never fail the job for that
    }
}

async fn scan(app: AppHandle, state: State<'_, AppState>, req: ScanRequest, on_event: Channel<ScanEvent>, resumed: Option<i64>) -> Res<Summary> {
    run_scan(Some(&app), &state, req, &emitter(on_event), resumed, false).await
}

/// One extraction job (one or several sources). Used by the «استخراج» page and by the scheduler.
pub async fn run_scan(app: Option<&AppHandle>, state: &AppState, req: ScanRequest, emit: Emit<'_>, resumed: Option<i64>, scheduled: bool) -> Res<Summary> {
    license::ensure(&state.db)?;
    let st = license::status(&state.db);
    let wanted = sources::requested(&req);
    let list: Vec<String> = wanted
        .iter()
        .filter(|id| {
            sources::get(id).is_some_and(|s| s.kind == "scan" && s.ready) && sources::enabled(&state.db, id) && license::source_allowed(&st, id).is_ok()
        })
        .cloned()
        .collect();
    if list.is_empty() {
        return Err(if wanted.is_empty() {
            "منبع استخراج را انتخاب کنید."
        } else {
            "منبع انتخاب‌شده خاموش است یا در اشتراک شما فعال نیست."
        }
        .into());
    }
    let mut req = req;
    if !sources::enabled(&state.db, "website") {
        req.enrich = false; // «سایت کسب‌وکار» switched off in Settings → Modules
    }
    if !state.db.list_exists(req.list_id) {
        return Err("بانک مقصد را انتخاب کنید.".into());
    }
    cloud::precheck(&state.db, "scan")?;
    let job = state.jobs.start("scan", &format!("استخراج «{}»", req.keywords.join("، ")), scheduled)?;
    let (token, id) = (job.token.clone(), job.id);
    let emit = &tee(&state.jobs, id, emit);
    let work = async {
        if let Some(id) = resumed {
            state.db.run_resumed(id);
        }
        let s: scanner::Settings = serde_json::from_value(Value::Object(settings(&state.db))).unwrap_or_default();
        let profile = state.dir.join("browser-profile");
        let db = &state.db;
        let ran = std::sync::Mutex::new(0i64); // sources that actually ran = units of the monthly quota
        let res = AssertUnwindSafe(sources::run_all(db, &s, &profile, &req, &list, &token, emit, |_, r| {
            if r.checked > 0 {
                *ran.lock().unwrap_or_else(|e| e.into_inner()) += 1;
            }
        }))
        .catch_unwind()
        .await;
        if let Ok(Ok(sum)) = &res {
            let names: Vec<&str> = list.iter().map(|id| sources::get(id).map_or(id.as_str(), |x| x.name)).collect();
            state.db.log(
                "scan",
                &format!("استخراج «{}» از {}: {} مخاطب جدید", req.keywords.join("، "), names.join("، "), sum.found),
            );
            let units = *ran.lock().unwrap_or_else(|e| e.into_inner());
            if units > 0 {
                if let Err(e) = cloud::consume(&state.db, "scan", units).await {
                    crate::log_line("WARN", &format!("scan quota: {e}"));
                }
            }
            if sum.found > 0 {
                if let Err(e) = cloud::mega_sync(&state.db).await {
                    crate::log_line("WARN", &format!("mega sync after scan: {e}"));
                }
            }
        }
        unwind(res)
    };
    run_job(app, state, job, work, summary_outcome).await
}

fn unwind(res: Result<Res<Summary>, Box<dyn std::any::Any + Send>>) -> Res<Summary> {
    res.unwrap_or_else(|_| {
        crate::log_line("PANIC", "job panicked");
        Err("خطای داخلی رخ داد؛ داده‌های ذخیره‌شده حفظ شده‌اند.".into())
    })
}

#[tauri::command]
pub async fn scan_start(app: AppHandle, state: State<'_, AppState>, req: ScanRequest, on_event: Channel<ScanEvent>) -> Res<Summary> {
    scan(app, state, req, on_event, None).await
}

/// «ادامه استخراج»: continues an interrupted run from its last checkpoint.
#[tauri::command]
pub async fn scan_resume(app: AppHandle, state: State<'_, AppState>, run_id: i64, on_event: Channel<ScanEvent>) -> Res<Summary> {
    let run = state.db.run(run_id)?;
    if run.resume.is_empty() || run.status == "done" {
        return Err("این استخراج کامل شده یا قابل ادامه نیست.".into());
    }
    let req: ScanRequest = serde_json::from_str(&run.resume).map_err(err)?;
    scan(app, state, req, on_event, Some(run_id)).await
}

/// «رد کردن و ادامه»: ends the current captcha / sign-in wait; that source is skipped and the job goes on.
#[tauri::command]
pub async fn wait_skip() -> Res<()> {
    scanner::trigger_wait_skip();
    Ok(())
}

/// Path of the Edge / Chrome the scanner will use (`custom` first), or null.
#[tauri::command]
pub async fn browser_detect(custom: String) -> Res<Option<String>> {
    Ok(scanner::detect_browser(&custom))
}

#[tauri::command]
pub async fn campaign_run(app: AppHandle, state: State<'_, AppState>, req: CampaignReq, on_event: Channel<ScanEvent>) -> Res<Summary> {
    license::ensure(&state.db)?;
    cloud::precheck(&state.db, "send")?;
    let account = channels::load(&state.db, &req.account_id)?;
    let job = state.jobs.start("campaign", "کمپین ارسال", false)?;
    let token = job.token.clone();
    let page = emitter(on_event);
    let emit = tee(&state.jobs, job.id, &page);
    let work = async {
        let res = outreach::run_campaign(&state.db, &req, account, &token, &emit).await;
        // counted after the run (only real sends); the server fills the quota to its cap on overshoot,
        // so at most one campaign can pass the cap and the next one is refused by precheck
        if let Ok(sum) = &res {
            if let Err(e) = cloud::consume(&state.db, "send", sum.found).await {
                crate::log_line("WARN", &format!("send quota: {e}"));
            }
        }
        res
    };
    run_job(Some(&app), &state, job, work, summary_outcome).await
}

/// Preview of a template for one place (what the recipient will read).
#[tauri::command]
pub async fn template_preview(state: State<'_, AppState>, place_id: i64, text: String) -> Res<String> {
    Ok(outreach::compose(&text, &state.db.get(place_id)?))
}

/// One post to your own Eitaa / Rubika channel.

// ---------------- chat assistant ----------------

#[tauri::command]
pub async fn assist_open(app: AppHandle, state: State<'_, AppState>, messenger: String, to: String, text: String) -> Res<bool> {
    license::ensure(&state.db)?;
    assist::open(&app, &state.dir.join("sessions"), &messenger, &to, &text)
}

/// The user sent the message in the assistant window: record it and mark the business as contacted.
#[tauri::command]
pub async fn assist_done(state: State<'_, AppState>, place_id: i64, contact_id: i64, messenger: String, recipient: String) -> Res<()> {
    if !assist::APPS.iter().any(|(k, _)| *k == messenger) {
        return Err("پیام‌رسان نامعتبر است.".into());
    }
    state.db.log_message(0, place_id, contact_id, &recipient, &messenger, "sent", "", "manual");
    state.db.mark_contacted(place_id);
    let _ = state.db.set_app(contact_id, &messenger, true); // the chat opened, so the number is on it
    Ok(())
}

/// The user saw in the assistant whether this mobile number has an account on `messenger`.
#[tauri::command]
pub async fn contact_app(state: State<'_, AppState>, contact_id: i64, messenger: String, has: bool) -> Res<()> {
    if !assist::APPS.iter().any(|(k, _)| *k == messenger) {
        return Err("پیام‌رسان نامعتبر است.".into());
    }
    state.db.set_app(contact_id, &messenger, has)
}

/// Assistant login: "open" shows the messenger's home (log in, log out or switch account there);
/// "reset" closes its window and deletes the saved login so the next open starts signed out.
#[tauri::command]
pub async fn assist_session(app: AppHandle, state: State<'_, AppState>, messenger: String, action: String) -> Res<()> {
    let sessions = state.dir.join("sessions");
    match action.as_str() {
        "open" => assist::home(&app, &sessions, &messenger),
        "reset" => assist::reset(&app, &sessions, &messenger).await,
        _ => Err("درخواست نامعتبر است.".into()),
    }
}

// ---------------- product & price intelligence ----------------

/// Collects prices from the chosen platforms (browser only when an API does not answer).
#[tauri::command]
pub async fn market_search(
    app: AppHandle,
    state: State<'_, AppState>,
    req: crate::market::MarketReq,
    on_event: Channel<ScanEvent>,
) -> Res<crate::market::Report> {
    license::ensure(&state.db)?;
    let job = state.jobs.start("market", &format!("تحلیل محصول «{}»", req.query.trim()), false)?;
    let (s, profile) = market_settings(&state);
    let token = job.token.clone();
    let page = emitter(on_event);
    let emit = tee(&state.jobs, job.id, &page);
    let work = crate::market::search(&state.db, &s, &profile, &req, &token, &emit);
    run_job(Some(&app), &state, job, work, |r| {
        Ok(format!("{} پیشنهاد معتبر از {} منبع", r.offers.len(), r.channels.len()))
    })
    .await
}

/// Builds a ready-to-copy price-enquiry (RFQ) message; nothing is sent.
#[tauri::command]
pub async fn market_rfq(req: crate::web::Rfq) -> Res<String> {
    if req.product.trim().is_empty() {
        return Err("نام کالا را برای استعلام بنویسید.".into());
    }
    Ok(crate::web::rfq_text(&req))
}

#[tauri::command]
pub async fn market_runs(state: State<'_, AppState>) -> Res<Vec<crate::market::RunRow>> {
    state.db.market_runs(100)
}

#[tauri::command]
pub async fn market_get(state: State<'_, AppState>, id: i64) -> Res<crate::market::Report> {
    state.db.market_get(id)
}

#[tauri::command]
pub async fn market_delete(state: State<'_, AppState>, id: i64) -> Res<()> {
    state.db.market_delete(id)
}

// ---------------- brand analysis ----------------

#[tauri::command]
pub async fn brands(state: State<'_, AppState>) -> Res<Vec<crate::brand::Brand>> {
    state.db.brands()
}

#[tauri::command]
pub async fn brand_save(state: State<'_, AppState>, brand: crate::brand::Brand) -> Res<i64> {
    state.db.brand_save(&brand)
}

#[tauri::command]
pub async fn brand_delete(state: State<'_, AppState>, id: i64) -> Res<()> {
    state.db.brand_delete(id)
}

#[tauri::command]
pub async fn brand_analyze(app: AppHandle, state: State<'_, AppState>, id: i64, on_event: Channel<ScanEvent>) -> Res<crate::brand::BrandReport> {
    license::ensure(&state.db)?;
    let b = state.db.brands()?.into_iter().find(|b| b.id == id).ok_or("برند پیدا نشد.")?;
    let job = state.jobs.start("brand", &format!("تحلیل برند «{}»", b.name.trim()), false)?;
    let (s, profile) = market_settings(&state);
    let token = job.token.clone();
    let page = emitter(on_event);
    let emit = tee(&state.jobs, job.id, &page);
    let work = crate::brand::analyze(&state.db, &s, &profile, &b, &token, &emit);
    run_job(Some(&app), &state, job, work, |r| Ok(format!("اعتماد داده {}٪", r.confidence))).await
}

#[tauri::command]
pub async fn brand_runs(state: State<'_, AppState>) -> Res<Vec<crate::brand::BrandRunRow>> {
    state.db.brand_runs(200)
}

#[tauri::command]
pub async fn brand_run_get(state: State<'_, AppState>, id: i64) -> Res<crate::brand::BrandReport> {
    state.db.brand_run_get(id)
}

#[tauri::command]
pub async fn market_trends(state: State<'_, AppState>) -> Res<Vec<crate::market::Trend>> {
    state.db.market_trends(30)
}

/// Signs in to a platform (Divar, Sheypoor, Digikala…) in a visible window; the session is kept
/// in the browser profile the product searches use. Returns when the window is closed.
#[tauri::command]
pub async fn market_login(state: State<'_, AppState>, platform: String) -> Res<()> {
    let name = crate::market::platform(&platform).map_or(platform.as_str(), |p| p.name);
    let job = state.jobs.start("login", &format!("ورود به {name}"), false)?;
    let (s, profile) = market_settings(&state);
    let token = job.token.clone();
    let work = crate::market::login(&s, &profile, &platform, &token);
    run_job(None, &state, job, work, |_| Ok("پنجره ورود بسته شد؛ نشست ذخیره شد.".into())).await
}

// ---------------- daily watchlist & the seller's catalogue ----------------

fn market_settings(state: &AppState) -> (scanner::Settings, PathBuf) {
    (
        serde_json::from_value(Value::Object(settings(&state.db))).unwrap_or_default(),
        state.dir.join("browser-profile"),
    )
}

#[tauri::command]
pub async fn watch_list(state: State<'_, AppState>) -> Res<Vec<crate::watch::Watch>> {
    crate::watch::list(&state.db)
}

#[tauri::command]
pub async fn watch_save(state: State<'_, AppState>, item: crate::watch::Watch) -> Res<i64> {
    crate::watch::save(&state.db, &item)
}

#[tauri::command]
pub async fn watch_delete(state: State<'_, AppState>, id: i64) -> Res<()> {
    crate::watch::delete(&state.db, id)
}

/// The daily check time: read, or set when `at` is given.
#[tauri::command]
pub async fn watch_time(state: State<'_, AppState>, at: Option<String>) -> Res<String> {
    if let Some(at) = at {
        crate::watch::set_time(&state.db, at.trim())?;
    }
    Ok(crate::watch::time(&state.db))
}

/// Re-analyses watched products now (all enabled ones when `ids` is empty).
#[tauri::command]
pub async fn watch_run(state: State<'_, AppState>, ids: Vec<i64>, on_event: Channel<ScanEvent>) -> Res<Summary> {
    run_watch(&state, &ids, &emitter(on_event), false).await
}

/// Also used by the scheduler for the daily check.
pub async fn run_watch(state: &AppState, ids: &[i64], emit: Emit<'_>, scheduled: bool) -> Res<Summary> {
    license::ensure(&state.db)?;
    let job = state.jobs.start("watch", "پایش روزانه قیمت", scheduled)?;
    let (s, profile) = market_settings(state);
    let token = job.token.clone();
    let emit = tee(&state.jobs, job.id, emit);
    let work = crate::watch::run(&state.db, &s, &profile, ids, &token, &emit);
    run_job(None, state, job, work, summary_outcome).await
}

#[tauri::command]
pub async fn catalog_list(state: State<'_, AppState>) -> Res<Vec<crate::watch::Item>> {
    crate::watch::items(&state.db)
}

/// `data` = the Excel / CSV file as base64.
#[tauri::command]
pub async fn catalog_import(state: State<'_, AppState>, file_name: String, data: String) -> Res<crate::watch::CatalogImport> {
    if data.len() > 28_000_000 {
        return Err("فایل بزرگ‌تر از ۲۰ مگابایت است.".into());
    }
    let bytes = logic::b64_decode(&data).ok_or("فایل خوانده نشد.")?;
    crate::watch::import(&state.db, &file_name, bytes)
}

#[tauri::command]
pub async fn catalog_delete(state: State<'_, AppState>, ids: Vec<i64>) -> Res<()> {
    crate::watch::delete_items(&state.db, &ids)
}

#[tauri::command]
pub async fn catalog_check(state: State<'_, AppState>, ids: Vec<i64>, platforms: Vec<String>, margin: f64, on_event: Channel<ScanEvent>) -> Res<Summary> {
    license::ensure(&state.db)?;
    let job = state.jobs.start("catalog", "مقایسه کالاهای من با بازار", false)?;
    let (s, profile) = market_settings(&state);
    let token = job.token.clone();
    let page = emitter(on_event);
    let emit = tee(&state.jobs, job.id, &page);
    let work = crate::watch::check(&state.db, &s, &profile, &ids, &platforms, margin, &token, &emit);
    run_job(None, &state, job, work, summary_outcome).await
}

/// The business photo for the card, as a data: URL (the UI may only show local images).
#[tauri::command]
pub async fn place_photo(state: State<'_, AppState>, id: i64) -> Res<String> {
    let url = state.db.get(id)?.photo;
    if url.is_empty() {
        return Ok(String::new());
    }
    crate::enrich::image_data_url(&url).await
}

/// A photo of a city for the extraction banner: the lead image of its Persian Wikipedia article, as a
/// data: URL, cached on disk. Empty when there is none or Wikipedia is unreachable (the UI then keeps
/// the bundled province photo).
#[tauri::command]
pub async fn city_photo(state: State<'_, AppState>, city: String) -> Res<String> {
    let city = city.trim();
    if city.is_empty() || city.chars().count() > 40 {
        return Ok(String::new());
    }
    let dir = state.dir.join("cache").join("photos");
    let file = dir.join(format!("{}.txt", crate::logic::pct_encode(city).replace('%', "")));
    if let Ok(cached) = std::fs::read_to_string(&file) {
        return Ok(cached);
    }
    let api = format!(
        "https://fa.wikipedia.org/w/api.php?action=query&format=json&redirects=1&prop=pageimages&piprop=thumbnail&pithumbsize=960&titles={}",
        crate::logic::pct_encode(city)
    );
    let fetch = async {
        let v: serde_json::Value = crate::enrich::client().get(&api).send().await.ok()?.json().await.ok()?;
        let pages = v.pointer("/query/pages")?.as_object()?;
        let src = pages
            .values()
            .find_map(|p| p.pointer("/thumbnail/source").and_then(|s| s.as_str()))?
            .to_string();
        // maps, flags and seals are not photos
        let low = src.to_lowercase();
        if [".svg", "map", "flag", "seal", "logo", "location"].iter().any(|k| low.contains(k)) {
            return Some(String::new());
        }
        crate::enrich::image_data_url(&src).await.ok()
    };
    let out = tokio::time::timeout(std::time::Duration::from_secs(8), fetch).await.ok().flatten();
    match out {
        Some(data) => {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&file, &data); // "" is cached too: no photo, don't ask again
            Ok(data)
        }
        None => Ok(String::new()), // offline / filtered: try again next time
    }
}

// ---------------- export / import ----------------

#[tauri::command]
pub async fn export_data(app: AppHandle, state: State<'_, AppState>, filter: Filter, format: String) -> Res<String> {
    license::ensure(&state.db)?;
    let dir = app.path().download_dir().unwrap_or_else(|_| state.dir.join("exports")).join("HamistaSeller");
    let path = export::export(&state.db, &filter, &format, &dir)?;
    state.db.log("export", &format!("خروجی {} گرفته شد", format.to_uppercase()));
    Ok(path)
}

/// `data` = the file as base64 (read by the UI), so no file-picker plugin is needed.
#[tauri::command]
pub async fn import_data(state: State<'_, AppState>, list_id: i64, file_name: String, data: String) -> Res<export::ImportReport> {
    if !state.db.list_exists(list_id) {
        return Err("بانک مقصد را انتخاب کنید.".into());
    }
    if data.len() > 28_000_000 {
        return Err("فایل بزرگ‌تر از ۲۰ مگابایت است.".into());
    }
    let bytes = logic::b64_decode(&data).ok_or("فایل خوانده نشد.")?;
    export::import(&state.db, list_id, &file_name, bytes)
}

// ---------------- banks ----------------

fn bank_name(name: &str) -> Res<String> {
    let n = clean_text(name, 60, "نام بانک")?;
    if n.is_empty() {
        return Err("نام بانک را وارد کنید.".into());
    }
    Ok(n)
}

#[tauri::command]
pub async fn lists(state: State<'_, AppState>) -> Res<Vec<ListInfo>> {
    state.db.lists()
}

#[tauri::command]
pub async fn list_create(state: State<'_, AppState>, name: String, color: String) -> Res<i64> {
    let id = state.db.list_create(&bank_name(&name)?, &clean_text(&color, 20, "رنگ")?)?;
    state.db.log("bank", &format!("بانک «{}» ساخته شد", name.trim()));
    Ok(id)
}

#[tauri::command]
pub async fn list_update(state: State<'_, AppState>, id: i64, name: String, color: String, note: String) -> Res<()> {
    state
        .db
        .list_update(id, &bank_name(&name)?, &clean_text(&color, 20, "رنگ")?, &clean_text(&note, 300, "توضیح")?)
}

#[tauri::command]
pub async fn list_delete(state: State<'_, AppState>, id: i64) -> Res<()> {
    if !state.jobs.running().is_empty() {
        return Err("ابتدا عملیات در حال اجرا را متوقف کنید.".into());
    }
    let name = state.db.list_name(id);
    state.db.list_delete(id)?;
    state.db.log("bank", &format!("بانک «{name}» حذف شد"));
    Ok(())
}

#[tauri::command]
pub async fn transfer(state: State<'_, AppState>, ids: Vec<i64>, to: i64, copy: bool) -> Res<usize> {
    if !state.db.list_exists(to) {
        return Err("بانک مقصد پیدا نشد.".into());
    }
    state.db.transfer(&ids, to, copy)
}

// ---------------- places ----------------

#[derive(Serialize)]
pub struct PlacesPage {
    rows: Vec<Place>,
    total: i64,
}

#[tauri::command]
pub async fn places(state: State<'_, AppState>, filter: Filter, offset: i64, limit: i64) -> Res<PlacesPage> {
    let (rows, total) = state.db.list(&filter, offset.max(0), limit.clamp(1, 500))?;
    Ok(PlacesPage { rows, total })
}

#[tauri::command]
pub async fn place(state: State<'_, AppState>, id: i64) -> Res<Place> {
    state.db.get(id)
}

#[derive(Serialize)]
pub struct Saved {
    id: i64,
    duplicates: Vec<String>, // values another place in the bank already owns (skipped)
}

/// Manual add / edit. Every contact is normalised; invalid ones are rejected with a clear message.
#[tauri::command]
pub async fn place_save(state: State<'_, AppState>, place: Place) -> Res<Saved> {
    let mut p = place;
    p.name = clean_text(&p.name, 160, "نام کسب‌وکار")?;
    if p.name.is_empty() {
        return Err("نام کسب‌وکار را وارد کنید.".into());
    }
    if p.id == 0 && !state.db.list_exists(p.list_id) {
        return Err("بانک مقصد را انتخاب کنید.".into());
    }
    p.contact_person = clean_text(&p.contact_person, 80, "نام مسئول")?;
    p.contact_role = clean_text(&p.contact_role, 40, "سمت")?;
    p.note = clean_text(&p.note, 2000, "یادداشت")?;
    p.tags = logic::normalize_tags(&p.tags);
    if p.status.is_empty() {
        p.status = "new".into();
    }
    check_status(&p.status)?;
    let mut contacts: Vec<Contact> = vec![];
    for c in &p.contacts {
        if !KINDS.contains(&c.kind.as_str()) {
            return Err("نوع راه ارتباطی نامعتبر است.".into());
        }
        let value = logic::normalize_contact(&c.kind, &c.value).ok_or_else(|| format!("«{}» معتبر نیست.", c.value.trim()))?;
        if !contacts.iter().any(|x| x.kind == c.kind && x.value == value) {
            contacts.push(Contact {
                value,
                label: clean_text(&c.label, 30, "برچسب")?,
                ..c.clone()
            });
        }
    }
    p.contacts = contacts;
    if p.contacts.is_empty() {
        return Err("حداقل یک راه ارتباطی (شماره، ایمیل یا شبکه اجتماعی) وارد کنید.".into());
    }
    let (id, duplicates) = state.db.save_place(&p)?;
    Ok(Saved { id, duplicates })
}

#[tauri::command]
pub async fn place_status(state: State<'_, AppState>, id: i64, status: String, note: String) -> Res<()> {
    check_status(&status)?;
    state.db.update_status(id, &status, &clean_text(&note, 2000, "یادداشت")?)
}

#[tauri::command]
pub async fn places_status(state: State<'_, AppState>, ids: Vec<i64>, status: String) -> Res<usize> {
    check_status(&status)?;
    state.db.set_status(&ids, &status)
}

#[tauri::command]
pub async fn places_tag(state: State<'_, AppState>, ids: Vec<i64>, tag: String) -> Res<usize> {
    let tag = logic::normalize_tags(&tag);
    if tag.is_empty() || tag.contains(',') {
        return Err("یک برچسب وارد کنید.".into());
    }
    state.db.add_tag(&ids, &tag)
}

#[tauri::command]
pub async fn places_delete(state: State<'_, AppState>, ids: Vec<i64>) -> Res<usize> {
    let n = state.db.delete(&ids)?;
    state.db.log("delete", &format!("{n} مخاطب حذف شد"));
    Ok(n)
}

#[tauri::command]
pub async fn facets(state: State<'_, AppState>, list_id: i64) -> Res<Facets> {
    state.db.facets(list_id)
}

/// «تحلیل» of a bank (or of the current filter): province, county, line of business, status, channels.
#[tauri::command]
pub async fn bank_stats(state: State<'_, AppState>, filter: Filter) -> Res<crate::db::BankStats> {
    state.db.bank_stats(&filter)
}

#[tauri::command]
pub async fn dashboard(state: State<'_, AppState>) -> Res<Dashboard> {
    state.db.dashboard()
}

#[tauri::command]
pub async fn runs(state: State<'_, AppState>) -> Res<Vec<Run>> {
    state.db.runs()
}

#[tauri::command]
pub async fn campaigns(state: State<'_, AppState>) -> Res<Vec<Campaign>> {
    state.db.campaigns()
}

#[tauri::command]
pub async fn activity(state: State<'_, AppState>, limit: i64) -> Res<Vec<Activity>> {
    state.db.activity(limit.clamp(1, 200))
}

// ---------------- settings ----------------

/// Stored as one JSON object; each engine reads its own keys with defaults.
pub fn settings(db: &Db) -> Map<String, Value> {
    db.kv_get("settings").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Res<Map<String, Value>> {
    Ok(settings(&state.db))
}

#[tauri::command]
pub async fn settings_save(state: State<'_, AppState>, patch: Map<String, Value>) -> Res<Map<String, Value>> {
    let mut s = settings(&state.db);
    s.extend(patch);
    state.db.kv_set("settings", &serde_json::to_string(&s).map_err(err)?)?;
    Ok(s)
}

// ---------------- sending accounts ----------------

const ACCOUNT_KINDS: &[&str] = &["sms"];

#[tauri::command]
pub async fn accounts(state: State<'_, AppState>) -> Res<Vec<Account>> {
    state.db.accounts()
}

/// `secret` (API key, passwords, tokens) is encrypted with DPAPI; `null` keeps the stored one.
#[tauri::command]
pub async fn account_save(state: State<'_, AppState>, account: Account, secret: Option<Map<String, Value>>) -> Res<()> {
    let mut a = account;
    if !ACCOUNT_KINDS.contains(&a.kind.as_str()) {
        return Err("نوع حساب نامعتبر است.".into());
    }
    a.name = clean_text(&a.name, 40, "نام حساب")?;
    if a.name.is_empty() {
        return Err("برای حساب یک نام وارد کنید.".into());
    }
    if a.id.is_empty() {
        a.id = format!("{}-{}", a.kind, state.db.stamp());
    }
    a.per_minute = a.per_minute.clamp(1, 1200);
    let sealed = match secret {
        Some(s) => Some(secrets::seal_str(&Value::Object(s).to_string())?),
        None => None,
    };
    state.db.account_save(&a, sealed)?;
    state.db.log("account", &format!("حساب «{}» ذخیره شد", a.name));
    Ok(())
}

#[tauri::command]
pub async fn account_delete(state: State<'_, AppState>, id: String) -> Res<()> {
    state.db.account_delete(&id)
}

/// Decrypted secret for the sending engines (never sent to the UI).
pub fn account_secret(db: &Db, id: &str) -> Res<Map<String, Value>> {
    match db.account_secret(id)? {
        Some(blob) => serde_json::from_str(&secrets::open_str(&blob)?).map_err(err),
        None => Ok(Map::new()),
    }
}

// ---------------- templates ----------------

#[tauri::command]
pub async fn templates(state: State<'_, AppState>) -> Res<Vec<Template>> {
    state.db.templates()
}

#[tauri::command]
pub async fn template_save(state: State<'_, AppState>, template: Template) -> Res<i64> {
    let mut t = template;
    t.name = clean_text(&t.name, 60, "نام قالب")?;
    t.subject = clean_text(&t.subject, 150, "موضوع")?;
    t.body = clean_text(&t.body, 4000, "متن پیام")?;
    if t.name.is_empty() || t.body.is_empty() {
        return Err("نام و متن قالب را وارد کنید.".into());
    }
    state.db.template_save(&t)
}

#[tauri::command]
pub async fn template_delete(state: State<'_, AppState>, id: i64) -> Res<()> {
    state.db.template_delete(id)
}

// ---------------- do-not-contact ----------------

#[tauri::command]
pub async fn optouts(state: State<'_, AppState>) -> Res<Vec<String>> {
    state.db.optouts()
}

#[tauri::command]
pub async fn optouts_save(state: State<'_, AppState>, text: String) -> Res<usize> {
    let mut v: Vec<String> = text
        .split(['\n', ',', '،', ';'])
        .map(|s| {
            let s = s.trim();
            if s.contains('@') {
                s.to_lowercase()
            } else {
                logic::normalize_phone(s)
            }
        })
        .filter(|s| !s.is_empty())
        .collect();
    v.sort();
    v.dedup();
    state.db.set_optouts(&v)
}

// ---------------- backups ----------------

pub fn backups_dir(state: &AppState) -> PathBuf {
    state.dir.join("backups")
}

#[derive(Serialize)]
pub struct BackupFile {
    name: String,
    size: u64,
}

#[tauri::command]
pub async fn backups(state: State<'_, AppState>) -> Res<Vec<BackupFile>> {
    let mut v: Vec<BackupFile> = std::fs::read_dir(backups_dir(&state))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "db"))
                .map(|e| BackupFile {
                    name: e.file_name().to_string_lossy().into_owned(),
                    size: e.metadata().map(|m| m.len()).unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(v)
}

pub fn make_backup(state: &AppState, prefix: &str) -> Res<String> {
    let dir = backups_dir(state);
    std::fs::create_dir_all(&dir).map_err(err)?;
    let name = format!("{prefix}-{}.db", state.db.stamp());
    state.db.backup_to(&dir.join(&name))?;
    // keep the 10 newest of each kind
    let mut same: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(err)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{prefix}-"))))
        .collect();
    same.sort();
    for old in same.iter().rev().skip(10) {
        let _ = std::fs::remove_file(old);
    }
    Ok(name)
}

#[tauri::command]
pub async fn backup_now(state: State<'_, AppState>) -> Res<String> {
    let name = make_backup(&state, "manual")?;
    state.db.log("backup", "پشتیبان دستی گرفته شد");
    Ok(name)
}

#[tauri::command]
pub async fn backup_restore(state: State<'_, AppState>, name: String) -> Res<()> {
    if !state.jobs.running().is_empty() {
        return Err("ابتدا عملیات در حال اجرا را متوقف کنید.".into());
    }
    if name.contains(['/', '\\', ':']) || !name.ends_with(".db") {
        return Err("نام فایل پشتیبان نامعتبر است.".into());
    }
    let src = backups_dir(&state).join(&name);
    if !src.is_file() {
        return Err("فایل پشتیبان پیدا نشد.".into());
    }
    // safety copy of the current data first (its own prefix, so it never prunes `src`)
    make_backup(&state, "restore")?;
    state.db.restore_from(&src)?;
    state.db.log("backup", &format!("داده‌ها از پشتیبان «{name}» بازگردانی شد"));
    Ok(())
}

// ---------------- shell ----------------

#[tauri::command]
pub async fn reveal(path: String) -> Res<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.replace('"', "")))
            .spawn()
            .map_err(err)?;
    }
    #[cfg(not(windows))]
    {
        let p = std::path::Path::new(&path);
        let dir = if p.is_dir() { p } else { p.parent().unwrap_or(std::path::Path::new(".")) };
        std::process::Command::new(if cfg!(target_os = "macos") { "open" } else { "xdg-open" })
            .arg(dir)
            .spawn()
            .map_err(err)?;
    }
    Ok(())
}

/// Opens web links (map, site, profile) in the default browser.
#[tauri::command]
pub async fn open_url(url: String) -> Res<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.contains(['"', '\n', ' ']) {
        return Err("آدرس نامعتبر است.".into());
    }
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new(if cfg!(target_os = "macos") { "open" } else { "xdg-open" })
        .arg(&url)
        .spawn();
    r.map(|_| ()).map_err(err)
}

#[tauri::command]
pub async fn logs_folder(state: State<'_, AppState>) -> Res<String> {
    Ok(state.dir.join("logs").to_string_lossy().into_owned())
}

// ---------------- online account (customer copy) ----------------

fn customer_only() -> Res<()> {
    if license::MASTER || license::ISSUER {
        Err("این بخش مخصوص نسخه مشتری است.".into())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn cloud_login(state: State<'_, AppState>, url: String, username: String, password: String) -> Res<license::Status> {
    customer_only()?;
    let st = cloud::login(&state.db, &url, &username, &password).await?;
    let _ = cloud::sync_all(&state.db).await; // first upload + scenario right away; failures are retried later
    Ok(st)
}

#[tauri::command]
pub async fn cloud_logout(state: State<'_, AppState>) -> Res<license::Status> {
    customer_only()?;
    Ok(cloud::logout(&state.db))
}

#[tauri::command]
pub async fn cloud_sync(state: State<'_, AppState>) -> Res<license::Status> {
    customer_only()?;
    cloud::sync_all(&state.db).await
}

#[tauri::command]
pub async fn cloud_notifications(state: State<'_, AppState>) -> Res<Value> {
    if state.db.kv_get("cloud_token").is_none() {
        return Ok(serde_json::json!({ "items": [] }));
    }
    cloud::notifications(&state.db).await
}

#[tauri::command]
pub async fn cloud_notification_read(state: State<'_, AppState>, id: i64) -> Res<()> {
    cloud::notification_read(&state.db, id).await
}

#[tauri::command]
pub async fn cloud_version(state: State<'_, AppState>) -> Res<Value> {
    cloud::agent_version(&state.db).await
}

#[tauri::command]
pub async fn mega_search(state: State<'_, AppState>, search: cloud::MegaSearch) -> Res<Value> {
    let st = license::status(&state.db);
    if !(st.valid && st.megabank) {
        return Err("دسترسی به بانک جامع در اشتراک شما فعال نیست.".into());
    }
    cloud::mega_search(&state.db, &search).await
}

#[derive(Serialize)]
pub struct Imported {
    places: usize,
    contacts: usize,
}

#[tauri::command]
pub async fn mega_import(state: State<'_, AppState>, list_id: i64, rows: Vec<Value>) -> Res<Imported> {
    license::ensure(&state.db)?;
    if !state.db.list_exists(list_id) {
        return Err("بانک مقصد را انتخاب کنید.".into());
    }
    if rows.len() > 1000 {
        return Err("هر بار حداکثر ۱۰۰۰ مورد.".into());
    }
    let (places, contacts) = cloud::mega_import(&state.db, list_id, &rows)?;
    state.db.log("megabank", &format!("{places} کسب‌وکار از بانک جامع اضافه شد"));
    Ok(Imported { places, contacts })
}

/// Which settings of a source are filled in (values are never sent back to the page).
#[tauri::command]
pub async fn source_config(state: State<'_, AppState>, id: String) -> Res<Map<String, Value>> {
    Ok(sources::config_status(&state.db, &id))
}

#[tauri::command]
pub async fn source_config_save(state: State<'_, AppState>, id: String, values: Map<String, Value>) -> Res<Map<String, Value>> {
    sources::config_save(&state.db, &id, &values)?;
    state.db.log(
        "settings",
        &format!("تنظیمات ماژول «{}» ذخیره شد", sources::get(&id).map_or(id.as_str(), |s| s.name)),
    );
    Ok(sources::config_status(&state.db, &id))
}

#[tauri::command]
pub async fn sources_list(state: State<'_, AppState>) -> Res<Vec<Value>> {
    let st = license::status(&state.db);
    let counts = sources::stats(&state.db);
    Ok(sources::SOURCES
        .iter()
        .map(|s| {
            let mut v = serde_json::to_value(s).unwrap_or_default();
            v["allowed"] = Value::Bool(license::source_allowed(&st, s.id).is_ok());
            v["enabled"] = Value::Bool(sources::enabled(&state.db, s.id));
            v["switchable"] = Value::Bool(sources::switchable(s.id));
            v["config"] = Value::Object(sources::config_status(&state.db, s.id));
            let (total, recent) = counts.get(s.id).copied().unwrap_or((0, 0));
            v["places"] = Value::from(total);
            v["recent"] = Value::from(recent);
            v
        })
        .collect())
}

/// Settings → Modules: switch a source on or off.
#[tauri::command]
pub async fn source_toggle(state: State<'_, AppState>, id: String, on: bool) -> Res<()> {
    sources::set_enabled(&state.db, &id, on)?;
    let name = sources::get(&id).map_or(id.as_str(), |s| s.name);
    state.db.log("settings", &format!("ماژول «{name}» {}", if on { "روشن شد" } else { "خاموش شد" }));
    Ok(())
}

// ---------------- admin panel (issuer edition) ----------------

#[tauri::command]
pub async fn admin_stats(state: State<'_, AppState>) -> Res<admin::Stats> {
    issuer_only()?;
    admin::stats(&state.db)
}

#[tauri::command]
pub async fn admin_customers(state: State<'_, AppState>, q: String, view: String) -> Res<Vec<admin::Customer>> {
    issuer_only()?;
    admin::customers(&state.db, &q, &view)
}

#[tauri::command]
pub async fn admin_customer_save(state: State<'_, AppState>, customer: admin::Customer) -> Res<i64> {
    issuer_only()?;
    let id = admin::customer_save(&state.db, &customer)?;
    state.db.log("customer", &format!("مشتری «{}» ذخیره شد", customer.name.trim()));
    Ok(id)
}

#[tauri::command]
pub async fn admin_customer_delete(state: State<'_, AppState>, id: i64) -> Res<()> {
    issuer_only()?;
    let c = admin::customer(&state.db, id)?;
    admin::customer_delete(&state.db, id)?;
    state.db.log("customer", &format!("مشتری «{}» حذف شد", c.name));
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issued {
    code: String,
    customer_id: i64,
}

#[tauri::command]
pub async fn admin_issue(state: State<'_, AppState>, customer_id: i64, machine: String, customer: String, plan: String, expires: String) -> Res<Issued> {
    issuer_only()?;
    let (code, customer_id) = admin::issue(
        &state.db,
        customer_id,
        &machine,
        &clean_text(&customer, 80, "نام مشتری")?,
        &clean_text(&plan, 40, "نوع لایسنس")?,
        expires.trim(),
    )?;
    Ok(Issued { code, customer_id })
}

#[tauri::command]
pub async fn admin_licences(state: State<'_, AppState>, customer_id: i64, limit: i64) -> Res<Vec<admin::Licence>> {
    issuer_only()?;
    admin::licences(&state.db, customer_id, limit)
}

/// CSV of the local customers or (online) of the server's mega-bank, saved to Downloads.
#[tauri::command]
pub async fn admin_export(app: AppHandle, state: State<'_, AppState>, kind: String, filter: Option<Value>) -> Res<String> {
    issuer_only()?;
    let text = match kind.as_str() {
        "customers" => admin::customers_csv(&state.db)?,
        "megabank" => cloud::admin_call(&state.db, "POST", "/megabank/export", Some(filter.unwrap_or(serde_json::json!({})))).await?["text"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        _ => return Err("نوع خروجی نامعتبر است.".into()),
    };
    let dir = app.path().download_dir().unwrap_or_else(|_| state.dir.join("exports")).join("HamistaAdmin");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let path = dir.join(format!("{kind}-{}.csv", state.db.stamp()));
    std::fs::write(&path, text).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn admin_server(state: State<'_, AppState>) -> Res<cloud::AdminConn> {
    issuer_only()?;
    let key = license::key_info(&state.db)?.public_key;
    match cloud::admin_status(&state.db, &key).await {
        Ok(c) => Ok(c),
        Err(_) => Ok(cloud::admin_conn_info(&state.db)), // offline: the panel still opens
    }
}

#[tauri::command]
pub async fn admin_connect(state: State<'_, AppState>, url: String, username: String, password: String) -> Res<cloud::AdminConn> {
    issuer_only()?;
    let key = license::key_info(&state.db)?.public_key;
    cloud::admin_connect(&state.db, &url, &username, &password, &key).await
}

#[tauri::command]
pub async fn admin_disconnect(state: State<'_, AppState>) -> Res<()> {
    issuer_only()?;
    cloud::admin_disconnect(&state.db);
    Ok(())
}

/// Thin proxy to the server's admin API: `path` is relative to /api/v1/admin (e.g. `/users`).
#[tauri::command]
pub async fn admin_api(state: State<'_, AppState>, method: String, path: String, body: Option<Value>) -> Res<Value> {
    issuer_only()?;
    cloud::admin_call(&state.db, &method.to_uppercase(), &path, body).await
}

// ---------------- background & schedules ----------------

#[tauri::command]
pub async fn background_get(state: State<'_, AppState>) -> Res<crate::background::Prefs> {
    Ok(crate::background::prefs(&state))
}

#[tauri::command]
pub async fn background_set(state: State<'_, AppState>, prefs: crate::background::Prefs) -> Res<crate::background::Prefs> {
    let p = crate::background::set_prefs(&state, prefs)?;
    state.db.log(
        "settings",
        &format!(
            "اجرا در پس‌زمینه: {} · همراه ویندوز: {}",
            if p.tray { "روشن" } else { "خاموش" },
            if p.autostart { "روشن" } else { "خاموش" }
        ),
    );
    Ok(p)
}

#[tauri::command]
pub async fn schedules(state: State<'_, AppState>) -> Res<Vec<crate::schedule::Schedule>> {
    crate::schedule::list(&state.db)
}

#[tauri::command]
pub async fn schedule_save(state: State<'_, AppState>, schedule: crate::schedule::Schedule) -> Res<i64> {
    let id = crate::schedule::save(&state.db, &schedule)?;
    state.db.log("schedule", &format!("زمان‌بندی «{}» ذخیره شد", schedule.name.trim()));
    Ok(id)
}

#[tauri::command]
pub async fn schedule_toggle(state: State<'_, AppState>, id: i64, on: bool) -> Res<()> {
    crate::schedule::set_enabled(&state.db, id, on)
}

#[tauri::command]
pub async fn schedule_delete(state: State<'_, AppState>, id: i64) -> Res<()> {
    crate::schedule::delete(&state.db, id)
}

// ---------------- support report ----------------

/// A text file for the support team: versions, licence state, sources, recent runs and the last
/// log lines. Passwords, keys and tokens are never in it (settings with such names are dropped).
#[tauri::command]
pub async fn support_report(app: AppHandle, state: State<'_, AppState>) -> Res<String> {
    let st = license::status(&state.db);
    let mut out = String::new();
    let mut line = |k: &str, v: String| out.push_str(&format!("{k}: {v}\n"));
    line("برنامه", format!("هامیستا سلر {} ({})", env!("CARGO_PKG_VERSION"), license::EDITION));
    line("سیستم", format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));
    line("پوشه داده", state.dir.display().to_string());
    line("زمان", state.db.stamp());
    line("کد سیستم", st.machine.clone());
    line(
        "لایسنس",
        format!(
            "{} · {} · {} · تا {} · {}",
            if st.valid { "معتبر" } else { "نامعتبر" },
            st.mode,
            st.plan,
            st.expires,
            st.message
        ),
    );
    let lists = state.db.lists().unwrap_or_default();
    line(
        "بانک‌ها",
        lists.iter().map(|l| format!("{} ({})", l.name, l.count)).collect::<Vec<_>>().join("، "),
    );
    for s in sources::SOURCES {
        let cfg = sources::config_status(&state.db, s.id);
        line(
            &format!("ماژول {}", s.id),
            format!(
                "{} {}",
                if sources::enabled(&state.db, s.id) { "روشن" } else { "خاموش" },
                if cfg.is_empty() { String::new() } else { Value::Object(cfg).to_string() }
            ),
        );
    }
    let safe_settings: Map<String, Value> = settings(&state.db)
        .into_iter()
        .filter(|(k, _)| !["key", "token", "pass", "secret"].iter().any(|w| k.to_lowercase().contains(w)))
        .collect();
    line("تنظیمات", Value::Object(safe_settings).to_string());
    out.push_str("\n— آخرین استخراج‌ها —\n");
    for r in state.db.runs().unwrap_or_default().iter().take(15) {
        out.push_str(&format!(
            "{} | {} | {} {} | {}/{} | {} {}\n",
            r.started_at, r.keyword, r.province, r.city, r.found, r.checked, r.status, r.error
        ));
    }
    out.push_str("\n— گزارش برنامه (آخرین ۴۰۰ خط) —\n");
    let log = std::fs::read_to_string(state.dir.join("logs").join("hamista.log")).unwrap_or_default();
    let lines: Vec<&str> = log.lines().collect();
    out.push_str(&lines[lines.len().saturating_sub(400)..].join("\n"));
    let dir = app.path().download_dir().unwrap_or_else(|_| state.dir.join("exports")).join("HamistaSeller");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let path = dir.join(format!("hamista-support-{}.txt", state.db.stamp()));
    std::fs::write(&path, out).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn cloud_trial_info(url: String) -> Res<Value> {
    customer_only()?;
    cloud::trial_info(&url).await
}

#[tauri::command]
pub async fn cloud_trial(state: State<'_, AppState>, url: String, name: String, phone: String) -> Res<license::Status> {
    customer_only()?;
    let st = cloud::start_trial(&state.db, &url, &name, &phone).await?;
    let _ = cloud::sync_all(&state.db).await;
    Ok(st)
}
