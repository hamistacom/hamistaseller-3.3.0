//! «دریافت لیست محصول» end to end with a real Chromium against a local mock shop and t.me pages.
//! Runs with HS_TEST_BROWSER=<chrome / msedge path>.
use harness::products::{self, ListReq};
use harness::scanner::Settings;
use std::io::{Read, Write};
use std::net::TcpListener;
use tokio_util::sync::CancellationToken;

fn serve(reply: fn(&str) -> String) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = head.split(' ').nth(1).unwrap_or("/").to_string();
                let out = reply(&path);
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    port
}

fn card(href: &str, title: &str, price: &str) -> String {
    format!(r#"<a href="{href}"><img src="/img/{title}.jpg"><div>{title}</div><div>{price} تومان</div></a>"#)
}

fn shop(path: &str) -> String {
    match path {
        "/" => format!(r#"<html><body><a href="/category/honey">عسل</a>{}</body></html>"#, card("/product/1", "عسل گون یک کیلویی", "۴۵۰,۰۰۰")),
        "/category/honey" => format!(
            r#"<html><body>{}{}{}<a rel="next" href="/category/honey?page=2">بعدی</a></body></html>"#,
            card("/product/2", "عسل کنار نیم کیلویی", "۲۲۰,۰۰۰"),
            card("/product/1", "عسل گون یک کیلویی", "۴۵۰,۰۰۰"),
            card("https://othershop.example/p/9", "عسل تبلیغاتی سایت دیگر", "۹۹,۰۰۰")
        ),
        "/category/honey?page=2" => format!("<html><body>{}</body></html>", card("/product/3", "عسل آویشن", "۳۸۰,۰۰۰")),
        _ => "<html><body>404</body></html>".into(),
    }
}

fn settings() -> Option<Settings> {
    let exe = std::env::var("HS_TEST_BROWSER").ok()?;
    Some(Settings { browser_path: exe, concurrency: 1, delay_ms: 10, expand_areas: false, headless: true })
}

#[tokio::test(flavor = "multi_thread")]
async fn whole_site_and_one_category_stay_in_scope() {
    let Some(s) = settings() else { return };
    let port = serve(shop);
    let dir = std::env::temp_dir().join(format!("hs-plist-e2e-{}", std::process::id()));
    let tok = CancellationToken::new();
    // whole site: home + category + its next page, never the other shop's ad
    let req = ListReq { url: format!("http://127.0.0.1:{port}/"), ..Default::default() };
    let r = products::run(&s, &dir.join("p"), &req, &tok, &|_| {}).await.unwrap();
    let mut titles: Vec<_> = r.items.iter().map(|i| i.title.as_str()).collect();
    titles.sort();
    assert_eq!(titles, ["عسل آویشن", "عسل کنار نیم کیلویی", "عسل گون یک کیلویی"], "{:?}", r.notes);
    let gon = r.items.iter().find(|i| i.title.contains("گون")).unwrap();
    assert_eq!(gon.price, 450_000);
    assert_eq!(gon.url, format!("http://127.0.0.1:{port}/product/1"));
    assert!(gon.image.ends_with(".jpg"), "{}", gon.image);
    assert!(r.notes.iter().any(|n| n.contains("خارج از محدوده")), "{:?}", r.notes);
    assert_eq!((r.stats.min, r.stats.max), (220_000, 450_000));
    // the Excel file opens with every row
    let path = products::export_xlsx(&r, &dir).unwrap();
    use calamine::Reader;
    let mut wb: calamine::Xlsx<_> = calamine::open_workbook(&path).unwrap();
    assert_eq!(wb.worksheet_range("محصولات").unwrap().height(), 4);
    // a filter keeps only matching names
    let req = ListReq { url: format!("http://127.0.0.1:{port}/category/honey"), query: "آویشن".into(), ..Default::default() };
    let r = products::run(&s, &dir.join("p"), &req, &tok, &|_| {}).await.unwrap();
    assert_eq!(r.items.len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn telegram_channel_posts_page_by_page() {
    let port = serve(|path| {
        let post = |id: u32, text: &str| format!(r#"<div class="tgme_widget_message_wrap"><div data-post="honey_shop/{id}"><div class="tgme_widget_message_text js-message_text" dir="auto">{text}</div><time datetime="2026-10-0{}T10:00:00+00:00"></time></div></div>"#, id % 9 + 1);
        if path.contains("before=3") {
            post(1, "عسل گون<br/>۴۰۰ هزار تومان") + &post(2, "عسل کنار ۲۵۰,۰۰۰ تومان")
        } else if path.contains("before=") {
            String::new()
        } else {
            r#"<meta property="og:title" content="Honey Shop">"#.to_string() + &post(3, "آویشن ۳۰۰,۰۰۰ ت") + &post(4, "تعطیلات")
        }
    });
    std::env::set_var("HAMISTA_TME_BASE", format!("http://127.0.0.1:{port}"));
    let s = Settings::default();
    let req = ListReq { url: "https://t.me/honey_shop".into(), ..Default::default() };
    let r = products::run(&s, std::path::Path::new("."), &req, &CancellationToken::new(), &|_| {}).await.unwrap();
    std::env::remove_var("HAMISTA_TME_BASE");
    let got: Vec<_> = r.items.iter().map(|i| (i.url.as_str(), i.price)).collect();
    assert_eq!(
        got,
        [
            ("https://t.me/honey_shop/4", 0),
            ("https://t.me/honey_shop/3", 300_000),
            ("https://t.me/honey_shop/2", 250_000),
            ("https://t.me/honey_shop/1", 400_000)
        ]
    );
    assert_eq!(r.items[0].seller, "Honey Shop");
}

#[tokio::test(flavor = "multi_thread")]
async fn product_analysis_reads_only_the_users_own_source() {
    let Some(s) = settings() else { return };
    let port = serve(shop);
    let dir = std::env::temp_dir().join(format!("hs-plist-mkt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = harness::db::Db::open(&dir.join("t.db")).unwrap();
    let req = harness::market::MarketReq {
        query: "عسل".into(),
        platforms: vec!["torob".into()], // ignored: the given source wins
        site: format!("http://127.0.0.1:{port}/category/honey"),
        exact: false,
        ..Default::default()
    };
    let r = harness::market::search(&db, &s, &dir.join("p"), &req, &CancellationToken::new(), &|_| {}).await.unwrap();
    assert!(r.offers.iter().all(|o| o.platform == "web" && o.url.contains("127.0.0.1")), "{:?}", r.offers);
    assert_eq!(r.offers.len(), 3);
    assert_eq!(r.platforms.len(), 1);
    assert_eq!(r.platforms[0].0, "web");
    let _ = std::fs::remove_dir_all(dir);
}
