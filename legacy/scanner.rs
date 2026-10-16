//! Google Maps engine: one tab lists results, N tabs read place pages in parallel.
//! Everything races a CancellationToken, so Stop drops all in-flight work at once.
use crate::db::{err, Db, Place, Res};
use crate::logic::{coords, jitter, place_key, plan_queries, search_url, Contacts};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::js_protocol::runtime::EvaluateParams;
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
            headless: false,
            delay_ms: 400,
            expand_areas: true,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ScanRequest {
    pub list_id: i64,
    pub enrich: bool, // also read the business website for email / socials / extra numbers
    pub keywords: Vec<String>,
    pub province: String,
    pub city: String,
    pub limit: i64,
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
        place: Place,
    },
    Log {
        level: &'static str,
        message: String,
    },
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

pub fn detect_browser(custom: &str) -> Option<String> {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let mut c = vec![custom.trim().to_string()];
    for base in [env("ProgramFiles(x86)"), env("ProgramFiles"), env("LOCALAPPDATA")] {
        if !base.is_empty() {
            c.push(format!(r"{base}\Microsoft\Edge\Application\msedge.exe"));
            c.push(format!(r"{base}\Google\Chrome\Application\chrome.exe"));
        }
    }
    c.extend(
        [
            "/usr/bin/chromium",
            "/usr/bin/google-chrome",
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ]
        .map(String::from),
    );
    c.into_iter().find(|p| !p.is_empty() && Path::new(p).is_file())
}

const BLOCKED: &str = "گوگل موقتاً دسترسی را محدود کرده (Captcha). چند دقیقه صبر کنید، تعداد تب‌ها را کم کنید یا مرورگر را آشکار اجرا کنید و Captcha را حل کنید.";

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
const clean = s => (s || '').replace(/[-]/g, '').trim();
const latin = s => (s || '').replace(/[۰-۹]/g, d => '۰۱۲۳۴۵۶۷۸۹'.indexOf(d)).replace(/[٠-٩]/g, d => '٠١٢٣٤٥٦٧٨٩'.indexOf(d));
const h1 = q('h1');
const ad = q('[data-item-id="address"]');
const f7 = q('div.F7nice');
const nums = f7 ? latin(f7.innerText).replace(/٫/g, '.').replace(/[٬,](?=\d{3}\b)/g, '').match(/\d+(\.\d+)?/g) || [] : [];
const phones = [...document.querySelectorAll('[data-item-id^="phone:tel:"]')].map(b => (b.getAttribute('data-item-id') || '').slice(10) || clean(b.innerText));
const links = [...document.querySelectorAll('a[data-item-id="authority"], a[data-item-id^="action:"], a[href*="instagram.com"], a[href*="t.me/"], a[href*="wa.me/"]')].map(a => a.href);
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
}

/// Runs a script body and returns its value; JSON string transport works with every CDP version.
async fn js<T: DeserializeOwned>(page: &Page, body: &str) -> Res<T> {
    let expr = format!("JSON.stringify((() => {{{body}\n}})())");
    let r = page.evaluate_expression(EvaluateParams::new(expr)).await.map_err(err)?;
    let s: String = r.into_value().map_err(err)?;
    serde_json::from_str(&s).map_err(err)
}

/// Navigation errors/timeouts are ignored on purpose: callers poll the DOM with their own deadline.
async fn goto(page: &Page, url: &str) {
    let _ = tokio::time::timeout(Duration::from_secs(15), page.goto(url)).await;
}

struct Engine {
    browser: Browser,
    alive: Arc<AtomicBool>,
    list: Page,
    workers: Vec<Page>,
}

