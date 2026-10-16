//! Google Maps engine.
//! * one tab scrolls the result list, N tabs read place pages in parallel, and a pool of
//!   plain-HTTP workers reads business websites (no tab is spent on a slow site);
//! * headless by default — no browser window appears while extracting;
//! * Google captcha → a visible window opens only for that; once solved the scan carries on;
//! * a crashed browser is restarted (twice at most) and the keyword continues;
//! * progress is checkpointed into `runs.resume`, so «ادامه استخراج» can pick up later.
//!
//! Everything races a CancellationToken, so Stop ends all in-flight work at once.
use crate::db::{err, Contact, Db, Place, Res};
use crate::logic::{coords, jitter, place_key, plan_queries, search_url, Contacts};
use chromiumoxide::browser::Browser;
use chromiumoxide::cdp::browser_protocol::network::SetUserAgentOverrideParams;
use chromiumoxide::cdp::js_protocol::runtime::EvaluateParams;
use chromiumoxide::error::CdpError;
use chromiumoxide::Page;
use futures::StreamExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

/// Global signal triggered when the user clicks «ادامه / رد کردن این مرحله» during a captcha/login wait.
static SKIP_WAIT: AtomicBool = AtomicBool::new(false);

pub fn clear_wait_skip() {
    SKIP_WAIT.store(false, Relaxed);
}

pub fn trigger_wait_skip() {
    SKIP_WAIT.store(true, Relaxed);
}

pub fn take_wait_skip() -> bool {
    SKIP_WAIT.swap(false, Relaxed)
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub browser_path: String,
    pub concurrency: usize,
    pub headless: bool,
    pub delay_ms: u64,
    pub expand_areas: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            browser_path: String::new(),
            concurrency: 3,
            headless: true,
            delay_ms: 400,
            expand_areas: true,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct ScanRequest {
    pub list_id: i64,
    pub enrich: bool, // also read the business website for email / socials / manager name
    pub keywords: Vec<String>,
    pub province: String,
    pub city: String,
    pub limit: i64,       // places to check per keyword
    pub first_limit: i64, // resume only: what is left of the first keyword (0 = `limit`)
    pub source: String,   // registry id (sources.rs); "" = maps
    /// Exact look-ups instead of a keyword search: "<business name> <city>" per entry, first
    /// matching Maps result only (used by sources that know names but not phone numbers, e.g. Neshan).
    pub lookups: Vec<String>,
    /// Neshan: keywords still to search after these look-ups (so a resumed run finishes them too).
    pub rest: Vec<String>,
    /// Several sources in one job, one after another ([] = just `source`).
    pub sources: Vec<String>,
    /// «شهر / منطقه»: one exact city, district or neighbourhood inside `city` (no widening).
    pub district: String,
    /// «همه شهرهای استان»: every county of the province, searched one after another.
    pub areas: Vec<String>,
}

impl ScanRequest {
    /// Where the search happens: "district county" / county / province.
    pub fn area(&self) -> String {
        let (d, c, p) = (self.district.trim(), self.city.trim(), self.province.trim());
        if !d.is_empty() {
            format!("{d} {}", if c.is_empty() { p } else { c }).trim().to_string()
        } else if !c.is_empty() {
            c.to_string()
        } else {
            p.to_string()
        }
    }

    /// City shown in the history: the exact district when there is one.
    pub fn run_city(&self) -> String {
        if self.district.trim().is_empty() { self.city.trim().to_string() } else { self.area() }
    }

    /// Search queries for one keyword, and whether they are exactly what the user asked for
    /// (an exact district, or every county of the province) and so never trimmed.
    pub fn queries(&self, kw: &str) -> (Vec<String>, bool) {
        let kw = kw.trim();
        if !self.district.trim().is_empty() {
            return (vec![format!("{kw} {}", self.area())], true);
        }
        if self.city.trim().is_empty() {
            let mut q: Vec<String> = vec![];
            for a in self.areas.iter().map(|a| a.trim()).filter(|a| !a.is_empty()) {
                let one = format!("{kw} {a}");
                if !q.contains(&one) {
                    q.push(one);
                }
            }
            if !q.is_empty() {
                return (q, true);
            }
        }
        (plan_queries(kw, &self.province, &self.city), false)
    }
}

#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ScanEvent {
    Status {
        message: String,
    },
    Progress {
        keyword: String,
        collected: i64,
        checked: i64,
        found: i64,
        target: i64,
    },
    Found {
        place: Box<Place>,
    },
    Log {
        level: &'static str,
        message: String,
    },
    Captcha {
        message: String,
    }, // a visible browser window is waiting for the user
    Sending {
        sent: i64,
        failed: i64,
        skipped: i64,
        total: i64,
    },
}

#[derive(Serialize, Default)]
pub struct Summary {
    pub checked: i64,
    pub found: i64,
    pub cancelled: bool,
    pub error: bool,
    pub message: String,
}

pub type Emit<'a> = &'a (dyn Fn(ScanEvent) + Send + Sync);

pub fn status(emit: Emit, m: impl Into<String>) {
    emit(ScanEvent::Status { message: m.into() });
}
pub fn log(emit: Emit, level: &'static str, m: impl Into<String>) {
    emit(ScanEvent::Log { level, message: m.into() });
}

/// Also accepts `"__skip_wait__"` from the UI to immediately advance past any active captcha/login wait
/// without needing a separate IPC command lock.
pub fn detect_browser(custom: &str) -> Option<String> {
    if custom == "__skip_wait__" {
        trigger_wait_skip();
        return None;
    }
    browsers(custom).into_iter().next()
}

const CAPTCHA_WAIT: &str = "منبع داده تأیید «ربات نیستم» خواست. یک پنجره مرورگر باز شد؛ تأیید را در آن انجام دهید یا دکمه «ادامه» را بزنید تا استخراج ادامه پیدا کند.";
const CAPTCHA_GIVE_UP: &str = "منبع داده چند بار پشت‌سرهم تأیید خواست. چند دقیقه صبر کنید یا تعداد تب‌ها را در تنظیمات کم کنید؛ داده‌های ذخیره‌شده حفظ شده‌اند.";
const BROWSER_DIED: &str = "مرورگر چند بار از کار افتاد؛ داده‌های ذخیره‌شده حفظ شده‌اند.";
const ENRICHERS: usize = 4;

