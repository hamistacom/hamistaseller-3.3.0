//! Public web search (Bing, in the app's own headless browser) used by the sources that have no
//! directory API of their own: Balad place pages, public Telegram channels, Instagram business pages.
//! Only result links + their public snippets are read. The admin can change the engine remotely:
//! scenario keys `searchUrl` (with `{q}` and `{first}`) and `searchJs`.
use crate::db::Res;
use crate::scanner::{goto, js, script};
use chromiumoxide::Page;
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub url: String,
    pub title: String,
    pub snippet: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Page0 {
    items: Vec<(String, String, String, String)>, // href, title, snippet, cite
    blocked: bool,
    ready: bool,
}

const SEARCH_JS: &str = r#"
const t = el => (el && el.innerText || '').replace(/\s+/g, ' ').trim();
const items = [...document.querySelectorAll('#b_results > li.b_algo')].map(li => {
  const a = li.querySelector('h2 a');
  const p = li.querySelector('.b_caption p, p.b_lineclamp1, p.b_lineclamp2, p.b_lineclamp3, p.b_lineclamp4, .b_algoSlug, .b_snippet');
  return [a ? a.href : '', t(a), t(p), t(li.querySelector('cite'))];
});
return {
  items,
  blocked: !!document.querySelector('#b_captcha, .b_captcha, #turnstile-widget, iframe[src*="challenge"]'),
  ready: document.readyState === 'complete' && !!document.querySelector('#b_results, #b_content, main'),
};
"#;

fn search_url(query: &str, first: usize) -> String {
    let base = crate::logic::test_seam("HAMISTA_SEARCH_BASE")
        .map(|b| format!("{b}/search?q={{q}}&first={{first}}"))
        .unwrap_or_else(|| script("searchUrl", "https://www.bing.com/search?q={q}&first={first}&setlang=fa&cc=IR"));
    base.replace("{q}", &crate::logic::pct_encode(query)).replace("{first}", &first.to_string())
}

/// Bing wraps results in `bing.com/ck/a?…&u=a1<base64url of the real link>`.
pub fn real_url(href: &str, cite: &str) -> String {
    if href.contains("bing.com/ck/a") {
        if let Some(u) = crate::logic::query_param(href, "u") {
            let u = u.strip_prefix("a1").unwrap_or(&u);
            if let Some(bytes) = crate::logic::b64_decode(u) {
                if let Ok(s) = String::from_utf8(bytes) {
                    if s.starts_with("http") {
                        return s;
                    }
                }
            }
        }
        // fall back to the visible address under the title
        let c = cite.replace(" › ", "/").split_whitespace().next().unwrap_or_default().to_string();
        if !c.is_empty() {
            return if c.starts_with("http") { c } else { format!("https://{c}") };
        }
    }
    href.to_string()
}

/// Runs `query` and reads up to `pages` result pages (10 results each). Stops early on an empty page.
pub async fn search(page: &Page, query: &str, pages: usize, token: &CancellationToken) -> Res<Vec<Hit>> {
    let mut out: Vec<Hit> = vec![];
    for n in 0..pages.max(1) {
        if token.is_cancelled() {
            break;
        }
        goto(page, &search_url(query, n * 10 + 1)).await;
        let t = Instant::now();
        let got = loop {
            let r: Page0 = js(page, &script("searchJs", SEARCH_JS)).await.unwrap_or_default();
            if r.blocked {
                return if out.is_empty() {
                    Err("موتور جستجو تأیید «ربات نیستم» خواست؛ چند دقیقه بعد دوباره امتحان کنید یا تعداد را کم کنید.".into())
                } else {
                    Ok(out)
                };
            }
            if (r.ready && !r.items.is_empty()) || t.elapsed() > Duration::from_secs(12) {
                break r;
            }
            tokio::select! {
                _ = sleep(Duration::from_millis(300)) => {}
                _ = token.cancelled() => return Ok(out),
            }
        };
        let before = out.len();
        for (href, title, snippet, cite) in got.items {
            let url = real_url(&href, &cite);
            if url.starts_with("http") && !out.iter().any(|h| h.url == url) {
                out.push(Hit { url, title, snippet });
            }
        }
        if out.len() == before {
            break; // no more results
        }
        // polite pacing between result pages
        tokio::select! {
            _ = sleep(Duration::from_millis(crate::logic::jitter(1200, 2500))) => {}
            _ = token.cancelled() => break,
        }
    }
    Ok(out)
}

const VERIFY: &str = "موتور جستجو تأیید «ربات نیستم» خواست. یک پنجره مرورگر باز شد؛ تأیید را در آن انجام دهید تا کار خودکار ادامه پیدا کند.";

/// `search`, and when the engine asks «are you a robot», a visible window for the user to pass
/// the check (10 minutes at most) — the results are then read from that window.
pub async fn search_attended(
    page: &Page,
    s: &crate::scanner::Settings,
    profile: &std::path::Path,
    query: &str,
    pages: usize,
    token: &CancellationToken,
    emit: crate::scanner::Emit<'_>,
) -> Res<Vec<Hit>> {
    match search(page, query, pages, token).await {
        Err(e) if e.contains("ربات") => {}
        other => return other,
    }
    emit(crate::scanner::ScanEvent::Captcha { message: VERIFY.into() });
    let mut eng = crate::scanner::launch(s, profile, false, 0).await?;
    goto(&eng.list, &search_url(query, 1)).await;
    let deadline = Instant::now() + Duration::from_secs(600);
    let solved = loop {
        tokio::select! {
            _ = sleep(Duration::from_secs(2)) => {}
            _ = token.cancelled() => break false,
        }
        if eng.proc.exited() || !eng.alive.load(std::sync::atomic::Ordering::Relaxed) || Instant::now() > deadline {
            break false;
        }
        let r: Page0 = js(&eng.list, &script("searchJs", SEARCH_JS)).await.unwrap_or_default();
        if !r.blocked && r.ready {
            break true;
        }
    };
    let out = if solved { search(&eng.list, query, pages, token).await } else { Err("تأیید «ربات نیستم» انجام نشد؛ پنجره بسته شد یا ۱۰ دقیقه گذشت.".into()) };
    crate::scanner::shut(Some(eng)).await;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bing_redirects_are_unwrapped() {
        // a1 + base64url("https://t.me/lamiz_cafe")
        let href = "https://www.bing.com/ck/a?!&&p=abc&u=a1aHR0cHM6Ly90Lm1lL2xhbWl6X2NhZmU&ntb=1";
        assert_eq!(real_url(href, ""), "https://t.me/lamiz_cafe");
        assert_eq!(real_url("https://www.bing.com/ck/a?p=x", "https://instagram.com › cafe"), "https://instagram.com/cafe");
        assert_eq!(real_url("https://balad.ir/p/x", ""), "https://balad.ir/p/x");
    }
}