async fn launch(s: &Settings, profile: &Path) -> Res<Engine> {
    let exe = detect_browser(&s.browser_path).ok_or("مرورگر Edge یا Chrome پیدا نشد؛ مسیر آن را در تنظیمات وارد کنید.")?;
    std::fs::create_dir_all(profile).map_err(err)?;
    for f in ["SingletonLock", "SingletonCookie", "SingletonSocket"] {
        let _ = std::fs::remove_file(profile.join(f)); // left behind by a crashed run
    }
    let mut cfg = BrowserConfig::builder()
        .chrome_executable(exe)
        .user_data_dir(profile)
        .with_head()
        .window_size(1280, 900)
        .viewport(None)
        .request_timeout(Duration::from_secs(20))
        .args([
            "--lang=fa",
            "--blink-settings=imagesEnabled=false",
            "--disable-notifications",
            "--mute-audio",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-features=Translate",
            // background tabs must run at full speed for parallel workers
            "--disable-background-timer-throttling",
            "--disable-renderer-backgrounding",
            "--disable-backgrounding-occluded-windows",
        ]);
    if s.headless {
        cfg = cfg.arg("--headless=new");
    }
    let (browser, mut handler) = Browser::launch(cfg.build()?).await.map_err(|e| format!("اجرای مرورگر ناموفق بود: {e}"))?;
    let alive = Arc::new(AtomicBool::new(true));
    let flag = alive.clone();
    tokio::spawn(async move {
        while handler.next().await.is_some() {}
        flag.store(false, Relaxed); // browser process is gone
    });
    let list = browser.new_page("about:blank").await.map_err(err)?;
    let mut workers = vec![];
    for _ in 0..s.concurrency.clamp(1, 8) {
        workers.push(browser.new_page("about:blank").await.map_err(err)?);
    }
    Ok(Engine { browser, alive, list, workers })
}

pub async fn run(db: &Db, s: &Settings, profile: &Path, req: &ScanRequest, token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    let keywords: Vec<String> = req.keywords.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect();
    if keywords.is_empty() {
        return Err("حداقل یک کلمه کلیدی وارد کنید.".into());
    }
    if req.province.trim().is_empty() && req.city.trim().is_empty() {
        return Err("استان یا شهر را انتخاب کنید.".into());
    }
    let limit = req.limit.clamp(1, 2000);
    status(emit, "در حال آماده‌سازی مرورگر…");
    let mut engine = tokio::select! {
        e = launch(s, profile) => e?,
        _ = token.cancelled() => return Ok(Summary { cancelled: true, message: "متوقف شد.".into(), ..Default::default() }),
    };
    let http = crate::outreach::client();
    let mut sum = Summary::default();
    let mut failure = None;
    for kw in &keywords {
        if token.is_cancelled() {
            break;
        }
        match scan_keyword(db, &http, s, (&engine.list, &engine.workers, &engine.alive), req, kw, limit, token, emit).await {
            Ok((c, f)) => {
                sum.checked += c;
                sum.found += f;
            }
            Err((c, f, e)) => {
                sum.checked += c;
                sum.found += f;
                failure = Some(e);
                break;
            }
        }
    }
    status(emit, "در حال بستن مرورگر…");
    let _ = tokio::time::timeout(Duration::from_secs(3), engine.browser.close()).await;
    drop(engine); // Browser's Drop kills the process if close() did not finish
    sum.cancelled = token.is_cancelled();
    sum.error = failure.is_some();
    sum.message = match (&failure, sum.cancelled) {
        (Some(e), _) => e.clone(),
        (None, true) => format!("متوقف شد. {} مکان بررسی و {} شماره جدید ذخیره شد.", sum.checked, sum.found),
        (None, false) => format!("پایان عملیات. {} مکان بررسی و {} شماره جدید ذخیره شد.", sum.checked, sum.found),
    };
    match failure {
        Some(e) if sum.checked == 0 => Err(e),
        _ => Ok(sum),
    }
}

struct Ctx<'a> {
    db: &'a Db,
    http: &'a reqwest::Client,
    s: &'a Settings,
    req: &'a ScanRequest,
    kw: &'a str,
    limit: i64,
    emit: Emit<'a>,
    collected: AtomicI64,
    checked: AtomicI64,
    found: AtomicI64,
    abort: CancellationToken,
    reason: Mutex<Option<String>>,
    rx: tokio::sync::Mutex<mpsc::Receiver<(String, String)>>,
}