// ---- page scripts (DOM of Google Maps; stable attributes first) ----
const LIST_JS: &str = r#"
const feed = document.querySelector('div[role="feed"]');
const seen = new Set(), items = [];
const anchors = feed ? feed.querySelectorAll('a[href*="/maps/place/"]') : document.querySelectorAll('a.hfpxzc[href*="/maps/place/"]');
for (const a of anchors) {
  if (seen.has(a.href)) continue;
  seen.add(a.href);
  items.push([a.href, a.getAttribute('aria-label') || '']);
}
const tail = feed ? feed.innerText.slice(-600) : '';
return {
  href: location.href,
  items,
  end: /پایان فهرست|end of the list/i.test(tail),
  place: !feed && location.pathname.includes('/maps/place/') && !!document.querySelector('h1'),
  consent: location.hostname.startsWith('consent.'),
  blocked: location.pathname.startsWith('/sorry') || !!document.querySelector('#captcha-form'),
};"#;

const SCROLL_JS: &str = r#"
let el = document.querySelector('div[role="feed"]');
if (!el) {
  el = document.querySelector('a.hfpxzc');
  while (el && !(el.scrollHeight > el.clientHeight && /(auto|scroll)/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
}
if (!el) return false;
el.scrollTop = el.scrollHeight;
el.dispatchEvent(new WheelEvent('wheel', { deltaY: 1200, bubbles: true }));
return true;"#;

const CONSENT_JS: &str = r#"
const b = document.querySelector('form[action*="consent"] button');
if (b) b.click();
return !!b;"#;

const PLACE_JS: &str = r#"
const q = s => document.querySelector(s);
const clean = s => (s || '').replace(/[\uE000-\uF8FF\u200e\u200f]/g, '').replace(/\s+/g, ' ').trim();
const latin = s => (s || '').replace(/[۰-۹]/g, d => '۰۱۲۳۴۵۶۷۸۹'.indexOf(d)).replace(/[٠-٩]/g, d => '٠١٢٣٤٥٦٧٨٩'.indexOf(d));
const h1 = q('h1');
const ad = q('[data-item-id="address"]');
const f7 = q('div.F7nice');
const nums = f7 ? latin(f7.innerText).replace(/٫/g, '.').replace(/[٬,](?=\d{3}\b)/g, '').match(/\d+(\.\d+)?/g) || [] : [];
const phones = [...document.querySelectorAll('[data-item-id^="phone:tel:"]')].map(b => (b.getAttribute('data-item-id') || '').slice(10) || clean(b.innerText));
const links = [...document.querySelectorAll('a[data-item-id="authority"], a[data-item-id^="action:"], a[href*="instagram.com"], a[href*="t.me/"], a[href*="wa.me/"], a[href*="eitaa.com/"], a[href*="ble.ir/"], a[href*="rubika.ir/"]')].map(a => a.href);
return {
  href: location.href,
  ready: !!(h1 && h1.innerText.trim()),
  blocked: location.pathname.startsWith('/sorry') || !!q('#captcha-form'),
  name: clean(h1 && h1.innerText),
  phones,
  links,
  address: ad ? (clean(ad.innerText) || clean((ad.getAttribute('aria-label') || '').replace(/^[^:]*:/, ''))) : '',
  category: clean((q('button[jsaction*="category"]') || {}).innerText),
  rating: nums[0] ? (parseFloat(nums[0]) || null) : null,
  reviews: nums[1] ? (parseInt(nums[1], 10) || null) : null,
  photo: ((q('button[jsaction*="heroHeaderImage"] img') || q('.RZ66Rb img') || {}).src || ''),
};"#;

#[derive(Deserialize, Default)]
#[serde(default)]
struct ListState {
    href: String,
    items: Vec<(String, String)>,
    end: bool,
    place: bool,
    consent: bool,
    blocked: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PlaceInfo {
    href: String,
    ready: bool,
    blocked: bool,
    name: String,
    phones: Vec<String>,
    links: Vec<String>,
    address: String,
    category: String,
    rating: Option<f64>,
    reviews: Option<i64>,
    photo: String,
}

/// Google Maps search page. `HAMISTA_MAPS_BASE` points it at a local mock (end-to-end test only).
fn maps_search(query: &str) -> String {
    match crate::logic::test_seam("HAMISTA_MAPS_BASE").ok_or(()) {
        Ok(base) => format!("{base}/maps/search/{}", crate::logic::pct_encode(query)),
        Err(_) => search_url(query),
    }
}

// ---------------- remote fixes («سناریو») ----------------
// When Google changes its page layout, the admin publishes replacement page scripts on the server
// ({"listJs","scrollJs","consentJs","placeJs"}); they run only inside the Maps tab, like the built-in ones.

static SCENARIO: Mutex<Option<serde_json::Value>> = Mutex::new(None);

pub fn set_scenario(body: &serde_json::Value) {
    *SCENARIO.lock().unwrap_or_else(|e| e.into_inner()) = body.is_object().then(|| body.clone());
}

pub(crate) fn script(name: &str, built_in: &str) -> String {
    SCENARIO
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|b| b[name].as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(built_in)
        .to_string()
}

/// Runs a script body and returns its value; JSON string transport works with every CDP version.
pub(crate) async fn js<T: DeserializeOwned>(page: &Page, body: &str) -> Res<T> {
    let expr = format!("JSON.stringify((() => {{{body}\n}})())");
    let r = page.evaluate_expression(EvaluateParams::new(expr)).await.map_err(err)?;
    let s: String = r.into_value().map_err(err)?;
    serde_json::from_str(&s).map_err(err)
}

/// Navigation errors/timeouts are ignored on purpose: callers poll the DOM with their own deadline.
pub(crate) async fn goto(page: &Page, url: &str) {
    let _ = tokio::time::timeout(Duration::from_secs(15), page.goto(url)).await;
}

// ---------------- browser ----------------

pub(crate) struct Engine {
    pub(crate) browser: Browser,
    pub(crate) alive: Arc<AtomicBool>,
    pub(crate) list: Page,
    pub(crate) workers: Vec<Page>,
    pub(crate) proc: Proc,
    /// Holds the shared browser profile for this job while the browser is open (see `Lease`).
    pub(crate) _lease: Option<Lease>,
}

tokio::task_local! {
    /// The job (commands.rs job registry) the current task belongs to; 0 outside a job.
    pub static JOB_ID: u64;
}

/// The id of the job running this task (0 when not inside a job).
pub fn job_id() -> u64 {
    JOB_ID.try_with(|v| *v).unwrap_or(0)
}

/// One browser profile (logins, cookies) is shared by every job, but a profile can only be open in
/// one browser at a time. Jobs therefore take turns: a job holds the profile while any of its
/// browsers is open; another job's `launch` waits (cancellable) until it is released. The same job
/// may launch again (e.g. a visible window after closing its headless one).
static PROFILE: Mutex<Option<(u64, usize)>> = Mutex::new(None);

pub(crate) struct Lease(u64);

impl Lease {
    fn try_take(job: u64) -> Option<Lease> {
        let mut g = PROFILE.lock().unwrap_or_else(|e| e.into_inner());
        match *g {
            None => *g = Some((job, 1)),
            Some((owner, n)) if owner == job => *g = Some((job, n + 1)),
            Some(_) => return None,
        }
        Some(Lease(job))
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut g = PROFILE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((owner, n)) = *g {
            if owner == self.0 {
                *g = if n > 1 { Some((owner, n - 1)) } else { None };
            }
        }
    }
}

/// Waits until this job may open the shared browser profile (callers race their Stop token).
async fn take_profile() -> Lease {
    let job = job_id();
    loop {
        if let Some(l) = Lease::try_take(job) {
            return l;
        }
        sleep(Duration::from_millis(250)).await;
    }
}

/// The browser process we started; killed when dropped so no browser is ever left behind.
pub(crate) struct Proc(std::process::Child);

impl Proc {
    pub(crate) fn exited(&mut self) -> bool {
        matches!(self.0.try_wait(), Ok(Some(_)))
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if !self.exited() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// Starts the browser, trying hard before giving up:
/// every installed browser (the one in Settings first, then Edge, then Chrome), and for each the
/// normal profile and then a fresh one. A browser that exits at once (another Edge/Chrome holds the
/// profile, or the browser handed the window to a running copy) is closed and retried.
pub(crate) async fn launch(s: &Settings, profile: &Path, headless: bool, tabs: usize) -> Res<Engine> {
    // the start-up clean-up may still be running: never start a browser it would then end
    while matches!(CLEANING.try_lock(), Err(std::sync::TryLockError::WouldBlock)) {
        sleep(Duration::from_millis(100)).await;
    }
    // another job has the browser profile open: wait for it (the caller races its own Stop token)
    let mut lease = Some(take_profile().await);
    let browsers = browsers(&s.browser_path);
    if browsers.is_empty() {
        return Err("مرورگر Edge یا Chrome پیدا نشد. یکی از آن‌ها را نصب کنید یا مسیر آن را در «تنظیمات ← عمومی» وارد کنید.".into());
    }
    let mut last = String::new();
    for exe in &browsers {
        close_leftover(profile).await;
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let fresh = profile.with_file_name(format!("browser-profile-{secs}"));
        for dir in [profile, fresh.as_path()] {
            match launch_in(exe, dir, headless, tabs).await {
                Ok(mut e) => {
                    e._lease = lease.take();
                    return Ok(e);
                }
                Err(e) => {
                    crate::log_line("WARN", &format!("browser launch failed ({exe}, {}): {e}", dir.display()));
                    kill_profile_users(dir);
                    last = e;
                }
            }
        }
    }
    Err(format!("مرورگر اجرا نشد ({last}). همه پنجره‌های Edge و Chrome را ببندید و دوباره امتحان کنید؛ اگر باز هم نشد، در «تنظیمات ← عمومی» مسیر مرورگر دیگری را وارد کنید. جزئیات در پوشه logs است."))
}

/// Installed browsers, the one chosen in Settings first.
fn browsers(custom: &str) -> Vec<String> {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let mut c = vec![custom.trim().to_string()];
    for base in [env("ProgramFiles(x86)"), env("ProgramFiles"), env("LOCALAPPDATA")] {
        if !base.is_empty() {
            c.push(format!(r"{base}\Microsoft\Edge\Application\msedge.exe"));
            c.push(format!(r"{base}\Google\Chrome\Application\chrome.exe"));
        }
    }
    c.extend(["/usr/bin/chromium", "/usr/bin/google-chrome"].map(String::from));
    let mut out: Vec<String> = vec![];
    for p in c {
        if !p.is_empty() && Path::new(&p).is_file() && !out.iter().any(|x| x.eq_ignore_ascii_case(&p)) {
            out.push(p);
        }
    }
    out
}

async fn close_leftover(profile: &Path) {
    let file = profile.join("DevToolsActivePort");
    if let Some(url) = active_port(profile) {
        if let Ok(Ok((mut old, mut handler))) = tokio::time::timeout(Duration::from_secs(3), Browser::connect(url)).await {
            tokio::spawn(async move { while let Some(Ok(_)) = handler.next().await {} });
            let _ = tokio::time::timeout(Duration::from_secs(3), old.close()).await;
            sleep(Duration::from_millis(1500)).await; // let the process release the profile
        }
    }
    let _ = std::fs::remove_file(file);
    kill_profile_users(profile);
}

static LAUNCHED: AtomicBool = AtomicBool::new(false);
static CLEANING: Mutex<()> = Mutex::new(());

/// At start-up and exit: ends every browser this data folder started (a crash or a forced quit
/// can leave one running, which then holds the profile and shows as a blank white window), and
/// deletes the spare profiles made when the main one was locked.
pub fn cleanup(data_dir: &Path, at_exit: bool) {
    if at_exit && !LAUNCHED.load(Relaxed) {
        return;
    }
    let _busy = CLEANING.lock().unwrap_or_else(|e| e.into_inner()); // launch() waits for this
    kill_profile_users(data_dir);
    if !at_exit {
        if let Ok(rd) = std::fs::read_dir(data_dir) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().starts_with("browser-profile-") {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
    }
}

/// Ends any Edge/Chrome still running with this profile (left by a crashed or killed run).
fn kill_profile_users(profile: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let dir = profile.display().to_string().replace('\'', "''");
        let ps = format!(
            "Get-CimInstance Win32_Process -Filter \"Name='msedge.exe' OR Name='chrome.exe'\" | Where-Object {{ $_.CommandLine -and $_.CommandLine.Contains('{dir}') }} | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }}"
        );
        let _ = std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &ps]).creation_flags(0x0800_0000).output();
    }
    #[cfg(not(windows))]
    let _ = profile;
}

/// "ws://127.0.0.1:PORT/devtools/browser/ID" from the file Chrome/Edge write into the profile.
fn active_port(profile: &Path) -> Option<String> {
    let txt = std::fs::read_to_string(profile.join("DevToolsActivePort")).ok()?;
    let mut lines = txt.lines();
    let (port, path) = (lines.next()?.trim(), lines.next()?.trim());
    (!port.is_empty() && path.starts_with('/')).then(|| format!("ws://127.0.0.1:{port}{path}"))
}

/// Starts one browser ourselves and connects over the DevTools port it writes into the profile.
/// (Reading the port from the profile instead of stderr works however Windows wires the pipes.)
async fn launch_in(exe: &str, profile: &Path, headless: bool, tabs: usize) -> Res<Engine> {
    std::fs::create_dir_all(profile).map_err(err)?;
    for f in ["SingletonLock", "SingletonCookie", "SingletonSocket", "lockfile", "DevToolsActivePort"] {
        let _ = std::fs::remove_file(profile.join(f)); // left behind by a crashed run
    }
    let log = std::fs::File::create(profile.with_file_name("browser-last.log")).ok();
    let mut cmd = std::process::Command::new(exe);
    cmd.arg(format!("--user-data-dir={}", profile.display()))
        .args([
            "--remote-debugging-port=0",
            "--remote-allow-origins=*",
            "--window-size=1280,900",
            "--lang=fa",
            "--blink-settings=imagesEnabled=false",
            "--disable-notifications",
            "--mute-audio",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-default-apps",
            "--disable-sync",
            "--disable-extensions",
            "--disable-component-update",
            "--disable-session-crashed-bubble",
            "--hide-crash-restore-bubble",
            "--disable-features=Translate,msEdgeStartupBoost,msImplicitSignin,msEdgeSidebarV2",
            "--disable-blink-features=AutomationControlled", // navigator.webdriver = false
            // background tabs must run at full speed for parallel workers
            "--disable-background-timer-throttling",
            "--disable-renderer-backgrounding",
            "--disable-backgrounding-occluded-windows",
        ]);
    if headless {
        cmd.arg("--headless=new");
    }
    cmd.arg("about:blank").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null());
    match log {
        Some(f) => cmd.stderr(f),
        None => cmd.stderr(std::process::Stdio::null()),
    };
    let mut proc = Proc(cmd.spawn().map_err(|e| format!("اجرای {exe} ممکن نشد: {e}"))?);
    LAUNCHED.store(true, Relaxed);
    // wait (20 s max) for the DevTools port; a quick exit means the profile is held elsewhere
    let deadline = Instant::now() + Duration::from_secs(20);
    let url = loop {
        if let Some(u) = active_port(profile) {
            break u;
        }
        if let Ok(Some(st)) = proc.0.try_wait() {
            return Err(format!("مرورگر بلافاصله بسته شد (کد {})", st.code().unwrap_or(-1)));
        }
        if Instant::now() > deadline {
            return Err("مرورگر در ۲۰ ثانیه آماده نشد".into());
        }
        sleep(Duration::from_millis(200)).await;
    };
    let config = chromiumoxide::handler::HandlerConfig {
        viewport: None,
        request_timeout: Duration::from_secs(20),
        ..Default::default()
    };
    let (browser, mut handler) = Browser::connect_with_config(url, config).await.map_err(|e| format!("اتصال به مرورگر ممکن نشد: {e}"))?;
    let alive = Arc::new(AtomicBool::new(true));
    let flag = alive.clone();
    tokio::spawn(async move {
        // Most errors are CDP events this crate version can't parse: keep going.
        // A websocket / IO error means the browser process is gone.
        while let Some(r) = handler.next().await {
            if matches!(r, Err(CdpError::Ws(_) | CdpError::Io(_))) {
                break;
            }
        }
        flag.store(false, Relaxed);
    });
    // headless announces itself as "HeadlessChrome", which Google treats as a bot
    let ua = browser.user_agent().await.unwrap_or_default().replace("HeadlessChrome", "Chrome");
    let ua = if headless { ua } else { String::new() };
    let list = new_tab(&browser, &ua).await?;
    let mut workers = vec![];
    for _ in 0..tabs {
        workers.push(new_tab(&browser, &ua).await?);
    }
    Ok(Engine { browser, alive, list, workers, proc, _lease: None })
}

async fn new_tab(browser: &Browser, ua: &str) -> Res<Page> {
    let p = browser.new_page("about:blank").await.map_err(err)?;
    if !ua.is_empty() {
        let _ = p.execute(SetUserAgentOverrideParams::new(ua)).await;
    }
    Ok(p)
}

/// Closes the browser and waits for the process, so the profile is free for the next launch.
pub(crate) async fn shut(engine: Option<Engine>) {
    if let Some(mut e) = engine {
        let _ = tokio::time::timeout(Duration::from_secs(3), e.browser.close()).await;
        for _ in 0..25 {
            if e.proc.exited() {
                break;
            }
            sleep(Duration::from_millis(200)).await;
        }
        // Proc's Drop kills the process if it is still there
    }
}

/// Opens a visible window on the blocked search and waits (15 min max) until the user passes the check or clicks Continue.
async fn solve_captcha(s: &Settings, profile: &Path, url: &str, token: &CancellationToken, emit: Emit<'_>) -> Res<bool> {
    clear_wait_skip();
    emit(ScanEvent::Captcha { message: CAPTCHA_WAIT.into() });
    let mut eng = launch(s, profile, false, 0).await?;
    goto(&eng.list, url).await;
    let deadline = Instant::now() + Duration::from_secs(15 * 60);
    let result = loop {
        tokio::select! {
            _ = sleep(Duration::from_secs(2)) => {}
            _ = token.cancelled() => break Ok(false),
        }
        if take_wait_skip() || !eng.alive.load(Relaxed) || eng.proc.exited() {
            break Ok(true);
        }
        if let Ok(st) = js::<ListState>(&eng.list, &script("listJs", LIST_JS)).await {
            if !st.blocked && st.href.contains("/maps") {
                break Ok(true);
            }
        }
        if Instant::now() > deadline {
            break Err("تأیید «ربات نیستم» انجام نشد (۱۵ دقیقه). دوباره «ادامه استخراج» را بزنید.".to_string());
        }
    };
    shut(Some(eng)).await;
    if result == Ok(true) {
        status(emit, "ادامه استخراج در پس‌زمینه…");
    }
    result
}

// ---------------- run ----------------

enum Stop {
    Captcha,
    Died,
}

struct KwOut {
    checked: i64,
    found: i64,
    stop: Option<Stop>,
}

pub async fn run(db: &Db, s: &Settings, profile: &Path, req: &ScanRequest, token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    let keywords: Vec<String> = req.keywords.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect();
    if keywords.is_empty() {
        return Err("حداقل یک کلمه کلیدی وارد کنید.".into());
    }
    if req.area().is_empty() && req.lookups.is_empty() {
        return Err("استان یا شهر را انتخاب کنید.".into());
    }
    let limit = req.limit.clamp(1, 2000);
    let tabs = s.concurrency.clamp(1, 8);
    let http = crate::enrich::client();
    let mut sum = Summary::default();
    let mut failure: Option<String> = None;
    let (mut captchas, mut restarts) = (0, 0);
    let mut engine: Option<Engine> = None;

    let mut i = 0;
    // exact look-ups restart from the first entry on resume (found ones are skipped as known)
    let mut target = if req.first_limit > 0 && req.lookups.is_empty() { req.first_limit.min(limit) } else { limit };
    let mut cur: Option<(i64, i64, i64)> = None; // (run id, checked, found) of the current keyword
    while i < keywords.len() && !token.is_cancelled() {
        let kw = &keywords[i];
        let (run_id, done_c, done_f) = match cur {
            Some(r) => r,
            None => {
                let id = db.run_start(req.list_id, kw, &req.province, &req.run_city(), target)?;
                // resumable from the first moment, even if the browser never starts
                let resume = ScanRequest {
                    keywords: keywords[i..].to_vec(),
                    limit,
                    first_limit: target,
                    ..req.clone()
                };
                db.run_checkpoint(id, 0, 0, &serde_json::to_string(&resume).unwrap_or_default());
                (id, 0, 0)
            }
        };
        cur = Some((run_id, done_c, done_f));
        if engine.is_none() {
            status(emit, "در حال آماده‌سازی مرورگر…");
            engine = tokio::select! {
                e = launch(s, profile, s.headless, tabs) => match e {
                    Ok(e) => Some(e),
                    Err(e) => { failure = Some(e); break; }
                },
                _ = token.cancelled() => break,
            };
        }
        let Some(eng) = engine.as_ref() else { break };
        let resume = ScanRequest {
            keywords: keywords[i..].to_vec(),
            limit,
            ..req.clone()
        };
        let job = KwJob {
            run_id,
            kw,
            want: target - done_c,
            base: (done_c, done_f),
            resume: &resume,
            target,
        };
        let out = scan_keyword(db, &http, s, (&eng.list, &eng.workers, &eng.alive), req, &job, token, emit).await;
        sum.checked += out.checked;
        sum.found += out.found;
        let (c, f) = (done_c + out.checked, done_f + out.found);
        cur = Some((run_id, c, f));
        let retry = |what: &str| log(emit, "warning", format!("{what}؛ «{kw}» از همان‌جا ادامه می‌یابد."));
        match out.stop {
            _ if token.is_cancelled() => break,
            None => {
                let _ = db.run_finish(run_id, c, f, "done", "");
                cur = None;
                i += 1;
                target = limit;
            }
            Some(Stop::Captcha) if captchas < 3 => {
                captchas += 1;
                shut(engine.take()).await;
                match solve_captcha(s, profile, &maps_search(&req.queries(kw).0[0]), token, emit).await {
                    Ok(true) => retry("تأیید انجام شد"),
                    Ok(false) => break,
                    Err(e) => {
                        failure = Some(e);
                        break;
                    }
                }
            }
            Some(Stop::Captcha) => {
                failure = Some(CAPTCHA_GIVE_UP.into());
                break;
            }
            Some(Stop::Died) if restarts < 2 => {
                restarts += 1;
                shut(engine.take()).await;
                retry("مرورگر از کار افتاد و دوباره اجرا می‌شود");
            }
            Some(Stop::Died) => {
                failure = Some(BROWSER_DIED.into());
                break;
            }
        }
    }
    status(emit, "در حال بستن مرورگر…");
    shut(engine).await;
    if let Some((id, c, f)) = cur {
        // the resume checkpoint stays on this row, so it can be continued later
        let _ = db.run_finish(
            id,
            c,
            f,
            if failure.is_some() { "failed" } else { "cancelled" },
            failure.as_deref().unwrap_or(""),
        );
    }
    sum.cancelled = token.is_cancelled();
    sum.error = failure.is_some();
    sum.message = match (&failure, sum.cancelled) {
        (Some(e), _) => e.clone(),
        (None, true) => format!("متوقف شد. {} مکان بررسی و {} مخاطب جدید ذخیره شد.", sum.checked, sum.found),
        (None, false) => format!("پایان استخراج. {} مکان بررسی و {} مخاطب جدید ذخیره شد.", sum.checked, sum.found),
    };
    match failure {
        Some(e) if sum.checked == 0 => Err(e),
        _ => Ok(sum),
    }
}

struct KwJob<'a> {
    run_id: i64,
    kw: &'a str,
    want: i64,        // places still to check for this keyword
    base: (i64, i64), // checked / found before this attempt (after a captcha or a restart)
    resume: &'a ScanRequest,
    target: i64,
}

/// A place read from Maps, waiting for its website to be read and then saved.
struct Pending {
    place: Place,
    contacts: Contacts,
    from_maps: Vec<(&'static str, String)>,
}

struct Ctx<'a> {
    db: &'a Db,
    http: &'a reqwest::Client,
    s: &'a Settings,
    req: &'a ScanRequest,
    job: &'a KwJob<'a>,
    emit: Emit<'a>,
    collected: AtomicI64,
    checked: AtomicI64,
    found: AtomicI64,
    abort: CancellationToken,
    stop: Mutex<Option<Stop>>,
    rx: tokio::sync::Mutex<mpsc::Receiver<(String, String)>>,
    pending: tokio::sync::Mutex<mpsc::Receiver<Pending>>,
}

impl Ctx<'_> {
    /// Ends this keyword's work (user stop, captcha, dead browser).
    fn fail(&self, why: Stop) {
        self.stop.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(why);
        self.abort.cancel();
    }
    fn progress(&self) -> ScanEvent {
        ScanEvent::Progress {
            keyword: self.job.kw.to_string(),
            collected: self.job.base.0 + self.collected.load(Relaxed),
            checked: self.job.base.0 + self.checked.load(Relaxed),
            found: self.job.base.1 + self.found.load(Relaxed),
            target: self.job.target,
        }
    }
    fn checkpoint(&self) {
        let (c, f) = (self.checked.load(Relaxed), self.found.load(Relaxed));
        let resume = ScanRequest {
            first_limit: (self.job.want - c).max(1),
            ..self.job.resume.clone()
        };
        let json = serde_json::to_string(&resume).unwrap_or_default();
        self.db.run_checkpoint(self.job.run_id, self.job.base.0 + c, self.job.base.1 + f, &json);
    }
}

