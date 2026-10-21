// Page registry («ماژول‌ها»). Adding a page later = one entry here + its file in pages/.
//   editions: which builds show it (customer / master / issuer)
//   when(info): extra condition from app_info (licence, plan…) — re-checked when the licence changes
//   group: sidebar section heading · badge: small label next to the menu item
//   render: optional export name when one file holds several pages (default "render")
const USER = ["customer", "master"];
const load = (file, fn = "render") => () => import(`./pages/${file}.js`).then((m) => ({ ...m, render: m[fn] }));

export const PAGES = [
  // ---------- seller app (customer / master) ----------
  { id: "dashboard", space: "crm", group: "نمای کلی", title: "داشبورد", sub: "نمای کلی بانک مشتریان و فعالیت‌ها", icon: "dashboard", editions: USER, load: load("dashboard") },
  { id: "scan", space: "crm", group: "نمای کلی", title: "استخراج", sub: "جمع‌آوری کسب‌وکارها از منابع عمومی", icon: "scan", editions: USER, load: load("scan") },
  { id: "schedule", space: "crm", group: "نمای کلی", title: "زمان‌بندی", sub: "استخراج خودکار در روز و ساعت مشخص، و اجرا در پس‌زمینه", icon: "clock", editions: USER, load: load("schedule") },
  { id: "contacts", space: "crm", group: "نمای کلی", title: "بانک‌ها و مخاطبان", sub: "جستجو، ویرایش، برچسب و خروجی", icon: "bank", editions: USER, load: load("contacts") },
  {
    id: "megabank",
    space: "crm",
    group: "نمای کلی",
    title: "بانک جامع",
    badge: "جدید",
    sub: "جستجو در بانک مشترک کسب‌وکارها و افزودن به بانک خودتان",
    icon: "database",
    editions: ["customer"],
    when: (i) => i.license?.valid && i.license?.megabank,
    load: load("megabank"),
  },
  { id: "outreach", space: "crm", group: "ارتباط با مشتری", title: "کمپین‌ها", sub: "یک پیام برای گروهی از مخاطبان — پیامک خودکار یا پیام‌رسان با حساب خودتان", icon: "send", editions: USER, load: load("outreach") },
  { id: "assist", space: "crm", group: "ارتباط با مشتری", title: "دستیار گفتگو", sub: "واتساپ، تلگرام، ایتا و بله با حساب خودتان، داخل برنامه", icon: "chat", editions: USER, load: load("assist") },
  // ---------- product & price intelligence ----------
  { id: "market", space: "market", group: "تحلیل محصول", title: "تحلیل محصول و قیمت", sub: "برای فروشنده: قیمت روز، رقبا و قیمت پیشنهادی · برای خریدار: بهترین جای خرید · برندها، کانال‌های فروش و روند قیمت", icon: "trend", editions: USER, load: load("market") },
  { id: "watch", space: "market", group: "تحلیل محصول", title: "پایش روزانه قیمت", sub: "کالاهای مهم هر روز خودکار تحلیل می‌شوند: نوسان، روند ۷ و ۳۰ روزه، نمودار هر منبع و هشدار تغییر قیمت", icon: "bell", editions: USER, load: load("watch") },
  { id: "catalog", space: "market", group: "تحلیل محصول", title: "کالاهای من", sub: "فهرست کالاهای خودتان از اکسل؛ هر کالا دقیق در بازار شناسایی و قیمتش با بازار مقایسه می‌شود", icon: "store", editions: USER, load: load("watch", "renderCatalog") },
  { id: "plist", space: "market", group: "تحلیل محصول", title: "دریافت لیست محصول", badge: "جدید", sub: "لیست کامل محصولات هر سایت، غرفه باسلام یا کانال تلگرام با عکس، لینک، قیمت و جزئیات — با خروجی اکسل", icon: "download", editions: USER, load: load("plist") },
  { id: "market-runs", space: "market", group: "تحلیل محصول", title: "تحلیل‌های قبلی", sub: "همه تحلیل‌های محصول و روند قیمت هر کالا", icon: "history", editions: USER, load: load("market", "renderRuns") },
  // ---------- brand analysis ----------
  { id: "brands", space: "brand", group: "تحلیل برند", title: "برند ما و رقبا", sub: "سلامت برند، آمیخته بازاریابی (4P)، SWOT، کانال‌های فروش، رقبا و اقدام‌های اجرایی", icon: "award", editions: USER, load: load("brands") },
  { id: "ai", group: "پشتیبانی", title: "هوش مصنوعی", badge: "جدید", sub: "پرسش از هوش مصنوعی گوگل (Gemini) با خلاصه تحلیل‌های شما — بیرون از برنامه، در مرورگر خودتان", icon: "sparkle", editions: USER, load: load("ai") },
  { id: "history", group: "پشتیبانی", title: "تاریخچه", sub: "استخراج‌ها، تحلیل‌های محصول و برند، کمپین‌ها و رویدادها", icon: "history", editions: USER, load: load("history") },
  { id: "help", group: "پشتیبانی", title: "راهنما", sub: "شروع سریع، راهنمای بخش‌ها، خطاهای رایج و پشتیبانی", icon: "book", editions: USER, load: load("help") },
  { id: "settings", group: "پشتیبانی", title: "تنظیمات", sub: "حساب‌های ارسال، قالب‌ها، ماژول‌ها، پشتیبان و لایسنس", icon: "settings", editions: USER, load: load("settings") },

  // ---------- admin panel (issuer edition): works offline; server pages need «اتصال سرور» ----------
  { id: "home", group: "دفتر مشتریان", title: "داشبورد مدیر", sub: "مشتریان، لایسنس‌ها و وضعیت سرور", icon: "dashboard", editions: ["issuer"], load: load("admin-home") },
  { id: "customers", group: "دفتر مشتریان", title: "مشتریان", sub: "دفتر مشتریان و لایسنس‌های آفلاین — بدون نیاز به سرور", icon: "users", editions: ["issuer"], load: load("admin-customers") },
  { id: "issuer", group: "دفتر مشتریان", title: "صدور لایسنس", sub: "کد لایسنس آفلاین برای سیستم مشتری", icon: "key", editions: ["issuer"], load: load("issuer") },
  { id: "accounts", group: "سرور", title: "کاربران آنلاین", sub: "حساب‌ها، اشتراک‌ها و دستگاه‌های مشتریان روی سرور", icon: "cpu", editions: ["issuer"], cloud: true, load: load("admin-cloud", "renderUsers") },
  { id: "plans", group: "سرور", title: "پلن‌ها", sub: "سقف دستگاه، سهمیه ماهانه، منابع و بانک جامع", icon: "layers", editions: ["issuer"], cloud: true, load: load("admin-cloud", "renderPlans") },
  { id: "mega", group: "سرور", title: "ابربانک", sub: "داده عمومی کسب‌وکارها از همه مشتریان — تفکیک و خروجی", icon: "database", editions: ["issuer"], cloud: true, load: load("admin-cloud", "renderMega") },
  { id: "notify", group: "سرور", title: "اعلان و به‌روزرسانی", sub: "پیام به مشتریان، نسخه جدید، سناریوی استخراج، لیست عدم ارسال", icon: "bell", editions: ["issuer"], cloud: true, load: load("admin-cloud", "renderNotify") },
  { id: "logs", group: "سرور", title: "گزارش فعالیت", sub: "ورودها، استخراج‌ها، کمپین‌ها و ارسال به ابربانک", icon: "history", editions: ["issuer"], cloud: true, load: load("admin-cloud", "renderLogs") },
  { id: "server", group: "تنظیمات", title: "اتصال سرور", sub: "اتصال پنل به سرور و راهنمای راه‌اندازی", icon: "server", editions: ["issuer"], load: load("admin-server") },
  { id: "keys", group: "تنظیمات", title: "کلید و پشتیبان", sub: "کلید امضای لایسنس و پشتیبان‌گیری", icon: "shield", editions: ["issuer"], load: load("issuer", "renderKeys") },
];

/** The three workspaces of the seller app (sidebar switcher). Pages without `space` show in all. */
export const SPACES = [
  { id: "crm", title: "اطلاعات مشتریان", short: "مشتریان", sub: "استخراج، بانک مشتریان و بازاریابی", icon: "users", home: "dashboard" },
  { id: "market", title: "تحلیل محصول", short: "محصول", sub: "قیمت روز، رقبا، بهترین جای خرید", icon: "trend", home: "market" },
  { id: "brand", title: "تحلیل برند", short: "برند", sub: "برند ما و رقبا", icon: "award", home: "brands" },
];

/** {id: page} for this build and licence, in menu order. */
export function routesFor(info) {
  return Object.fromEntries(PAGES.filter((p) => p.editions.includes(info.edition) && (!p.when || p.when(info))).map((p) => [p.id, p]));
}
