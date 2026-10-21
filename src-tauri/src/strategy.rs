//! «استراتژی و بیزینس پلن» of a brand: business model canvas, business model, positioning and
//! branding, advertising mix with budget, a 12-month growth roadmap, unit economics and a 12-month
//! projection, risks and KPIs — built from the brand's live analysis (market, store, site,
//! Instagram, competitors) plus a few answers from the owner (sector, model, stage, goal, budget…).
//!
//! Everything is deterministic and offline: a sector playbook (Iranian market channels, seasons,
//! licences, partners) is combined with the measured facts. Each number says what it rests on, and
//! the assumptions are the owner's own inputs, so the plan can be re-run as they change. A ready
//! prompt carries the whole plan to an AI assistant for longer prose.
use crate::brand::{Brand, BrandReport};
use serde::{Deserialize, Serialize};

// ---------------- inputs ----------------

/// The owner's answers (all optional; empty = inferred from the analysis).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Inputs {
    /// Sector key (`SECTORS`), "" = from the brand's category.
    pub sector: String,
    /// b2c_online · d2c · b2b · manufacturer · retail · service
    pub model: String,
    /// idea · startup · growth · established
    pub stage: String,
    /// sales · awareness · launch · wholesale · export
    pub goal: String,
    pub audience: String,
    /// What makes the product different, in the owner's words.
    pub usp: String,
    pub region: String,
    /// Monthly marketing budget (Toman).
    pub budget: i64,
    /// Selling price / unit cost (Toman); 0 = from the market / the sector's usual margin.
    pub price: i64,
    pub unit_cost: i64,
    /// Fixed monthly costs (rent, salaries, tools …) in Toman.
    pub fixed_cost: i64,
    /// Units the business can deliver per month (0 = not limited).
    pub capacity: i64,
    /// Sales-channel commission, % of the price (None = the sector's usual rate).
    pub channel_fee_pct: Option<f64>,
    /// VAT charged on the channel's commission, % (None = 10).
    pub vat_pct: Option<f64>,
    /// Packaging + delivery per unit, Toman (None = the sector's usual share of the price).
    pub pack_cost: Option<i64>,
    /// Share of orders returned, % (None = the sector's usual rate).
    pub return_pct: Option<f64>,
    // ---- report cover (white label for consultants and agencies) ----
    /// Who prepared the plan, and for whom.
    pub advisor: String,
    pub client: String,
    /// The advisor's / brand's logo as a small data: URL (the page shrinks it before saving).
    pub logo: String,
}

// ---------------- output ----------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Block {
    pub key: String,
    pub title: String,
    pub items: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AdChannel {
    pub name: String,
    pub share: i64, // % of the budget
    pub budget: i64,
    pub why: String,
    pub kpi: String,
    pub actions: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Phase {
    pub title: String,
    pub window: String,
    pub goal: String,
    pub tasks: Vec<String>,
    pub kpis: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Month {
    pub month: i64,
    pub units: i64,
    pub revenue: i64,
    pub gross: i64,
    pub marketing: i64,
    pub fixed: i64,
    pub profit: i64,
    pub cumulative: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Finance {
    pub price: i64,
    pub unit_cost: i64,
    pub channel_fee: i64,
    /// Per unit: VAT on the commission, packaging + delivery, and the expected cost of returns.
    pub vat: i64,
    pub pack: i64,
    pub returns: i64,
    pub fee_pct: f64,
    pub vat_pct: f64,
    pub return_pct: f64,
    pub unit_margin: i64,
    pub margin_pct: f64,
    pub fixed: i64,
    pub budget: i64,
    pub break_even_units: i64,
    pub months: Vec<Month>,
    /// Where each number comes from (input, market median, sector default).
    pub basis: Vec<String>,
}

/// A colour of the suggested palette.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Swatch {
    pub role: String,
    pub hex: String,
    pub usage: String,
}

/// One axis of the tone-of-voice matrix: `value` 0 = the left pole, 100 = the right pole.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ToneAxis {
    pub left: String,
    pub right: String,
    pub value: i64,
}

/// The visual identity suggested for the brand: palette, fonts and tone of voice.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Brandbook {
    pub archetype: String,
    pub palette: Vec<Swatch>,
    pub heading_font: String,
    pub body_font: String,
    pub font_note: String,
    pub tone: Vec<ToneAxis>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Strategy {
    pub brand: String,
    pub sector: String,
    pub model: String,
    pub stage: String,
    pub goal: String,
    pub summary: Vec<String>,
    /// Positioning statement, value proposition, archetype, tone, tagline ideas, differentiators.
    pub positioning: Vec<Block>,
    /// The nine blocks of the Business Model Canvas.
    pub canvas: Vec<Block>,
    /// Business model: type, revenue streams, pricing strategy, defensibility.
    pub business_model: Vec<Block>,
    /// Business plan sections: market, competition, operations, organisation.
    pub plan: Vec<Block>,
    /// Branding: identity, packaging, digital presence, trust, customer experience.
    pub branding: Vec<Block>,
    pub ads: Vec<AdChannel>,
    pub content: Vec<String>,
    pub seasons: Vec<String>,
    pub roadmap: Vec<Phase>,
    pub finance: Finance,
    pub risks: Vec<(String, String)>,
    pub kpis: Vec<(String, String)>,
    /// The measured facts this plan rests on (from the brand analysis).
    pub evidence: Vec<String>,
    pub inputs: Inputs,
    pub brandbook: Brandbook,
    /// The plan as a prompt, to deepen any part with an AI assistant.
    pub prompt: String,
}

// ---------------- sector playbooks ----------------

struct Sector {
    key: &'static str,
    name: &'static str,
    keywords: &'static [&'static str],
    /// Usual gross margin when the owner gives no cost.
    margin: f64,
    /// Usual commission of the sector's sales channel (%), share of orders returned (%), and packaging +
    /// delivery as a share of the price (%) in the Iranian market. Estimates; the owner can overwrite each.
    fee: f64,
    ret: f64,
    pack: f64,
    /// Suggested palette: main, complement, background, accent.
    palette: [&'static str; 4],
    /// Suggested Persian heading / body fonts.
    fonts: (&'static str, &'static str),
    /// Tone of voice, 0–100 on: رسمی→صمیمی, کلاسیک→مدرن, منطقی→احساسی, جدی→بازیگوش.
    tone: [i64; 4],
    segments: &'static [&'static str],
    channels: &'static [&'static str],
    partners: &'static [&'static str],
    resources: &'static [&'static str],
    licences: &'static [&'static str],
    seasons: &'static [&'static str],
    content: &'static [&'static str],
    /// (channel key, weight) — the base advertising mix.
    mix: &'static [(&'static str, i64)],
    archetype: (&'static str, &'static str),
}