#[allow(clippy::too_many_arguments)]
async fn scan_keyword(
    db: &Db,
    http: &reqwest::Client,
    s: &Settings,
    (list, workers, alive): (&Page, &[Page], &AtomicBool), // not &Engine: Browser need not be Sync
    req: &ScanRequest,
    job: &KwJob<'_>,
    token: &CancellationToken,
    emit: Emit<'_>,
) -> KwOut {
    let (tx, rx) = mpsc::channel::<(String, String)>(64);
    let (tx_pending, rx_pending) = mpsc::channel::<Pending>(256);
    let ctx = Ctx {
        db,
        http,
        s,
        req,
        job,
        emit,
        collected: AtomicI64::new(0),
        checked: AtomicI64::new(0),
        found: AtomicI64::new(0),
        abort: token.child_token(),
        stop: Mutex::new(None),
        rx: tokio::sync::Mutex::new(rx),
        pending: tokio::sync::Mutex::new(rx_pending),
    };
    ctx.checkpoint();
    status(emit, format!("جستجوی «{}»…", job.kw));
    let ctx = &ctx;
    // `tx` / `tx_pending` move into their producers; dropping them lets the consumers finish
    let maps = async move {
        futures::future::join_all(workers.iter().map(|p| worker(ctx, p, &tx_pending))).await;
    };
    let sites = futures::future::join_all((0..ENRICHERS).map(|_| enricher(ctx)));
    let work = async {
        tokio::join!(collector(ctx, list, tx), maps, sites);
    };
    tokio::pin!(work);
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    let (mut last_save, mut stopping) = (Instant::now(), None::<Instant>);
    loop {
        tokio::select! {
            _ = &mut work => break,
            _ = tick.tick() => {
                emit(ctx.progress());
                if !alive.load(Relaxed) {
                    ctx.fail(Stop::Died);
                }
                if last_save.elapsed() > Duration::from_secs(5) {
                    ctx.checkpoint();
                    last_save = Instant::now();
                }
                // every part listens to `abort` and saves what it holds; give it 5 s, then drop it
                if ctx.abort.is_cancelled() && stopping.get_or_insert_with(Instant::now).elapsed() > Duration::from_secs(5) {
                    break;
                }
            }
        }
    }
    emit(ctx.progress());
    ctx.checkpoint();
    let stop = ctx.stop.lock().unwrap_or_else(|e| e.into_inner()).take();
    KwOut {
        checked: ctx.checked.load(Relaxed),
        found: ctx.found.load(Relaxed),
        stop,
    }
}

