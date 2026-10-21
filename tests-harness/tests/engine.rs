//! End-to-end checks of the Phase 3 engines on a real SQLite file and local mock servers.
use harness::channels;
use harness::db::{Account, Contact, Db, Filter, Place};
use harness::logic::Contacts;
use harness::outreach::{run_campaign, CampaignReq};
use harness::scanner::ScanEvent;
use harness::{enrich, export, secrets};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("hs-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Tiny HTTP server: `reply(path, body) -> (content type, body)`; returns port and request log.
fn serve(reply: impl Fn(&str, &str) -> (&'static str, String) + Send + Sync + 'static) -> (u16, Arc<Mutex<Vec<String>>>) {
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
            let (head, body) = loop {
                let n = s.read(&mut tmp).unwrap_or(0);
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&tmp[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(i) = text.find("\r\n\r\n") {
                    let len = text[..i]
                        .lines()
                        .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    if buf.len() >= i + 4 + len {
                        break (text[..i].to_string(), String::from_utf8_lossy(&buf[i + 4..i + 4 + len]).to_string());
                    }
                }
            };
            let path = head.split(' ').nth(1).unwrap_or("").to_string();
            log2.lock().unwrap().push(format!("{path}\n{body}"));
            let (ct, out) = reply(&path, &body);
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    (port, log)
}

fn place(list_id: i64, key: &str, name: &str, mobiles: &[&str]) -> Place {
    Place {
        list_id,
        place_key: key.into(),
        name: name.into(),
        contact_person: "رضا".into(),
        city: "تهران".into(),
        status: "new".into(),
        contacts: mobiles
            .iter()
            .map(|m| Contact { kind: "mobile".into(), value: m.to_string(), origin: "maps".into(), ..Default::default() })
            .collect(),
        ..Default::default()
    }
}

fn sms_account(db: &Db, port: u16) {
    let a = Account {
        id: "sms-1".into(),
        kind: "sms".into(),
        name: "پنل تست".into(),
        config: json!({"method": "GET", "url": format!("http://127.0.0.1:{port}/send?key={{apikey}}&to={{to}}&text={{text}}"), "bodyType": "none", "okPath": "status", "okValues": ["ok"]}),
        per_minute: 1200,
        has_secret: true,
    };
    db.account_save(&a, Some(secrets::seal_str(r#"{"apiKey":"K1"}"#).unwrap())).unwrap();
}

#[test]
fn import_export_roundtrip() {
    let dir = tmp("ie");
    let db = Db::open(&dir.join("t.db")).unwrap();
    let csv = "نام کسب‌وکار,نام مسئول,سمت,موبایل,تلفن,ایمیل,اینستاگرام,شهر,برچسب‌ها,تاریخ\n\
               \"کافه لمیز\",علی امینی,مدیرعامل,\"0912 345 6789، 09351112233\",021-88776655,Info@Lamiz.ir,@cafe.lamiz,تهران,vip,1403\n\
               بی‌شماره,,,,,,,,,\n\
               دوم,,,09123456789,,,,کرج,,\n";
    let rep = export::import(&db, 1, "a.csv", csv.as_bytes().to_vec()).unwrap();
    assert_eq!((rep.added, rep.duplicates, rep.skipped), (1, 1, 1));
    assert_eq!(rep.ignored, vec!["تاریخ"]);
    let (rows, total) = db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    assert_eq!(total, 1);
    let p = &rows[0];
    assert_eq!((p.name.as_str(), p.contact_person.as_str(), p.contact_role.as_str(), p.tags.as_str()), ("کافه لمیز", "علی امینی", "مدیرعامل", "vip"));
    let kinds: Vec<(&str, &str)> = p.contacts.iter().map(|c| (c.kind.as_str(), c.value.as_str())).collect();
    for want in [
        ("mobile", "09123456789"),
        ("mobile", "09351112233"),
        ("landline", "02188776655"),
        ("email", "info@lamiz.ir"),
        ("instagram", "https://instagram.com/cafe.lamiz"),
    ] {
        assert!(kinds.contains(&want), "{want:?} missing in {kinds:?}");
    }
    // same file again: nothing new
    let again = export::import(&db, 1, "a.csv", csv.as_bytes().to_vec()).unwrap();
    assert_eq!(again.added, 0);

    let all = Filter { list_id: 1, ..Default::default() };
    let out = dir.join("out");
    for fmt in ["xlsx", "csv"] {
        let path = export::export(&db, &all, fmt, &out).unwrap();
        let bank = db.list_create(&format!("from {fmt}"), "").unwrap();
        let rep = export::import(&db, bank, &path, std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(rep.added, 1, "{fmt}: {:?} / ignored {:?}", rep.columns, rep.ignored);
        let (rows, _) = db.list(&Filter { list_id: bank, ..Default::default() }, 0, 0).unwrap();
        let q = &rows[0];
        assert_eq!((q.name.as_str(), q.contact_person.as_str(), q.contact_role.as_str()), ("کافه لمیز", "علی امینی", "مدیرعامل"), "{fmt}");
        assert_eq!(q.contacts.len(), p.contacts.len(), "{fmt}: {:?}", q.contacts);
    }
    {
        use calamine::Reader;
        let x = export::export(&db, &all, "xlsx", &out).unwrap();
        let mut wb = calamine::open_workbook_auto(&x).unwrap();
        assert_eq!(wb.sheet_names(), vec!["مخاطبان", "تحلیل"]);
        let r = wb.worksheet_range("تحلیل").unwrap();
        let cells: Vec<String> = r.rows().next().unwrap().iter().map(|c| c.to_string()).collect();
        assert!(cells.contains(&"شهرستان".to_string()) && cells.contains(&"حوزه کاری".to_string()), "{cells:?}");
        assert_eq!(r.rows().nth(1).unwrap()[3].to_string(), "تهران"); // county block (the file has a city, no province)
    }
    let txt = std::fs::read_to_string(export::export(&db, &all, "txt", &out).unwrap()).unwrap();
    assert_eq!(txt, "09123456789\r\n09351112233\r\n");
    let vcf = std::fs::read_to_string(export::export(&db, &all, "vcf", &out).unwrap()).unwrap();
    assert!(vcf.contains("FN:علی امینی (کافه لمیز)") && vcf.contains("TEL;TYPE=CELL:09351112233"), "{vcf}");
    assert!(export::export(&db, &Filter { list_id: 999, ..Default::default() }, "csv", &out).is_err());
    // Excel on Persian Windows saves CSV as Windows-1256; pandas writes phone numbers as floats
    let (cp1256, _, _) = encoding_rs::WINDOWS_1256.encode("نام,شماره\nنانوايي,9351234567.0\n");
    let bank = db.list_create("cp1256", "").unwrap();
    let rep = export::import(&db, bank, "excel.csv", cp1256.into_owned()).unwrap();
    assert_eq!(rep.added, 1, "{:?}", rep.columns);
    let (rows, _) = db.list(&Filter { list_id: bank, ..Default::default() }, 0, 0).unwrap();
    assert_eq!((rows[0].name.as_str(), rows[0].contacts[0].value.as_str()), ("نانوایی", "09351234567"));
}

#[test]
fn run_resume_checkpoint() {
    let db = Db::open(&tmp("runs").join("t.db")).unwrap();
    let id = db.run_start(1, "کافه", "تهران", "تهران", 50).unwrap();
    db.run_checkpoint(id, 5, 2, r#"{"keywords":["کافه"]}"#);
    db.run_finish(id, 5, 2, "cancelled", "").unwrap();
    assert_eq!(db.run(id).unwrap().resume, r#"{"keywords":["کافه"]}"#);
    db.run_finish(id, 9, 3, "done", "").unwrap();
    assert_eq!(db.run(id).unwrap().resume, "");
}

#[tokio::test(flavor = "multi_thread")]
async fn sms_campaign() {
    let (port, log) = serve(|path, _| ("application/json", if path.contains("to=09120000003") { r#"{"status":"bad"}"# } else { r#"{"status":"ok"}"# }.into()));
    let db = Db::open(&tmp("camp").join("t.db")).unwrap();
    for p in [
        place(1, "a", "الف", &["09120000001"]),
        place(1, "b", "ب", &["09120000002", "09120000003"]),
        place(1, "c", "ج", &["09120000004"]),
    ] {
        db.insert_place(&p).unwrap();
    }
    db.set_optouts(&["09120000004".into()]).unwrap();
    sms_account(&db, port);
    let req = CampaignReq {
        filter: Filter { list_id: 1, ..Default::default() },
        account_id: "sms-1".into(),
        message: "سلام {person} از {name}".into(),
        optout_footer: true,
        every_number: true,
        ..Default::default()
    };
    let events = Mutex::new(0);
    let emit = |_e: ScanEvent| *events.lock().unwrap() += 1;
    let acc = channels::load(&db, "sms-1").unwrap();
    let sum = run_campaign(&db, &req, acc, &CancellationToken::new(), &emit).await.unwrap();
    assert_eq!((sum.found, sum.checked, sum.error), (2, 4, false), "{}", sum.message);
    let reqs = log.lock().unwrap().clone();
    assert_eq!(reqs.len(), 3);
    assert!(reqs.iter().all(|r| r.contains("key=K1")), "secret not merged: {reqs:?}");
    let first = reqs.iter().find(|r| r.contains("09120000001")).unwrap();
    let text = harness::logic::pct_decode(first.split("text=").nth(1).unwrap().split('\n').next().unwrap());
    assert_eq!(text, "سلام رضا از الف\nلغو۱۱");
    let c = &db.campaigns().unwrap()[0];
    assert_eq!((c.sent, c.failed, c.skipped, c.status.as_str()), (2, 1, 1, "done"));
    let (rows, _) = db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    assert!(rows.iter().filter(|p| p.name != "ج").all(|p| p.status == "contacted"));
    assert_eq!(rows.iter().find(|p| p.name == "ج").unwrap().status, "new");
    assert!(db.messaged_within("09120000001", "sms", 7));

    // skip_recent_days: everyone reached this week is skipped now
    let acc = channels::load(&db, "sms-1").unwrap();
    let sum = run_campaign(&db, &CampaignReq { skip_recent_days: 7, ..req }, acc, &CancellationToken::new(), &emit).await.unwrap();
    assert_eq!(sum.found, 0, "{}", sum.message);
}

#[tokio::test(flavor = "multi_thread")]
async fn failing_panel_stops_after_five() {
    let (port, _log) = serve(|_, _| ("application/json", r#"{"status":"denied"}"#.into()));
    let db = Db::open(&tmp("fail").join("t.db")).unwrap();
    for i in 0..8 {
        db.insert_place(&place(1, &format!("k{i}"), "x", &[&format!("0912000010{i}")])).unwrap();
    }
    sms_account(&db, port);
    let req = CampaignReq { filter: Filter { list_id: 1, ..Default::default() }, account_id: "sms-1".into(), message: "hi".into(), ..Default::default() };
    let acc = channels::load(&db, "sms-1").unwrap();
    let sum = run_campaign(&db, &req, acc, &CancellationToken::new(), &|_| {}).await.unwrap();
    assert!(sum.error && sum.message.contains("۵ ارسال"), "{}", sum.message);
    let c = &db.campaigns().unwrap()[0];
    assert_eq!((c.failed, c.status.as_str()), (5, "failed"));
    // test message goes to one number only and reports the panel error
    let acc = channels::load(&db, "sms-1").unwrap();
    let r = run_campaign(&db, &CampaignReq { test_to: "09129999999".into(), ..req }, acc, &CancellationToken::new(), &|_| {}).await;
    assert!(r.err().unwrap().contains("پنل پیامک خطا داد"));
}

#[tokio::test(flavor = "multi_thread")]
async fn website_enrichment() {
    let (port, _) = serve(|path, _| {
        let html = if path.starts_with("/contact") {
            r#"<p>مدیرعامل: علی امینی</p><p>ایمیل: sales@example.ir</p><a href="https://t.me/lamiz_cafe">t</a>"#.to_string()
        } else {
            r#"<a href="/contact-us">تماس با ما</a><a href="https://instagram.com/lamiz">ig</a> 0912 111 2233"#.to_string()
        };
        ("text/html; charset=utf-8", html)
    });
    let mut c = Contacts { website: format!("http://127.0.0.1:{port}/"), ..Default::default() };
    enrich::enrich(&enrich::client(), &mut c).await;
    assert_eq!(c.instagram, "https://instagram.com/lamiz");
    assert_eq!(c.telegram, "https://t.me/lamiz_cafe");
    assert_eq!(c.emails, vec!["sales@example.ir"]);
    assert_eq!(c.mobiles, vec!["09121112233"]);
    assert_eq!((c.person.as_str(), c.person_role.as_str()), ("علی امینی", "مدیرعامل"));
}

/// Mock Google Maps: a 6-place feed, place cards with phones + website, websites with a manager's name.
fn mock_maps(slow_sites: bool) -> u16 {
    let (port, _) = serve(move |path, _| {
        let html = if path.starts_with("/maps/search/") {
            let items: String = (1..=6)
                .map(|i| format!(r#"<a class="hfpxzc" href="/maps/place/P{i}/data=!3d35.7{i}!4d51.4{i}!1s0x1{i}:0xa{i}" aria-label="کافه {i}">کافه {i}</a>"#))
                .collect();
            format!(r#"<div role="feed" style="height:200px;overflow:auto">{items}<p>You've reached the end of the list.</p></div>"#)
        } else if let Some(rest) = path.strip_prefix("/maps/place/P") {
            let i = &rest[..1];
            format!(r#"<h1>کافه {i}</h1><button data-item-id="phone:tel:0912000000{i}">x</button><a data-item-id="authority" href="/site{i}">site</a><div data-item-id="address">تهران {i}</div>"#)
        } else if path.starts_with("/site") {
            if slow_sites {
                std::thread::sleep(std::time::Duration::from_secs(20));
            }
            let i = &path[5..6];
            format!(r#"<p>مدیر فروش: سارا رضایی</p><p>sales{i}@example.ir</p>"#)
        } else {
            String::new()
        };
        ("text/html; charset=utf-8", html)
    });
    port
}

fn scan_settings() -> Option<harness::scanner::Settings> {
    let exe = std::env::var("HS_TEST_BROWSER").ok()?;
    Some(harness::scanner::Settings { browser_path: exe, concurrency: 2, delay_ms: 10, expand_areas: false, headless: true })
}

#[tokio::test(flavor = "multi_thread")]
async fn scan_end_to_end_and_stop() {
    let Some(s) = scan_settings() else { return };
    let dir = tmp("scan");
    let db = Db::open(&dir.join("t.db")).unwrap();
    std::env::set_var("HAMISTA_MAPS_BASE", format!("http://127.0.0.1:{}", mock_maps(false)));
    let req = harness::scanner::ScanRequest {
        list_id: 1,
        enrich: true,
        keywords: vec!["کافه".into()],
        city: "تهران".into(),
        limit: 4,
        ..Default::default()
    };
    let found = Mutex::new(vec![]);
    let emit = |e: ScanEvent| {
        if let ScanEvent::Found { place } = e {
            found.lock().unwrap().push(place.name.clone());
        }
    };
    let sum = harness::scanner::run(&db, &s, &dir.join("profile"), &req, &CancellationToken::new(), &emit).await.unwrap();
    assert_eq!((sum.checked, sum.found, sum.cancelled, sum.error), (4, 4, false, false), "{}", sum.message);
    assert_eq!(found.lock().unwrap().len(), 4);
    let (rows, _) = db.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    let p = rows.iter().find(|p| p.name == "کافه 1").expect("place 1 saved");
    assert_eq!((p.contact_person.as_str(), p.contact_role.as_str()), ("سارا رضایی", "مدیر فروش"));
    assert_eq!(p.first("mobile").map(|c| (c.value.as_str(), c.origin.as_str())), Some(("09120000001", "maps")));
    assert_eq!(p.first("email").map(|c| (c.value.as_str(), c.origin.as_str())), Some(("sales1@example.ir", "website")));
    assert_eq!((p.lat, p.address.as_str()), (Some(35.71), "تهران 1"));
    let run = &db.runs().unwrap()[0];
    assert_eq!((run.status.as_str(), run.checked, run.found, run.resume.as_str()), ("done", 4, 4, ""));

    // second scan: the 4 known places are skipped, the remaining 2 are new
    let sum = harness::scanner::run(&db, &s, &dir.join("profile"), &req, &CancellationToken::new(), &|_| {}).await.unwrap();
    assert_eq!((sum.checked, sum.found), (2, 2), "{}", sum.message);

    // Stop while websites hang: the places already read from Maps are still saved, and the run can resume
    let db2 = Db::open(&dir.join("t2.db")).unwrap();
    std::env::set_var("HAMISTA_MAPS_BASE", format!("http://127.0.0.1:{}", mock_maps(true)));
    let token = CancellationToken::new();
    let t2 = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        t2.cancel();
    });
    let started = std::time::Instant::now();
    let sum = harness::scanner::run(&db2, &s, &dir.join("profile"), &req, &token, &|_| {}).await.unwrap();
    assert!(sum.cancelled, "{}", sum.message);
    assert!(started.elapsed() < std::time::Duration::from_secs(16), "stop took {:?}", started.elapsed());
    let (rows, _) = db2.list(&Filter { list_id: 1, ..Default::default() }, 0, 0).unwrap();
    assert_eq!(rows.len(), 4, "places read before Stop must be saved");
    assert!(rows.iter().all(|p| p.contact_person.is_empty() && p.first("mobile").is_some()));
    let run = &db2.runs().unwrap()[0];
    assert_eq!(run.status, "cancelled");
    let resume: harness::scanner::ScanRequest = serde_json::from_str(&run.resume).unwrap();
    assert_eq!(resume.keywords, vec!["کافه"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_crash_is_restarted_and_captcha_is_reported() {
    let Some(mut s) = scan_settings() else { return };
    let dir = tmp("crash");
    let db = Db::open(&dir.join("t.db")).unwrap();
    std::env::set_var("HAMISTA_MAPS_BASE", format!("http://127.0.0.1:{}", mock_maps(false)));
    s.delay_ms = 1000; // slow enough to kill the browser mid-scan
    let profile = dir.join("profile-crash");
    let p2 = profile.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        let _ = std::process::Command::new("pkill").args(["-9", "-f", &p2.to_string_lossy()]).status();
    });
    let req = harness::scanner::ScanRequest { list_id: 1, keywords: vec!["کافه".into()], city: "تهران".into(), limit: 6, ..Default::default() };
    let logs = Mutex::new(vec![]);
    let emit = |e: ScanEvent| {
        if let ScanEvent::Log { message, .. } = e {
            logs.lock().unwrap().push(message);
        }
    };
    let sum = harness::scanner::run(&db, &s, &profile, &req, &CancellationToken::new(), &emit).await.unwrap();
    assert!(logs.lock().unwrap().iter().any(|m| m.contains("دوباره اجرا")), "no restart: {:?}", logs.lock().unwrap());
    assert_eq!((sum.found, sum.error), (6, false), "{}", sum.message);
    assert_eq!(db.runs().unwrap().len(), 1, "a restart continues the same run row");

    // captcha: reported, and the run keeps its checkpoint for «ادامه استخراج»
    let (port, _) = serve(|_, _| ("text/html", r#"<form id="captcha-form"></form>"#.into()));
    std::env::set_var("HAMISTA_MAPS_BASE", format!("http://127.0.0.1:{port}"));
    let saw = Mutex::new(false);
    let emit = |e: ScanEvent| {
        if let ScanEvent::Captcha { .. } = e {
            *saw.lock().unwrap() = true;
        }
    };
    let db2 = Db::open(&dir.join("t2.db")).unwrap();
    let r = harness::scanner::run(&db2, &s, &profile, &req, &CancellationToken::new(), &emit).await;
    assert!(*saw.lock().unwrap());
    let run = &db2.runs().unwrap()[0];
    println!("captcha outcome: {:?} / run {} {}", r.as_ref().err(), run.status, run.error);
    assert!(!run.resume.is_empty());
}

#[test]
fn dashboard_series() {
    let dir = tmp("dash");
    let db = Db::open(&dir.join("t.db")).unwrap();
    let mut a = place(1, "a", "کافه یک", &["09120000001"]);
    a.source = "maps".into();
    db.insert_place(&a).unwrap();
    let mut b = place(1, "b", "کافه دو", &["09120000002"]);
    b.source = "neshan".into();
    db.insert_place(&b).unwrap();
    let d = db.dashboard().unwrap();
    assert_eq!(d.weekly.len(), 12);
    assert_eq!(d.weekly.last().unwrap().1, 2, "both added this week");
    assert_eq!(d.weekly.iter().map(|w| w.1).sum::<i64>(), 2);
    assert_eq!((d.month, d.prev_month, d.contacts), (2, 0, 2));
    let mut s = d.sources.clone();
    s.sort();
    assert_eq!(s, vec![("maps".to_string(), 1, 1), ("neshan".to_string(), 1, 1)]);
}

#[test]
fn bank_breakdown() {
    let db = Db::open(&tmp("stats").join("t.db")).unwrap();
    let mk = |k: &str, prov: &str, city: &str, cat: &str, m: &str, email: bool| {
        let mut p = place(1, k, k, &[m]);
        (p.province, p.city, p.category) = (prov.into(), city.into(), cat.into());
        if email {
            p.contacts.push(Contact { kind: "email".into(), value: format!("{k}@x.ir"), ..Default::default() });
        }
        db.insert_place(&p).unwrap();
    };
    mk("a", "تهران", "تهران", "کافه", "09120000001", true);
    mk("b", "تهران", "تهران", "رستوران", "09120000002", false);
    mk("c", "تهران", "شهریار", "کافه", "09120000003", false);
    mk("d", "اصفهان", "کاشان", "کافه", "09120000004", true);
    let s = db.bank_stats(&Filter { list_id: 1, ..Default::default() }).unwrap();
    assert_eq!(s.total, 4);
    assert_eq!(s.provinces, vec![("تهران".to_string(), 3), ("اصفهان".to_string(), 1)]);
    assert_eq!(s.cities[0], ("تهران".to_string(), "تهران".to_string(), 2));
    assert_eq!(s.categories[0], ("کافه".to_string(), 3));
    assert!(s.channels.contains(&("email".to_string(), 2)) && s.channels.contains(&("mobile".to_string(), 4)));
    assert!(s.channels.contains(&("person".to_string(), 4)));
    // drill-down: same breakdown inside one province and one line of business
    let s = db.bank_stats(&Filter { list_id: 1, province: "تهران".into(), category: "کافه".into(), ..Default::default() }).unwrap();
    assert_eq!(s.total, 2);
    assert_eq!(s.cities.len(), 2);
    let (rows, _) = db.list(&Filter { category: "رستوران".into(), ..Default::default() }, 0, 0).unwrap();
    assert_eq!(rows.len(), 1);
}

#[test]
fn schedules_compute_next_runs() {
    use harness::schedule::{self, Schedule};
    let dir = tmp("sched");
    let db = Db::open(&dir.join("t.db")).unwrap();
    let req = harness::scanner::ScanRequest { list_id: 1, keywords: vec!["کافه".into()], city: "تهران".into(), limit: 20, ..Default::default() };
    let mut s = Schedule { name: "کافه‌های تهران".into(), req: req.clone(), every: "daily".into(), at: "03:00".into(), ..Default::default() };
    let id = schedule::save(&db, &s).unwrap();
    let got = schedule::get(&db, id).unwrap();
    assert_eq!(got.next_run.len(), 16);
    assert!(got.next_run.ends_with(" 03:00"), "{}", got.next_run);
    assert!(got.next_run > db.today(), "daily is always in the future");
    // weekly: the next given weekday
    s.id = id;
    s.every = "weekly".into();
    s.weekday = 6; // Saturday
    schedule::save(&db, &s).unwrap();
    let wk = schedule::get(&db, id).unwrap().next_run;
    let wd: i64 = db_weekday(&db, &wk);
    assert_eq!(wd, 6, "{wk}");
    // a one-off in the past is refused; nothing is due yet
    s.every = "once".into();
    s.on_date = "2021-01-01".into();
    assert!(schedule::save(&db, &s).is_err());
    assert!(schedule::due(&db).is_none());
    // switched off → no next run; validation
    schedule::set_enabled(&db, id, false).unwrap();
    assert_eq!(schedule::get(&db, id).unwrap().next_run, "");
    assert!(schedule::save(&db, &Schedule { name: "x".into(), req, every: "daily".into(), at: "25:00".into(), ..Default::default() }).is_err());
    schedule::delete(&db, id).unwrap();
    assert!(schedule::list(&db).unwrap().is_empty());
}

fn db_weekday(db: &Db, when: &str) -> i64 {
    // through the app's own SQLite (same date rules as the scheduler)
    let p = std::env::temp_dir().join(format!("hs-wd-{}.db", std::process::id()));
    let _ = db;
    let c = rusqlite::Connection::open(p).unwrap();
    c.query_row("SELECT CAST(strftime('%w', ?1) AS INTEGER)", [when], |r| r.get(0)).unwrap()
}

#[test]
fn photo_and_messenger_marks() {
    let db = Db::open(&tmp("apps").join("t.db")).unwrap();
    let list = db.list_create("x", "navy").unwrap();
    let mut p = place(list, "k1", "کافه", &["09121112233"]);
    p.photo = "https://lh5.googleusercontent.com/p/x".into();
    db.insert_place(&p).unwrap();
    let id = db.list(&Filter { list_id: list, ..Default::default() }, 0, 10).unwrap().0[0].id;
    let got = db.get(id).unwrap();
    assert_eq!(got.photo, p.photo, "photo kept for the card");
    let c = got.contacts[0].id;
    db.set_app(c, "whatsapp", true).unwrap();
    db.set_app(c, "telegram", false).unwrap();
    db.set_app(c, "whatsapp", false).unwrap(); // a later answer replaces the earlier one
    assert_eq!(db.get(id).unwrap().contacts[0].apps, "telegram:0,whatsapp:0");
    // the photo never leaves in an export
    let csv = export::export(&db, &Filter { list_id: list, ..Default::default() }, "csv", &tmp("apps-out")).unwrap();
    assert!(!std::fs::read_to_string(csv).unwrap().contains("googleusercontent"));
}

#[tokio::test(flavor = "multi_thread")]
async fn market_search_end_to_end() {
    let (port, log) = serve(|path, _| {
        let body = if path.starts_with("/torob") {
            json!({"results": [
                {"name1": "عسل گون خوانسار ۱ کیلویی", "price": 590000, "shop_text": "در ۹ فروشگاه", "web_client_absolute_url": "/p/1/"},
                {"name1": "عسل کنار شهدینه ۱ کیلویی", "price": 650000, "shop_text": "در ۴ فروشگاه"},
                {"name1": "قاشق عسل خوری", "price": 90000}
            ]})
        } else if path.starts_with("/digikala") {
            json!({"data": {"products": [
                {"title_fa": "عسل طبیعی سبلان ۵۰۰ گرمی", "data_layer": {"brand": "سبلان"}, "default_variant": {"price": {"selling_price": 3_100_000}, "seller": {"title": "دیجی‌کالا"}}, "rating": {"rate": 4.3, "count": 210}},
                {"title_fa": "عسل خوانسار ۹۶۰ گرمی", "default_variant": {"price": {"selling_price": 6_400_000}}, "rating": {"rate": 4.6, "count": 80}}
            ]}})
        } else {
            json!({})
        };
        ("application/json", body.to_string())
    });
    std::env::set_var("HAMISTA_MARKET_BASE", format!("http://127.0.0.1:{port}"));
    let db = Db::open(&tmp("market").join("t.db")).unwrap();
    let req = harness::market::MarketReq { query: "عسل".into(), platforms: vec!["torob".into(), "digikala".into()], target_margin: 20.0, ..Default::default() };
    let emit = |_: ScanEvent| {};
    let s = harness::scanner::Settings::default();
    let r = harness::market::search(&db, &s, &tmp("market"), &req, &CancellationToken::new(), &emit).await.unwrap();
    assert_eq!(r.offers.len(), 5, "4 honey jars + a honey spoon (still matches «عسل»)");
    assert!(r.channels.iter().any(|c| c.name == "دیجی‌کالا") && r.channels.iter().any(|c| c.name == "ترب"));
    assert!(r.offers.iter().any(|o| o.platform == "digikala" && o.price == 310_000), "Rial → Toman");
    assert!(r.brands.iter().any(|b| b.name == "خوانسار" && b.count == 2));
    assert!(r.id > 0 && r.srp > 0);
    assert_eq!(r.platforms.len(), 2);
    // a second search the same day → one history point; saved runs are listed and reloadable
    let r2 = harness::market::search(&db, &s, &tmp("market"), &req, &CancellationToken::new(), &emit).await.unwrap();
    assert_eq!(r2.history.len(), 1);
    assert_eq!(db.market_runs(10).unwrap().len(), 2);
    assert_eq!(db.market_get(r.id).unwrap().offers.len(), 5);
    assert!(log.lock().unwrap().iter().any(|l| l.starts_with("/torob?q=")));
    // brand analysis reads the saved searches
    let id = db.brand_save(&harness::brand::Brand { name: "خوانسار".into(), category: "عسل".into(), own: true, ..Default::default() }).unwrap();
    let b = db.brands().unwrap().into_iter().find(|b| b.id == id).unwrap();
    let rep = harness::brand::analyze(&db, &s, &tmp("market"), &b, &CancellationToken::new(), &emit).await.unwrap();
    assert!(!rep.channels.is_empty() && rep.mix.len() == 4, "live market reading: {:?}", rep.channels);
    assert_eq!(rep.market.offers, 2);
    assert!(rep.market.price_index > 0.0 && rep.presence.is_some());
    assert!(db.brands().unwrap()[0].report.is_some(), "report stored");
    // strategy: the owner's answers are kept (schema v12) and the plan is built on the live analysis
    let inp = harness::strategy::Inputs { budget: 20_000_000, unit_cost: 300_000, ..Default::default() };
    db.brand_plan_save(id, &inp).unwrap();
    let b = db.brands().unwrap().remove(0);
    assert_eq!(b.plan.as_ref(), Some(&inp));
    let st = harness::strategy::build(&b, &inp);
    assert!(st.sector.contains("غذایی") && st.canvas.len() == 9, "{}", st.sector);
    assert!(st.finance.price > 0 && st.finance.months.len() == 12 && st.evidence.iter().any(|e| e.contains("عرضه")), "{:?}", st.evidence);
    assert_eq!(r2.source_history.len(), 2, "one median per source for today");
    // daily watchlist: a watched product is re-analysed into its own history
    use harness::watch;
    let wid = watch::save(&db, &watch::Watch { req: req.clone(), alert_pct: 5.0, ..Default::default() }).unwrap();
    assert_eq!(watch::save(&db, &watch::Watch { req: req.clone(), ..Default::default() }).unwrap(), wid, "watched once");
    let sum = watch::run(&db, &s, &tmp("market"), &[], &CancellationToken::new(), &emit).await.unwrap();
    assert_eq!((sum.checked, sum.error), (1, false));
    let w = watch::list(&db).unwrap().remove(0);
    assert!(w.median > 0 && w.last_id > 0 && w.history.len() == 1 && w.swing.points == 1, "{w:?}");
    assert!(watch::due(&db).is_empty(), "checked today");
    // catalogue: imported from CSV, then identified in the market by brand and size
    let csv = "نام کالا,قیمت فروش,برند\nعسل گون خوانسار ۱ کیلویی,650000,\nروغن زیتون ممتاز,1,\n";
    let imp = watch::import(&db, "items.csv", csv.as_bytes().to_vec()).unwrap();
    assert_eq!((imp.added, imp.updated), (2, 0));
    assert_eq!(watch::import(&db, "items.csv", csv.as_bytes().to_vec()).unwrap().updated, 2, "re-import updates");
    let two = vec!["torob".to_string(), "digikala".to_string()];
    watch::check(&db, &s, &tmp("market"), &[], &two, 25.0, &CancellationToken::new(), &emit).await.unwrap();
    let items = watch::items(&db).unwrap();
    assert_eq!((items[0].matched, items[0].confidence.as_str(), items[0].min), (1, "high", 590_000), "{:?}", items[0]);
    assert_eq!(items[0].position, 100.0);
    assert_eq!(items[1].confidence, "none");
    std::env::remove_var("HAMISTA_MARKET_BASE");
}

#[tokio::test(flavor = "multi_thread")]
async fn market_page_fallback_in_browser() {
    let Some(s) = scan_settings() else { return };
    let (port, _) = serve(|path, _| {
        if path.starts_with("/emalls") {
            let html = r#"<html><body>
              <script id="__NEXT_DATA__" type="application/json">{"props":{"items":[{"title":"زعفران سرگل نیم مثقال","price":420000,"shop_name":"زعفران‌سرای الف"}]}}</script>
              <a href="https://emalls.ir/p/1"><div>زعفران نگین یک مثقالی</div><div>۸۹۰٬۰۰۰ تومان</div></a>
              <a href="https://emalls.ir/p/2"><div>زعفران پوشال</div><div>۶,۵۰۰,۰۰۰ ریال</div></a>
            </body></html>"#;
            ("text/html; charset=utf-8", html.to_string())
        } else {
            ("application/json", "{}".to_string())
        }
    });
    std::env::set_var("HAMISTA_MARKET_PAGE", format!("http://127.0.0.1:{port}"));
    let db = Db::open(&tmp("market-page").join("t.db")).unwrap();
    let req = harness::market::MarketReq { query: "زعفران".into(), platforms: vec!["emalls".into()], ..Default::default() };
    let emit = |_: ScanEvent| {};
    let r = harness::market::search(&db, &s, &tmp("market-page").join("profile"), &req, &CancellationToken::new(), &emit).await.unwrap();
    std::env::remove_var("HAMISTA_MARKET_PAGE");
    let mut prices: Vec<i64> = r.offers.iter().map(|o| o.price).collect();
    prices.sort();
    assert_eq!(prices, vec![420_000, 650_000, 890_000], "embedded data + visible cards, Rial converted");
    assert!(r.offers.iter().any(|o| o.seller == "زعفران‌سرای الف"));
}
