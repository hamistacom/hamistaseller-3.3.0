//! The new sources end to end with a real Chromium against local mocks of Neshan's API, Google Maps,
//! the web search engine, Balad pages, t.me pages and the Instagram Graph API.
//! Runs with HS_TEST_BROWSER=<chrome / msedge path>; run with --test-threads=1 (shared env vars).
use harness::db::{Db, Filter};
use harness::scanner::{ScanEvent, ScanRequest, Settings};
use harness::{cloud, logic, neshan, sources};
use serde_json::{json, Map};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("hs-src-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Tiny HTTP server: `reply(path) -> (content type, body)`; returns port and the request log.
fn serve(reply: impl Fn(&str) -> (&'static str, String) + Send + Sync + 'static) -> (u16, Arc<Mutex<Vec<String>>>) {
    let reply = Arc::new(reply);
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let log = Arc::new(Mutex::new(vec![]));
    let log2 = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let (reply, log2) = (reply.clone(), log2.clone());
            std::thread::spawn(move || {
                let mut buf = vec![];
                let mut tmp = [0u8; 4096];
                let head = loop {
                    let n = s.read(&mut tmp).unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        break text[..i].to_string();
                    }
                };
                let path = logic::pct_decode(head.split(' ').nth(1).unwrap_or(""));
                log2.lock().unwrap().push(format!("{path} | {}", head.lines().find(|l| l.to_lowercase().starts_with("api-key")).unwrap_or("")));
                let (ct, out) = reply(&path);
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    (port, log)
}

fn settings() -> Option<Settings> {
    let exe = std::env::var("HS_TEST_BROWSER").ok()?;
    Some(Settings { browser_path: exe, concurrency: 2, delay_ms: 10, expand_areas: false, headless: true })
}

fn html(h: String) -> (&'static str, String) {
    ("text/html; charset=utf-8", h)
}

/// Google Maps mock answering exact look-ups: a direct jump for one name, a list for another,
/// and an unrelated business for a name Maps doesn't know.
fn mock_maps() -> u16 {
    serve(|path| {
        if let Some(q) = path.strip_prefix("/maps/search/") {
            return html(if q.contains("روبرتو") {
                "<script>location.replace('/maps/place/R/data=!3d35.71!4d51.41!1s0x17:0xa7')</script>".into()
            } else if q.contains("لمیز") {
                r#"<div role="feed"><a class="hfpxzc" href="/maps/place/L/data=!3d35.72!4d51.42!1s0x18:0xa8" aria-label="کافه لمیز">x</a><p>You've reached the end of the list.</p></div>"#.into()
            } else {
                r#"<div role="feed"><a class="hfpxzc" href="/maps/place/Z/data=!3d35.73!4d51.43!1s0x19:0xa9" aria-label="رستوران شب">x</a><p>You've reached the end of the list.</p></div>"#.into()
            });
        }
        html(match path.get(..13) {
            Some("/maps/place/R") => r#"<h1>کافه روبرتو</h1><button data-item-id="phone:tel:09121110001">x</button><div data-item-id="address">پاسداران</div>"#.into(),
            Some("/maps/place/L") => r#"<h1>کافه لمیز</h1><button data-item-id="phone:tel:09121110002">x</button>"#.into(),
            Some("/maps/place/Z") => r#"<h1>رستوران شب</h1><button data-item-id="phone:tel:09121110003">x</button>"#.into(),
            _ => String::new(),
        })
    })
    .0
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "source not offered in this version (engine kept for later)"]
async fn neshan_finds_names_then_phones_on_maps() {
    let Some(s) = settings() else { return };
    let dir = tmp("neshan");
    let db = Db::open(&dir.join("t.db")).unwrap();
    let (np, nlog) = serve(|path| {
        let body = if path.starts_with("/v4/geocoding") {
            json!({"status": "OK", "location": {"x": 51.4, "y": 35.7}})
        } else {
            json!({"count": 4, "items": [
                {"title": "کافه روبرتو", "category": "place", "type": "cafe", "location": {"x": 51.41, "y": 35.71}},
                {"title": "كافه روبرتو", "category": "place", "type": "cafe"},
                {"title": "خیابان پاسداران", "category": "municipal", "type": "primary"},
                {"title": "کافه لمیز", "category": "place", "type": "cafe"},
                {"title": "کافه ناشناس", "category": "place", "type": "cafe"}
            ]})
        };
        ("application/json", body.to_string())
    });
    std::env::set_var("HAMISTA_NESHAN_BASE", format!("http://127.0.0.1:{np}"));
    std::env::set_var("HAMISTA_MAPS_BASE", format!("http://127.0.0.1:{}", mock_maps()));
    let req = ScanRequest { list_id: 1, keywords: vec!["کافه".into()], city: "تهران".into(), limit: 3, source: "neshan".into(), ..Default::default() };

    // no key yet → clear message
    let Err(e) = neshan::run(&db, &s, &dir.join("p"), &req, &CancellationToken::new(), &|_| {}).await else { panic!("ran without a key") };
    assert!(e.contains("کلید API نشان"), "{e}");
    let mut cfg = Map::new();
    cfg.insert("apiKey".into(), json!("service.test-key"));
    sources::config_save(&db, "neshan", &cfg).unwrap();
    assert_eq!(sources::config_status(&db, "neshan")["apiKey"], true);

    let found = Mutex::new(vec![]);
    let emit = |e: ScanEvent| {
        if let ScanEvent::Found { place } = e {
            found.lock().unwrap().push(place.name.clone());
        }
    };
    let sum = neshan::run(&db, &s, &dir.join("p"), &req, &CancellationToken::new(), &emit).await.unwrap();
    assert_eq!((sum.checked, sum.found, sum.error), (3, 2, false), "{}", sum.message);
    let mut names = found.lock().unwrap().clone();
    names.sort();
    assert_eq!(names, vec!["کافه روبرتو", "کافه لمیز"], "the unrelated Maps result is not saved");
    let (rows, _) = db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    assert!(rows.iter().all(|p| p.source == "neshan"));
    let r = rows.iter().find(|p| p.name == "کافه روبرتو").unwrap();
    assert_eq!(r.first("mobile").map(|c| c.value.as_str()), Some("09121110001"));
    let log = nlog.lock().unwrap().join("\n");
    assert!(log.contains("Api-Key: service.test-key") || log.contains("api-key: service.test-key"), "{log}");
    assert!(log.contains("/v4/geocoding?address=تهران"), "{log}");
    // one run row per keyword, not per business; its resume holds the exact look-ups
    let runs = db.runs().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].keyword, "کافه");
    // mega-bank takes Neshan finds (public), with their Maps contacts
    let (batch, _) = cloud::public_batch(&db, 0, 50).unwrap();
    assert_eq!(batch.len(), 2);
    assert!(batch.iter().all(|b| b.source == "neshan" && !b.mobiles.is_empty()));
}

/// Search engine mock (Bing-shaped) + Balad pages + t.me pages + Graph API, all on one port.
fn mock_web() -> (u16, Arc<Mutex<Vec<String>>>) {
    let port = Arc::new(Mutex::new(0u16));
    let p2 = port.clone();
    let (p, log) = serve(move |path| {
        let me = *p2.lock().unwrap();
        let li = |url: &str, title: &str, snippet: &str| {
            format!(r#"<li class="b_algo"><h2><a href="{url}">{title}</a></h2><div class="b_caption"><p>{snippet}</p></div><cite>{url}</cite></li>"#)
        };
        if let Some(q) = path.strip_prefix("/search?q=") {
            if !q.contains("first=1&") && !q.ends_with("first=1") {
                return html(r#"<ol id="b_results"></ol>"#.into()); // only one page of results
            }
            let items = if q.contains("balad.ir") {
                [
                    li(&format!("http://127.0.0.1:{me}/p/کافه-روبرتو-tehran_coffee-shop-4ctFksRaCpebRg"), "کافه روبرتو؛ آدرس، تلفن", ""),
                    li(&format!("http://127.0.0.1:{me}/p/کافه-دریم-tehran_cafe-5w2mk52kBnsyL0"), "کافه دریم", ""),
                    li(&format!("http://127.0.0.1:{me}/p/کافه-بی‌تلفن-tehran_cafe-9xxQQ11zzAA"), "کافه بی‌تلفن", ""),
                    li("https://balad.ir/blog/best-cafe-tehran/", "بلاگ", ""),
                ]
                .concat()
            } else if q.contains("t.me") {
                [
                    // the real engine wraps links: a1 + base64url("https://t.me/lamiz_cafe")
                    li("https://www.bing.com/ck/a?!&&p=1&u=a1aHR0cHM6Ly90Lm1lL2xhbWl6X2NhZmU&ntb=1", "Lamiz Cafe – Telegram", ""),
                    li("https://t.me/s/lamiz_cafe/77", "Lamiz post", ""),
                    li("https://t.me/lamiz_group", "Lamiz group", ""),
                    li("https://t.me/ali_person", "Ali", ""),
                    li("https://t.me/joinchat/AAAAA", "join", ""),
                ]
                .concat()
            } else {
                [
                    li("https://www.instagram.com/cafe.lamiz/", "Cafe Lamiz (@cafe.lamiz) • Instagram photos and videos", "کافه لمیز تهران ☕ سفارش: 09124445566 lamiz.ir"),
                    li("https://www.instagram.com/someone.private/", "Someone (@someone.private) • Instagram", "عکس‌های من از کافه‌گردی"),
                    li("https://www.instagram.com/p/Cx12/", "post", "09120000000"),
                ]
                .concat()
            };
            return html(format!(r#"<div id="b_content"><ol id="b_results">{items}</ol></div>"#));
        }
        if path.starts_with("/p/کافه-روبرتو") {
            return html(r#"<h1>کافه روبرتو</h1><script type="application/ld+json">{"@type":"CafeOrCoffeeShop","name":"کافه روبرتو","telephone":"021-22223333","address":{"streetAddress":"خیابان پاسداران","addressLocality":"تهران"},"geo":{"latitude":35.77,"longitude":51.47}}</script><a href="https://instagram.com/roberto.cafe">ig</a>"#.into());
        }
        if path.starts_with("/p/کافه-دریم") {
            return html(r#"<h1>کافه رستوران دریم</h1><a href="tel:09125556677">تماس</a>"#.into());
        }
        if path.starts_with("/p/") {
            return html("<h1>کافه بی‌تلفن</h1>".into());
        }
        // t.me pages
        if path == "/lamiz_cafe" {
            return html(r#"<meta property="og:title" content="Lamiz Cafe | کافه لمیز"><meta property="og:description" content="منوی روز و رزرو: 09127778899 &#8211; اینستاگرام https://instagram.com/cafe.lamiz"><div class="tgme_page_extra">12 345 subscribers</div>"#.into());
        }
        if path == "/lamiz_group" {
            return html(r#"<meta property="og:title" content="Lamiz group"><div class="tgme_page_extra">310 members, 12 online</div>"#.into());
        }
        if path.starts_with("/ali_person") {
            return html(r#"<meta property="og:title" content="Ali"><meta property="og:description" content="You can contact @ali_person right away.">"#.into());
        }
        // Graph API business_discovery
        if path.starts_with("/v21.0/IGID") {
            let body = if path.contains("username(cafe.lamiz)") {
                json!({"business_discovery": {"username": "cafe.lamiz", "name": "کافه لمیز", "biography": "رزرو 09124445566", "website": "https://lamiz.ir", "followers_count": 5120}, "id": "IGID"})
            } else {
                json!({"error": {"message": "Invalid user id", "code": 110}})
            };
            return ("application/json", body.to_string());
        }
        html(String::new())
    });
    *port.lock().unwrap() = p;
    (p, log)
}

async fn run_source(db: &Db, s: &Settings, dir: &std::path::Path, source: &str, city: &str) -> (harness::scanner::Summary, Vec<String>) {
    let req = ScanRequest { list_id: 1, keywords: vec!["کافه".into()], city: city.into(), limit: 10, source: source.into(), ..Default::default() };
    let found = Mutex::new(vec![]);
    let emit = |e: ScanEvent| {
        if let ScanEvent::Found { place } = e {
            found.lock().unwrap().push(place.name.clone());
        }
    };
    let sum = sources::run_search(db, s, &dir.join("p"), &req, &CancellationToken::new(), &emit).await.unwrap();
    let mut names = found.into_inner().unwrap();
    names.sort();
    (sum, names)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "source not offered in this version (engine kept for later)"]
async fn balad_telegram_instagram() {
    let Some(s) = settings() else { return };
    let dir = tmp("web");
    let (port, log) = mock_web();
    let base = format!("http://127.0.0.1:{port}");
    std::env::set_var("HAMISTA_SEARCH_BASE", &base);
    std::env::set_var("HAMISTA_TME_BASE", &base);
    std::env::set_var("HAMISTA_GRAPH_BASE", &base);

    // ---- Balad: public place pages, phone from structured data or tel: links
    let db = Db::open(&dir.join("balad.db")).unwrap();
    let (sum, names) = run_source(&db, &s, &dir, "balad", "تهران").await;
    assert_eq!((sum.checked, sum.found, sum.error), (3, 2, false), "{}", sum.message);
    assert_eq!(names, vec!["کافه رستوران دریم", "کافه روبرتو"]);
    let (rows, _) = db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    let r = rows.iter().find(|p| p.name == "کافه روبرتو").unwrap();
    assert_eq!((r.source.as_str(), r.address.as_str(), r.lat), ("balad", "خیابان پاسداران، تهران", Some(35.77)));
    assert_eq!(r.first("landline").map(|c| (c.value.as_str(), c.origin.as_str())), Some(("02122223333", "balad")));
    assert_eq!(r.first("instagram").map(|c| c.value.as_str()), Some("https://instagram.com/roberto.cafe"));
    assert!(log.lock().unwrap().iter().any(|l| l.contains("site:balad.ir/p کافه تهران")));
    // a second run reads nothing again
    let (sum, _) = run_source(&db, &s, &dir, "balad", "تهران").await;
    assert_eq!(sum.checked, 0);

    // ---- Telegram: only the public channel, never groups or people
    let db = Db::open(&dir.join("tg.db")).unwrap();
    let (sum, names) = run_source(&db, &s, &dir, "telegram", "").await;
    assert_eq!((sum.checked, sum.found), (3, 1), "{}", sum.message); // channel (once), group, person
    assert_eq!(names, vec!["Lamiz Cafe | کافه لمیز"]);
    let (rows, _) = db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    let c = &rows[0];
    let tg = c.first("telegram").unwrap();
    assert_eq!((tg.value.as_str(), tg.label.as_str()), ("https://t.me/lamiz_cafe", "12345 عضو"));
    assert_eq!(c.first("mobile").map(|x| x.value.as_str()), Some("09127778899"));
    assert_eq!(c.first("instagram").map(|x| x.value.as_str()), Some("https://instagram.com/cafe.lamiz"));

    // ---- Instagram without the API: only pages publishing a business contact
    let db = Db::open(&dir.join("ig.db")).unwrap();
    let (sum, names) = run_source(&db, &s, &dir, "instagram", "تهران").await;
    assert_eq!((sum.checked, sum.found), (2, 1), "{}", sum.message);
    assert_eq!(names, vec!["Cafe Lamiz"]);
    let (rows, _) = db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    assert_eq!(rows[0].first("mobile").map(|x| x.value.as_str()), Some("09124445566"));
    assert!(rows[0].first("instagram").is_some());

    // ---- Instagram with the official API: bio / website / followers from business_discovery
    let db = Db::open(&dir.join("ig2.db")).unwrap();
    let mut cfg = Map::new();
    cfg.insert("igUserId".into(), json!("IGID"));
    cfg.insert("accessToken".into(), json!("EAAB-token"));
    sources::config_save(&db, "instagram", &cfg).unwrap();
    let (sum, names) = run_source(&db, &s, &dir, "instagram", "تهران").await;
    assert_eq!((sum.checked, sum.found), (2, 1), "{}", sum.message);
    assert_eq!(names, vec!["کافه لمیز"]);
    let (rows, _) = db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    let ig = rows[0].first("instagram").unwrap();
    assert_eq!(ig.label, "5120 دنبال‌کننده");
    assert_eq!(rows[0].website, "https://lamiz.ir");
    // ---- all selected sources in one job: one failing (Neshan without a key) doesn't stop the others
    let db = Db::open(&dir.join("all.db")).unwrap();
    let req = ScanRequest { list_id: 1, keywords: vec!["کافه".into()], city: "تهران".into(), limit: 10, sources: vec!["telegram".into(), "neshan".into(), "balad".into()], ..Default::default() };
    let list = sources::requested(&req);
    assert_eq!(list, vec!["neshan", "balad", "telegram"], "registry order");
    let logs = Mutex::new(vec![]);
    let emit = |e: ScanEvent| {
        if let ScanEvent::Log { message, .. } = e {
            logs.lock().unwrap().push(message);
        }
    };
    let mut per = vec![];
    let sum = sources::run_all(&db, &s, &dir.join("p"), &req, &list, &CancellationToken::new(), &emit, |id, r| per.push((id.to_string(), r.found))).await.unwrap();
    assert_eq!(per, vec![("balad".to_string(), 2), ("telegram".to_string(), 1)]);
    assert_eq!((sum.found, sum.error), (3, false), "{}", sum.message);
    assert!(logs.lock().unwrap().iter().any(|l| l.contains("نقشه نشان") && l.contains("کلید API")), "the failing source is reported");
    let (rows, _) = db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    let mut srcs: Vec<_> = rows.iter().map(|p| p.source.clone()).collect();
    srcs.sort();
    assert_eq!(srcs, vec!["balad", "balad", "telegram"]);

    let reqs = log.lock().unwrap().join("\n");
    assert!(reqs.contains("access_token=EAAB-token"), "the token goes only to the Graph API");
    assert!(!reqs.contains("followers{") && !reqs.contains("/followers"), "followers are never requested");
}