async fn collector(ctx: &Ctx<'_>, page: &Page, tx: mpsc::Sender<(String, String)>) {
    let mut seen = HashSet::new();
    let exact = !ctx.req.lookups.is_empty();
    let area = ctx.req.area();
    let (mut queries, fixed) = if exact {
        (ctx.req.lookups.iter().map(|t| format!("{t} {area}").trim().to_string()).collect(), true)
    } else {
        ctx.req.queries(ctx.job.kw)
    };
    if !ctx.s.expand_areas && !fixed {
        queries.truncate(1);
    }
    for (i, q) in queries.iter().enumerate() {
        let want = if exact { 1 } else { ctx.job.want - ctx.collected.load(Relaxed) };
        if ctx.collected.load(Relaxed) >= ctx.job.want || ctx.abort.is_cancelled() {
            break;
        }
        if exact {
            status(ctx.emit, format!("یافتن شماره «{q}»"));
        } else if i > 0 {
            status(ctx.emit, format!("تکمیل ظرفیت در «{q}»"));
        }
        let label = exact.then(|| ctx.req.lookups[i].as_str());
        let r = tokio::select! {
            r = collect(ctx.db, ctx.req.list_id, page, q, want, label, &tx, &mut seen, &ctx.collected) => r,
            _ = ctx.abort.cancelled() => return,
        };
        match r {
            Ok(()) => {}
            Err(e) if e == BLOCKED => return ctx.fail(Stop::Captcha),
            Err(e) => log(ctx.emit, "warning", format!("جستجوی «{q}»: {e}")),
        }
    }
}