const SECTORS: &[Sector] = &[
    Sector {
        key: "food",
        name: "مواد غذایی، کشاورزی و ارگانیک",
        keywords: &["عسل", "زعفران", "خشکبار", "چای", "برنج", "روغن", "ادویه", "خرما", "شیرینی", "لبنیات", "قهوه", "کشمش", "پسته", "گردو", "بادام", "مربا", "ترشی", "سبزی", "میوه", "غذا", "خوراکی", "ارگانیک", "شکلات", "نان", "آجیل"],
        margin: 0.35,
        fee: 9.0,
        ret: 2.0,
        pack: 6.0,
        palette: ["#8A4B0F","#E9A23B","#FFF6E5","#3F6B3A"],
        fonts: ("لاله‌زار","وزیرمتن"),
        tone: [75,35,70,45],
        segments: &["خانواده‌های سلامت‌محور شهری (۲۵ تا ۵۵ سال)", "خریداران هدیه مناسبتی (یلدا، نوروز، سازمانی)", "کافه‌ها، شیرینی‌فروشی‌ها و رستوران‌ها (خرید عمده)", "فروشگاه‌های ارگانیک و هایپرمارکت‌ها"],
        channels: &["غرفه باسلام و فروشگاه دیجی‌کالا", "سایت اختصاصی با اینماد", "اینستاگرام و فروش دایرکت", "سوپرمارکت‌های آنلاین (اسنپ‌مارکت، اکالا)", "پخش‌کننده‌های منطقه‌ای و بازار عمده", "هدایای سازمانی (B2B)"],
        partners: &["زنبوردار/کشاورز طرف قرارداد و تأمین پایدار مواد اولیه", "آزمایشگاه معتمد برای آزمون کیفیت (مثلاً آزمون ساکارز/HMF عسل)", "چاپخانه و تأمین‌کننده بسته‌بندی بهداشتی", "شرکت‌های پست و پیک سرد/گرم", "پخش‌کننده‌ها و عمده‌فروش‌ها"],
        resources: &["منبع تأمین قابل ردیابی و داستان مبدأ", "مجوزهای بهداشتی و استاندارد", "برگه آزمایش کیفیت", "انبار با دما و رطوبت کنترل‌شده"],
        licences: &["پروانه ساخت/بهره‌برداری و پروانه بهداشتی (سازمان غذا و دارو)", "کد IRC روی بسته", "نشان استاندارد ملی (در صورت اجباری بودن)", "اینماد برای سایت", "گواهی حلال/ارگانیک برای صادرات"],
        seasons: &["یلدا (آذر): هدیه و بسته‌های ویژه", "نوروز (اسفند): پرفروش‌ترین ماه؛ بسته هدیه و سفارش سازمانی", "رمضان: محصولات سحری و افطاری", "شروع پاییز و زمستان: تقویت ایمنی و محصولات گرم", "محرم: سفارش نذری عمده"],
        content: &["ویدیوی کوتاه از مبدأ (مزرعه/کندو) و فرایند برداشت", "تست کیفیت جلوی دوربین و نشان دادن برگه آزمایش", "دستور پخت و کاربرد روزمره محصول", "مقایسه اصل و تقلبی (آموزشی، بدون تخریب رقیب)", "نظر و ویدیوی مشتری واقعی"],
        mix: &[("instagram", 30), ("influencer", 20), ("marketplace", 20), ("search", 10), ("messenger", 8), ("retention", 12)],
        archetype: ("مراقب (Caregiver)", "خوراکی سالم یعنی مراقبت از خانواده؛ اعتماد، اصالت و گرما محور پیام است."),
    },
    Sector {
        key: "fashion",
        name: "پوشاک، کیف و کفش و اکسسوری",
        keywords: &["پوشاک", "لباس", "مانتو", "کیف", "کفش", "شال", "روسری", "تیشرت", "شلوار", "اکسسوری", "زیورآلات", "جواهر", "ساعت", "عینک", "کت", "پیراهن"],
        margin: 0.5,
        fee: 15.0,
        ret: 8.0,
        pack: 4.0,
        palette: ["#1C1C1E","#C9A27E","#F7F3EE","#B5495B"],
        fonts: ("استعداد","وزیرمتن"),
        tone: [55,80,75,55],
        segments: &["زنان ۱۸ تا ۴۰ ساله شهری، فعال در اینستاگرام", "خریداران هدیه", "بوتیک‌ها و فروشگاه‌های چندبرندی (عمده)"],
        channels: &["اینستاگرام (پیج فروشگاهی و استوری)", "سایت اختصاصی با اینماد و امکان تعویض", "دیجی‌استایل/دیجی‌کالا و باسلام", "بوتیک‌ها و پاپ‌آپ‌استور", "نمایشگاه‌ها و بازارچه‌ها"],
        partners: &["تولیدی/کارگاه دوخت و تأمین پارچه", "عکاس و مدل", "اینفلوئنسرهای سبک زندگی", "شرکت ارسال با امکان مرجوعی"],
        resources: &["طراحی و الگوی اختصاصی", "عکس کاتالوگ حرفه‌ای", "سایزبندی دقیق و جدول سایز", "موجودی چرخشی فصل‌به‌فصل"],
        licences: &["اینماد برای سایت", "ثبت علامت تجاری", "شناسه کالا (ایران‌کد) برای فروشگاه‌های بزرگ"],
        seasons: &["نوروز و عید فطر", "شروع مهر (بازگشت به مدرسه/دانشگاه)", "حراج پایان فصل (تابستان و زمستان)", "بلک‌فرایدی (آذر)", "روز مادر و ولنتاین"],
        content: &["استایل‌کردن یک محصول به سه شکل", "پشت صحنه تولید و انتخاب پارچه", "راهنمای سایز و جنس", "ویدیوی پرو مشتریان واقعی", "کالکشن فصلی با داستان"],
        mix: &[("instagram", 35), ("influencer", 25), ("marketplace", 10), ("search", 10), ("messenger", 5), ("retention", 15)],
        archetype: ("عاشق (Lover)", "پوشاک یعنی حس زیبایی و تعلق؛ زیبایی‌شناسی، جزئیات و تجربه خرید محور پیام است."),
    },
    Sector {
        key: "beauty",
        name: "آرایشی، بهداشتی و مراقبت پوست",
        keywords: &["آرایشی", "بهداشتی", "کرم", "شامپو", "عطر", "ادکلن", "صابون", "رژ", "پوست", "مو", "ماسک", "سرم", "لوسیون"],
        margin: 0.5,
        fee: 12.0,
        ret: 3.0,
        pack: 4.0,
        palette: ["#C2185B","#F3C6D3","#FFF7F9","#2E7D6B"],
        fonts: ("استعداد","شبنم"),
        tone: [65,70,85,55],
        segments: &["زنان ۲۰ تا ۴۵ ساله با دغدغه مراقبت پوست", "مردان مراقبت شخصی", "سالن‌های زیبایی و آرایشگاه‌ها (عمده)", "داروخانه‌ها"],
        channels: &["داروخانه‌های آنلاین و دیجی‌کالا", "سایت اختصاصی با اینماد", "اینستاگرام", "داروخانه‌ها و فروشگاه‌های آرایشی", "سالن‌های زیبایی"],
        partners: &["کارخانه تولید قراردادی (فاسون) دارای مجوز", "پزشک/متخصص پوست برای تأیید علمی", "اینفلوئنسرهای زیبایی", "پخش دارویی"],
        resources: &["فرمولاسیون و مستندات", "مجوز بهداشتی", "عکس و ویدیوی قبل/بعد (واقعی و مجاز)"],
        licences: &["پروانه ساخت و شماره ثبت آرایشی‌بهداشتی (سازمان غذا و دارو)", "برچسب اصالت/هولوگرام", "اینماد برای سایت"],
        seasons: &["نوروز", "تابستان: ضدآفتاب و مراقبت", "پاییز و زمستان: آبرسان و ترمیم", "روز زن و روز مادر", "بلک‌فرایدی"],
        content: &["روتین روزانه با محصول", "آموزش ترکیبات (مواد مؤثره) به زبان ساده", "پاسخ متخصص به سؤال‌های رایج", "نتیجه مصرف مشتریان (با رضایت)", "پشت صحنه آزمایش و کنترل کیفیت"],
        mix: &[("instagram", 30), ("influencer", 30), ("marketplace", 15), ("search", 10), ("messenger", 5), ("retention", 10)],
        archetype: ("فرزانه/مراقب (Sage + Caregiver)", "مراقبت پوست بر اعتماد علمی بنا می‌شود؛ شفافیت ترکیبات و توصیه متخصص محور پیام است."),
    },
    Sector {
        key: "home",
        name: "لوازم خانگی، دکوراسیون و مبلمان",
        keywords: &["مبل", "مبلمان", "دکور", "دکوراسیون", "فرش", "پرده", "ظروف", "آشپزخانه", "لوازم خانگی", "روشنایی", "لوستر", "میز", "صندلی", "تخت", "کمد", "سرویس"],
        margin: 0.35,
        fee: 10.0,
        ret: 4.0,
        pack: 8.0,
        palette: ["#2F4858","#D4A373","#F6F1EA","#5B8C85"],
        fonts: ("وزیرمتن (ضخیم)","شبنم"),
        tone: [55,50,60,35],
        segments: &["زوج‌های جوان در حال تهیه جهیزیه", "خانواده‌های در حال نوسازی خانه", "طراحان داخلی و معماران", "هتل‌ها، کافه‌ها و دفاتر (پروژه‌ای)"],
        channels: &["دیجی‌کالا، باسلام و ترب", "سایت با کاتالوگ و ارسال/نصب", "اینستاگرام و پینترست", "شوروم و فروشگاه حضوری", "همکاری با طراحان داخلی (B2B)"],
        partners: &["کارگاه تولید و تأمین متریال", "باربری و نصاب", "طراحان داخلی", "فروشگاه‌های اقساطی/اعتباری"],
        resources: &["شوروم یا کاتالوگ سه‌بعدی", "گارانتی و خدمات پس از فروش", "ناوگان ارسال"],
        licences: &["اینماد برای سایت", "گارانتی معتبر", "استاندارد (برای لوازم برقی)"],
        seasons: &["فصل جهیزیه (تابستان)", "نوروز و خانه‌تکانی", "جشنواره‌های فروش اقساطی", "بلک‌فرایدی"],
        content: &["قبل و بعد چیدمان", "راهنمای انتخاب اندازه و رنگ", "ویدیوی کیفیت ساخت و متریال", "پروژه‌های اجراشده", "نکات نگهداری"],
        mix: &[("instagram", 30), ("marketplace", 20), ("search", 20), ("influencer", 10), ("messenger", 5), ("retention", 15)],
        archetype: ("آفرینشگر (Creator)", "خانه بیان سلیقه است؛ طراحی، کیفیت ساخت و الهام‌بخشی محور پیام است."),
    },
    Sector {
        key: "electronics",
        name: "کالای دیجیتال و الکترونیک",
        keywords: &["موبایل", "گوشی", "لپ‌تاپ", "لپ تاپ", "هدفون", "شارژر", "کابل", "قاب", "دیجیتال", "کامپیوتر", "تبلت", "ساعت هوشمند", "اسپیکر", "دوربین"],
        margin: 0.15,
        fee: 6.0,
        ret: 3.0,
        pack: 2.0,
        palette: ["#0B3D91","#00B8D9","#F4F7FB","#FF6B35"],
        fonts: ("وزیرمتن (ضخیم)","وزیرمتن"),
        tone: [35,90,30,45],
        segments: &["جوانان ۱۶ تا ۳۵ ساله فناوری‌محور", "کسب‌وکارهای کوچک و دفاتر", "تعمیرکاران و فروشگاه‌های موبایل (عمده)"],
        channels: &["دیجی‌کالا و ترب (مقایسه قیمت)", "سایت با اینماد و گارانتی", "اینستاگرام و تلگرام", "فروشگاه حضوری در پاساژهای تخصصی"],
        partners: &["واردکننده/نمایندگی رسمی و گارانتی", "شرکت‌های ارسال سریع", "تولیدکنندگان محتوای تکنولوژی"],
        resources: &["گارانتی معتبر", "موجودی و قیمت به‌روز", "پشتیبانی فنی"],
        licences: &["اینماد", "کد رهگیری/رجیستری برای گوشی", "گارانتی رسمی"],
        seasons: &["بلک‌فرایدی", "شروع سال تحصیلی", "نوروز", "عرضه مدل‌های جدید"],
        content: &["بررسی و مقایسه صادقانه", "آموزش استفاده و ترفندها", "تست سرعت/کیفیت", "پاسخ به سؤال‌های خرید"],
        mix: &[("search", 30), ("marketplace", 25), ("instagram", 15), ("influencer", 15), ("messenger", 5), ("retention", 10)],
        archetype: ("فرزانه (Sage)", "خریدار کالای دیجیتال مقایسه می‌کند؛ اطلاعات دقیق، گارانتی و تخصص محور پیام است."),
    },
    Sector {
        key: "handicraft",
        name: "صنایع دستی، هنری و هدیه",
        keywords: &["صنایع دستی", "سفال", "گلیم", "قالی", "فیروزه", "مینا", "خاتم", "چرم", "هنری", "دست‌ساز", "دستساز", "هدیه", "کادو", "شمع", "تابلو", "سرامیک", "بافت"],
        margin: 0.45,
        fee: 12.0,
        ret: 3.0,
        pack: 7.0,
        palette: ["#7B3F00","#2A7F8E","#FBF4E8","#D1495B"],
        fonts: ("لاله‌زار","شبنم"),
        tone: [70,30,85,60],
        segments: &["خریداران هدیه و سوغات", "دوستداران هنر و دکور اصیل", "گردشگران و خریداران خارجی", "شرکت‌ها برای هدیه سازمانی"],
        channels: &["غرفه باسلام", "اینستاگرام و پینترست", "سایت چندزبانه با اینماد", "بازارچه‌ها، نمایشگاه‌ها و فروشگاه‌های سوغات", "بازارهای بین‌المللی (Etsy از طریق واسطه)"],
        partners: &["هنرمندان و کارگاه‌های محلی", "فروشگاه‌های سوغات و هتل‌ها", "آژانس‌های گردشگری", "شرکت‌های ارسال بین‌المللی"],
        resources: &["مهارت و اصالت هنرمند", "داستان هر اثر", "بسته‌بندی هدیه"],
        licences: &["شناسنامه/مهر اصالت صنایع دستی", "اینماد", "مجوز صادرات (برای فروش خارجی)"],
        seasons: &["نوروز و یلدا", "فصل گردشگری (بهار و تابستان)", "روز جهانی صنایع دستی (۲۰ خرداد)", "ولنتاین و روز مادر"],
        content: &["فرایند ساخت از صفر تا صد", "معرفی هنرمند و منطقه", "ایده هدیه برای مناسبت‌ها", "چیدمان اثر در خانه"],
        mix: &[("instagram", 35), ("influencer", 15), ("marketplace", 20), ("search", 10), ("messenger", 5), ("retention", 15)],
        archetype: ("آفرینشگر (Creator)", "هر اثر داستان دست و هنر است؛ اصالت، هنرمند و منحصربه‌فرد بودن محور پیام است."),
    },
    Sector {
        key: "health",
        name: "سلامت، مکمل و گیاهان دارویی",
        keywords: &["مکمل", "ویتامین", "گیاهی", "دارویی", "طب سنتی", "عرقیات", "دمنوش", "پروتئین", "ورزشی", "سلامت", "رژیمی"],
        margin: 0.45,
        fee: 10.0,
        ret: 2.0,
        pack: 4.0,
        palette: ["#1B7F5B","#A8D5BA","#F3FAF6","#E4572E"],
        fonts: ("استعداد","وزیرمتن"),
        tone: [40,55,55,15],
        segments: &["افراد سلامت‌محور و ورزشکاران", "میانسالان و سالمندان", "داروخانه‌ها و عطاری‌ها", "باشگاه‌های ورزشی"],
        channels: &["داروخانه‌های آنلاین و دیجی‌کالا", "سایت با اینماد و مشاوره", "اینستاگرام", "داروخانه‌ها، عطاری‌ها و باشگاه‌ها"],
        partners: &["تولیدکننده دارای مجوز", "متخصص تغذیه/پزشک", "پخش دارویی", "مربیان ورزشی"],
        resources: &["مجوزها و مستندات علمی", "مشاوره تخصصی", "کیفیت پایدار"],
        licences: &["پروانه و شماره ثبت سازمان غذا و دارو", "اینماد", "رعایت قوانین تبلیغ کالای سلامت‌محور"],
        seasons: &["شروع پاییز (تقویت ایمنی)", "قبل از تابستان (تناسب اندام)", "رمضان", "نوروز"],
        content: &["آموزش علمی و مستند", "پاسخ متخصص به سؤال‌ها", "روتین سلامت روزانه", "تجربه مشتری (بدون ادعای درمانی)"],
        mix: &[("instagram", 25), ("influencer", 20), ("search", 20), ("marketplace", 15), ("messenger", 5), ("retention", 15)],
        archetype: ("فرزانه (Sage)", "در سلامت، اعتماد با دانش ساخته می‌شود؛ مستندات و توصیه متخصص محور پیام است."),
    },
    Sector {
        key: "industrial",
        name: "صنعتی، ساختمانی و تجهیزات (B2B)",
        keywords: &["صنعتی", "قطعه", "قطعات", "تجهیزات", "مصالح", "ساختمانی", "ابزار", "ماشین", "دستگاه", "لوله", "کابل برق", "رنگ", "شیرآلات", "پمپ", "یراق", "پلاستیک", "بسته‌بندی"],
        margin: 0.25,
        fee: 5.0,
        ret: 2.0,
        pack: 5.0,
        palette: ["#1F2A37","#F2A900","#EEF1F4","#2E86AB"],
        fonts: ("وزیرمتن (ضخیم)","وزیرمتن"),
        tone: [15,45,15,10],
        segments: &["پیمانکاران و شرکت‌های ساختمانی", "کارخانه‌ها و واحدهای تولیدی", "فروشگاه‌های ابزار و مصالح (عمده)", "پروژه‌های دولتی و مناقصه‌ها"],
        channels: &["فروش مستقیم و تیم فروش میدانی", "سایت کاتالوگی با اینماد و فرم استعلام", "دایرکتوری‌های B2B و ترب", "نمایشگاه‌های تخصصی", "نمایندگی‌های استانی"],
        partners: &["تأمین‌کنندگان مواد اولیه", "نمایندگان فروش استانی", "مشاوران فنی و مهندسان ناظر", "شرکت‌های حمل بار"],
        resources: &["کاتالوگ فنی و استانداردها", "تیم فروش و پشتیبانی فنی", "ظرفیت تولید و انبار"],
        licences: &["استاندارد ملی/ISO", "گواهی آزمون محصول", "رتبه/صلاحیت برای مناقصه", "اینماد برای سایت"],
        seasons: &["فصل ساخت‌وساز (بهار و تابستان)", "پایان سال مالی و بودجه پروژه‌ها", "نمایشگاه‌های تخصصی سالانه"],
        content: &["کاتالوگ و دیتاشیت فنی", "پروژه‌های اجراشده (Case Study)", "ویدیوی تست و مقایسه فنی", "راهنمای انتخاب و محاسبه"],
        mix: &[("search", 35), ("b2b", 30), ("messenger", 10), ("instagram", 10), ("retention", 15)],
        archetype: ("حاکم (Ruler)", "خریدار صنعتی ریسک نمی‌پذیرد؛ ثبات، استاندارد و اعتبار محور پیام است."),
    },
    Sector {
        key: "services",
        name: "خدمات (کافه، رستوران، آموزش، زیبایی و …)",
        keywords: &["کافه", "رستوران", "آموزش", "کلاس", "آرایشگاه", "سالن", "خدمات", "مشاوره", "باشگاه", "کلینیک", "آموزشگاه", "تعمیر"],
        margin: 0.6,
        fee: 8.0,
        ret: 4.0,
        pack: 0.0,
        palette: ["#2B2D42","#EF8354","#F8F9FA","#4F9D9D"],
        fonts: ("استعداد","وزیرمتن"),
        tone: [60,60,65,45],
        segments: &["ساکنان و شاغلان محدوده جغرافیایی", "مشتریان وفادار و تکراری", "سازمان‌ها (قرارداد گروهی)"],
        channels: &["گوگل‌مپ و نشان/بلد", "اینستاگرام محلی", "سایت رزرو آنلاین", "پلتفرم‌های رزرو/سفارش (اسنپ‌فود و …)", "معرفی مشتری به مشتری"],
        partners: &["تأمین‌کنندگان مواد/لوازم", "پلتفرم‌های رزرو و سفارش", "کسب‌وکارهای مکمل محله"],
        resources: &["مکان و تجربه حضوری", "نیروی انسانی ماهر", "استاندارد خدمت"],
        licences: &["پروانه کسب و مجوزهای صنفی", "مجوز بهداشت", "اینماد برای فروش آنلاین"],
        seasons: &["تعطیلات و مناسبت‌ها", "شروع فصل‌ها (ثبت‌نام دوره‌ها)", "روزهای ویژه (ولنتاین، روز مادر)"],
        content: &["پشت صحنه و تیم", "تجربه و نظر مشتریان", "پیشنهاد ویژه هفته", "آموزش کوتاه تخصصی"],
        mix: &[("local", 30), ("instagram", 30), ("influencer", 10), ("messenger", 10), ("retention", 20)],
        archetype: ("همراه (Everyman)", "خدمت خوب یعنی حس آشنا و قابل اعتماد؛ تجربه، صمیمیت و ثبات کیفیت محور پیام است."),
    },
    Sector {
        key: "general",
        name: "کالای مصرفی عمومی",
        keywords: &[],
        margin: 0.3,
        fee: 10.0,
        ret: 4.0,
        pack: 5.0,
        palette: ["#1F2933","#3E7CB1","#F5F7FA","#F5A623"],
        fonts: ("وزیرمتن (ضخیم)","شبنم"),
        tone: [55,55,50,40],
        segments: &["خریداران آنلاین قیمت‌محور", "خریداران کیفیت‌محور و وفادار", "خرده‌فروشان و عمده‌فروشان"],
        channels: &["دیجی‌کالا، باسلام و ترب", "سایت اختصاصی با اینماد", "اینستاگرام و تلگرام", "پخش‌کننده‌ها و فروشگاه‌های حضوری"],
        partners: &["تأمین‌کنندگان و تولیدکنندگان", "شرکت‌های ارسال", "پخش‌کننده‌ها"],
        resources: &["محصول با کیفیت پایدار", "زنجیره تأمین قابل اتکا", "خدمات مشتری"],
        licences: &["اینماد", "ثبت علامت تجاری", "استاندارد/مجوز اختصاصی کالا"],
        seasons: &["نوروز", "یلدا", "بلک‌فرایدی", "شروع مهر"],
        content: &["معرفی کاربرد محصول", "مقایسه و راهنمای خرید", "نظر مشتریان", "پشت صحنه کسب‌وکار"],
        mix: &[("instagram", 25), ("marketplace", 25), ("search", 15), ("influencer", 15), ("messenger", 5), ("retention", 15)],
        archetype: ("همراه (Everyman)", "محصول روزمره باید قابل اعتماد و در دسترس باشد؛ صداقت و ارزش محور پیام است."),
    },
];

