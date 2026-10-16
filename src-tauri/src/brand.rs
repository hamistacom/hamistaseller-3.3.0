//! Brand analysis («تحلیل برند»): your brand and competitors, scored on four pillars with public data
//!   * حضور در بازار (salience) — share of the offers in your product searches, platforms present on
//!   * کیفیت درک‌شده (perceived quality) — buyers' ratings and review volume on those platforms
//!   * حضور دیجیتال — the brand's own website: reachable, secure, findable, contactable, fast
//!   * شبکه اجتماعی — public Instagram business-profile size (followers / posts), when it is readable
//!
//! Plus price positioning (index against the market median), strengths, weaknesses and actions.
use crate::db::{err, Db, Res};
use crate::market::{words, Report};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Brand {
    pub id: i64,
    pub name: String,
    pub own: bool,
    pub website: String,
    pub instagram: String,
    pub category: String, // product searched in «محصول و قیمت» (e.g. «عسل»)
    pub note: String,
    pub report: Option<BrandReport>,
    pub analyzed_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Site {
    pub ok: bool,
    pub https: bool,
    pub ms: u64,
    pub kb: u64,
    pub title: String,
    pub description: String,
    pub h1: bool,
    pub schema: bool,
    pub contact: bool,
    pub socials: Vec<String>,
    pub error: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Social {
    pub handle: String,
    pub followers: Option<i64>,
    pub posts: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Market {
    pub offers: usize,
    pub share_pct: f64,
    pub platforms: Vec<String>,
    pub median: i64,
    pub market_median: i64,
    pub price_index: f64, // 100 = market median
    pub tier: String,
    pub rating: Option<f64>,
    pub reviews: i64,
    pub searches: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct BrandReport {
    pub presence: Option<f64>,
    pub quality: Option<f64>,
    pub digital: Option<f64>,
    pub social: Option<f64>,
    pub overall: f64,
    pub market: Market,
    pub site: Site,
    pub insta: Social,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub actions: Vec<String>,
    // ---- live market reading (v3.2) ----
    /// The website / Instagram were found automatically from the brand name.
    pub found_site: bool,
    pub found_instagram: bool,
    /// Per sales channel: offers, sellers and price level of this brand.
    pub channels: Vec<Channel>,
    /// A few of the brand's products (with photos) as they are sold now.
    pub products: Vec<crate::market::Offer>,
    /// Product lines seen in the brand's titles («عسل گون», «عسل کنار» …).
    pub lines: Vec<String>,
    pub sellers: usize,
    pub top_sellers: Vec<(String, usize)>,
    pub price_min: i64,
    pub price_max: i64,
    /// The other brands in the same market, by share of offers.
    pub competitors: Vec<crate::market::Share>,
    /// Marketing mix (4P): product, price, place, promotion.
    pub mix: Vec<MixItem>,
    pub opportunities: Vec<String>,
    pub threats: Vec<String>,
    pub category_run: i64, // the saved product search used (0 = none)
}

/// (run id, brand id, name, overall score, analysed at)
pub type BrandRunRow = (i64, i64, String, Option<i64>, String);

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Channel {
    pub platform: String,
    pub offers: usize,
    pub sellers: usize,
    pub min: i64,
    pub median: i64,
    pub max: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct MixItem {
    pub key: String, // product | price | place | promotion
    pub score: Option<f64>,
    pub notes: Vec<String>,
}

// ---------------- storage ----------------

impl Db {
    pub fn brands(&self) -> Res<Vec<Brand>> {
        let c = self.c();
        let mut st = c.prepare("SELECT id,name,own,website,instagram,category,note,report,analyzed_at FROM brands ORDER BY own DESC, id").map_err(err)?;
        let rows = st
            .query_map([], |r| {
                let rep: String = r.get(7)?;
                Ok(Brand {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    own: r.get::<_, i64>(2)? != 0,
                    website: r.get(3)?,
                    instagram: r.get(4)?,
                    category: r.get(5)?,
                    note: r.get(6)?,
                    report: serde_json::from_str(&rep).ok(),
                    analyzed_at: r.get(8)?,
                })
            })
            .map_err(err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(err)
    }

    pub fn brand_save(&self, b: &Brand) -> Res<i64> {
        let name = b.name.trim();
        if name.chars().count() < 2 {
            return Err("نام برند را بنویسید.".into());
        }
        let site = b.website.trim();
        if !site.is_empty() && !site.contains('.') {
            return Err("آدرس سایت معتبر نیست؛ مثل example.ir".into());
        }
        let ig = b.instagram.trim().trim_start_matches('@').trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string();
        let c = self.c();
        if b.own {
            c.execute("UPDATE brands SET own=0", []).map_err(err)?; // one «برند ما»
        }
        if b.id == 0 {
            c.execute(
                "INSERT INTO brands(name,own,website,instagram,category,note) VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params![name, b.own as i64, site, ig, b.category.trim(), b.note.trim()],
            )
            .map_err(err)?;
            Ok(c.last_insert_rowid())
        } else {
            c.execute(
                "UPDATE brands SET name=?1,own=?2,website=?3,instagram=?4,category=?5,note=?6 WHERE id=?7",
                rusqlite::params![name, b.own as i64, site, ig, b.category.trim(), b.note.trim(), b.id],
            )
            .map_err(err)?;
            Ok(b.id)
        }
    }

    pub fn brand_delete(&self, id: i64) -> Res<()> {
        self.c().execute("DELETE FROM brands WHERE id=?1", [id]).map_err(err)?;
        Ok(())
    }

    pub fn brand_report_save(&self, id: i64, r: &BrandReport) -> Res<()> {
        let json = serde_json::to_string(r).map_err(err)?;
        let c = self.c();
        c.execute("UPDATE brands SET report=?1, analyzed_at=datetime('now','localtime') WHERE id=?2", rusqlite::params![json, id]).map_err(err)?;
        c.execute(
            "INSERT INTO brand_runs(brand_id,name,overall,report) SELECT id,name,?1,?2 FROM brands WHERE id=?3",
            rusqlite::params![r.overall.round() as i64, json, id],
        )
        .map_err(err)?;
        Ok(())
    }

    /// Every brand analysis, newest first: (run id, brand id, name, overall score, at).
    pub fn brand_runs(&self, limit: i64) -> Res<Vec<BrandRunRow>> {
        let c = self.c();
        let mut st = c.prepare("SELECT id,brand_id,name,overall,created_at FROM brand_runs ORDER BY id DESC LIMIT ?1").map_err(err)?;
        let rows = st.query_map([limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).map_err(err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)
    }

    pub fn brand_run_get(&self, id: i64) -> Res<BrandReport> {
        let raw: String = self.c().query_row("SELECT report FROM brand_runs WHERE id=?1", [id], |r| r.get(0)).map_err(err)?;
        serde_json::from_str(&raw).map_err(err)
    }

    /// The latest saved product searches (for market presence), newest first.
    pub fn market_recent(&self, limit: i64) -> Res<Vec<Report>> {
        let ids: Vec<i64> = self.market_runs(limit)?.into_iter().map(|r| r.0).collect();
        Ok(ids.into_iter().filter_map(|id| self.market_get(id).ok()).collect())
    }
}

// ---------------- collecting ----------------

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36")
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(6))
        .build()
        .unwrap_or_default()
}

/// Reads the brand's homepage: reachable, https, speed, title/description, structured data, contact.
pub async fn audit_site(url: &str) -> Site {
    let url = url.trim();
    if url.is_empty() {
        return Site { error: "سایت ثبت نشده".into(), ..Default::default() };
    }
    let full = if url.starts_with("http") { url.to_string() } else { format!("https://{url}") };
    let t = Instant::now();
    let r = match http().get(&full).send().await {
        Ok(r) => r,
        Err(e) => return Site { error: format!("سایت باز نشد: {}", e.without_url()), ..Default::default() },
    };
    let https = r.url().scheme() == "https";
    let ok = r.status().is_success();
    let body = r.text().await.unwrap_or_default();
    let ms = t.elapsed().as_millis() as u64;
    let low = body.to_lowercase();
    let title = crate::logic::html_unescape(
        low.find("<title").and_then(|i| body[i..].find('>').map(|j| i + j + 1)).and_then(|s| body[s..].find("</").map(|e| body[s..s + e].trim().to_string())).unwrap_or_default().as_str(),
    );
    let mut c = crate::logic::Contacts::default();
    crate::logic::extract_contacts(&body, &mut c);
    let socials: Vec<String> = c.pairs().into_iter().filter(|(k, _)| !matches!(*k, "mobile" | "landline" | "email")).map(|(k, _)| k.to_string()).collect();
    Site {
        ok,
        https,
        ms,
        kb: (body.len() / 1024) as u64,
        title: title.chars().take(140).collect(),
        description: crate::logic::meta_content(&body, "description").unwrap_or_default().chars().take(300).collect(),
        h1: low.contains("<h1"),
        schema: low.contains("application/ld+json"),
        contact: !(c.mobiles.is_empty() && c.landlines.is_empty() && c.emails.is_empty()),
        socials,
        error: if ok { String::new() } else { "سایت پاسخ درست نداد".into() },
    }
}

/// «1,234 Followers, 56 Following, 78 Posts» / «1.2K Followers» from the public profile page.
pub fn parse_counts(desc: &str) -> (Option<i64>, Option<i64>) {
    let d = crate::logic::ascii_digits(desc).replace(['٬', ','], "");
    let grab = |label: &[&str]| -> Option<i64> {
        let toks: Vec<&str> = d.split_whitespace().collect();
        toks.windows(2).find_map(|w| {
            if !label.iter().any(|l| w[1].to_lowercase().starts_with(l)) {
                return None;
            }
            let t = w[0].to_lowercase();
            let (n, mul) = if let Some(x) = t.strip_suffix('k') { (x, 1_000.0) } else if let Some(x) = t.strip_suffix('m') { (x, 1_000_000.0) } else { (t.as_str(), 1.0) };
            n.parse::<f64>().ok().map(|v| (v * mul).round() as i64)
        })
    };
    (grab(&["followers", "دنبال"]), grab(&["posts", "پست"]))
}

pub async fn read_instagram(handle: &str) -> Social {
    let h = handle.trim().trim_start_matches('@');
    let mut s = Social { handle: h.to_string(), ..Default::default() };
    if h.is_empty() {
        return s;
    }
    if let Ok(r) = http().get(format!("https://www.instagram.com/{h}/")).send().await {
        let body = r.text().await.unwrap_or_default();
        if let Some(desc) = crate::logic::meta_content(&body, "og:description").or_else(|| crate::logic::meta_content(&body, "description")) {
            let (f, p) = parse_counts(&desc);
            s.followers = f;
            s.posts = p;
        }
    }
    s
}

/// This brand's place in the saved product searches (the latest one per product).
pub fn market_of(name: &str, category: &str, runs: &[Report]) -> Market {
    let key = words(name).join(" ");
    let cat = words(category).join(" ");
    let mut seen = std::collections::HashSet::new();
    let runs: Vec<&Report> = runs
        .iter()
        .filter(|r| r.brand.is_empty() && (cat.is_empty() || words(&r.query).join(" ").contains(&cat) || cat.contains(&words(&r.query).join(" "))))
        .filter(|r| seen.insert(words(&r.query).join(" ")))
        .collect();
    let mut m = Market { searches: runs.len(), ..Default::default() };
    let (mut total, mut mine, mut idx, mut idx_n) = (0usize, 0usize, 0.0f64, 0.0f64);
    let mut prices = vec![];
    let (mut rsum, mut rn) = (0.0f64, 0.0f64);
    for r in &runs {
        total += r.offers.len();
        let basis = |o: &crate::market::Offer| if r.stats.basis == "per_kg" { o.unit_price.unwrap_or(0) } else { o.price };
        let ours: Vec<_> = r.offers.iter().filter(|o| words(&o.brand).join(" ") == key || words(&o.title).join(" ").contains(&key)).collect();
        mine += ours.len();
        for o in &ours {
            if !m.platforms.contains(&o.platform) {
                m.platforms.push(o.platform.clone());
            }
            if let Some(rt) = o.rating {
                let w = 1.0 + (o.reviews as f64 + 1.0).log10();
                rsum += rt * w;
                rn += w;
            }
            m.reviews += o.reviews;
            if basis(o) > 0 && r.stats.median > 0 {
                idx += basis(o) as f64 * 100.0 / r.stats.median as f64;
                idx_n += 1.0;
                prices.push(basis(o));
            }
        }
        m.market_median = r.stats.median;
    }
    m.offers = mine;
    m.share_pct = if total > 0 { (mine as f64 * 1000.0 / total as f64).round() / 10.0 } else { 0.0 };
    prices.sort_unstable();
    m.median = prices.get(prices.len() / 2).copied().unwrap_or(0);
    m.price_index = if idx_n > 0.0 { (idx / idx_n).round() } else { 0.0 };
    m.rating = (rn > 0.0).then(|| (rsum / rn * 10.0).round() / 10.0);
    m.tier = match m.price_index as i64 {
        0 => String::new(),
        i if i < 85 => "اقتصادی".into(),
        i if i <= 105 => "رقابتی".into(),
        i if i <= 130 => "میان‌رده رو به بالا".into(),
        _ => "پریمیوم".into(),
    };
    m
}

// ---------------- scoring ----------------

pub fn score(m: Market, site: Site, insta: Social, has_site: bool) -> BrandReport {
    let presence = (m.searches > 0).then(|| (m.share_pct * 3.0 + m.platforms.len() as f64 * 12.0).min(100.0).round());
    let quality = m.rating.map(|r| ((r / 5.0) * 85.0 + ((m.reviews as f64 + 1.0).log10() / 3.0).min(1.0) * 15.0).round());
    let digital = has_site.then(|| {
        let mut s = 0.0;
        if site.ok {
            s += 25.0;
        }
        if site.https {
            s += 15.0;
        }
        if !site.title.is_empty() {
            s += 10.0;
        }
        if !site.description.is_empty() {
            s += 10.0;
        }
        if site.schema {
            s += 10.0;
        }
        if site.h1 {
            s += 5.0;
        }
        if site.contact {
            s += 10.0;
        }
        s += (site.socials.len() as f64 * 4.0).min(10.0);
        if site.ok && site.ms < 1500 {
            s += 5.0;
        }
        s
    });
    let social = insta.followers.map(|f| (((f as f64 + 1.0).log10() / 6.0) * 90.0 + if insta.posts.unwrap_or(0) >= 50 { 10.0 } else { 0.0 }).min(100.0).round());
    let pillars: Vec<f64> = [presence, quality, digital, social].into_iter().flatten().collect();
    let overall = if pillars.is_empty() { 0.0 } else { (pillars.iter().sum::<f64>() / pillars.len() as f64).round() };

    let (mut strengths, mut weaknesses, mut actions) = (vec![], vec![], vec![]);
    match presence {
        Some(p) if p >= 60.0 => strengths.push(format!("حضور پررنگ در بازار: {}٪ از عرضه‌ها در {} پلتفرم.", m.share_pct, m.platforms.len())),
        Some(_) if m.offers == 0 => {
            weaknesses.push("در جستجوهای محصول این دسته، کالایی با این برند دیده نشد.".into());
            actions.push("محصولات را با نام برند در ترب، دیجی‌کالا و باسلام ثبت کنید؛ نام برند را در عنوان کالا بیاورید تا در جستجو دیده شود.".into());
        }
        Some(_) => {
            weaknesses.push(format!("سهم کم از عرضه‌ها ({}٪)؛ برند در نقطه تماس خرید کم دیده می‌شود.", m.share_pct));
            actions.push("در پلتفرم‌هایی که حضور ندارید فروشگاه باز کنید و تنوع بسته‌بندی (اندازه‌ها) را بیشتر کنید.".into());
        }
        None => actions.push("در «محصول و قیمت» دسته محصولتان را جستجو کنید تا سهم بازار و جایگاه قیمتی برند محاسبه شود.".into()),
    }
    match m.rating {
        Some(r) if r >= 4.3 => strengths.push(format!("رضایت بالای خریداران: میانگین {r} از ۵ ({} نظر).", m.reviews)),
        Some(r) if r < 3.8 => {
            weaknesses.push(format!("امتیاز خریداران پایین است ({r} از ۵)."));
            actions.push("نظرهای منفی را دسته‌بندی کنید (کیفیت، بسته‌بندی، ارسال) و ریشه پرتکرارترین را اول حل کنید؛ بعد از خرید از مشتری راضی بخواهید نظر بدهد.".into());
        }
        Some(_) => actions.push("برای رسیدن به امتیاز ۴٫۳+ پیگیری پس از فروش و پاسخ به نظرها را منظم کنید.".into()),
        None => {}
    }
    if m.price_index > 0.0 {
        let gap = m.price_index - 100.0;
        if gap > 25.0 && m.rating.unwrap_or(0.0) < 4.3 {
            weaknesses.push(format!("قیمت {}٪ بالاتر از میانه بازار، بدون امتیاز کیفیت بالاتر برای توجیه آن.", gap.round()));
            actions.push("یا ارزش پریمیوم را قابل دیدن کنید (گواهی، آزمایش، داستان برند، بسته‌بندی) یا قیمت را به محدوده رقابتی نزدیک کنید.".into());
        } else if gap < -15.0 {
            strengths.push(format!("قیمت رقابتی: {}٪ زیر میانه بازار.", (-gap).round()));
            if m.rating.unwrap_or(0.0) >= 4.2 {
                actions.push("کیفیت خوب با قیمت پایین یعنی جای افزایش تدریجی قیمت دارید؛ ۵ تا ۱۰٪ افزایش را آزمایش کنید.".into());
            }
        }
    }
    if has_site {
        if !site.ok {
            weaknesses.push(format!("سایت برند در دسترس نبود ({}).", site.error));
            actions.push("در دسترس بودن سایت را پایش کنید؛ سایت از کار افتاده مستقیماً اعتماد خریدار را کم می‌کند.".into());
        } else {
            if !site.https {
                weaknesses.push("سایت بدون HTTPS است.".into());
                actions.push("گواهی SSL نصب کنید؛ مرورگرها سایت بدون HTTPS را «ناامن» نشان می‌دهند.".into());
            }
            if site.description.is_empty() || !site.schema {
                actions.push("برای دیده شدن در گوگل: توضیحات متا و داده ساخت‌یافته (Organization / Product) به صفحه اول اضافه کنید.".into());
            }
            if !site.contact {
                actions.push("شماره تماس و ایمیل را در صفحه اول یا فوتر سایت بنویسید.".into());
            }
            if site.ms > 3000 {
                weaknesses.push(format!("سایت کند باز می‌شود ({} ثانیه).", (site.ms as f64 / 100.0).round() / 10.0));
            }
            if digital.unwrap_or(0.0) >= 75.0 {
                strengths.push("حضور دیجیتال کامل و فنی سالم سایت.".into());
            }
        }
    } else {
        actions.push("سایت رسمی برند را ثبت کنید تا حضور دیجیتال سنجیده شود؛ برند بدون سایت در جستجوی گوگل به فروشگاه‌ها واگذار می‌شود.".into());
    }
    match insta.followers {
        Some(f) if f >= 50_000 => strengths.push(format!("مخاطب قوی در اینستاگرام: {f} دنبال‌کننده.")),
        Some(f) if f < 3_000 => actions.push("محتوای اینستاگرام را با برنامه منظم (آموزش مصرف، پشت صحنه تولید، نظر مشتریان) تقویت کنید.".into()),
        None if !insta.handle.is_empty() => actions.push("اطلاعات عمومی پیج اینستاگرام خوانده نشد؛ مطمئن شوید پیج عمومی و کسب‌وکاری است.".into()),
        _ => {}
    }
    BrandReport { presence, quality, digital, social, overall, market: m, site, insta, strengths, weaknesses, actions, ..Default::default() }
}

/// Domains that are not a brand's own website.
const NOT_OWN_SITE: &[&str] = &[
    "torob.", "digikala.", "basalam.", "divar.", "sheypoor.", "emalls.", "instagram.", "t.me", "telegram.", "aparat.", "youtube.", "wikipedia.",
    "google.", "bing.", "facebook.", "twitter.", "x.com", "linkedin.", "snapp", "timcheh.", "digistyle.", "okala.", "esam.", "neshan.", "balad.",
    "eitaa.", "rubika.", "virgool.", "namnak.", "beytoote.", "tebyan.", "blogfa.", "pinterest.", "tiktok.",
];

fn host(url: &str) -> String {
    url.split("//").nth(1).unwrap_or(url).split('/').next().unwrap_or("").trim_start_matches("www.").to_lowercase()
}

/// The brand's own website and Instagram page, found by searching its name (Bing in the app's
/// own browser; a «robot» check is handed to the user in a visible window).
pub async fn discover(s: &crate::scanner::Settings, profile: &std::path::Path, b: &Brand, token: &tokio_util::sync::CancellationToken, emit: crate::scanner::Emit<'_>) -> (String, String) {
    let (mut site, mut insta) = (b.website.trim().to_string(), b.instagram.trim().trim_start_matches('@').to_string());
    // offline tests point the market at a local mock and have no web search to ask
    let offline = crate::logic::test_seam("HAMISTA_MARKET_BASE").is_some() && crate::logic::test_seam("HAMISTA_SEARCH_BASE").is_none();
    if (!site.is_empty() && !insta.is_empty()) || offline {
        return (site, insta);
    }
    let Ok(eng) = crate::scanner::launch(s, profile, s.headless, 0).await else { return (site, insta) };
    let name = b.name.trim();
    let extra = if b.category.trim().is_empty() { String::new() } else { format!(" {}", b.category.trim()) };
    if site.is_empty() {
        crate::scanner::status(emit, format!("پیدا کردن سایت «{name}»…"));
        if let Ok(hits) = crate::websearch::search_attended(&eng.list, s, profile, &format!("{name}{extra} سایت رسمی"), 1, token, emit).await {
            let nw = words(name);
            site = hits
                .iter()
                .filter(|h| h.url.starts_with("http") && !NOT_OWN_SITE.iter().any(|d| host(&h.url).contains(d)))
                .find(|h| {
                    let t = words(&format!("{} {}", h.title, h.snippet));
                    nw.iter().all(|w| t.iter().any(|x| crate::market::tok_eq(x, w)))
                })
                .map(|h| format!("https://{}", host(&h.url)))
                .unwrap_or_default();
        }
    }
    if insta.is_empty() && !token.is_cancelled() {
        crate::scanner::status(emit, format!("پیدا کردن اینستاگرام «{name}»…"));
        if let Ok(hits) = crate::websearch::search_attended(&eng.list, s, profile, &format!("{name}{extra} instagram"), 1, token, emit).await {
            insta = hits
                .iter()
                .filter(|h| host(&h.url).contains("instagram.com"))
                .filter_map(|h| h.url.split("instagram.com/").nth(1).map(|p| p.split(['/', '?']).next().unwrap_or("").to_string()))
                .find(|hd| !hd.is_empty() && !["p", "reel", "reels", "explore", "stories", "tv", "accounts"].contains(&hd.as_str()))
                .unwrap_or_default();
        }
    }
    crate::scanner::shut(Some(eng)).await;
    (site, insta)
}

fn pct_of(sorted: &[i64], p: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

/// Adds the live market reading (channels, products, sellers, competitors), the 4P marketing
/// mix, opportunities and threats to a scored report.
pub fn enrich(r: &mut BrandReport, name: &str, mine: Option<&Report>, category: Option<&Report>) {
    let key = crate::market::brand_key(name);
    let nw = words(name);
    let is_mine = |o: &crate::market::Offer| {
        (!o.brand.is_empty() && crate::market::brand_key(&o.brand) == key) || {
            let t = words(&o.title);
            nw.iter().all(|w| t.iter().any(|x| crate::market::tok_eq(x, w)))
        }
    };
    let offers: Vec<&crate::market::Offer> = mine.map(|m| m.offers.iter().filter(|o| is_mine(o)).collect()).unwrap_or_default();
    // channels
    let mut by: std::collections::BTreeMap<String, Vec<&crate::market::Offer>> = Default::default();
    for o in &offers {
        by.entry(o.platform.clone()).or_default().push(o);
    }
    r.channels = by
        .into_iter()
        .map(|(platform, os)| {
            let mut ps: Vec<i64> = os.iter().map(|o| o.price).collect();
            ps.sort_unstable();
            let mut sellers: Vec<&str> = os.iter().map(|o| o.seller.as_str()).filter(|x| !x.is_empty()).collect();
            sellers.sort_unstable();
            sellers.dedup();
            Channel { platform, offers: os.len(), sellers: sellers.len(), min: ps[0], median: pct_of(&ps, 0.5), max: *ps.last().unwrap_or(&0) }
        })
        .collect();
    r.channels.sort_by_key(|c| std::cmp::Reverse(c.offers));
    let mut prices: Vec<i64> = offers.iter().map(|o| o.price).collect();
    prices.sort_unstable();
    r.price_min = prices.first().copied().unwrap_or(0);
    r.price_max = prices.last().copied().unwrap_or(0);
    let mut sellers: std::collections::HashMap<String, usize> = Default::default();
    offers.iter().filter(|o| !o.seller.is_empty()).for_each(|o| *sellers.entry(o.seller.clone()).or_default() += 1);
    r.sellers = sellers.len();
    let mut top: Vec<(String, usize)> = sellers.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top.truncate(6);
    r.top_sellers = top;
    let mut prod: Vec<crate::market::Offer> = offers.iter().map(|o| (*o).clone()).collect();
    prod.sort_by(|a, b| b.reviews.cmp(&a.reviews).then_with(|| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal)));
    prod.dedup_by(|a, b| words(&a.title) == words(&b.title));
    prod.truncate(12);
    r.products = prod;
    r.lines = mine.map(|m| m.subcategories.iter().take(8).cloned().collect()).unwrap_or_default();
    if let Some(c) = category {
        r.category_run = c.id;
        r.competitors = c.brands.iter().filter(|b| b.name != "سایر / بدون برند" && crate::market::brand_key(&b.name) != key).take(8).cloned().collect();
    }

    // ---- 4P marketing mix ----
    let product_score = r.quality.map(|q| (q * 0.7 + (r.lines.len().min(6) as f64 / 6.0) * 30.0).round());
    let mut product = vec![];
    if !r.lines.is_empty() {
        product.push(format!("خطوط محصول دیده‌شده: {}.", r.lines.iter().take(5).cloned().collect::<Vec<_>>().join("، ")));
    }
    product.push(match r.market.rating {
        Some(x) => format!("کیفیت درک‌شده از نگاه خریدار: {x} از ۵ ({} نظر).", r.market.reviews),
        None => "امتیاز خریداران برای این برند در دسترس نبود.".into(),
    });
    let spread = if r.price_min > 0 { r.price_max as f64 / r.price_min as f64 } else { 0.0 };
    let price_score = (r.market.price_index > 0.0).then(|| {
        let fit = 100.0 - (r.market.price_index - 100.0).abs().min(60.0);
        let consistency = if spread > 0.0 && spread < 2.5 { 15.0 } else { 0.0 };
        (fit * 0.85 + consistency).min(100.0).round()
    });
    let mut price = vec![];
    if r.market.price_index > 0.0 {
        price.push(format!("شاخص قیمت {} (۱۰۰ = میانه بازار) — جایگاه «{}».", r.market.price_index, if r.market.tier.is_empty() { "—" } else { &r.market.tier }));
    }
    if r.price_min > 0 {
        price.push(format!("بازه قیمت فروش برند: {} تا {} تومان.", r.price_min, r.price_max));
    }
    let place_score = (!r.channels.is_empty() || r.market.searches > 0).then(|| ((r.channels.len() as f64 * 18.0) + (r.sellers.min(20) as f64 * 2.0) + r.market.share_pct.min(20.0)).min(100.0).round());
    let mut place = vec![];
    if r.channels.is_empty() {
        place.push("در فروشگاه‌های اینترنتی بررسی‌شده کالایی با این برند پیدا نشد.".into());
    } else {
        place.push(format!("{} کانال فروش، {} فروشنده مختلف.", r.channels.len(), r.sellers));
    }
    let promo: Vec<f64> = [r.digital, r.social].into_iter().flatten().collect();
    let promo_score = (!promo.is_empty()).then(|| (promo.iter().sum::<f64>() / promo.len() as f64).round());
    let mut promotion = vec![];
    promotion.push(if r.site.ok { "سایت رسمی فعال است.".to_string() } else { "سایت رسمی فعالی پیدا نشد.".to_string() });
    if let Some(f) = r.insta.followers {
        promotion.push(format!("اینستاگرام @{}: {} دنبال‌کننده.", r.insta.handle, f));
    }
    r.mix = vec![
        MixItem { key: "product".into(), score: product_score, notes: product },
        MixItem { key: "price".into(), score: price_score, notes: price },
        MixItem { key: "place".into(), score: place_score, notes: place },
        MixItem { key: "promotion".into(), score: promo_score, notes: promotion },
    ];

    // ---- opportunities & threats ----
    if let Some(c) = category {
        for ch in c.channels.iter().filter(|ch| ch.pct >= 15.0) {
            let present = r.channels.iter().any(|x| crate::market::platform(&x.platform).map_or(x.platform.as_str(), |p| p.name) == ch.name);
            if !present {
                r.opportunities.push(format!("«{}» {}٪ از عرضه این بازار را دارد و برند شما آنجا نیست.", ch.name, ch.pct));
            }
        }
        if let (Some(rating), true) = (r.market.rating, r.market.price_index > 0.0) {
            if rating >= 4.3 && r.market.price_index < 95.0 {
                r.opportunities.push("کیفیت درک‌شده بالاتر از قیمت است؛ جای افزایش تدریجی قیمت هست.".into());
            }
        }
        if let Some(lead) = c.brands.iter().find(|b| b.name != "سایر / بدون برند" && crate::market::brand_key(&b.name) != key) {
            if lead.pct > r.market.share_pct * 2.0 && lead.pct >= 10.0 {
                r.threats.push(format!("«{}» با {}٪ از عرضه‌ها رهبر این بازار است.", lead.name, lead.pct));
            }
        }
    }
    if spread >= 2.5 {
        r.threats.push("اختلاف زیاد قیمت برند بین فروشندگان (بیش از ۲٫۵ برابر): نشانه تضاد کانال یا کالای تقلبی/قدیمی؛ قیمت‌گذاری کانال‌ها را هماهنگ کنید.".into());
    }
    if r.market.reviews < 20 && !offers.is_empty() {
        r.threats.push("تعداد نظرهای خریداران کم است؛ اعتماد خریدار تازه سخت‌تر جلب می‌شود.".into());
    }
    if r.channels.len() == 1 {
        r.threats.push("وابستگی به یک کانال فروش: تغییر سیاست همان پلتفرم کل فروش را تحت تأثیر می‌گذارد.".into());
    }
}

/// Analyses one brand: finds its site and Instagram when missing, reads the live market (its
/// category and its own products), audits the site and page, then scores it.
pub async fn analyze(
    db: &Db,
    s: &crate::scanner::Settings,
    profile: &std::path::Path,
    b: &Brand,
    token: &tokio_util::sync::CancellationToken,
    emit: crate::scanner::Emit<'_>,
) -> Res<BrandReport> {
    let (site_url, handle) = discover(s, profile, b, token, emit).await;
    let found_site = b.website.trim().is_empty() && !site_url.is_empty();
    let found_instagram = b.instagram.trim().is_empty() && !handle.is_empty();
    if found_site || found_instagram {
        let mut nb = b.clone();
        nb.website = site_url.clone();
        nb.instagram = handle.clone();
        let _ = db.brand_save(&nb);
    }
    // live market: the category as a whole (share, competitors) and the brand's own products
    let base = crate::market::MarketReq { platforms: vec!["torob".into(), "digikala".into(), "basalam".into()], limit: 60, ..Default::default() };
    let category = if b.category.trim().is_empty() || token.is_cancelled() {
        None
    } else {
        crate::market::search(db, s, profile, &crate::market::MarketReq { query: b.category.trim().into(), ..base.clone() }, token, emit).await.ok()
    };
    let mine = if token.is_cancelled() {
        None
    } else if b.category.trim().is_empty() {
        crate::market::search(db, s, profile, &crate::market::MarketReq { query: b.name.trim().into(), exact: false, ..base.clone() }, token, emit).await.ok()
    } else {
        crate::market::search(db, s, profile, &crate::market::MarketReq { query: b.category.trim().into(), brand: b.name.trim().into(), ..base.clone() }, token, emit).await.ok()
    };
    crate::scanner::status(emit, "بررسی سایت و صفحه اینستاگرام…");
    let runs = db.market_recent(60)?;
    let m = market_of(&b.name, &b.category, &runs);
    let site = if site_url.is_empty() { Site::default() } else { audit_site(&site_url).await };
    let insta = read_instagram(&handle).await;
    let mut r = score(m, site, insta, !site_url.is_empty());
    r.found_site = found_site;
    r.found_instagram = found_instagram;
    enrich(&mut r, &b.name, mine.as_ref(), category.as_ref());
    db.brand_report_save(b.id, &r)?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::{analyze as market, Offer};

    #[test]
    fn marketing_mix() {
        let o = |t: &str, p: i64, plat: &str, seller: &str| Offer { title: t.into(), price: p, platform: plat.into(), seller: seller.into(), rating: Some(4.5), reviews: 50, ..Default::default() };
        let cat = market("عسل", "", vec![o("عسل خوانسار گون", 300_000, "torob", "الف"), o("عسل خوانسار کنار", 320_000, "digikala", "ب"), o("عسل سبلان", 200_000, "basalam", "ج"), o("عسل سبلان آویشن", 210_000, "basalam", "د"), o("عسل سبلان گون", 220_000, "basalam", "ه")], 25.0);
        let mine = market("عسل", "خوانسار", cat.offers.clone(), 25.0);
        let mut r = score(market_of("خوانسار", "عسل", std::slice::from_ref(&cat)), Site::default(), Social::default(), false);
        enrich(&mut r, "خوانسار", Some(&mine), Some(&cat));
        assert_eq!(r.channels.len(), 2);
        assert_eq!((r.price_min, r.price_max, r.sellers), (300_000, 320_000, 2));
        assert_eq!(r.mix.len(), 4);
        assert!(r.competitors.iter().any(|c| c.name == "سبلان"));
        assert!(r.opportunities.iter().any(|x| x.contains("باسلام")), "{:?}", r.opportunities);
    }

    #[test]
    fn counts() {
        assert_eq!(parse_counts("12.5K Followers, 300 Following, 1,204 Posts - See Instagram photos"), (Some(12_500), Some(1204)));
        assert_eq!(parse_counts("۲٬۳۰۰ دنبال‌کننده، ۱۲۰ پست"), (Some(2300), Some(120)));
        assert_eq!(parse_counts("nothing"), (None, None));
    }

    #[test]
    fn scoring() {
        let o = |t: &str, p: i64, r: f64, plat: &str| Offer { platform: plat.into(), title: t.into(), price: p, rating: Some(r), reviews: 100, ..Default::default() };
        let rep = market("عسل", "", vec![o("عسل خوانسار", 300_000, 4.6, "torob"), o("عسل خوانسار گون", 320_000, 4.5, "digikala"), o("عسل سبلان", 200_000, 4.0, "torob"), o("عسل طبیعت", 210_000, 3.9, "basalam")], 25.0);
        let m = market_of("خوانسار", "عسل", &[rep]);
        assert_eq!((m.offers, m.platforms.len()), (2, 2));
        assert!(m.price_index > 100.0 && m.rating.unwrap() > 4.4);
        let site = Site { ok: true, https: true, ms: 800, title: "x".into(), description: "y".into(), schema: true, h1: true, contact: true, socials: vec!["instagram".into()], ..Default::default() };
        let r = score(m, site, Social { handle: "x".into(), followers: Some(80_000), posts: Some(400) }, true);
        assert!(r.presence.is_some() && r.quality.unwrap() > 80.0 && r.digital.unwrap() > 90.0 && r.social.unwrap() > 70.0);
        assert!(r.overall > 70.0 && !r.strengths.is_empty());
        // no data at all: no crash, actions tell what to do
        let none = score(Market::default(), Site::default(), Social::default(), false);
        assert_eq!(none.overall, 0.0);
        assert!(!none.actions.is_empty());
    }
}