const BLOCKED: &str = "captcha";

async fn worker(ctx: &Ctx<'_>, page: &Page, out: &mpsc::Sender<Pending>) {
    loop {
        let job = tokio::select! {
            j = async { ctx.rx.lock().await.recv().await } => j,
            _ = ctx.abort.cancelled() => None,
        };
        let Some((url, list_name)) = job else { break };
        let visited = tokio::select! {
            r = visit(page, &url) => r,
            _ = ctx.abort.cancelled() => break,
        };
        match visited {
            Ok(info) if !ctx.req.lookups.is_empty() && !crate::logic::same_business(&info.name, &list_name) => {
                log(ctx.emit, "muted", format!("{list_name} — پیدا نشد"));
            }
            Ok(info) => {
                let mut c = Contacts::default();
                info.phones.iter().for_each(|p| c.add_phone(p));
                info.links.iter().for_each(|l| c.add_link(l)); // an Instagram "website" lands in instagram, etc.
                let key = place_key(&url);
                let (lat, lng) = coords(&info.href).or_else(|| coords(&url)).map_or((None, None), |(a, b)| (Some(a), Some(b)));
                let place = Place {
                    list_id: ctx.req.list_id,
                    place_key: key,
                    name: if info.name.is_empty() { list_name } else { info.name },
                    category: info.category,
                    city: crate::logic::city_in(&info.address, &ctx.req.city, &ctx.req.areas),
                    address: crate::logic::clean_address(&info.address),
                    photo: crate::logic::photo_url(&info.photo),
                    rating: info.rating,
                    reviews: info.reviews,
                    lat,
                    lng,
                    maps_url: if info.href.contains("/maps/place/") { info.href } else { url.clone() },
                    keyword: ctx.job.kw.to_string(),
                    province: ctx.req.province.trim().to_string(),
                    status: "new".into(),
                    source: if ctx.req.source.is_empty() { "maps".into() } else { ctx.req.source.clone() },
                    ..Default::default()
                };
                let from_maps = c.pairs();
                if out.send(Pending { place, contacts: c, from_maps }).await.is_err() {
                    break;
                }
            }
            Err(e) if e == BLOCKED => return ctx.fail(Stop::Captcha),
            Err(e) => log(ctx.emit, "warning", format!("{list_name}: {e}")),
        }
        ctx.checked.fetch_add(1, Relaxed);
        tokio::select! {
            _ = sleep(Duration::from_millis(jitter(ctx.s.delay_ms, ctx.s.delay_ms * 2))) => {}
            _ = ctx.abort.cancelled() => break,
        }
    }
}

