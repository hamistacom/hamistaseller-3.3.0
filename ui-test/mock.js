// Mock of window.__TAURI__ for rendering the UI in a normal browser (tests and design review only).
(function () {
  const qs = new URLSearchParams(location.search);
  const edition = qs.get("edition") || "master";
  const licValid = qs.get("lic") !== "invalid";
  const empty = qs.get("empty") === "1";
  const NAMES = ["کافه لمیز", "رستوران شب‌های تهران با منوی ویژه و فضای باز خانوادگی", "کلینیک دندانپزشکی آوا", "آرایشگاه رز", "فروشگاه موبایل سام", "املاک پارس", "باشگاه بدنسازی المپیا", "داروخانه دکتر رضایی", "آموزشگاه زبان سفیر", "تعمیرگاه خودرو مهر", "مبلمان کلاسیک نیلو", "شرکت ساختمانی آرمان سازه البرز"];
  const PEOPLE = [["علی امینی", "مدیرعامل"], ["سارا رضایی", "مدیر فروش"], ["", ""], ["محمد کریمی", "مؤسس"], ["", ""], ["نگار احمدی", "مدیر"]];
  const CITIES = [["تهران", "تهران"], ["اصفهان", "اصفهان"], ["البرز", "کرج"], ["فارس", "شیراز"]];
  const KW = ["کافه", "رستوران", "دندانپزشکی", "آرایشگاه", "املاک"];
  const ST = ["new", "new", "contacted", "interested", "customer", "rejected"];
  const pad = (n, w) => String(n).padStart(w, "0");
  const places = empty
    ? []
    : Array.from({ length: 137 }, (_, i) => {
        const [person, role] = PEOPLE[i % PEOPLE.length];
        const [province, city] = CITIES[i % CITIES.length];
        const contacts = [{ id: i * 10 + 1, kind: "mobile", value: `0912${pad(1000000 + i * 37, 7)}`, label: "", origin: "maps", delivery: i % 9 === 0 ? "sent" : "", apps: i % 4 === 0 ? "whatsapp:1,telegram:0" : i % 4 === 1 ? "whatsapp:1" : "" }];
        if (i % 3 === 0) contacts.push({ id: i * 10 + 2, kind: "mobile", value: `0935${pad(2000000 + i, 7)}`, label: "", origin: "maps", delivery: "" });
        if (i % 2 === 0) contacts.push({ id: i * 10 + 3, kind: "landline", value: `021${pad(88000000 + i, 8)}`, label: "", origin: "maps", delivery: "" });
        if (i % 4 === 0) contacts.push({ id: i * 10 + 4, kind: "email", value: `info${i}@example.ir`, label: "", origin: "website", delivery: "" });
        if (i % 3 === 1) contacts.push({ id: i * 10 + 5, kind: "instagram", value: `https://instagram.com/shop.${i}`, label: "", origin: "website", delivery: "" });
        if (i % 5 === 0) contacts.push({ id: i * 10 + 6, kind: "telegram", value: `https://t.me/shop_${i}`, label: "", origin: "website", delivery: "" });
        if (i % 7 === 0) contacts.push({ id: i * 10 + 7, kind: "eitaa", value: `https://eitaa.com/shop${i}`, label: "", origin: "website", delivery: "" });
        return {
          id: 1000 + i,
          listId: i % 5 === 0 ? 2 : 1,
          placeKey: `0x${i}:0x${i}`,
          name: NAMES[i % NAMES.length] + (i >= NAMES.length ? ` ${i}` : ""),
          contactPerson: person,
          contactRole: role,
          category: KW[i % KW.length],
          address: `${city}، خیابان ولیعصر، پلاک ${i + 3}`,
          website: i % 4 === 0 ? `https://shop${i}.ir/` : "",
          rating: 3.5 + (i % 15) / 10,
          reviews: 10 + i * 3,
          lat: 35.7,
          lng: 51.4,
          mapsUrl: "https://www.google.com/maps/place/x",
          keyword: KW[i % KW.length],
          province,
          city,
          status: ST[i % ST.length],
          note: i % 8 === 0 ? "قرار تماس دوشنبه" : "",
          tags: i % 6 === 0 ? "VIP,تماس دوم" : "",
          source: i % 11 === 0 ? "import" : "maps",
          createdAt: `2026-09-${pad(1 + (i % 29), 2)} 1${i % 10}:20:00`,
          updatedAt: "",
          contacts,
        };
      });
  const lists = [
    { id: 1, name: "بانک اصلی", color: "navy", note: "", count: places.filter((p) => p.listId === 1).length, createdAt: "2026-09-01 10:00:00" },
    { id: 2, name: "مشتریان VIP تهران", color: "orange", note: "", count: places.filter((p) => p.listId === 2).length, createdAt: "2026-09-10 10:00:00" },
  ];
  const accounts = empty ? [] : [
    { id: "sms-1", kind: "sms", name: "کاوه‌نگار", config: { preset: "kavenegar", sender: "10004346", method: "POST", url: "https://api.kavenegar.com/v1/{apikey}/sms/send.json", bodyType: "form", body: { receptor: "{to}" }, headers: [], okPath: "return.status", okValues: ["200"] }, perMinute: 60, hasSecret: true },
  ];
  const templates = [{ id: 1, channel: "sms", name: "معرفی خدمات", subject: "", body: "سلام {person} عزیز، تیم هامیستا برای {name} در {city} پیشنهاد ویژه‌ای دارد." }];
  const runs = empty ? [] : [
    { id: 3, listId: 1, listName: "بانک اصلی", keyword: "کافه", province: "تهران", city: "تهران", requested: 50, checked: 50, found: 31, status: "done", error: "", resume: "", startedAt: "2026-09-30 10:20:00", finishedAt: "" },
    { id: 2, listId: 1, listName: "بانک اصلی", keyword: "دندانپزشکی", province: "اصفهان", city: "اصفهان", requested: 50, checked: 18, found: 12, status: "cancelled", error: "", resume: '{"keywords":["دندانپزشکی"]}', startedAt: "2026-09-29 18:02:00", finishedAt: "" },
    { id: 1, listId: 2, listName: "مشتریان VIP تهران", keyword: "عسل", province: "مازندران", city: "بروجن", requested: 20, checked: 1, found: 1, status: "failed", error: "مرورگر پیدا نشد", resume: "", startedAt: "2026-09-28 04:31:00", finishedAt: "" },
  ];
  const daily = Array.from({ length: 30 }, (_, i) => [`2026-09-${pad(i + 1, 2)}`, empty ? 0 : Math.round(3 + 10 * Math.abs(Math.sin(i / 3)) + (i === 29 ? 8 : 0))]);
  const filterPlaces = (f = {}) =>
    places.filter((p) => {
      if (f.listId && p.listId !== f.listId) return false;
      if (f.status && p.status !== f.status) return false;
      if (f.keyword && p.keyword !== f.keyword) return false;
      if (f.city && p.city !== f.city) return false;
      if (f.province && p.province !== f.province) return false;
      if (f.category && p.category !== f.category) return false;
      if (f.ids?.length && !f.ids.includes(p.id)) return false;
      if (f.channel && ["mobile", "landline", "email"].includes(f.channel) && !p.contacts.some((c) => c.kind === f.channel)) return false;
      if (f.channel === "person" && !p.contactPerson) return false;
      if (f.q && !(p.name.includes(f.q) || p.contactPerson.includes(f.q) || p.contacts.some((c) => c.value.includes(f.q)))) return false;
      return true;
    });
  const license = edition === "customer" ? (licValid ? { edition, valid: true, machine: "3F25-04E0-4F89-11D3", customer: "فروشگاه نمونه", plan: "سالانه", expires: "2026-10-05", daysLeft: 4, message: "۴ روز تا پایان اعتبار لایسنس" } : { edition, valid: false, machine: "3F25-04E0-4F89-11D3", customer: "", plan: "", expires: "", daysLeft: 0, message: "برنامه فعال نشده است." }) : { edition, valid: edition === "master", machine: "3F25-04E0-4F89-11D3", customer: "", plan: "", expires: "", daysLeft: 0, message: edition === "master" ? "نسخه کامل مدیر — بدون نیاز به لایسنس" : "نسخه صدور لایسنس" };
  // online account (customer edition, ?online=1)
  if (edition === "customer" && qs.get("online") === "1") {
    Object.assign(license, { mode: "online", server: "https://api.hamista.ir", user: "cafe_lamiz", sources: ["maps"], megabank: true, scanLeft: 27, sendLeft: -1, syncedAt: Math.floor(Date.now() / 1000) - 3600, expires: "2027-03-01", daysLeft: 151, message: "" });
  } else if (edition === "customer") license.mode = "offline";
  const srvOn = qs.get("srv") === "1";
  const nowUtc = (minAgo) => new Date(Date.now() - minAgo * 60000).toISOString().slice(0, 19).replace("T", " ");
  const SOURCES = [
    { id: "maps", name: "استخراج از وب", desc: "کسب‌وکارهای دارای اطلاعات عمومی در وب، با استان، شهرستان یا منطقه و کلمه کلیدی", icon: "map", kind: "scan", ready: true, allowed: true },
    { id: "website", name: "سایت کسب‌وکار", desc: "ایمیل، شبکه‌های اجتماعی و نام مدیر از سایت خود کسب‌وکار", icon: "globe", kind: "enrich", ready: true, allowed: true },
    { id: "import", name: "ورود فایل", desc: "فهرست مشتریان خودتان از Excel یا CSV", icon: "upload", kind: "import", ready: true, allowed: true },
  ];

  // ---- product & price intelligence / brands (mock) ----
  const mkOffers = [
    ["torob", "عسل گون خوانسار ۱ کیلویی", "خوانسار", 590000, "عسل‌سرای خوانسار", 4.6, 320, 9],
    ["digikala", "عسل طبیعی سبلان ۵۰۰ گرمی", "سبلان", 310000, "دیجی‌کالا", 4.3, 210, 0],
    ["basalam", "عسل چهل گیاه زنبوردار ۱ کیلویی", "", 480000, "زنبورستان دماوند", 4.9, 62, 0],
    ["torob", "عسل کنار شهدینه ۹۶۰ گرمی", "شهدینه", 700000, "شهد ناب", 4.1, 45, 4],
    ["digikala", "عسل خوانسار ۹۶۰ گرمی", "خوانسار", 640000, "هایپر سلامت", 4.5, 80, 0],
    ["emalls", "عسل آویشن طبیعت ۵۰۰ گرمی", "طبیعت", 290000, "فروشگاه طبیعت", 4.0, 12, 0],
    ["basalam", "عسل وحشی کوهستان ۱ کیلویی", "", 820000, "کندوی سبلان", 4.7, 30, 0],
    ["torob", "عسل اورازان ۱ کیلویی", "اورازان", 560000, "اورازان", 4.2, 150, 6],
  ].map(([platform, title, brand, price, seller, rating, reviews, shops], i) => ({ platform, title, brand, price, seller, rating, reviews, shops, url: `https://torob.com/p/${i}/`, image: i % 3 === 2 ? "" : "img/m-honey-c.webp", updated: nowUtc([90, 600, 3000, 20, 7000, 150, 40, 1200][i]), sales: [120, 0, 35, 0, 64, 0, 8, 0][i], location: platform === "basalam" ? "دماوند" : "", unitPrice: Math.round((price * 1000) / (title.includes("۵۰۰") ? 500 : title.includes("۹۶۰") ? 960 : 1000)), value: [88, 81, 79, 52, 70, 74, 48, 77][i] }));
  const mkReport = (query = "عسل", brand = "") => ({
    id: 7, query, brand, createdAt: nowUtc(30), targetMargin: 25,
    stats: { count: 8, min: 560000, max: 820000, median: 620000, p25: 580000, p75: 700000, weighted: 628000, outliers: 1, basis: "per_kg" },
    srp: 700000,
    offers: mkOffers.filter((o) => !brand || o.brand === brand).sort((a, b) => b.value - a.value),
    brands: [{ name: "خوانسار", count: 2, pct: 25, median: 640000 }, { name: "سایر / بدون برند", count: 2, pct: 25, median: 650000 }, { name: "سبلان", count: 1, pct: 12.5, median: 620000 }, { name: "شهدینه", count: 1, pct: 12.5, median: 729000 }, { name: "طبیعت", count: 1, pct: 12.5, median: 580000 }, { name: "اورازان", count: 1, pct: 12.5, median: 560000 }],
    channels: [{ name: "ترب", count: 3, pct: 37.5, median: 590000 }, { name: "دیجی‌کالا", count: 2, pct: 25, median: 646000 }, { name: "باسلام", count: 2, pct: 25, median: 650000 }, { name: "ایمالز", count: 1, pct: 12.5, median: 580000 }],
    tiers: [{ name: "اقتصادی", count: 2, pct: 25, median: 560000 }, { name: "رقابتی", count: 2, pct: 25, median: 580000 }, { name: "میان‌رده", count: 2, pct: 25, median: 620000 }, { name: "پریمیوم", count: 2, pct: 25, median: 700000 }],
    subcategories: ["عسل طبیعی", "عسل گون", "عسل کنار", "عسل چهل گیاه", "عسل آویشن", "ژل رویال"],
    brandChips: ["خوانسار", "سبلان", "شهدینه", "طبیعت", "اورازان", "گل‌نوش"],
    advice: { buyFrom: "عسل‌سرای خوانسار · ترب", buyPrice: 590000, buyUrl: "https://torob.com/p/0/", sellOn: "باسلام", sellPrice: 700000, marginPct: 18.6, marketMarginPct: 5.1, notes: ["حاشیه سود ۲۵٪ در این بازار جا نمی‌شود؛ قیمت پیشنهادی روی سقف رقابتی بازار گذاشته شد.", "۱ قیمت غیرعادی از محاسبه میانگین کنار گذاشته شد.", "قیمت‌ها بر اساس هر کیلوگرم (یا لیتر) مقایسه شده‌اند."] },
    history: [["2026-09-20", 600000, 540000], ["2026-09-27", 610000, 550000], ["2026-10-04", 615000, 552000], ["2026-10-11", 620000, 560000]],
    platforms: [["torob", 3, ""], ["digikala", 2, ""], ["basalam", 2, ""], ["emalls", 1, ""]],
    brandList: [["اورازان", 1, ["torob"], 560000], ["خوانسار", 2, ["torob", "digikala"], 640000], ["سبلان", 1, ["digikala"], 620000], ["شهدینه", 1, ["torob"], 729000], ["طبیعت", 1, ["emalls"], 580000], ["گل‌نوش", 0, [], 0], ["پگاه", 0, [], 0]].map(([name, count, platforms, median]) => ({ name, count, platforms, median, min: median })),
    related: ["عسل طبیعی کوهستان", "عسل آویشن ارگانیک", "عسل خوانسار اصل"],
    dropped: 3, outOfRange: 2,
    groups: [{ title: "عسل خوانسار ۱ کیلویی", brand: "خوانسار", image: "img/m-honey-c.webp", offers: [0, 4], platforms: ["torob", "digikala"], min: 590000, median: 615000, max: 640000, spreadPct: 8.5 }],
    sourceHistory: [["2026-09-20", "ترب", 580000], ["2026-09-27", "ترب", 585000], ["2026-10-11", "ترب", 590000], ["2026-09-20", "دیجی‌کالا", 630000], ["2026-10-04", "دیجی‌کالا", 640000], ["2026-10-11", "دیجی‌کالا", 646000]],
    req: { query, brand, limit: 60, sort: "relevance", exact: true },
  });
  const mkBrands = empty ? [] : [
    { id: 1, name: "خوانسار", own: true, website: "khansar.example", instagram: "khansar_honey", category: "عسل", note: "", analyzedAt: nowUtc(120), report: { presence: 62, quality: 88, digital: 80, social: 71, overall: 75, market: { offers: 2, sharePct: 25, platforms: ["torob", "digikala"], median: 615000, marketMedian: 620000, priceIndex: 104, tier: "رقابتی", rating: 4.6, reviews: 400, searches: 3 }, site: { ok: true, https: true, ms: 900, kb: 120, title: "عسل خوانسار", description: "عسل طبیعی", h1: true, schema: false, contact: true, socials: ["instagram"] }, insta: { handle: "khansar_honey", followers: 48000, posts: 610 }, strengths: ["رضایت بالای خریداران: میانگین ۴٫۶ از ۵ (۴۰۰ نظر)."], weaknesses: [], actions: ["برای دیده شدن در گوگل: توضیحات متا و داده ساخت‌یافته (Organization / Product) به صفحه اول اضافه کنید."] } },
    { id: 2, name: "شهدینه", own: false, website: "shahdineh.example", instagram: "", category: "عسل", note: "", analyzedAt: nowUtc(200), report: { presence: 40, quality: 79, digital: 65, social: null, overall: 61, market: { offers: 1, sharePct: 12.5, platforms: ["torob"], median: 729000, marketMedian: 620000, priceIndex: 118, tier: "میان‌رده رو به بالا", rating: 4.1, reviews: 45, searches: 3 }, site: { ok: true, https: true, ms: 2100, title: "شهدینه", description: "", h1: true, schema: false, contact: false, socials: [] }, insta: { handle: "" }, strengths: [], weaknesses: ["قیمت ۱۸٪ بالاتر از میانه بازار، بدون امتیاز کیفیت بالاتر برای توجیه آن."], actions: ["یا ارزش پریمیوم را قابل دیدن کنید یا قیمت را به محدوده رقابتی نزدیک کنید."] } },
    { id: 3, name: "طبیعت", own: false, website: "", instagram: "", category: "عسل", note: "", analyzedAt: "", report: null },
  ];
  if (mkBrands[0]) Object.assign(mkBrands[0].report, {
    foundSite: true, foundInstagram: false, sellers: 6, priceMin: 560000, priceMax: 690000, lines: ["عسل گون", "عسل کنار", "عسل آویشن"],
    channels: [{ platform: "torob", offers: 5, sellers: 4, min: 560000, median: 600000, max: 690000 }, { platform: "digikala", offers: 3, sellers: 2, min: 610000, median: 640000, max: 660000 }],
    products: mkOffers.filter((o) => o.brand === "خوانسار"),
    topSellers: [["عسل‌سرای خوانسار", 3], ["هایپر سلامت", 2]],
    competitors: [{ name: "سبلان", count: 6, pct: 18, median: 600000 }, { name: "شهدینه", count: 4, pct: 12, median: 729000 }],
    mix: [{ key: "product", score: 72, notes: ["خطوط محصول دیده‌شده: عسل گون، عسل کنار، عسل آویشن.", "کیفیت درک‌شده از نگاه خریدار: ۴٫۶ از ۵ (۴۰۰ نظر)."] }, { key: "price", score: 81, notes: ["شاخص قیمت ۱۰۴ (۱۰۰ = میانه بازار) — جایگاه «رقابتی»."] }, { key: "place", score: 58, notes: ["۲ کانال فروش، ۶ فروشنده مختلف."] }, { key: "promotion", score: 76, notes: ["سایت رسمی فعال است.", "اینستاگرام @khansar_honey: ۴۸۰۰۰ دنبال‌کننده."] }],
    opportunities: ["«باسلام» ۲۵٪ از عرضه این بازار را دارد و برند شما آنجا نیست."],
    threats: ["«سبلان» با ۱۸٪ از عرضه‌ها رهبر این بازار است."],
  });
  const megaRows = Array.from({ length: 50 }, (_, i) => ({ id: i + 1, name: NAMES[i % NAMES.length], mobiles: `0912${pad(3000000 + i, 7)}${i % 3 ? "" : `,0935${pad(4000000 + i, 7)}`}`, landlines: i % 2 ? `021${pad(77000000 + i, 8)}` : "", emails: i % 4 ? "" : `info${i}@shop.ir`, website: i % 4 ? "" : `https://shop${i}.ir`, instagram: "", telegram: "", whatsapp: "", eitaa: "", bale: "", rubika: "", address: "", province: CITIES[i % 4][0], city: CITIES[i % 4][1], category: KW[i % KW.length], source: "maps", reportCount: 1 + (i % 6), lastSeen: nowUtc(i * 90) }));
  const admCustomers = empty ? [] : [
    { id: 1, name: "فروشگاه نمونه", phone: "09121234567", email: "", machine: "3F25-04E0-4F89-11D3", plan: "سالانه", expires: "2026-10-09", status: "active", note: "", cloudUser: "", createdAt: "2026-01-10 10:00:00", updatedAt: "", licenses: 2, daysLeft: 8 },
    { id: 2, name: "کلینیک دندانپزشکی آوا با نام طولانی برای آزمون چیدمان", phone: "", email: "ava@clinic.ir", machine: "A1B2-C3D4-E5F6-0718", plan: "شش‌ماهه", expires: "2027-02-01", status: "active", note: "", cloudUser: "ava_clinic", createdAt: "2026-05-01 10:00:00", updatedAt: "", licenses: 1, daysLeft: 123 },
    { id: 3, name: "املاک پارس", phone: "02188000000", email: "", machine: "1111-2222-3333-4444", plan: "ماهانه", expires: "2026-09-20", status: "active", note: "", cloudUser: "", createdAt: "2026-08-20 10:00:00", updatedAt: "", licenses: 1, daysLeft: -11 },
    { id: 4, name: "آموزشگاه سفیر", phone: "", email: "", machine: "", plan: "", expires: "", status: "paused", note: "منتظر پرداخت", cloudUser: "", createdAt: "2026-09-25 10:00:00", updatedAt: "", licenses: 0, daysLeft: null },
  ];
  const plans = [
    { id: 1, name: "ماهانه پایه", maxDevices: 1, scanQuota: 30, sendQuota: 2000, sources: "maps", megabank: false, note: "", users: 14 },
    { id: 2, name: "سالانه طلایی", maxDevices: 3, scanQuota: 0, sendQuota: 0, sources: "", megabank: true, note: "همه امکانات", users: 6 },
  ];
  const users = [
    { id: 11, username: "cafe_lamiz", customerName: "کافه لمیز", status: "active", expires: "2027-03-01", note: "", createdAt: nowUtc(60 * 24 * 30), plan: "سالانه طلایی", planId: 2, devices: 2 },
    { id: 12, username: "ava_clinic", customerName: "کلینیک آوا", status: "suspended", expires: "2026-10-04", note: "", createdAt: nowUtc(60 * 24 * 90), plan: "ماهانه پایه", planId: 1, devices: 1 },
  ];
  const ADMIN = {
    "GET /overview": () => ({ users: 20, activeUsers: 18, devices: 27, megabank: 15873, loginsToday: 9, newMegaToday: 312, publicKey: "8f2d", serverVersion: "1.0.0" }),
    "GET /users": () => ({ items: users }),
    "GET /users/11/devices": () => ({ items: [{ id: 1, machineId: "9A3C-11D2-7E4F-0B6A", name: "LAPTOP-ALI", appVersion: "2.0.0", lastIp: "5.120.1.2", revoked: false, firstSeen: nowUtc(5000), lastSeen: nowUtc(30) }] }),
    "GET /plans": () => ({ items: plans }),
    "POST /megabank/search": (b) => ({ rows: megaRows.slice(b.offset || 0, (b.offset || 0) + (b.limit || 50)), total: 15873 }),
    "POST /megabank/facets": () => ({ provinces: [["تهران", 6200], ["اصفهان", 2100], ["فارس", 1500], ["البرز", 900]], cities: [["تهران", 5800], ["اصفهان", 1900], ["شیراز", 1300], ["کرج", 850]], categories: KW.map((k, i) => [k, 3000 - i * 400]), sources: [["maps", 15873]] }),
    "GET /notifications": () => ({ items: [{ id: 1, title: "نسخه ۲.۱ منتشر شد", body: "", tag: "system", audience: "all", createdAt: nowUtc(200) }] }),
    "GET /version": () => ({ version: "3.3.0", url: "", notes: "", mandatory: false }),
    "GET /scenarios": () => ({ items: [] }),
    "GET /optouts": () => ({ items: ["09120000000"] }),
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  window.__calls = [];
  async function scan(args) {
    const ch = args.onEvent;
    const send = (e) => ch.onmessage && ch.onmessage(e);
    send({ type: "status", message: "در حال آماده‌سازی مرورگر…" });
    for (let i = 1; i <= 12; i++) {
      await sleep(qs.get("fast") ? 5 : 120);
      send({ type: "progress", keyword: "کافه", collected: i + 3, checked: i, found: Math.floor(i * 0.7), target: 50 });
      send({ type: "log", level: i % 4 ? "success" : "muted", message: `${NAMES[i % NAMES.length]} — 0912${pad(1000000 + i, 7)} · ایمیل` });
      if (i % 2) send({ type: "found", place: places[i] });
      if (i === 6 && qs.get("captcha")) send({ type: "captcha", message: "گوگل تأیید «ربات نیستم» خواست. یک پنجره مرورگر باز شد؛ تأیید را در آن انجام دهید." });
    }
    if (qs.get("hold")) await new Promise(() => {});
    return { checked: 12, found: 8, cancelled: false, error: false, message: "پایان استخراج. ۱۲ مکان بررسی و ۸ مخاطب جدید ذخیره شد." };
  }
  async function campaign(args) {
    if (args.req.testTo) return { checked: 0, found: 0, cancelled: false, error: false, message: `پیام آزمایشی به ${args.req.testTo} ارسال شد.` };
    const ch = args.onEvent;
    for (let i = 1; i <= 10; i++) {
      await sleep(80);
      ch.onmessage && ch.onmessage({ type: "sending", sent: i, failed: i > 7 ? 1 : 0, skipped: 1, total: 20 });
    }
    if (qs.get("hold")) await new Promise(() => {});
    return { checked: 12, found: 10, cancelled: false, error: false, message: "کمپین تمام شد. ۱۰ ارسال موفق." };
  }
  const H = {
    app_info: () => ({ version: "3.3.0", edition, dataDir: "D:\\Apps\\HamistaSeller-Data", license, migrated: null, defaultServer: "" }),
    sources_list: () => SOURCES.map((x, i) => ({ ...x, enabled: !(qs.get("off") || "").split(",").includes(x.id), switchable: x.id === "website", places: [82, 0, 10, 24, 9, 0, 12][i], recent: [31, 0, 0, 18, 6, 0, 9][i] })),
    source_toggle: () => null,
    schedules: () => (empty ? [] : [
      { id: 1, name: "کافه‌های تهران — هر شب", req: { listId: 1, keywords: ["کافه", "کافی‌شاپ"], province: "تهران", city: "تهران", limit: 40, sources: ["maps", "neshan"] }, every: "daily", at: "02:30", weekday: 6, onDate: "", enabled: true, nextRun: "2026-10-02 02:30", lastRun: "2026-10-01 02:30", lastResult: "پایان استخراج از همه منابع. ۷۲ مورد بررسی و ۳۱ مخاطب جدید — گوگل‌مپ: ۲۲ مخاطب جدید، نقشه نشان: ۹ مخاطب جدید", createdAt: "2026-09-20 10:00:00" },
      { id: 2, name: "لوازم خانگی شیراز", req: { listId: 2, keywords: ["لوازم خانگی"], province: "فارس", city: "شیراز", limit: 30, source: "maps" }, every: "weekly", at: "09:00", weekday: 6, onDate: "", enabled: false, nextRun: "", lastRun: "", lastResult: "", createdAt: "2026-09-25 10:00:00" },
    ]),
    schedule_save: () => 3,
    schedule_toggle: () => null,
    schedule_delete: () => null,
    background_get: () => ({ tray: true, autostart: false }),
    background_set: (a) => a.prefs,
    support_report: () => "C:\\Users\\me\\Downloads\\HamistaSeller\\hamista-support-20261001-120000.txt",
    cloud_trial_info: () => ({ enabled: true, days: 3 }),
    cloud_trial: () => license,
    source_config_save: (a) => Object.fromEntries(Object.keys(a.values).map((k) => [k, a.values[k] !== null])),
    cloud_notifications: () => ({ items: [{ id: 2, title: "به‌روزرسانی سناریوی گوگل‌مپ", body: "استخراج‌ها با ظاهر جدید گوگل‌مپ هماهنگ شد.", tag: "system", createdAt: nowUtc(40), read: false }, { id: 1, title: "خوش آمدید", body: "", tag: "system", createdAt: nowUtc(3000), read: true }] }),
    cloud_notification_read: () => null,
    cloud_version: () => ({ version: "2.1.0", url: "https://hamista.ir/download", notes: "بانک جامع سریع‌تر شد.", mandatory: false }),
    cloud_login: () => license,
    cloud_sync: () => license,
    cloud_logout: () => license,
    mega_search: (a) => ({ rows: megaRows.slice(a.search.offset, a.search.offset + a.search.limit), total: 1240 }),
    mega_import: (a) => ({ places: a.rows.length, contacts: a.rows.length * 2 }),
    admin_stats: () => ({ customers: admCustomers.length, active: 2, expiring: 1, expired: 1, licences: 4, licencesMonth: 2 }),
    admin_customers: (a) => admCustomers.filter((c) => (a.view === "expiring" ? c.daysLeft != null && c.daysLeft >= 0 && c.daysLeft <= 14 : a.view === "expired" ? c.daysLeft != null && c.daysLeft < 0 : a.view === "paused" ? c.status === "paused" : true) && (!a.q || c.name.includes(a.q) || c.machine.includes(a.q))),
    admin_licences: () => [{ id: 2, customerId: 1, customer: "فروشگاه نمونه", machine: "3F25-04E0-4F89-11D3", plan: "سالانه", expires: "2026-10-09", code: "HS2.x.y", issuedAt: "2025-10-09 10:00:00" }],
    admin_issue: () => ({ code: "HS2.SFMyfDNGMjUtMDRFMC00Rjg5LTExRDN8MjAyNy0wOS0zMHwyMDI2LTEwLTAx.dGVzdHNpZ25hdHVyZQ", customerId: 1 }),
    admin_customer_save: () => 5,
    admin_customer_delete: () => null,
    admin_export: () => "C:\\Users\\me\\Downloads\\HamistaAdmin\\customers.csv",
    admin_server: () => (srvOn ? { url: "https://api.hamista.ir", user: "admin", connected: true, keyMatches: qs.get("match") !== "0", serverPublicKey: "", serverVersion: "1.0.0", overview: ADMIN["GET /overview"]() } : { url: "", user: "", connected: false, keyMatches: null, serverPublicKey: "", serverVersion: "", overview: null }),
    admin_connect: () => ({ url: "https://api.hamista.ir", user: "admin", connected: true, keyMatches: true, serverVersion: "1.0.0", overview: null }),
    admin_api: (a) => {
      const key = `${a.method} ${a.path.split("?")[0]}`;
      return ADMIN[key] ? ADMIN[key](a.body || {}) : { ok: true, id: 9, version: 1 };
    },
    license_status: () => license,
    lists: () => lists,
    job_current: () => null,
    jobs_list: () => [],
    jobs_clear: () => null,
    market_rfq: () => "با سلام و احترام،\n...",
    dashboard: () => ({ total: places.length, mobile: 137, landline: 69, email: 35, social: 58, website: 35, person: 91, today: 8, week: 41, prevWeek: 33, contacted: 69, interested: 23, customers: 11, interestedWaiting: empty ? 0 : 6, sentWeek: 120, sentPrevWeek: 96, month: 98, prevMonth: 71, contacts: 412, daily, weekly: Array.from({ length: 12 }, (_, i) => [`2026-${pad(7 + Math.floor(i / 4), 2)}-${pad(1 + (i % 4) * 7, 2)}`, empty ? 0 : [12, 18, 9, 22, 30, 26, 19, 34, 41, 28, 37, 46][i]]), sources: [["maps", 82, 31], ["neshan", 24, 18], ["telegram", 12, 9], ["balad", 9, 6], ["import", 10, 0]], topKeywords: empty ? [] : [["کافه", 41], ["رستوران", 30], ["دندانپزشکی", 26], ["آرایشگاه", 22], ["املاک", 18]], topCities: empty ? [] : [["تهران", 52], ["اصفهان", 35], ["کرج", 28], ["شیراز", 22]] }),
    places: (a) => {
      const rows = filterPlaces(a.filter);
      return { rows: rows.slice(a.offset, a.offset + a.limit), total: rows.length };
    },
    place: (a) => places.find((p) => p.id === a.id),
    bank_stats: (a) => {
      const rows = filterPlaces(a.filter);
      const count = (fn) => Object.entries(rows.reduce((m, p) => { const k = fn(p); if (k) m[k] = (m[k] || 0) + 1; return m; }, {})).sort((x, y) => y[1] - x[1]);
      const cities = count((p) => `${p.province}|${p.city}`).map(([k, n]) => [...k.split("|"), n]);
      const kinds = ["mobile", "landline", "email", "instagram", "telegram", "eitaa"].map((k) => [k, rows.filter((p) => p.contacts.some((c) => c.kind === k)).length]).filter((x) => x[1]);
      return { total: rows.length, provinces: count((p) => p.province), cities, categories: count((p) => p.category), keywords: count((p) => p.keyword), statuses: count((p) => p.status), channels: [...kinds, ["website", rows.filter((p) => p.website).length], ["person", rows.filter((p) => p.contactPerson).length]] };
    },
    facets: () => ({ keywords: KW.map((k) => [k, 27]), categories: KW.map((k) => [k, 27]), provinces: CITIES.map((c) => [c[0], 30]), cities: CITIES.map((c) => [c[1], 30]), tags: [["VIP", 22], ["تماس دوم", 22]] }),
    runs: () => runs,
    activity: () => (empty ? [] : [{ at: "2026-10-01 09:10:00", kind: "scan", text: "استخراج «کافه»: ۳۱ مخاطب جدید" }, { at: "2026-09-30 18:00:00", kind: "campaign", text: "کمپین «کاوه‌نگار»: ۱۲۰ ارسال موفق از ۱۲۴" }, { at: "2026-09-30 12:00:00", kind: "license", text: "لایسنس برای «فروشگاه نمونه» تا 2027-09-30 صادر شد" }, { at: "2026-09-29 08:00:00", kind: "backup", text: "پشتیبان دستی گرفته شد" }]),
    settings_get: () => ({ concurrency: 3, headless: true }),
    settings_save: (a) => a.patch,
    browser_detect: () => "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    accounts: () => accounts,
    templates: () => templates,
    optouts: () => ["09120000000", "spam@example.com"],
    optouts_save: () => 2,
    backups: () => [{ name: "auto-20261001-0910.db", size: 2400000 }, { name: "manual-20260930-1800.db", size: 2300000 }],
    campaigns: () => (empty ? [] : [{ id: 1, channel: "sms", listName: "بانک اصلی", accountName: "کاوه‌نگار", subject: "", message: "سلام {person}…", total: 124, sent: 120, failed: 3, skipped: 1, status: "done", error: "", createdAt: "2026-09-30 18:00:00" }]),
    template_preview: (a) => a.text.replaceAll("{name}", places[0].name).replaceAll("{person}", places[0].contactPerson).replaceAll("{city}", places[0].city),
    scan_start: scan,
    scan_resume: scan,
    campaign_run: campaign,
    job_stop: () => true,
    list_create: () => 3,
    license_key_info: () => ({ publicKey: "8f2d4c1a9b7e6f5d3c2b1a0f9e8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3a2f1e0d", embeddedMatches: qs.get("match") === "1" }),
    license_issue: () => "HS2.SFMyfDNGMjUtMDRFMC00Rjg5LTExRDN8MjAyNy0wOS0zMHwyMDI2LTEwLTAxfNiz2KfZhNin2YbZh3zZgdix2YjYtNqv2KfZhw.dGVzdHNpZ25hdHVyZXRlc3RzaWduYXR1cmV0ZXN0c2lnbmF0dXJldGVzdHNpZ25hdHVyZQ",
    license_export_key: () => "0".repeat(64),
    logs_folder: () => "D:\\Apps\\HamistaSeller-Data\\logs",
    export_data: () => "C:\\Users\\me\\Downloads\\HamistaSeller\\hamista-20261001.xlsx",
    import_data: () => ({ added: 42, duplicates: 5, skipped: 2, truncated: 0, columns: ["نام", "موبایل", "ایمیل"], ignored: ["تاریخ"] }),
    assist_open: (a) => a.messenger === "whatsapp",
    assist_session: () => null,
    contact_app: () => null,
    place_photo: () => "",
    market_search: async (a) => {
      await sleep(300);
      return mkReport(a.req.query, a.req.brand);
    },
    market_runs: () => (empty ? [] : [[7, "عسل", "", 8, 620000, nowUtc(30)], [6, "زعفران سرگل", "", 14, 1450000, nowUtc(3000)]]),
    market_get: () => mkReport(),
    market_trends: () => (empty ? [] : [{ query: "عسل", searches: 4, firstMedian: 600000, lastMedian: 620000, lastAt: nowUtc(30), lastId: 7, channels: [{ name: "ترب", median: 590000 }, { name: "دیجی‌کالا", median: 646000 }] }, { query: "زعفران سرگل", searches: 2, firstMedian: 1500000, lastMedian: 1450000, lastAt: nowUtc(3000), lastId: 6, channels: [] }]),
    watch_list: () => (empty ? [] : [{ id: 1, name: "عسل", req: { query: "عسل", platforms: ["torob"] }, ownPrice: 650000, alertPct: 5, enabled: true, lastRun: nowUtc(60), lastId: 7, median: 620000, min: 560000, basis: "per_kg", alert: "میانه قیمت ۵٪ بالا رفت", error: "", history: [["2026-09-20", 600000, 540000], ["2026-09-27", 610000, 550000], ["2026-10-04", 615000, 552000], ["2026-10-11", 620000, 560000]], swing: { points: 4, changePct: 1.6, change7: 1.6, change30: 3.3, low: 540000, high: 620000, volatility: 1.2, direction: "up" } }, { id: 2, name: "زعفران سرگل", req: { query: "زعفران سرگل", platforms: [] }, ownPrice: 0, alertPct: 5, enabled: true, lastRun: "", lastId: 0, median: 0, min: 0, basis: "unit", alert: "", error: "", history: [], swing: { points: 0, changePct: 0, change7: 0, change30: 0, low: 0, high: 0, volatility: 0, direction: "flat" } }]),
    watch_time: () => "09:00",
    watch_save: () => 1,
    watch_delete: () => null,
    watch_run: async () => ({ message: "۲ کالا بررسی شد", error: false }),
    catalog_list: () => (empty ? [] : [{ id: 1, name: "عسل خوانسار ۱ کیلویی", sku: "H-1", brand: "خوانسار", ownPrice: 650000, cost: 480000, matched: 3, confidence: "high", min: 590000, median: 615000, max: 640000, srp: 630000, position: 82, matches: [{ platform: "torob", title: "عسل گون خوانسار ۱ کیلویی", price: 590000, seller: "عسل‌سرای خوانسار", url: "https://torob.com/p/0/" }], runId: 7, note: "قیمت شما ۶٪ بالاتر از میانه است.", checkedAt: nowUtc(60), query: "عسل خوانسار ۱ کیلویی" }, { id: 2, name: "زعفران نیم مثقال", sku: "", brand: "", ownPrice: 0, cost: 0, matched: 0, confidence: "", min: 0, median: 0, max: 0, srp: 0, position: -1, matches: [], runId: 0, note: "", checkedAt: "", query: "زعفران مثقال" }]),
    catalog_import: () => ({ added: 2, updated: 0, skipped: 0, columns: ["نام", "قیمت"] }),
    catalog_delete: () => null,
    catalog_check: async () => ({ message: "۲ کالا بررسی شد" }),
    market_login: () => null,
    brand_runs: () => (empty ? [] : [[3, 1, "خوانسار", 75, nowUtc(120)], [2, 2, "شهدینه", 61, nowUtc(200)], [1, 1, "خوانسار", 70, nowUtc(9000)]]),
    brand_run_get: () => mkBrands[0]?.report,
    city_photo: () => "",
    market_delete: () => null,
    brands: () => mkBrands,
    brand_save: (a) => a.brand.id || 9,
    brand_delete: () => null,
    brand_analyze: (a) => mkBrands.find((b) => b.id === a.id)?.report || mkBrands[0].report,
    place_save: () => ({ id: 5000, duplicates: [] }),
    places_status: (a) => a.ids.length,
    places_tag: (a) => a.ids.length,
    places_delete: (a) => a.ids.length,
    transfer: (a) => a.ids.length,
    template_save: () => 2,
    license_activate: () => ({ ...license, valid: true, customer: "فروشگاه نمونه", plan: "سالانه", expires: "2027-09-30", daysLeft: 364, message: "" }),
  };
  window.__TAURI__ = {
    core: {
      Channel: class {
        constructor() {
          this.onmessage = null;
        }
      },
      invoke: async (cmd, args = {}) => {
        window.__calls.push([cmd, args]);
        await sleep(5);
        if (!(cmd in H)) return null;
        const r = H[cmd](args);
        return r instanceof Promise ? await r : JSON.parse(JSON.stringify(r ?? null));
      },
    },
  };
})();
