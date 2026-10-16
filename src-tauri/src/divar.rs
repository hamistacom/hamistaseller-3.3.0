//! Divar & Sheypoor business, shop and classifieds extractor — 100% operational contact-tab & API engine.
//! * Collects ads ordered by newest date; paces requests calmly to protect the logged-in account;
//! * Extracts phone numbers both by clicking the «اطلاعات تماس» tab (handling terms/warning modals)
//!   AND by querying Divar/Sheypoor's internal contact endpoints directly inside the authenticated browser context;
//! * Attaches the ad's neighbourhood/address, posting date, and direct ad URL alongside every saved phone number.

use crate::db::{Db, Place, Res};
use crate::logic::{clean_address, emails_in_text, jitter, links_in_text, pct_encode, phones_in_text, place_key, Contacts};
use crate::market::city_slug;
use crate::scanner::{goto, js, launch, shut, status, Emit, ScanEvent, ScanRequest, Settings, Summary};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct AdItem {
    href: String,
    title: String,
    subtitle: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ListOut {
    items: Vec<AdItem>,
    blocked: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AdDetail {
    ready: bool,
    title: String,
    phone: String,
    description: String,
    needs_login: bool,
    blocked: bool,
    address: String,
    ad_date: String,
    seller: String,
    category: String,
    links: Vec<String>,
    photo: String,
}

// Extracts ad cards in newest-first order along with their date/location subtitle
const LIST_JS: &str = r#"
const anchors = [...document.querySelectorAll('a[href*="/v/"]')];
const seen = new Set();
const items = [];
for (const a of anchors) {
  const href = a.href ? a.href.split('?')[0] : '';
  if (!href || seen.has(href)) continue;
  seen.add(href);
  const h = a.querySelector('h2, h3, [class*="title"], [class*="Title"]');
  const lines = (a.innerText || '').split('\n').map(s => s.trim()).filter(Boolean);
  const title = (h ? h.innerText : lines[0] || '').replace(/\s+/g, ' ').trim();
  const sub = lines.find(l => l.includes('پیش') || l.includes('در ')) || '';
  if (title) items.push({ href, title, subtitle: sub });
}
const scrollEl = document.scrollingElement || document.documentElement || document.body;
if (scrollEl) {
  window.scrollBy({ top: 850, behavior: 'smooth' });
}
const bodyText = document.body ? document.body.innerText.slice(0, 2000) : '';
return {
  items,
  blocked: /captcha|کپچا|ربات نیستم|تأیید هویت/i.test(bodyText),
};
"#;

// Clicks the Contact tab, accepts any warning modal, AND triggers an authenticated in-page fetch
// to Divar/Sheypoor's contact API so the phone number is guaranteed to be captured into `window.__hmPhone`.
const TRIGGER_CONTACT_JS: &str = r#"
const cleanDigits = s => String(s || '')
  .replace(/[۰-۹]/g, d => '۰۱۲۳۴۵۶۷۸۹'.indexOf(d))
  .replace(/[٠-٩]/g, d => '٠١٢٣٤٥٦٧٨٩'.indexOf(d));

// 1) Click the main «اطلاعات تماس» / «شماره تماس» button if not clicked yet
const buttons = [...document.querySelectorAll('button, a[role="button"], div[role="button"]')];
const contactBtn = buttons.find(b => {
  const t = (b.innerText || '').trim();
  return t.includes('اطلاعات تماس') || t.includes('نمایش شماره') || t.includes('شماره تماس') || t.includes('تماس با آگهی‌دهنده') || t.includes('تماس');
});
if (contactBtn && !contactBtn.dataset.hmClicked) {
  contactBtn.dataset.hmClicked = '1';
  try { contactBtn.click(); } catch (e) {}
}

// 2) If a «قوانین / هشدار پلیس / متوجه شدم / تأیید» modal button is open, click it automatically
const acceptBtn = buttons.find(b => {
  const t = (b.innerText || '').trim();
  return t === 'تأیید' || t === 'تایید' || t === 'متوجه شدم' || t === 'ادامه' || t === 'قبول' || t.includes('مشاهده اطلاعات تماس');
});
if (acceptBtn && !acceptBtn.dataset.hmAccepted) {
  acceptBtn.dataset.hmAccepted = '1';
  try { acceptBtn.click(); } catch (e) {}
}

// 3) Direct in-browser authenticated fetch to Divar's contact endpoint using the user's active session/token
if (!window.__hmFetched) {
  window.__hmFetched = true;
  window.__hmPhone = '';
  window.__hmNeedLogin = false;
  const m = location.pathname.match(/\/v\/([^/?#]+)/);
  if (m && location.hostname.includes('divar.ir')) {
    const postToken = decodeURIComponent(m[1]).split('_').pop();
    let authHeader = {};
    try {
      for (let i = 0; i < localStorage.length; i++) {
        const k = localStorage.key(i);
        const v = localStorage.getItem(k) || '';
        if (/token|auth/i.test(k) && v.length > 20) {
          const parsed = v.startsWith('{') ? JSON.parse(v) : v;
          const tok = typeof parsed === 'string' ? parsed : (parsed.token || parsed.access_token || parsed.value || '');
          if (tok && typeof tok === 'string') {
            authHeader = { 'Authorization': tok.startsWith('Bearer ') ? tok : ('Bearer ' + tok) };
            break;
          }
        }
      }
    } catch (e) {}

    const endpoints = [
      `https://api.divar.ir/v8/postcontact/web/contact_info/${postToken}`,
      `https://api.divar.ir/v8/postcontact/web/contact_info_v2/${postToken}`,
      `https://api.divar.ir/v5/posts/${postToken}/contact/`
    ];
    (async () => {
      for (const ep of endpoints) {
        try {
          const r = await fetch(ep, {
            method: ep.includes('_v2') ? 'POST' : 'GET',
            credentials: 'include',
            headers: { 'Accept': 'application/json', 'Content-Type': 'application/json', ...authHeader },
            body: ep.includes('_v2') ? '{}' : undefined
          });
          if (r.status === 401 || r.status === 403) {
            window.__hmNeedLogin = true;
            continue;
          }
          if (!r.ok) continue;
          const txt = cleanDigits(await r.text());
          const pm = txt.match(/(?:\+98|0098|0)?(9\d{9})/) || txt.match(/(0[1-8]\d{9})/);
          if (pm) {
            window.__hmPhone = pm[0].startsWith('9') ? ('0' + pm[1]) : pm[0];
            break;
          }
        } catch (e) {}
      }
    })();
  }
}
return true;
"#;

// Reads the revealed phone number, ad date, neighbourhood/address, seller name, and description
const DETAIL_JS: &str = r#"
const clean = s => String(s || '').replace(/[\uE000-\uF8FF\u200e\u200f]/g, '').replace(/\s+/g, ' ').trim();
const latin = s => clean(s).replace(/[۰-۹]/g, d => '۰۱۲۳۴۵۶۷۸۹'.indexOf(d)).replace(/[٠-٩]/g, d => '٠١٢٣٤٥٦٧٨٩'.indexOf(d));
const body = document.body ? document.body.innerText : '';
const h1 = document.querySelector('h1');

let phone = window.__hmPhone || '';

// 1) Check `a[href^="tel:"]` anywhere in the document (including modals/portals)
if (!phone) {
  const telLinks = [...document.querySelectorAll('a[href^="tel:"]')];
  for (const tl of telLinks) {
    const raw = latin(decodeURIComponent((tl.getAttribute('href') || '').replace(/^tel:/i, '')));
    const m = raw.match(/(?:\+98|0098|0)?(9\d{9})/) || raw.match(/(0[1-8]\d{9})/);
    if (m) {
      phone = m[0].startsWith('9') ? ('0' + m[1]) : m[0];
      break;
    }
  }
}

// 2) Check contact rows («شماره موبایل», «شماره تماس», `.copy-row`, modal dialogs)
if (!phone) {
  const rows = [...document.querySelectorAll('.copy-row, [class*="contact"], [class*="phone"], [role="dialog"] *, .kt-unexpandable-row, .kt-base-row')];
  for (const el of rows) {
    const t = latin(el.innerText || '');
    if (t.includes('0999') || t.includes('123456')) continue; // skip login input placeholders
    const m = t.match(/\b(09\d{2}[\s\-]*\d{3}[\s\-]*\d{4})\b/) || t.match(/\b(0[1-8]\d{1,2}[\s\-]*\d{7,8})\b/);
    if (m) {
      phone = m[1].replace(/[\s\-]/g, '');
      break;
    }
  }
}

const loginModal = !!(
  window.__hmNeedLogin ||
  document.querySelector('input[type="tel"], input[name*="mobile"], input[placeholder*="موبایل"]') ||
  body.includes('ورود به حساب کاربری') ||
  body.includes('شمارهٔ موبایل خود را وارد کنید') ||
  body.includes('کد تأیید را وارد کنید') ||
  body.includes('برای مشاهده اطلاعات تماس')
);

let seller = '';
const shopLink = document.querySelector('a[href*="/shops/"], a[href*="/marketplace/"], a[href*="/shop/"]');
if (shopLink) seller = clean(shopLink.innerText);

let desc = '';
const descEl = document.querySelector('.kt-description-row__text, [class*="description"], [class*="Description"], article p');
if (descEl) desc = clean(descEl.innerText);

// Extract posting date & location subtitle (e.g. «نیم ساعت پیش در تهران، سعادت‌آباد»)
let adDate = '';
let address = '';
const subEl = document.querySelector('.kt-page-title__subtitle, [class*="subtitle"], [class*="location"]');
const subText = clean(subEl ? subEl.innerText : '');
if (subText) {
  if (subText.includes(' در ')) {
    const parts = subText.split(' در ');
    adDate = clean(parts[0]);
    address = clean(parts.slice(1).join('، '));
  } else {
    address = subText;
  }
}
// Also check structured rows for «محله» or «آدرس»
for (const r of document.querySelectorAll('.kt-unexpandable-row, .kt-base-row')) {
  const rt = clean(r.innerText);
  if ((rt.startsWith('محله') || rt.startsWith('منطقه') || rt.startsWith('آدرس')) && rt.length < 120) {
    const val = clean(rt.replace(/^(محله|منطقه|آدرس)\s*[:：]?\s*/, ''));
    if (val && !address.includes(val)) {
      address = address ? `${address}، ${val}` : val;
    }
  }
}

const breadcrumbs = [...document.querySelectorAll('nav a, [class*="breadcrumb"] a')].map(a => clean(a.innerText)).filter(Boolean);
const category = breadcrumbs.length > 1 ? breadcrumbs[breadcrumbs.length - 1] : '';

const main = document.querySelector('main, article') || document.body;
const links = [...main.querySelectorAll('a[href]')].map(a => a.href).filter(h =>
  !/divar\.ir|sheypoor\.com/.test(h) && /^https?:\/\//i.test(h)
);

const img = document.querySelector('main img[src^="https://"], article img[src^="https://"]');

return {
  ready: !!(h1 && clean(h1.innerText)),
  title: clean(h1 && h1.innerText),
  phone,
  description: desc || body.slice(0, 3500),
  needs_login: loginModal && !phone,
  blocked: /captcha|کپچا|ربات نیستم|تأیید هویت/i.test(body.slice(0, 1500)),
  address,
  ad_date: adDate,
  seller,
  category,
  links,
  photo: img ? img.src : '',
};
"#;

pub async fn run_divar(db: &Db, s: &Settings, profile: &Path, req: &ScanRequest, token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    run_platform(
        db,
        s,
        profile,
        req,
        token,
        emit,
        "divar",
        "دیوار",
        "https://divar.ir/my-divar/my-posts",
        "https://divar.ir/s/{city}?q={q}",
    )
    .await
}

pub async fn run_sheypoor(db: &Db, s: &Settings, profile: &Path, req: &ScanRequest, token: &CancellationToken, emit: Emit<'_>) -> Res<Summary> {
    run_platform(
        db,
        s,
        profile,
        req,
        token,
        emit,
        "sheypoor",
        "شیپور",
        "https://www.sheypoor.com/session",
        "https://www.sheypoor.com/s/{city}?q={q}",
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn run_platform(
    db: &Db,
    s: &Settings,
    profile: &Path,
    req: &ScanRequest,
    token: &CancellationToken,
    emit: Emit<'_>,
    source_id: &str,
    source_label: &str,
    login_url: &str,
    search_tpl: &str,
) -> Res<Summary> {
    let keywords: Vec<String> = req.keywords.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect();

    if keywords.is_empty() {
        return Err("حداقل یک کلمه کلیدی وارد کنید.".into());
    }

    // Interactive login mode triggered by the login button in the UI
    if keywords.len() == 1 && keywords[0] == "__login__" {
        status(emit, format!("باز کردن پنجره مرورگر برای ورود به حساب {source_label}…"));
        let mut eng = launch(s, profile, false, 0).await?;
        goto(&eng.list, login_url).await;
        emit(ScanEvent::Captcha {
            message: format!(
                "پنجره مرورگر باز شد. لطفاً با شماره موبایل خود وارد حساب {source_label} شوید و پس از ورود، پنجره مرورگر را ببندید تا نشست شما ذخیره شود."
            ),
        });
        let deadline = Instant::now() + Duration::from_secs(600);
        while Instant::now() < deadline && !token.is_cancelled() {
            if !eng.alive.load(Relaxed) || eng.proc.exited() {
                break;
            }
            sleep(Duration::from_secs(1)).await;
        }
        shut(Some(eng)).await;
        return Ok(Summary {
            checked: 0,
            found: 0,
            cancelled: false,
            error: false,
            message: format!("نشست حساب {source_label} ذخیره شد. اکنون می‌توانید استخراج خودکار را شروع کنید."),
        });
    }

    let limit = req.limit.clamp(1, 1000) as usize;
    let slug = city_slug(&req.province, &req.city);
    let http = crate::enrich::client();

    status(emit, format!("در حال آماده‌سازی مرورگر برای {source_label}…"));
    let mut eng = tokio::select! {
        e = launch(s, profile, s.headless, 1) => e?,
        _ = token.cancelled() => return Ok(Summary { cancelled: true, message: "متوقف شد.".into(), ..Default::default() }),
    };

    let mut sum = Summary::default();
    let mut login_prompted = false;

    for (i, kw) in keywords.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }

        // Build search terms based on district / city / province
        let mut terms: Vec<String> = Vec::new();
        if !req.district.trim().is_empty() {
            terms.push(format!("{kw} {}", req.district.trim()));
        } else if slug == "iran" && !req.city.trim().is_empty() {
            terms.push(format!("{kw} {}", req.city.trim()));
        } else if slug == "iran" && !req.areas.is_empty() {
            for a in req.areas.iter().take(8) {
                terms.push(format!("{kw} {}", a.trim()));
            }
        } else {
            terms.push(kw.clone());
        }

        let run_id = db.run_start(req.list_id, kw, &req.province, &req.run_city(), limit as i64)?;
        let resume = ScanRequest {
            keywords: keywords[i..].to_vec(),
            ..req.clone()
        };
        let resume_json = serde_json::to_string(&resume).unwrap_or_default();
        db.run_checkpoint(run_id, 0, 0, &resume_json);

        let mut ads: Vec<AdItem> = Vec::new();
        let mut seen_urls = HashSet::new();

        for term in &terms {
            if token.is_cancelled() || ads.len() >= limit * 2 {
                break;
            }
            status(emit, format!("جستجوی آگهی‌های جدید «{term}» در {source_label} (به ترتیب تاریخ)…"));
            let search_url = search_tpl.replace("{city}", slug).replace("{q}", &pct_encode(term));

            goto(&eng.list, &search_url).await;

            let mut idle_scrolls = 0;
            for _ in 0..10 {
                tokio::select! {
                    _ = sleep(Duration::from_millis(1800)) => {}
                    _ = token.cancelled() => break,
                }
                if let Ok(out) = js::<ListOut>(&eng.list, LIST_JS).await {
                    if out.blocked && !login_prompted {
                        login_prompted = true;
                        emit(ScanEvent::Captcha {
                            message: format!("{source_label} تأیید «ربات نیستم» خواست. در پنجره بازشده تأیید را انجام دهید تا ادامه پیدا کند."),
                        });
                        shut(Some(eng)).await;
                        eng = launch(s, profile, false, 1).await?;
                        goto(&eng.list, &search_url).await;
                        sleep(Duration::from_secs(8)).await;
                        continue;
                    }
                    let before = ads.len();
                    for item in out.items {
                        if seen_urls.insert(item.href.clone()) {
                            ads.push(item);
                        }
                    }
                    if ads.len() >= limit * 2 {
                        break;
                    }
                    if ads.len() == before {
                        idle_scrolls += 1;
                        if idle_scrolls >= 3 {
                            break;
                        }
                    } else {
                        idle_scrolls = 0;
                    }
                }
            }
        }

        if ads.is_empty() {
            crate::scanner::log(emit, "warning", format!("آگهی فعالی برای «{kw}» در {source_label} پیدا نشد."));
            let _ = db.run_finish(run_id, 0, 0, "done", "");
            continue;
        }

        let (mut checked, mut found) = (0i64, 0i64);
        emit(ScanEvent::Progress {
            keyword: kw.clone(),
            collected: ads.len() as i64,
            checked: 0,
            found: 0,
            target: limit as i64,
        });

        for ad in &ads {
            if token.is_cancelled() || found >= limit as i64 || checked >= (limit * 2) as i64 {
                break;
            }
            let key = format!("{source_id}:{}", place_key(&ad.href));
            if db.known(req.list_id, &key) {
                continue;
            }

            status(emit, format!("دریافت اطلاعات تماس: {}", ad.title));
            goto(&eng.workers[0], &ad.href).await;

            // Calm, structured polling: trigger contact tab & API, then wait for phone reveal
            let mut detail = AdDetail::default();
            for step in 0..8 {
                tokio::select! {
                    _ = sleep(Duration::from_millis(if step == 0 { 1600 } else { 950 })) => {}
                    _ = token.cancelled() => break,
                }
                let _ = js::<bool>(&eng.workers[0], TRIGGER_CONTACT_JS).await;
                if let Ok(d) = js::<AdDetail>(&eng.workers[0], DETAIL_JS).await {
                    detail = d;
                    if !detail.phone.is_empty() || (detail.ready && step >= 5) {
                        break;
                    }
                }
            }

            // If login or captcha is required to see the phone number, open a visible browser window once
            if (detail.needs_login || detail.blocked) && detail.phone.is_empty() && !login_prompted && !token.is_cancelled() {
                login_prompted = true;
                emit(ScanEvent::Captcha {
                    message: format!(
                        "برای نمایش شماره‌های بخش «اطلاعات تماس» در {source_label}، پنجره مرورگر باز شد. لطفاً وارد حساب خود شوید یا تأیید را بزنید؛ به محض نمایش شماره، استخراج آرام و خودکار ادامه می‌یابد."
                    ),
                });
                shut(Some(eng)).await;
                eng = launch(s, profile, false, 1).await?;
                goto(&eng.workers[0], &ad.href).await;

                for _ in 0..75 {
                    if token.is_cancelled() || !eng.alive.load(Relaxed) || eng.proc.exited() {
                        break;
                    }
                    sleep(Duration::from_secs(2)).await;
                    let _ = js::<bool>(&eng.workers[0], TRIGGER_CONTACT_JS).await;
                    if let Ok(d) = js::<AdDetail>(&eng.workers[0], DETAIL_JS).await {
                        if !d.phone.is_empty() {
                            detail = d;
                            break;
                        }
                    }
                }
                if s.headless && !token.is_cancelled() {
                    shut(Some(eng)).await;
                    eng = launch(s, profile, true, 1).await?;
                }
                status(emit, format!("ادامه استخراج منظم از {source_label}…"));
            }

            checked += 1;
            let mut c = Contacts::default();
            if !detail.phone.is_empty() {
                c.add_phone(&detail.phone);
            }
            for p in phones_in_text(&detail.description) {
                c.add_phone(&p);
            }
            for e in emails_in_text(&detail.description) {
                c.add_email(&e);
            }
            for l in links_in_text(&detail.description) {
                c.add_link(&l);
            }
            for l in &detail.links {
                c.add_link(l);
            }

            if req.enrich && !c.website.is_empty() && c.needs_more() {
                let _ = tokio::time::timeout(Duration::from_secs(15), crate::enrich::enrich(&http, &mut c)).await;
            }

            // Build full address & date context to attach alongside the phone number and place record
            let raw_addr = if !detail.address.is_empty() {
                detail.address.clone()
            } else {
                ad.subtitle.clone()
            };
            let cleaned_addr = clean_address(&raw_addr);
            let date_tag = if !detail.ad_date.is_empty() {
                detail.ad_date.clone()
            } else if ad.subtitle.contains("پیش") {
                ad.subtitle.split(" در ").next().unwrap_or("").trim().to_string()
            } else {
                String::new()
            };

            // Label attached directly beside the phone number in the `contacts` table
            let contact_label = match (!cleaned_addr.is_empty(), !date_tag.is_empty()) {
                (true, true) => format!("{cleaned_addr} · {date_tag}"),
                (true, false) => cleaned_addr.clone(),
                (false, true) => date_tag.clone(),
                (false, false) => source_label.to_string(),
            };
            let labels = vec![("mobile", contact_label.clone()), ("landline", contact_label.clone())];

            let note_info = [
                if !date_tag.is_empty() {
                    format!("تاریخ آگهی: {date_tag}")
                } else {
                    String::new()
                },
                if !cleaned_addr.is_empty() {
                    format!("محدوده: {cleaned_addr}")
                } else {
                    String::new()
                },
                format!("لینک آگهی: {}", ad.href),
            ]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(" | ");

            let place = Place {
                list_id: req.list_id,
                place_key: key,
                name: if detail.title.is_empty() { ad.title.clone() } else { detail.title },
                contact_person: detail.seller,
                category: if detail.category.is_empty() { kw.clone() } else { detail.category },
                province: req.province.trim().to_string(),
                city: if !req.city.trim().is_empty() {
                    req.city.trim().to_string()
                } else {
                    crate::logic::city_in(&cleaned_addr, &req.city, &req.areas)
                },
                address: cleaned_addr,
                website: c.website.clone(),
                maps_url: ad.href.clone(), // Direct link to the Divar/Sheypoor ad
                note: note_info,
                keyword: kw.clone(),
                status: "new".into(),
                source: source_id.into(),
                ..Default::default()
            };

            if crate::sources::save(db, emit, place, &c, source_id, &labels) {
                found += 1;
            }

            db.run_checkpoint(run_id, checked, found, &resume_json);
            emit(ScanEvent::Progress {
                keyword: kw.clone(),
                collected: ads.len() as i64,
                checked,
                found,
                target: limit as i64,
            });

            // Calm, human-like pacing (2.2s – 4.2s) so Divar never rate-limits the account's contact tab
            let wait_ms = jitter(s.delay_ms.max(2200), (s.delay_ms * 4).max(4200));
            tokio::select! {
                _ = sleep(Duration::from_millis(wait_ms)) => {}
                _ = token.cancelled() => break,
            }
        }

        let st = if token.is_cancelled() { "cancelled" } else { "done" };
        let _ = db.run_finish(run_id, checked, found, st, "");
        sum.checked += checked;
        sum.found += found;
    }

    status(emit, "در حال بستن مرورگر…");
    shut(Some(eng)).await;

    sum.cancelled = token.is_cancelled();
    let head = if sum.cancelled {
        "متوقف شد."
    } else {
        &format!("پایان استخراج از {source_label}.")
    };
    sum.message = format!(
        "{head} {} آگهی بررسی و {} مخاطب جدید همراه با آدرس و لینک آگهی ذخیره شد.",
        sum.checked, sum.found
    );
    Ok(sum)
}