/// Reads websites for queued places, then saves them. After a stop it still saves (without the website).
async fn enricher(ctx: &Ctx<'_>) {
    loop {
        let next = ctx.pending.lock().await.recv().await;
        let Some(mut p) = next else { break }; // workers finished and the queue is empty
        if ctx.req.enrich && !p.contacts.website.is_empty() && (p.contacts.needs_more() || p.contacts.person.is_empty()) {
            tokio::select! {
                _ = tokio::time::timeout(Duration::from_secs(30), crate::enrich::enrich(ctx.http, &mut p.contacts)) => {}
                _ = ctx.abort.cancelled() => {}
            }
        }
        save(ctx, p);
    }
}

fn save(ctx: &Ctx<'_>, p: Pending) {
    let Pending {
        mut place,
        contacts: c,
        from_maps,
    } = p;
    if !c.has_any() {
        // kept (hidden from lists) so later runs and resumes don't visit it again
        let _ = ctx.db.insert_place(&place);
        log(ctx.emit, "muted", format!("{} — راه ارتباطی نداشت", place.name));
        return;
    }
    place.website = c.website.clone();
    (place.contact_person, place.contact_role) = (c.person.clone(), c.person_role.clone());
    place.contacts = c
        .pairs()
        .into_iter()
        .map(|(kind, value)| Contact {
            origin: if from_maps.iter().any(|(k, v)| *k == kind && *v == value) {
                "maps"
            } else {
                "website"
            }
            .into(),
            kind: kind.into(),
            value,
            ..Default::default()
        })
        .collect();
    match ctx.db.insert_place(&place) {
        Ok((true, added)) if added > 0 => {
            ctx.found.fetch_add(1, Relaxed);
            log(ctx.emit, "success", format!("{} — {}", place.name, summary_of(&place)));
            if let Ok(saved) = ctx.db.get_by_key(place.list_id, &place.place_key) {
                (ctx.emit)(ScanEvent::Found { place: Box::new(saved) });
            }
        }
        Ok((true, _)) => log(ctx.emit, "muted", format!("{} — راه‌های ارتباطی‌اش قبلاً در این بانک ثبت شده", place.name)),
        Ok((false, _)) => {}
        Err(e) => log(ctx.emit, "error", format!("ذخیره نشد: {e}")),
    }
}