/// Advertising channel catalogue: (key, name, why, KPI, first actions).
const CHANNELS: &[(&str, &str, &str, &str, &[&str])] = &[
    (
        "instagram",
        "اینستاگرام (محتوا + تبلیغ در پیج‌ها)",
        "بیشترین زمان حضور خریدار ایرانی؛ جای ساختن اعتماد با تصویر، ویدیو و نظر مشتری.",
        "نرخ تعامل بالای ۳٪، دایرکت خرید هفتگی، هزینه هر سفارش",
        &["هفته‌ای ۳ ریلز + استوری روزانه طبق تقویم محتوا", "هایلایت‌های ثابت: محصولات، نظر مشتری، نحوه خرید، مجوزها", "لینک خرید (غرفه/سایت) در بیو با UTM برای سنجش"],
    ),
    (
        "influencer",
        "اینفلوئنسر و تولیدکننده محتوا",
        "اعتماد قرض‌گرفته؛ برای برند تازه سریع‌ترین راه دیده‌شدن.",
        "کد تخفیف اختصاصی هر اینفلوئنسر و فروش هر کد",
        &["۵ تا ۱۰ میکرواینفلوئنسر (۱۰ تا ۱۰۰ هزار) مرتبط با حوزه به‌جای یک چهره گران", "ارسال محصول برای تجربه واقعی قبل از قرارداد", "پرداخت ترکیبی: ثابت کم + درصد فروش کد اختصاصی"],
    ),
    (
        "marketplace",
        "تبلیغات داخل بازارگاه‌ها (دیجی‌کالا، باسلام، ترب)",
        "خریدار آماده خرید آنجاست؛ دیده‌شدن در صفحه اول جستجو مستقیماً فروش می‌آورد.",
        "رتبه در جستجوی کلمه اصلی، ROAS بالای ۳",
        &["عنوان‌ها با الگوی «نوع + ویژگی + اندازه + برند» و عکس اول با زمینه سفید", "تبلیغ کالای پرفروش در جستجوی کلمه اصلی دسته", "ثبت در ترب برای مقایسه قیمت و بازدید رایگان"],
    ),
    (
        "search",
        "سئو و تبلیغات جستجو (گوگل، یکتانت)",
        "خریدار با نیت مشخص جستجو می‌کند؛ ماندگارترین ورودی ارزان در بلندمدت.",
        "کلیک ارگانیک ماهانه، رتبه ۱۰ کلمه اصلی، هزینه هر سفارش از تبلیغ کلیکی",
        &["صفحه هر محصول با توضیح کامل، سؤالات متداول و داده ساخت‌یافته (Schema)", "۴ مقاله راهنمای خرید در ماه برای کلمات پرجستجو", "ثبت در گوگل‌مپ و بلد/نشان با عکس و ساعت کاری"],
    ),
    (
        "messenger",
        "تلگرام، ایتا، بله و پیامک هدفمند",
        "ارزان، مستقیم و قابل تکرار؛ برای اطلاع‌رسانی تخفیف و محصول جدید به مخاطب گرم.",
        "نرخ کلیک پیامک بالای ۲٪، رشد اعضای کانال",
        &["کانال رسمی با محتوای انحصاری (کد تخفیف، پیش‌فروش)", "پیامک فقط به مشتریان قبلی و با لینک لغو عضویت", "تبلیغ در کانال‌های هم‌حوزه با کد تخفیف اختصاصی"],
    ),
    (
        "retention",
        "باشگاه مشتریان و خرید تکراری",
        "جذب مشتری تازه ۵ برابر گران‌تر از نگه‌داشتن مشتری فعلی است.",
        "نرخ خرید دوباره بالای ۲۵٪، تعداد نظر ثبت‌شده",
        &["پیام پیگیری بعد از تحویل و درخواست نظر", "تخفیف خرید دوم و برنامه معرفی دوست (هر دو طرف هدیه)", "بسته اشتراکی/دوره‌ای برای محصولات مصرفی"],
    ),
    (
        "b2b",
        "فروش سازمانی و نمایشگاه‌ها",
        "قراردادهای بزرگ و تکرارشونده؛ ستون درآمد پایدار کسب‌وکار صنعتی و عمده.",
        "تعداد استعلام و نرخ تبدیل استعلام به قرارداد",
        &["فهرست ۱۰۰ خریدار سازمانی هدف (از بخش «اطلاعات مشتریان») و تماس منظم", "کاتالوگ PDF و پیشنهاد قیمت استاندارد ۲۴ ساعته", "حضور در یک نمایشگاه تخصصی در سال"],
    ),
    (
        "local",
        "حضور محلی (گوگل‌مپ، بلد، نشان و تبلیغات محله)",
        "مشتری خدمات از چند کیلومتری می‌آید؛ دیده‌شدن روی نقشه و نظرها تعیین‌کننده است.",
        "تعداد و امتیاز نظرها روی نقشه، تماس و مسیریابی ماهانه",
        &["تکمیل پروفایل نقشه با عکس، منو/خدمات و ساعت کاری", "درخواست نظر از هر مشتری راضی", "همکاری با کسب‌وکارهای مکمل محله"],
    ),
];