impl Ctx<'_> {
    /// Stops this keyword's work (user stop or an internal fatal error such as a captcha).
    fn fail(&self, m: String) {
        self.reason.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(m);
        self.abort.cancel();
    }
    fn progress(&self) -> ScanEvent {
        ScanEvent::Progress {
            keyword: self.kw.to_string(),
            collected: self.collected.load(Relaxed),
            checked: self.checked.load(Relaxed),
            found: self.found.load(Relaxed),
            target: self.limit,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn scan_keyword(
    db: &Db,
    http: &reqwest::Client,
    s: &Settings,
    (list, workers, alive): (&Page, &[Page], &AtomicBool), // not &Engine: Browser need not be Sync
    req: &ScanRequest,
    kw: &str,
    limit: i64,
    token: &CancellationToken,
    emit: Emit<'_>,
) -> Result<(i64, i64), (i64, i64, String)> {
    let run_id = db.run_start(req.list_id, kw, &req.province, &req.city, limit).map_err(|e| (0, 0, e))?;
    let (tx, rx) = mpsc::channel::<(String, String)>(64);
    let ctx = Ctx {
        db,
        http,
        s,
        req,
        kw,
        limit,
        emit,
        collected: AtomicI64::new(0),
        checked: AtomicI64::new(0),
        found: AtomicI64::new(0),
        abort: token.child_token(),
        reason: Mutex::new(None),
        rx: tokio::sync::Mutex::new(rx),
    };
    status(emit, format!("جستجوی «{kw}»…"));
    let work = async {
        // `tx` moves into the collector; dropping it closes the queue so workers finish
        tokio::join!(collector(&ctx, list, tx), futures::future::join_all(workers.iter().map(|p| worker(&ctx, p))));
    };
    tokio::pin!(work);
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    loop {
        tokio::select! {
            _ = ctx.abort.cancelled() => break,
            _ = &mut work => break,
            _ = tick.tick() => {
                emit(ctx.progress());
                if !alive.load(Relaxed) {
                    ctx.fail("مرورگر بسته شد یا از کار افتاد؛ داده‌های ذخیره‌شده حفظ شده‌اند.".into());
                }
            }
        }
    }
    emit(ctx.progress());
    let (c, f) = (ctx.checked.load(Relaxed), ctx.found.load(Relaxed));
    let reason = ctx.reason.lock().unwrap_or_else(|e| e.into_inner()).take();
    let state = if reason.is_some() {
        "failed"
    } else if token.is_cancelled() {
        "cancelled"
    } else {
        "done"
    };
    let _ = db.run_finish(run_id, c, f, state);
    match reason {
        Some(r) => Err((c, f, r)),
        None => Ok((c, f)),
    }
}

async fn collector(ctx: &Ctx<'_>, page: &Page, tx: mpsc::Sender<(String, String)>) {
    let mut seen = HashSet::new();
    let mut queries = plan_queries(ctx.kw, &ctx.req.province, &ctx.req.city);
    if !ctx.s.expand_areas {
        queries.truncate(1);
    }
    for (i, q) in queries.iter().enumerate() {
        let want = ctx.limit - ctx.collected.load(Relaxed);
        if want <= 0 {
            break;
        }
        if i > 0 {
            status(ctx.emit, format!("تکمیل ظرفیت در «{q}»"));
        }
        match collect(ctx.db, ctx.req.list_id, page, q, want, &tx, &mut seen, &ctx.collected).await {
            Ok(()) => {}
            Err(e) if e == BLOCKED => return ctx.fail(e),
            Err(e) => log(ctx.emit, "warning", format!("جستجوی «{q}»: {e}")),
        }
    }
}

async fn worker(ctx: &Ctx<'_>, page: &Page) {
    let emit = ctx.emit;
    loop {
        let job = ctx.rx.lock().await.recv().await;
        let Some((url, list_name)) = job else { break };
        match visit(page, &url).await {
            Ok(info) => {
                let mut c = Contacts::default();
                info.phones.iter().for_each(|p| c.add_phone(p));
                info.links.iter().for_each(|l| c.add_link(l)); // an Instagram "website" lands in instagram, etc.
                if ctx.req.enrich && !c.website.is_empty() && c.needs_more() {
                    crate::outreach::enrich(ctx.http, &mut c).await;
                }
                let key = place_key(&url);
                let (lat, lng) = coords(&info.href).or_else(|| coords(&url)).map_or((None, None), |(a, b)| (Some(a), Some(b)));
                let mut place = Place {
                    list_id: ctx.req.list_id,
                    place_key: key.clone(),
                    name: if info.name.is_empty() { list_name } else { info.name },
                    category: info.category,
                    mobiles: c.mobiles.join(","),
                    landlines: c.landlines.join(","),
                    emails: c.emails.join(","),
                    website: c.website,
                    instagram: c.instagram,
                    telegram: c.telegram,
                    whatsapp: c.whatsapp,
                    eitaa: c.eitaa,
                    bale: c.bale,
                    rubika: c.rubika,
                    address: info.address,
                    rating: info.rating,
                    reviews: info.reviews,
                    lat,
                    lng,
                    url: if info.href.contains("/maps/place/") { info.href } else { url.clone() },
                    keyword: ctx.kw.to_string(),
                    province: ctx.req.province.trim().to_string(),
                    city: ctx.req.city.trim().to_string(),
                    status: "new".into(),
                    ..Default::default()
                };
                match ctx.db.insert(&mut place) {
                    Ok(true) => {
                        ctx.found.fetch_add(1, Relaxed);
                        log(emit, "success", format!("{} — {}", place.name, summary_of(&place)));
                        emit(ScanEvent::Found {
                            place: ctx.db.get(place.list_id, &key).unwrap_or(place),
                        });
                    }
                    Ok(false) => log(emit, "muted", format!("{} — راه ارتباطی جدیدی نداشت", place.name)),
                    Err(e) => log(emit, "error", format!("ذخیره نشد: {e}")),
                }
            }
            Err(e) if e == BLOCKED => return ctx.fail(e),
            Err(e) => log(emit, "warning", format!("{list_name}: {e}")),
        }
        ctx.checked.fetch_add(1, Relaxed);
        sleep(Duration::from_millis(jitter(ctx.s.delay_ms, ctx.s.delay_ms * 2))).await;
    }
}

/// "09121234567 · ایمیل · اینستاگرام" for the live log.
fn summary_of(p: &Place) -> String {
    let mut parts: Vec<&str> = vec![];
    if !p.mobiles.is_empty() {
        parts.push(Place::first(&p.mobiles));
    } else if !p.landlines.is_empty() {
        parts.push(Place::first(&p.landlines));
    }
    for (v, label) in [
        (&p.emails, "ایمیل"),
        (&p.instagram, "اینستاگرام"),
        (&p.telegram, "تلگرام"),
        (&p.whatsapp, "واتساپ"),
    ] {
        if !v.is_empty() {
            parts.push(label);
        }
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
    tx: &mpsc::Sender<(String, String)>,
    seen: &mut HashSet<String>,
    collected: &AtomicI64,
) -> Res<()> {
    goto(page, &search_url(query)).await;
    let t = Instant::now();
    let mut st: ListState = loop {
        // the JS context is briefly invalid around navigation; retry until the deadline
        let st: ListState = match js(page, LIST_JS).await {
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
            let _ = js::<bool>(page, CONSENT_JS).await;
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
            if tx.send((href.clone(), name.clone())).await.is_err() {
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
        if !js::<bool>(page, SCROLL_JS).await? {
            return Ok(());
        }
        let t = Instant::now();
        loop {
            sleep(Duration::from_millis(300)).await;
            if let Ok(s) = js(page, LIST_JS).await {
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
        let Ok(info) = js::<PlaceInfo>(page, PLACE_JS).await else {
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