/// "09121234567 · ایمیل · اینستاگرام · مدیرعامل: علی امینی" for the live log.
fn summary_of(p: &Place) -> String {
    let mut parts: Vec<String> = vec![];
    if let Some(c) = p.first("mobile").or_else(|| p.first("landline")) {
        parts.push(c.value.clone());
    }
    for (kind, label) in [
        ("email", "ایمیل"),
        ("instagram", "اینستاگرام"),
        ("telegram", "تلگرام"),
        ("whatsapp", "واتساپ"),
        ("eitaa", "ایتا"),
        ("bale", "بله"),
    ] {
        if p.first(kind).is_some() {
            parts.push(label.into());
        }
    }
    if !p.contact_person.is_empty() {
        let role = if p.contact_role.is_empty() { "مسئول" } else { &p.contact_role };
        parts.push(format!("{role}: {}", p.contact_person));
    }
    parts.join(" · ")
}

/// Scrolls one search result list and queues unseen places until `want` are queued.
#[allow(clippy::too_many_arguments)]
async fn collect(
    db: &Db,
    list_id: i64,
    page: &Page,
    query: &str,
    want: i64,
    label: Option<&str>, // exact look-up: the name we are looking for travels with the link
    tx: &mpsc::Sender<(String, String)>,
    seen: &mut HashSet<String>,
    collected: &AtomicI64,
) -> Res<()> {
    goto(page, &maps_search(query)).await;
    let t = Instant::now();
    let mut st: ListState = loop {
        // the JS context is briefly invalid around navigation; retry until the deadline
        let st: ListState = match js(page, &script("listJs", LIST_JS)).await {
            Ok(st) => st,
            Err(e) if t.elapsed() > Duration::from_secs(15) => return Err(e),
            Err(_) => {
                sleep(Duration::from_millis(300)).await;
                continue;
            }
        };
        if st.blocked {
            return Err(BLOCKED.into());
        }
        if st.consent {
            let _ = js::<bool>(page, &script("consentJs", CONSENT_JS)).await;
        }
        if !st.items.is_empty() || st.place || st.end || t.elapsed() > Duration::from_secs(15) {
            break st;
        }
        sleep(Duration::from_millis(300)).await;
    };
    if st.items.is_empty() && st.place {
        st.items.push((st.href.clone(), String::new())); // Maps jumped straight to a single place
        st.end = true;
    }
    let (mut sent, mut idle) = (0, 0);
    loop {
        for (href, name) in &st.items {
            let key = place_key(href);
            if !seen.insert(key.clone()) || db.known(list_id, &key) {
                continue;
            }
            if tx.send((href.clone(), label.map_or_else(|| name.clone(), String::from))).await.is_err() {
                return Ok(()); // workers are gone (stop)
            }
            sent += 1;
            collected.fetch_add(1, Relaxed);
            if sent >= want {
                return Ok(());
            }
        }
        if st.end {
            return Ok(());
        }
        let before = st.items.len();
        if !js::<bool>(page, &script("scrollJs", SCROLL_JS)).await? {
            return Ok(());
        }
        let t = Instant::now();
        loop {
            sleep(Duration::from_millis(300)).await;
            if let Ok(s) = js(page, &script("listJs", LIST_JS)).await {
                st = s;
            }
            if st.blocked {
                return Err(BLOCKED.into());
            }
            if st.items.len() > before || st.end || t.elapsed() > Duration::from_secs(3) {
                break;
            }
        }
        idle = if st.items.len() > before { 0 } else { idle + 1 };
        if idle >= 5 {
            return Ok(()); // list stopped growing
        }
    }
}