fn sector_of(key: &str, category: &str, name: &str) -> &'static Sector {
    if let Some(s) = SECTORS.iter().find(|s| s.key == key) {
        return s;
    }
    let text = format!("{category} {name}");
    SECTORS
        .iter()
        .find(|s| s.keywords.iter().any(|k| crate::fees::has_word(&text, k)))
        .unwrap_or(&SECTORS[SECTORS.len() - 1])
}

fn label(v: &str, table: &[(&str, &str)], default: &'static str) -> String {
    table.iter().find(|(k, _)| *k == v).map_or(default.to_string(), |(_, l)| l.to_string())
}

const MODELS: &[(&str, &str)] = &[
    ("b2c_online", "فروش آنلاین به مصرف‌کننده (بازارگاه + شبکه اجتماعی)"),
    ("d2c", "برند مستقیم به مصرف‌کننده (D2C، سایت اختصاصی)"),
    ("b2b", "فروش عمده و سازمانی (B2B)"),
    ("manufacturer", "تولیدکننده با فروش چندکاناله"),
    ("retail", "فروشگاه حضوری + آنلاین"),
    ("service", "ارائه خدمات"),
];
const STAGES: &[(&str, &str)] = &[("idea", "ایده و پیش از راه‌اندازی"), ("startup", "نوپا (کمتر از ۲ سال)"), ("growth", "در حال رشد"), ("established", "تثبیت‌شده")];
const GOALS: &[(&str, &str)] = &[
    ("sales", "افزایش فروش"),
    ("awareness", "شناخته‌شدن برند"),
    ("launch", "عرضه محصول/برند جدید"),
    ("wholesale", "ورود به فروش عمده و سازمانی"),
    ("export", "صادرات"),
];

fn fa(n: f64) -> String {
    crate::logic::fa_num(n)
}

fn toman(n: i64) -> String {
    format!("{} تومان", crate::logic::fa_num(n as f64))
}

/// Of the goods in a returned order, the share that cannot be sold again (damaged, opened, expired).
const RETURN_WRITE_OFF: f64 = 0.3;

/// Unit economics and a 12-month base scenario from the owner's numbers, the market and the sector.
///
/// Per unit sold: price − cost − channel commission − VAT on that commission − packaging and
/// delivery − the expected cost of returns. A return costs the delivery both ways and the share of
/// the goods that cannot be resold, spread over the units that stay sold: `r / (1 − r) × (2 × pack +
/// write-off × cost)` for a return rate `r`.
fn finance(inp: &Inputs, market_median: i64, store_median: i64, sector: &Sector, stage: &str) -> Finance {
    let margin = sector.margin;
    let mut basis = vec![];
    let price = if inp.price > 0 {
        basis.push("قیمت فروش: عددی که خودتان وارد کردید.".into());
        inp.price
    } else if store_median > 0 {
        basis.push("قیمت فروش: میانه قیمت فروشگاه خود برند.".into());
        store_median
    } else if market_median > 0 {
        basis.push("قیمت فروش: میانه قیمت بازار در آخرین تحلیل.".into());
        market_median
    } else {
        0
    };
    let unit_cost = if inp.unit_cost > 0 {
        basis.push("بهای تمام‌شده: عددی که خودتان وارد کردید.".into());
        inp.unit_cost
    } else {
        basis.push(format!("بهای تمام‌شده: فرض حاشیه ناخالص معمول این حوزه ({}٪)؛ برای دقت، بهای تمام‌شده واقعی را وارد کنید.", fa((margin * 100.0).round())));
        (price as f64 * (1.0 - margin)) as i64
    };
    let pick = |own: Option<f64>, usual: f64, hi: f64| own.filter(|v| v.is_finite()).map_or((usual, false), |v| (v.clamp(0.0, hi), true));
    let ((fee_pct, fee_own), (vat_pct, vat_own), (return_pct, ret_own)) = (pick(inp.channel_fee_pct, sector.fee, 40.0), pick(inp.vat_pct, 10.0, 30.0), pick(inp.return_pct, sector.ret, 60.0));
    let channel_fee = (price as f64 * fee_pct / 100.0).round() as i64;
    basis.push(format!("کارمزد کانال فروش: {}٪ قیمت ({}).", fa(fee_pct), if fee_own { "عدد شما" } else { "نرخ معمول این حوزه؛ تعرفه واقعی پلتفرم خود را وارد کنید" }));
    let vat = (channel_fee as f64 * vat_pct / 100.0).round() as i64;
    basis.push(format!("مالیات بر ارزش افزوده: {}٪ روی کارمزد کانال ({}).", fa(vat_pct), if vat_own { "عدد شما" } else { "نرخ قانونی" }));
    let pack = inp.pack_cost.map_or_else(|| (price as f64 * sector.pack / 100.0).round() as i64, |v| v.max(0));
    basis.push(if inp.pack_cost.is_some() { "بسته‌بندی و ارسال هر واحد: عدد شما.".into() } else { format!("بسته‌بندی و ارسال هر واحد: {}٪ قیمت (برآورد معمول این حوزه).", fa(sector.pack)) });
    let r = return_pct / 100.0;
    let returns = (r / (1.0 - r) * (2.0 * pack as f64 + RETURN_WRITE_OFF * unit_cost as f64)).round() as i64;
    basis.push(format!("مرجوعی: {}٪ سفارش‌ها ({}؛ هزینه ارسال رفت‌وبرگشت و {}٪ کالای برگشتی غیرقابل فروش).", fa(return_pct), if ret_own { "عدد شما" } else { "نرخ معمول این حوزه" }, fa(RETURN_WRITE_OFF * 100.0)));
    let unit_margin = price - unit_cost - channel_fee - vat - pack - returns;
    let fixed = inp.fixed_cost.max(0);
    let budget = inp.budget.max(0);
    let break_even_units = if unit_margin > 0 { ((fixed + budget) as f64 / unit_margin as f64).ceil() as i64 } else { 0 };
    // base scenario: start and monthly growth by stage, capped by capacity
    let (start_share, growth) = match stage {
        "idea" => (0.25, 0.18),
        "startup" => (0.45, 0.12),
        "growth" => (0.8, 0.07),
        _ => (1.1, 0.03),
    };
    let mut units = ((break_even_units.max(30)) as f64 * start_share).round();
    let mut months = vec![];
    let mut cumulative = 0;
    if price > 0 {
        for m in 1..=12 {
            let u = if inp.capacity > 0 { (units as i64).min(inp.capacity) } else { units as i64 };
            let revenue = u * price;
            let gross = u * unit_margin;
            let profit = gross - budget - fixed;
            cumulative += profit;
            months.push(Month {
                month: m,
                units: u,
                revenue,
                gross,
                marketing: budget,
                fixed,
                profit,
                cumulative,
            });
            units *= 1.0 + growth;
        }
        basis.push(format!(
            "سناریوی پایه: شروع از {}٪ نقطه سربه‌سر و رشد ماهانه {}٪ (متناسب با مرحله کسب‌وکار){}.",
            fa((start_share * 100.0).round()),
            fa((growth * 100.0).round()),
            if inp.capacity > 0 { "، محدود به ظرفیت تولید" } else { "" }
        ));
    }
    Finance {
        price,
        unit_cost,
        channel_fee,
        vat,
        pack,
        returns,
        fee_pct,
        vat_pct,
        return_pct,
        unit_margin,
        margin_pct: if price > 0 { (unit_margin as f64 * 1000.0 / price as f64).round() / 10.0 } else { 0.0 },
        fixed,
        budget,
        break_even_units,
        months,
        basis,
    }
}

/// Splits the monthly budget over the sector's channels, shifted by goal and model.
pub fn ad_mix(sector_mix: &[(&str, i64)], goal: &str, model: &str, budget: i64) -> Vec<AdChannel> {
    let mut w: Vec<(&str, i64)> = sector_mix.to_vec();
    let mut bump = |key: &str, by: i64| match w.iter_mut().find(|x| x.0 == key) {
        Some(x) => x.1 += by,
        None => w.push((key_static(key), by)),
    };
    match goal {
        "awareness" | "launch" => {
            bump("influencer", 10);
            bump("instagram", 5);
        }
        "sales" => {
            bump("marketplace", 8);
            bump("retention", 5);
        }
        "wholesale" | "export" => {
            bump("b2b", 25);
            bump("search", 5);
        }
        _ => {}
    }
    if model == "b2b" {
        bump("b2b", 20);
    }
    if model == "service" || model == "retail" {
        bump("local", 15);
    }
    let total: i64 = w.iter().map(|x| x.1.max(0)).sum::<i64>().max(1);
    let mut out: Vec<AdChannel> = w
        .iter()
        .filter_map(|(k, wt)| {
            let (_, name, why, kpi, actions) = CHANNELS.iter().find(|c| c.0 == *k)?;
            let share = (*wt * 100 + total / 2) / total;
            Some(AdChannel {
                name: name.to_string(),
                share,
                budget: budget * share / 100,
                why: why.to_string(),
                kpi: kpi.to_string(),
                actions: actions.iter().map(|s| s.to_string()).collect(),
            })
        })
        .filter(|c| c.share > 0)
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse(c.share));
    // rounding: the shares always add up to 100
    let sum: i64 = out.iter().map(|c| c.share).sum();
    if let Some(first) = out.first_mut() {
        first.share += 100 - sum;
        first.budget = budget * first.share / 100;
    }
    out
}

fn key_static(k: &str) -> &'static str {
    CHANNELS.iter().find(|c| c.0 == k).map_or("instagram", |c| c.0)
}