async fn visit(page: &Page, url: &str) -> Res<PlaceInfo> {
    goto(page, url).await;
    let want = place_key(url);
    let strict = want.starts_with("0x");
    let t = Instant::now();
    let mut ready_at: Option<Instant> = None;
    loop {
        let Ok(info) = js::<PlaceInfo>(page, &script("placeJs", PLACE_JS)).await else {
            if t.elapsed() > Duration::from_secs(12) {
                return Err("صفحه مکان بارگذاری نشد".into());
            }
            sleep(Duration::from_millis(250)).await;
            continue;
        };
        if info.blocked {
            return Err(BLOCKED.into());
        }
        // guards against reading the previous place while the new one is still loading
        let ok = info.ready && (!strict || info.href.to_lowercase().contains(&want));
        if ok {
            let since = *ready_at.get_or_insert_with(Instant::now);
            if !info.phones.is_empty() || since.elapsed() > Duration::from_millis(1500) {
                return Ok(info);
            }
        }
        if t.elapsed() > Duration::from_secs(12) {
            return if ok {
                Ok(info)
            } else {
                Err("صفحه مکان بارگذاری نشد".into())
            };
        }
        sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_modes() {
        let r = |p: &str, c: &str, d: &str, a: &[&str]| ScanRequest {
            province: p.into(),
            city: c.into(),
            district: d.into(),
            areas: a.iter().map(|x| x.to_string()).collect(),
            ..Default::default()
        };
        // county: the county plus its surroundings (may be trimmed)
        let (q, fixed) = r("البرز", "کرج", "", &[]).queries("کافه");
        assert_eq!((q[0].as_str(), fixed), ("کافه کرج", false));
        // every county of the province, exactly
        let (q, fixed) = r("البرز", "", "", &["کرج", "فردیس", "کرج"]).queries("کافه");
        assert_eq!((q, fixed), (vec!["کافه کرج".to_string(), "کافه فردیس".to_string()], true));
        // one exact district: nothing else
        let x = r("تهران", "تهران", " تجریش ", &["ری"]);
        assert_eq!(x.queries("کافه"), (vec!["کافه تجریش تهران".to_string()], true));
        assert_eq!((x.area(), x.run_city()), ("تجریش تهران".to_string(), "تجریش تهران".to_string()));
        assert_eq!(r("فارس", "", "", &[]).area(), "فارس");
        assert!(r("", "", "", &[]).area().is_empty());
    }

    /// Real browser smoke test. Runs only with `HS_TEST_BROWSER=<path to msedge.exe / chrome>`.
    #[tokio::test(flavor = "multi_thread")]
    async fn headless_engine_reads_a_place_card() {
        let Ok(exe) = std::env::var("HS_TEST_BROWSER") else { return };
        let s = Settings {
            browser_path: exe,
            ..Default::default()
        };
        let profile = std::env::temp_dir().join(format!("hs-profile-{}", std::process::id()));
        let eng = launch(&s, &profile, true, 1).await.unwrap();
        let ua: String = js(&eng.workers[0], "return navigator.userAgent").await.unwrap();
        assert!(ua.contains("Chrome/") && !ua.contains("Headless"), "{ua}");
        // shaped like a Google Maps place card
        let html = r#"<h1>کافه لمیز</h1>
            <button data-item-id="phone:tel:09123456789">۰۹۱۲</button>
            <button data-item-id="phone:tel:02188776655">۰۲۱</button>
            <a data-item-id="authority" href="https://lamiz.ir/">سایت</a>
            <a href="https://instagram.com/cafe.lamiz">ig</a>
            <div data-item-id="address">تهران، خیابان ولیعصر</div>"#;
        goto(&eng.workers[0], &format!("data:text/html;charset=utf-8,{}", crate::logic::pct_encode(html))).await;
        let info: PlaceInfo = js(&eng.workers[0], &script("placeJs", PLACE_JS)).await.unwrap();
        assert!(info.ready && !info.blocked);
        assert_eq!(info.name, "کافه لمیز");
        assert_eq!(info.phones, vec!["09123456789", "02188776655"]);
        assert_eq!(info.links, vec!["https://lamiz.ir/", "https://instagram.com/cafe.lamiz"]);
        assert_eq!(info.address, "تهران، خیابان ولیعصر");
        let st: ListState = js(&eng.list, &script("listJs", LIST_JS)).await.unwrap();
        assert!(st.items.is_empty() && !st.blocked);
        assert!(eng.alive.load(Relaxed));
        // the app was killed and its browser kept running: the next launch must still work
        std::mem::forget(eng);
        let eng = launch(&s, &profile, true, 1).await.expect("leftover browser blocked the profile");
        assert!(profile.join("DevToolsActivePort").exists(), "launch did not reuse the profile");
        // a crashed browser must be noticed (the scan restarts it)
        let mut eng = eng;
        let _ = eng.proc.0.kill();
        let t = Instant::now();
        while eng.alive.load(Relaxed) && t.elapsed() < Duration::from_secs(10) {
            sleep(Duration::from_millis(100)).await;
        }
        assert!(!eng.alive.load(Relaxed), "dead browser not detected");
        shut(Some(eng)).await;
        let _ = std::fs::remove_dir_all(profile);
    }
}