fn block(key: &str, title: &str, items: Vec<String>) -> Block {
    Block {
        key: key.into(),
        title: title.into(),
        items: items.into_iter().filter(|s| !s.trim().is_empty()).collect(),
    }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// The whole strategy pack for one brand.
pub fn build(b: &Brand, inp: &Inputs) -> Strategy {
    let r: BrandReport = b.report.clone().unwrap_or_default();
    let sector = sector_of(&inp.sector, &b.category, &b.name);
    let model_key = if inp.model.is_empty() {
        if sector.key == "industrial" {
            "b2b"
        } else if sector.key == "services" {
            "service"
        } else {
            "b2c_online"
        }
    } else {
        inp.model.as_str()
    };
    let stage_key = if inp.stage.is_empty() {
        if r.market.offers == 0 && r.store.as_ref().is_none_or(|x| x.products < 10) {
            "startup"
        } else {
            "growth"
        }
    } else {
        inp.stage.as_str()
    };
    let goal_key = if inp.goal.is_empty() { "sales" } else { inp.goal.as_str() };
    let name = b.name.trim();
    let product = if b.category.trim().is_empty() { "محصولات".to_string() } else { b.category.trim().to_string() };
    let audience = if inp.audience.trim().is_empty() { sector.segments[0].to_string() } else { inp.audience.trim().to_string() };
    let region = if inp.region.trim().is_empty() { "سراسر ایران".to_string() } else { inp.region.trim().to_string() };
    let store_median = r.store.as_ref().map_or(0, |x| x.median);
    let market_median = if r.market.market_median > 0 { r.market.market_median } else { r.market.median };
    let price_index = r.store.as_ref().and_then(|x| x.price_index).unwrap_or(r.market.price_index);
    let tier = if price_index >= 120.0 {
        "premium"
    } else if price_index > 0.0 && price_index <= 85.0 {
        "value"
    } else {
        "mid"
    };
    let fin = finance(inp, market_median, store_median, sector, stage_key);
    let competitors: Vec<String> = r.competitors.iter().take(4).map(|c| c.name.clone()).collect();
    let has_site = r.site.ok;
    let has_insta = r.insta.followers.is_some();
    let usp = inp.usp.trim();

    // ---- evidence ----
    let mut evidence = vec![];
    if r.overall > 0.0 {
        evidence.push(format!("شاخص سلامت برند: {} از ۱۰۰ (اعتماد داده {}٪).", fa(r.overall.round()), fa(r.confidence as f64)));
    }
    if r.market.offers > 0 {
        evidence.push(format!("حضور در بازار: {} عرضه، سهم {}٪ در {} پلتفرم.", fa(r.market.offers as f64), fa(r.market.share_pct), fa(r.market.platforms.len() as f64)));
    }
    if market_median > 0 {
        evidence.push(format!("میانه قیمت بازار این دسته: {}.", toman(market_median)));
    }
    if let Some(st) = r.store.as_ref().filter(|x| x.products > 0) {
        evidence.push(format!(
            "فروشگاه برند: {} محصول، قیمت {} تا {}{}.",
            fa(st.products as f64),
            toman(st.min),
            toman(st.max),
            st.rating.map(|x| format!("، امتیاز {} از ۵ با {} نظر", fa(x), fa(st.reviews as f64))).unwrap_or_default()
        ));
    }
    if let Some(f) = r.insta.followers {
        evidence.push(format!("اینستاگرام: {} دنبال‌کننده.", crate::logic::fa_num(f as f64)));
    }
    if has_site {
        evidence.push("سایت رسمی در دسترس است.".into());
    }
    if !competitors.is_empty() {
        evidence.push(format!("رقبای اصلی در بازار: {}.", competitors.join("، ")));
    }

    // ---- executive summary ----
    let tier_fa = match tier {
        "premium" => "پریمیوم (بالاتر از میانه بازار)",
        "value" => "اقتصادی (پایین‌تر از میانه بازار)",
        _ => "میان‌رده (هم‌سطح بازار)",
    };
    let mut summary = vec![format!(
        "«{name}» در حوزه {} با مدل «{}» و در مرحله «{}» فعالیت می‌کند. هدف این برنامه {} است؛ مخاطب اصلی «{audience}» در {region}.",
        sector.name,
        label(model_key, MODELS, "فروش آنلاین"),
        label(stage_key, STAGES, "نوپا"),
        label(goal_key, GOALS, "افزایش فروش"),
    )];
    summary.push(format!(
        "جایگاه قیمتی: {tier_fa}. {}",
        match tier {
            "premium" => "ارزش بالاتر باید دیده و اثبات شود: اصالت، کیفیت آزموده، بسته‌بندی و تجربه خرید.",
            "value" => "مزیت قیمت را با کیفیت قابل اتکا همراه کنید تا «ارزان» به «بی‌کیفیت» تعبیر نشود.",
            _ => "تمایز باید از راه کیفیت، خدمات و داستان برند ساخته شود، نه جنگ قیمت.",
        }
    ));
    if fin.price > 0 {
        summary.push(format!(
            "اقتصاد واحد: قیمت {}، سود هر واحد پس از بهای تمام‌شده و هزینه کانال {} ({}٪). نقطه سربه‌سر ماهانه با بودجه تبلیغات و هزینه ثابت: {} واحد.",
            toman(fin.price),
            toman(fin.unit_margin),
            fa(fin.margin_pct),
            fa(fin.break_even_units as f64)
        ));
    }
    let first_actions: Vec<String> = r.actions.iter().take(3).cloned().collect();
    if !first_actions.is_empty() {
        summary.push(format!("سه اقدام فوری بر پایه تحلیل: {}", first_actions.join(" · ")));
    }

    // ---- positioning ----
    let diff: Vec<String> = [
        (!usp.is_empty()).then(|| usp.to_string()),
        r.strengths.first().cloned(),
        Some(match tier {
            "premium" => "کیفیت ممتاز و قابل اثبات".to_string(),
            "value" => "بهترین ارزش در برابر قیمت".to_string(),
            _ => "کیفیت قابل اتکا با قیمت منصفانه".to_string(),
        }),
    ]
    .into_iter()
    .flatten()
    .collect();
    let positioning = vec![
        block(
            "statement",
            "بیانیه جایگاه‌یابی",
            vec![format!(
                "برای {audience} که به دنبال {product} {} هستند، «{name}» برندی است که {}؛ برخلاف {}، {}.",
                match tier {
                    "premium" => "اصیل و باکیفیت",
                    "value" => "خوب و به‌صرفه",
                    _ => "مطمئن",
                },
                diff.first().cloned().unwrap_or_else(|| "کیفیت ثابت و خرید بی‌دغدغه را تضمین می‌کند".into()),
                if competitors.is_empty() { "عرضه‌های بی‌نام و نشان بازار".to_string() } else { competitors.join(" و ") },
                if usp.is_empty() { "منشأ، کیفیت و پاسخ‌گویی خود را شفاف نشان می‌دهد" } else { "این تمایز را با شواهد واقعی نشان می‌دهد" }
            )],
        ),
        block(
            "value",
            "ارزش پیشنهادی",
            [
                vec![format!("کارکردی: {product} با کیفیت ثابت و مشخصات شفاف")],
                vec!["احساسی: آرامش خیال از انتخاب درست و اصیل".to_string()],
                vec!["اجتماعی: هدیه‌ای که به خریدار اعتبار می‌دهد".to_string()],
                diff.clone(),
            ]
            .concat(),
        ),
        block("archetype", "کهن‌الگوی برند", vec![sector.archetype.0.to_string(), sector.archetype.1.to_string()]),
        block(
            "tone",
            "لحن و صدای برند",
            s(match tier {
                "premium" => &["مطمئن و کم‌حرف؛ اغراق نکنید، شواهد بدهید", "واژه‌های حسی و دقیق (مبدأ، فرایند، جزئیات)", "طراحی خلوت با فضای سفید"],
                "value" => &["صمیمی، روشن و صادق", "عدد و مقایسه شفاف", "طراحی شاد و پرانرژی"],
                _ => &["گرم، حرفه‌ای و قابل اعتماد", "آموزشی و پاسخ‌گو", "طراحی تمیز با یک رنگ شاخص ثابت"],
            }),
        ),
        block(
            "taglines",
            "ایده شعار (برای آزمون با مشتری)",
            vec![
                format!("{name}؛ اصالتی که می‌شود چشید و دید."),
                format!("{name}، انتخاب مطمئن {product}."),
                format!("از مبدأ تا خانه شما؛ {name}."),
            ],
        ),
    ];

    // ---- business model canvas ----
    let mut channels: Vec<String> = s(sector.channels);
    if !r.channels.is_empty() {
        channels.insert(0, format!("کانال‌های فعلی دیده‌شده: {}", r.channels.iter().map(|c| crate::market::platform(&c.platform).map_or(c.platform.as_str(), |p| p.name)).collect::<Vec<_>>().join("، ")));
    }
    let revenue: Vec<String> = match model_key {
        "b2b" => s(&["قرارداد فروش عمده با تخفیف پلکانی حجمی", "سفارش‌های تکراری سازمانی", "تولید با برند مشتری (OEM/فاسون)", "خدمات فنی و پس از فروش"]),
        "service" => s(&["فروش خدمت/جلسه", "بسته‌ها و اشتراک‌های دوره‌ای", "قرارداد گروهی سازمان‌ها", "فروش محصولات جانبی"]),
        "manufacturer" => s(&["فروش خرده با برند خود (آنلاین)", "فروش عمده به پخش و فروشگاه‌ها", "تولید برای برندهای دیگر (فاسون)", "صادرات"]),
        _ => s(&["فروش تکی آنلاین (بازارگاه و سایت)", "بسته‌های هدیه و ترکیبی با حاشیه بالاتر", "اشتراک/خرید دوره‌ای محصولات مصرفی", "فروش عمده و سازمانی (هدایای مناسبتی)"]),
    };
    let canvas = vec![
        block("partners", "شرکای کلیدی", s(sector.partners)),
        block(
            "activities",
            "فعالیت‌های کلیدی",
            vec![
                "تأمین پایدار و کنترل کیفیت هر سری".into(),
                "تولید محتوا و مدیریت شبکه‌های اجتماعی".into(),
                "مدیریت غرفه‌ها و موجودی در کانال‌ها".into(),
                "خدمات مشتری، پیگیری پس از فروش و جمع‌آوری نظر".into(),
                if matches!(model_key, "b2b" | "manufacturer") { "فروش سازمانی و مدیریت نمایندگان".into() } else { "تحلیل قیمت رقبا و پایش روزانه بازار".into() },
            ],
        ),
        block("resources", "منابع کلیدی", [s(sector.resources), vec![format!("نام و هویت برند «{name}»"), "داده مشتریان و بازار (هامیستا سلر)".into()]].concat()),
        block("value", "ارزش پیشنهادی", diff.iter().cloned().chain([format!("{product} اصیل با کیفیت ثابت"), "خرید آسان و پاسخ‌گویی سریع".into()]).collect()),
        block(
            "relations",
            "ارتباط با مشتری",
            s(&["پاسخ‌گویی سریع در دایرکت/پیام‌رسان (زیر ۱ ساعت در ساعات کاری)", "پیگیری بعد از تحویل و درخواست نظر", "باشگاه مشتریان: تخفیف خرید دوم و معرفی دوست", "جامعه وفادار در کانال تلگرام/ایتا"]),
        ),
        block("channels", "کانال‌ها", channels),
        block("segments", "بخش‌های مشتری", [vec![audience.clone()], s(sector.segments)].concat().into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect()),
        block(
            "costs",
            "ساختار هزینه",
            vec![
                if fin.unit_cost > 0 { format!("بهای تمام‌شده هر واحد: {}", toman(fin.unit_cost)) } else { "بهای تمام‌شده محصول".into() },
                if fin.budget > 0 { format!("تبلیغات: {} در ماه", toman(fin.budget)) } else { "تبلیغات و تولید محتوا".into() },
                if fin.fixed > 0 { format!("هزینه ثابت: {} در ماه", toman(fin.fixed)) } else { "هزینه‌های ثابت (اجاره، حقوق، ابزارها)".into() },
                "کمیسیون بازارگاه‌ها، درگاه پرداخت، بسته‌بندی و ارسال".into(),
            ],
        ),
        block("revenue", "جریان‌های درآمدی", revenue.clone()),
    ];

    // ---- business model ----
    let pricing: Vec<String> = match tier {
        "premium" => s(&[
            "قیمت‌گذاری بر پایه ارزش؛ تخفیف عمومی ندهید، هدیه و ارتقای بسته بدهید",
            "یک محصول ورودی کوچک‌تر برای آزمودن برند توسط خریدار تازه",
            "بسته‌های هدیه با حاشیه بالاتر برای مناسبت‌ها",
        ]),
        "value" => s(&["قیمت رقابتی ثابت + تخفیف حجمی", "بسته‌های اقتصادی خانواده", "قیمت را با پایش روزانه رقبا هم‌سطح نگه دارید (بخش «پایش روزانه قیمت»)"]),
        _ => s(&["قیمت نزدیک میانه بازار با تمایز در کیفیت و خدمت", "قیمت‌گذاری پلکانی اندازه‌ها (کوچک/متوسط/خانواده)", "تخفیف فقط هدفمند: خرید دوم، مناسبت‌ها، باشگاه مشتریان"]),
    };
    let business_model = vec![
        block(
            "type",
            "نوع مدل",
            vec![label(model_key, MODELS, "فروش آنلاین"), format!("موتور رشد پیشنهادی: {}", match model_key {
                "b2b" => "فروش سازمانی و نمایندگی (قرارداد بلندمدت)",
                "service" => "تکرار مشتری و معرفی دهان‌به‌دهان",
                "d2c" => "سایت اختصاصی + جامعه وفادار (داده مشتری متعلق به خود برند)",
                _ => "بازارگاه‌ها برای فروش امروز + سایت و جامعه برای سود و وفاداری فردا",
            })],
        ),
        block("revenue", "جریان‌های درآمدی", revenue),
        block("pricing", "استراتژی قیمت", pricing),
        block(
            "moat",
            "مزیت پایدار و دفاع در برابر رقبا",
            s(&["داستان و منشأ اختصاصی که قابل کپی نیست", "نظرهای واقعی و امتیاز بالا در کانال‌ها (سرمایه اعتماد)", "داده و رابطه مستقیم با مشتری (باشگاه، کانال، شماره‌ها)", "علامت تجاری ثبت‌شده و بسته‌بندی شناخته‌شده"]),
        ),
    ];

    // ---- business plan sections ----
    let mut market = vec![format!("حوزه: {}؛ مخاطب: {audience}؛ منطقه: {region}.", sector.name)];
    if market_median > 0 {
        market.push(format!("میانه قیمت بازار {}{}.", toman(market_median), if r.market.searches > 0 { format!(" از {} جستجوی تحلیل‌شده", r.market.searches) } else { String::new() }));
    }
    market.extend(r.opportunities.iter().take(3).cloned());
    let mut competition: Vec<String> = r
        .competitors
        .iter()
        .take(5)
        .map(|c| format!("{}: {}٪ سهم عرضه{}", c.name, fa(c.pct), if c.median > 0 { format!("، میانه قیمت {}", toman(c.median)) } else { String::new() }))
        .collect();
    if competition.is_empty() {
        competition.push("رقبا هنوز سنجیده نشده‌اند: «دسته کالا» را در فرم برند بنویسید و دوباره تحلیل کنید تا سهم و قیمت رقبا بیاید.".into());
    }
    competition.extend(r.threats.iter().take(2).cloned());
    let plan = vec![
        block("market", "تحلیل بازار و مخاطب", market),
        block("competition", "رقبا و تهدیدها", competition),
        block(
            "operations",
            "عملیات و زنجیره تأمین",
            vec![
                "کنترل کیفیت هر سری تولید با چک‌لیست و نمونه‌برداری".into(),
                if inp.capacity > 0 { format!("ظرفیت فعلی {} واحد در ماه؛ پیش از رسیدن به ۸۰٪ ظرفیت، تأمین/تولید را گسترش دهید.", crate::logic::fa_num(inp.capacity as f64)) } else { "ظرفیت ماهانه تولید/تأمین را مشخص کنید تا رشد با کمبود موجودی متوقف نشود.".into() },
                "ارسال ۲۴ تا ۴۸ ساعته با بسته‌بندی ایمن و کارت تشکر داخل بسته".into(),
                "موجودی یکپارچه برای همه کانال‌ها (جلوگیری از فروش بیش از موجودی)".into(),
            ],
        ),
        block("licences", "مجوزها و الزامات", s(sector.licences)),
        block(
            "team",
            "تیم و نقش‌ها",
            s(match stage_key {
                "idea" | "startup" => &["مالک: محصول، تأمین و فروش", "تولید محتوا (پاره‌وقت یا برون‌سپاری)", "پاسخ‌گویی و ارسال (پاره‌وقت)"],
                _ => &["مدیر فروش و کانال‌ها", "مدیر بازاریابی و محتوا", "مسئول کیفیت و عملیات", "پشتیبانی مشتری"],
            }),
        ),
    ];

    // ---- branding ----
    let mut identity = s(&["لوگو ساده و خوانا در اندازه کوچک (آیکون پیج و برچسب)", "یک رنگ شاخص + یک رنگ مکمل، ثابت در همه جا", "فونت فارسی حرفه‌ای و یکسان (مثل یکان‌بخ/ایران‌سنس)", "ثبت علامت تجاری در اداره مالکیت معنوی"]);
    if r.store.as_ref().is_some_and(|x| x.products > 0 && x.named * 2 < x.products) {
        identity.push("نام برند را در عنوان همه محصولات بیاورید (الان در کمتر از نیمی از عنوان‌هاست).".into());
    }
    let mut digital = vec![];
    if !has_site {
        digital.push("سایت اختصاصی با دامنه برند، صفحه «درباره ما» با داستان و اینماد".to_string());
    } else if !r.site.schema {
        digital.push("داده ساخت‌یافته محصول (Schema) روی سایت برای دیده‌شدن قیمت و امتیاز در گوگل".into());
    }
    if !has_insta {
        digital.push("پیج اینستاگرام با همین نام، بیو با لینک خرید و هایلایت‌های اعتماد".into());
    }
    digital.extend(s(&["عکس محصول یکدست (زمینه، نور، زاویه) در همه کانال‌ها", "یک صفحه فرود برای کمپین‌ها با کد تخفیف"]));
    let branding = vec![
        block("identity", "هویت بصری و کلامی", identity),
        block(
            "packaging",
            "بسته‌بندی و تجربه باز کردن بسته",
            s(&["بسته‌بندی متناسب با جایگاه قیمتی (پریمیوم: جعبه/شیشه و برچسب برجسته)", "کارت تشکر با QR به کانال و درخواست نظر", "اطلاعات شفاف: مبدأ، تاریخ تولید، مجوزها و روش مصرف", "نسخه هدیه برای مناسبت‌ها"]),
        ),
        block("digital", "حضور دیجیتال", digital),
        block("trust", "نشانه‌های اعتماد", [s(sector.licences), s(&["نمایش نظر و امتیاز مشتریان واقعی", "ضمانت بازگشت وجه/تعویض روشن"])].concat()),
        block("story", "داستان برند", vec![format!("از کجا شروع شد، چرا «{name}» و چه قولی به مشتری می‌دهد؛ در یک بند کوتاه، ثابت در سایت، بیو و بسته."), "چهره بنیان‌گذار یا تولیدکننده را نشان دهید؛ برند بی‌چهره اعتماد کمتری می‌سازد.".into()]),
    ];

    // ---- advertising ----
    let ads = ad_mix(sector.mix, goal_key, model_key, inp.budget.max(0));

    // ---- roadmap ----
    let urgent: Vec<String> = r.actions.iter().take(4).cloned().collect();
    let mut p1 = vec![];
    p1.extend(urgent);
    if !has_site {
        p1.push("راه‌اندازی سایت ساده با اینماد (یا دست‌کم صفحه فروشگاه رسمی)".into());
    }
    p1.push("تکمیل هویت: لوگو، رنگ، قالب عکس و بیو در همه کانال‌ها".into());
    p1.push("تقویم محتوای ۳۰ روزه و ۹ پست پایه (معرفی، داستان، اعتماد)".into());
    let roadmap = vec![
        Phase {
            title: "پایه‌گذاری".into(),
            window: "روز ۱ تا ۳۰".into(),
            goal: "برند آماده دیده‌شدن و فروش باشد".into(),
            tasks: p1,
            kpis: s(&["عنوان و عکس همه محصولات اصلاح‌شده", "۳۰ پست/استوری منتشرشده", "۱۰ نظر تازه از مشتریان"]),
        },
        Phase {
            title: "جذب و آزمون".into(),
            window: "ماه ۲ و ۳".into(),
            goal: "کانال و پیام سودآور پیدا شود".into(),
            tasks: vec![
                format!("آزمون تبلیغات با {} در کانال‌های پیشنهادی (هر کانال جدا با کد/لینک قابل سنجش)", if inp.budget > 0 { toman(inp.budget) } else { "بودجه کوچک".into() }),
                "همکاری با ۵ میکرواینفلوئنسر و سنجش فروش هر کد".into(),
                "ورود به یک کانال فروش تازه که رقبا در آن کم‌اند".into(),
                "راه‌اندازی باشگاه مشتریان و پیام پیگیری بعد از خرید".into(),
            ],
            kpis: s(&["هزینه هر سفارش (CAC) کمتر از ۳۰٪ سود هر سفارش", "نرخ تبدیل غرفه/سایت بالای ۲٪", "رسیدن به نقطه سربه‌سر ماهانه"]),
        },
        Phase {
            title: "رشد".into(),
            window: "ماه ۴ تا ۶".into(),
            goal: "مقیاس دادن به کانال‌های سودآور".into(),
            tasks: vec![
                "دو برابر کردن بودجه کانال برنده و قطع کانال‌های زیان‌ده".into(),
                "گسترش سبد: اندازه‌های تازه، بسته‌های هدیه و ترکیبی".into(),
                format!("آماده‌سازی کمپین فصلی ({})", sector.seasons.first().copied().unwrap_or("مناسبت بعدی")),
                "پایش روزانه قیمت رقبا و تنظیم قیمت".into(),
            ],
            kpis: s(&["رشد ماهانه فروش ۱۰٪+", "نرخ خرید دوباره ۲۰٪+", "امتیاز میانگین ۴٫۵+"]),
        },
        Phase {
            title: "توسعه و تثبیت".into(),
            window: "ماه ۷ تا ۱۲".into(),
            goal: match goal_key {
                "export" => "آمادگی صادرات و اولین سفارش خارجی".into(),
                "wholesale" => "قراردادهای عمده و نمایندگی پایدار".into(),
                _ => "برند شناخته‌شده با سود پایدار".into(),
            },
            tasks: match goal_key {
                "export" => s(&["گواهی‌های لازم (حلال/ارگانیک/استاندارد مقصد)", "کاتالوگ و سایت انگلیسی/عربی", "حضور در یک نمایشگاه بین‌المللی", "یافتن بازرگان/توزیع‌کننده در کشور هدف"]),
                "wholesale" => s(&["فهرست قیمت عمده پلکانی", "جذب ۳ نماینده استانی", "قرارداد با ۲ فروشگاه زنجیره‌ای یا پخش", "استعلام منظم از خریداران سازمانی (بخش «اطلاعات مشتریان»)"]),
                _ => s(&["ورود به فروش سازمانی برای مناسبت‌ها", "محصول/خط تازه بر پایه نظر مشتریان", "کمپین برندسازی فصلی با اینفلوئنسر محوری", "بررسی صادرات یا نمایندگی استانی"]),
            },
            kpis: s(&["سهم عرضه برند در دسته (در «تحلیل محصول») دو برابر", "۳۰٪ فروش از مشتریان تکراری", "سود خالص مثبت ماهانه"]),
        },
    ];

    // ---- risks & KPIs ----
    let mut risks = vec![
        ("وابستگی به یک کانال فروش".to_string(), "حداقل سه کانال فعال و ساختن سایت/کانال مستقیم برای داده مشتری".to_string()),
        ("جنگ قیمت رقبا".into(), "تمایز در کیفیت، داستان و خدمت؛ پایش روزانه قیمت و پرهیز از تخفیف عمومی".into()),
        ("نوسان قیمت مواد اولیه و تورم".into(), "قرارداد تأمین بلندمدت، بازنگری ماهانه قیمت و بهای تمام‌شده".into()),
        ("کمبود موجودی در فصل اوج".into(), "پیش‌خرید مواد و پیش‌فروش قبل از مناسبت‌ها".into()),
    ];
    if fin.unit_margin <= 0 && fin.price > 0 {
        risks.insert(0, ("سود هر واحد منفی است".into(), "پیش از هر تبلیغی بهای تمام‌شده یا قیمت را اصلاح کنید؛ با این اعداد هر فروش زیان است.".into()));
    }
    let kpis = vec![
        ("فروش ماهانه (تعداد و مبلغ)".to_string(), if fin.break_even_units > 0 { format!("بالای {} واحد (سربه‌سر)", crate::logic::fa_num(fin.break_even_units as f64)) } else { "رشد ماهانه ۱۰٪".into() }),
        ("هزینه جذب هر مشتری (CAC)".into(), if fin.unit_margin > 0 { format!("کمتر از {}", toman(fin.unit_margin * 3 / 10)) } else { "کمتر از ۳۰٪ سود هر سفارش".into() }),
        ("نرخ خرید دوباره".into(), "۲۵٪ و بیشتر".into()),
        ("امتیاز و تعداد نظر".into(), "میانگین ۴٫۵+ و ۲۰ نظر تازه در ماه".into()),
        ("سهم عرضه برند در دسته".into(), if r.market.share_pct > 0.0 { format!("از {}٪ به {}٪ در ۱۲ ماه", fa(r.market.share_pct), fa((r.market.share_pct * 2.0).round())) } else { "دیده‌شدن در صفحه اول جستجوی دسته".into() }),
        ("نرخ تعامل اینستاگرام".into(), "بالای ۳٪".into()),
    ];

    let mut st = Strategy {
        brand: name.into(),
        sector: sector.name.into(),
        model: label(model_key, MODELS, "فروش آنلاین"),
        stage: label(stage_key, STAGES, "نوپا"),
        goal: label(goal_key, GOALS, "افزایش فروش"),
        summary,
        positioning,
        canvas,
        business_model,
        plan,
        branding,
        ads,
        content: s(sector.content),
        seasons: s(sector.seasons),
        roadmap,
        finance: fin,
        risks,
        kpis,
        evidence,
        inputs: inp.clone(),
        brandbook: brandbook(sector, tier),
        prompt: String::new(),
    };
    st.prompt = prompt(&st);
    st
}

/// The visual identity suggested from the sector's archetype: palette (the main colour is deepened for a
/// premium price position), a Persian font pair and the tone-of-voice matrix.
fn brandbook(sector: &Sector, tier: &str) -> Brandbook {
    let roles = [("رنگ اصلی", "لوگو، دکمه‌ها، تیتر و بسته‌بندی؛ حدود ۶۰٪ فضای بصری برند"), ("رنگ مکمل", "کارت‌ها، پس‌زمینه بخش‌ها، آیکن و جزئیات بسته؛ حدود ۳۰٪"), ("رنگ زمینه", "پس‌زمینه سایت، کاتالوگ و پست‌ها؛ فضای خالی و خوانایی"), ("رنگ تأکید", "فراخوان خرید، تخفیف و نشان‌ها؛ کمتر از ۱۰٪ و فقط جای مهم")];
    let mut palette: Vec<Swatch> = roles.iter().zip(sector.palette).map(|((role, usage), hex)| Swatch { role: (*role).into(), hex: hex.into(), usage: (*usage).into() }).collect();
    if tier == "premium" {
        palette[0].hex = shade(sector.palette[0], 0.7);
        palette[0].usage.push_str("؛ برای جایگاه پریمیوم کمی تیره‌تر انتخاب شد");
    }
    let axes = [("رسمی", "صمیمی"), ("کلاسیک", "مدرن"), ("منطقی", "احساسی"), ("جدی", "بازیگوش")];
    Brandbook {
        archetype: sector.archetype.0.into(),
        palette,
        heading_font: sector.fonts.0.into(),
        body_font: sector.fonts.1.into(),
        font_note: "فقط یک جفت فونت در همه‌جا (سایت، بسته، شبکه‌ها)؛ مجوز استفاده تجاری فونت را بررسی کنید.".into(),
        tone: axes.iter().zip(sector.tone).map(|((l, r), v)| ToneAxis { left: (*l).into(), right: (*r).into(), value: v }).collect(),
    }
}

/// `#rrggbb` darkened to `k` (0–1) of its brightness.
fn shade(hex: &str, k: f64) -> String {
    let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    let ch = |shift: u32| (f64::from((n >> shift) & 0xff) * k).round() as u32;
    format!("#{:02x}{:02x}{:02x}", ch(16), ch(8), ch(0))
}

impl Inputs {
    /// The owner's answers as they may be kept: bounded text, a logo that really is a small image.
    pub fn sanitized(mut self) -> Result<Self, String> {
        let cut = |s: &str, n: usize| s.trim().chars().take(n).collect::<String>();
        self.advisor = cut(&self.advisor, 80);
        self.client = cut(&self.client, 80);
        self.audience = cut(&self.audience, 200);
        self.usp = cut(&self.usp, 400);
        self.region = cut(&self.region, 80);
        self.logo = self.logo.trim().to_string();
        if !self.logo.is_empty() && (!self.logo.starts_with("data:image/") || self.logo.len() > 300_000) {
            return Err("لوگو باید یک تصویر کوچک (کمتر از ۲۰۰ کیلوبایت) باشد.".into());
        }
        for v in [self.budget, self.price, self.unit_cost, self.fixed_cost, self.capacity] {
            if v < 0 {
                return Err("مبلغ‌ها نمی‌توانند منفی باشند.".into());
            }
        }
        Ok(self)
    }
}

/// The plan, compact, as a request to an AI assistant for long-form writing.
fn prompt(st: &Strategy) -> String {
    let mut p = format!(
        "تو یک مشاور ارشد استراتژی، برندینگ و رشد کسب‌وکار در بازار ایران هستی. برای برند «{}» (حوزه {}، مدل {}، مرحله {}، هدف {}) یک بیزینس پلن کامل و حرفه‌ای در سطح جهانی بنویس: خلاصه مدیریتی، تحلیل بازار، رقبا، بوم کسب‌وکار، مدل درآمدی، استراتژی برند، برنامه بازاریابی و تبلیغات با بودجه، نقشه راه ۱۲ ماهه، پیش‌بینی مالی و ریسک‌ها. داده‌های واقعی:\n",
        st.brand, st.sector, st.model, st.stage, st.goal
    );
    for e in &st.evidence {
        p.push_str(&format!("- {e}\n"));
    }
    for s in &st.summary {
        p.push_str(&format!("- {s}\n"));
    }
    if !st.inputs.usp.trim().is_empty() {
        p.push_str(&format!("- تمایز از نگاه صاحب برند: {}\n", st.inputs.usp.trim()));
    }
    p.push_str("پیشنهادها باید عملیاتی، قابل سنجش و متناسب با کانال‌های ایرانی (باسلام، دیجی‌کالا، ترب، اینستاگرام، تلگرام/ایتا) باشد.");
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_detection() {
        assert_eq!(sector_of("", "عسل", "داراشکوه").key, "food");
        assert_eq!(sector_of("", "مانتو", "").key, "fashion");
        assert_eq!(sector_of("", "", "شیرآلات البرز").key, "industrial");
        assert_eq!(sector_of("beauty", "عسل", "").key, "beauty", "the owner's choice wins");
        assert_eq!(sector_of("", "چیزی دیگر", "").key, "general");
        assert_eq!(sector_of("", "مانتو زنانه", "").key, "fashion", "«نان» inside «زنانه» is not bread");
    }

    #[test]
    fn unit_economics_and_projection() {
        let general = sector_of("general", "", "");
        let inp = Inputs {
            price: 500_000,
            unit_cost: 250_000,
            fixed_cost: 20_000_000,
            budget: 10_000_000,
            channel_fee_pct: Some(8.0),
            vat_pct: Some(10.0),
            pack_cost: Some(20_000),
            return_pct: Some(0.0),
            ..Default::default()
        };
        let f = finance(&inp, 0, 0, general, "startup");
        // 500k − 250k cost − 40k commission − 4k VAT on it − 20k packaging
        assert_eq!((f.channel_fee, f.vat, f.pack, f.returns), (40_000, 4_000, 20_000, 0));
        assert_eq!(f.unit_margin, 186_000);
        assert_eq!(f.break_even_units, 162); // 30M / 186k
        assert_eq!(f.months.len(), 12);
        assert!(f.months[11].units > f.months[0].units);
        assert_eq!(f.months[0].profit, f.months[0].units * 186_000 - 30_000_000);
        // returns: 5% of orders, each costs 2 × packaging + 30% of the goods, spread over the kept sales
        let r = finance(&Inputs { return_pct: Some(5.0), ..inp.clone() }, 0, 0, general, "startup");
        assert_eq!(r.returns, 6_053);
        assert_eq!(r.unit_margin, 186_000 - 6_053);
        // no price anywhere → no projection, and no division by zero
        assert!(finance(&Inputs::default(), 0, 0, general, "idea").months.is_empty());
        // market median + the sector's usual margin, fee, packaging and returns when the owner gives nothing
        let food = sector_of("food", "", "");
        let g = finance(&Inputs::default(), 400_000, 0, food, "growth");
        assert_eq!((g.price, g.unit_cost), (400_000, 260_000));
        assert_eq!((g.fee_pct, g.vat_pct, g.return_pct), (9.0, 10.0, 2.0));
        assert_eq!((g.channel_fee, g.vat, g.pack), (36_000, 3_600, 24_000));
        assert!(g.unit_margin < 400_000 - 260_000 - 36_000, "every cost reduces the margin");
        assert!(g.basis.iter().any(|b| b.contains("مرجوعی")));
    }

    #[test]
    fn brandbook_follows_the_sector_and_the_price_position() {
        let food = sector_of("food", "", "");
        let mid = brandbook(food, "mid");
        assert_eq!(mid.palette.len(), 4);
        assert!(mid.palette.iter().all(|c| c.hex.len() == 7 && c.hex.starts_with('#')));
        assert_eq!(mid.tone.len(), 4);
        assert!(mid.tone.iter().all(|t| (0..=100).contains(&t.value)));
        let premium = brandbook(food, "premium");
        assert_ne!(premium.palette[0].hex, mid.palette[0].hex, "premium deepens the main colour");
        assert_eq!(premium.palette[1].hex, mid.palette[1].hex);
        assert_eq!(shade("#ffffff", 0.5), "#808080");
        assert!(SECTORS.iter().all(|x| x.palette.iter().all(|h| u32::from_str_radix(h.trim_start_matches('#'), 16).is_ok())));
    }

    #[test]
    fn answers_are_bounded_before_they_are_kept() {
        let ok = Inputs { advisor: "  دفتر مشاوره  ".into(), logo: "data:image/png;base64,AAAA".into(), ..Default::default() }.sanitized().unwrap();
        assert_eq!(ok.advisor, "دفتر مشاوره");
        assert!(Inputs { logo: "https://evil.example/x.png".into(), ..Default::default() }.sanitized().is_err(), "only an inline image");
        assert!(Inputs { logo: format!("data:image/png;base64,{}", "A".repeat(300_001)), ..Default::default() }.sanitized().is_err());
        assert!(Inputs { budget: -1, ..Default::default() }.sanitized().is_err());
        assert_eq!(Inputs { usp: "ا".repeat(900), ..Default::default() }.sanitized().unwrap().usp.chars().count(), 400);
    }

    #[test]
    fn sector_rates_differ_like_the_market() {
        // apparel is returned far more than digital goods; channel fees stay in the 5–18% band
        let (fashion, digital) = (sector_of("fashion", "", ""), sector_of("electronics", "", ""));
        assert!(fashion.ret > digital.ret && fashion.fee > digital.fee);
        assert!(SECTORS.iter().all(|x| (5.0..=18.0).contains(&x.fee) && (2.0..=8.0).contains(&x.ret)), "rates inside the documented band");
        // the owner's numbers are bounded, not trusted blindly
        let f = finance(&Inputs { price: 100_000, unit_cost: 50_000, channel_fee_pct: Some(900.0), return_pct: Some(f64::NAN), ..Default::default() }, 0, 0, fashion, "growth");
        assert_eq!(f.fee_pct, 40.0);
        assert_eq!(f.return_pct, fashion.ret, "a non-number falls back to the sector's rate");
    }

    #[test]
    fn ad_mix_adds_up_and_follows_the_goal() {
        let sector = sector_of("food", "", "");
        let sales = ad_mix(sector.mix, "sales", "b2c_online", 10_000_000);
        assert_eq!(sales.iter().map(|c| c.share).sum::<i64>(), 100);
        let b2b = ad_mix(sector.mix, "wholesale", "b2b", 10_000_000);
        assert!(b2b.iter().any(|c| c.name.contains("سازمانی")), "wholesale brings the B2B channel in");
        assert!(b2b[0].name.contains("سازمانی"));
    }

    #[test]
    fn full_plan_for_a_small_booth_brand() {
        let mut b = Brand {
            name: "داراشکوه".into(),
            category: "عسل".into(),
            ..Default::default()
        };
        b.report = Some(BrandReport {
            store: Some(crate::brand::Store {
                products: 6,
                median: 450_000,
                min: 300_000,
                max: 700_000,
                ..Default::default()
            }),
            actions: vec!["نام برند را در عنوان محصولات بیاورید.".into()],
            ..Default::default()
        });
        let st = build(&b, &Inputs { budget: 5_000_000, ..Default::default() });
        assert_eq!(st.sector, "مواد غذایی، کشاورزی و ارگانیک");
        assert_eq!(st.canvas.len(), 9);
        assert!(st.canvas.iter().all(|c| !c.items.is_empty()), "every canvas block is filled");
        assert_eq!(st.finance.price, 450_000, "the store's own median");
        assert_eq!(st.roadmap.len(), 4);
        assert!(st.roadmap[0].tasks[0].contains("نام برند"), "the analysis's own actions come first");
        assert_eq!(st.ads.iter().map(|a| a.budget).sum::<i64>() / 100_000, 50);
        assert!(st.prompt.contains("داراشکوه") && st.prompt.contains("فروشگاه برند"));
        assert!(st.positioning[0].items[0].contains("داراشکوه"));
    }
}
