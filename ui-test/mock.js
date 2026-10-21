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
  ].map(([platform, title, brand, price, seller, rating, reviews, shops], i) => ({ platform, title, brand, price, seller, rating, reviews, shops, url: `https://torob.com/p/${i}/`, image: i % 3 === 2 ? "" : "img/m-honey-c.webp", updated: nowUtc([90, 600, 3000, 20, 7000, 150, 40, 1200][i]), sales: [120, 0, 35, 0, 64, 0, 8, 0][i], location: platform === "basalam" ? "دماوند" : "", unitPrice: Math.round((price * 1000) / (title.includes("۵۰۰") ? 500 : title.includes("۹۶۰") ? 960 : 1000)), value: [88, 81, 79, 52, 70, 74, 48, 77][i], link: i % 4 === 3 ? "unverified" : "exact", sellerUrl: platform === "basalam" ? `https://basalam.com/shop${i}` : "" }));
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
    opportunity: { score: 78, level: "golden", demand: 0.82, lowCompetition: 0.7, qualityGap: 0.6, spread: 0.9, reasons: ["تقاضا بالاست: ۴٬۲۰۰ فروش و نظر ثبت شده.", "رقابت کم: فقط ۶ فروشنده.", "رضایت پایین (میانگین امتیاز ۳٫۵): محصول بهتر می‌تواند سهم بگیرد.", "بازه قیمت پهن است؛ برای جایگاه‌یابی (اقتصادی یا پریمیوم) جا دارید."], gold: [1, 3] },
    groups: [{ title: "عسل خوانسار ۱ کیلویی", brand: "خوانسار", image: "img/m-honey-c.webp", offers: [0, 4], platforms: ["torob", "digikala"], min: 590000, median: 615000, max: 640000, spreadPct: 8.5 }],
    web: [{ title: "عسل طبیعی خوانسار", domain: "asalkhoonsar.ir", url: "https://asalkhoonsar.ir/p/1", productUrl: "https://asalkhoonsar.ir/p/1", productName: "عسل طبیعی خوانسار ۱ کیلویی", brand: "خوانسار", price: 610000, availability: "موجود", pageType: "تولیدکننده", relevance: 100, confidence: 90, fetchedAt: nowUtc(10) }, { title: "فروشگاه عسل", domain: "shop.example.ir", url: "https://shop.example.ir/x", productUrl: "", productName: "", brand: "", price: 0, availability: "", pageType: "خرده‌فروش", relevance: 70, confidence: 40, fetchedAt: nowUtc(10) }],
    suppliers: [{ name: "صنایع عسل خوانسار", kind: "تولیدکننده", category: "عسل", city: "خوانسار", website: "https://asalkhoonsar.ir", url: "https://asalkhoonsar.ir", phone: "03157234567", email: "", moq: "دارد (در صفحه)", terms: ["قیمت همکاری", "فاکتور رسمی", "انبار"], confidence: 78 }, { name: "پخش عسل سبلان", kind: "پخش‌کننده", category: "عسل", city: "اردبیل", website: "https://sabalan.example", url: "https://sabalan.example", phone: "", email: "info@sabalan.example", moq: "", terms: ["عمده"], confidence: 52 }],
    sourceHistory: [["2026-09-20", "ترب", 580000], ["2026-09-27", "ترب", 585000], ["2026-10-11", "ترب", 590000], ["2026-09-20", "دیجی‌کالا", 630000], ["2026-10-04", "دیجی‌کالا", 640000], ["2026-10-11", "دیجی‌کالا", 646000]],
    req: { query, brand, limit: 60, sort: "relevance", exact: true },
  });
  const mkBrands = empty ? [] : [
    { id: 1, name: "خوانسار", own: true, website: "khansar.example", instagram: "khansar_honey", category: "عسل", note: "", plan: { budget: 20000000, goal: "sales" }, analyzedAt: nowUtc(120), report: { presence: 62, quality: 88, digital: 80, social: 71, overall: 75, market: { offers: 2, sharePct: 25, platforms: ["torob", "digikala"], median: 615000, marketMedian: 620000, priceIndex: 104, tier: "رقابتی", rating: 4.6, reviews: 400, searches: 3 }, site: { ok: true, https: true, ms: 900, kb: 120, title: "عسل خوانسار", description: "عسل طبیعی", h1: true, schema: false, contact: true, socials: ["instagram"] }, insta: { handle: "khansar_honey", followers: 48000, posts: 610 }, strengths: ["رضایت بالای خریداران: میانگین ۴٫۶ از ۵ (۴۰۰ نظر)."], weaknesses: [], actions: ["برای دیده شدن در گوگل: توضیحات متا و داده ساخت‌یافته (Organization / Product) به صفحه اول اضافه کنید."], confidence: 72, signals: [{ key: "coverage", label: "پوشش منابع", score: 70 }, { key: "sample", label: "حجم داده", score: 65 }, { key: "entity", label: "تطبیق هویت برند", score: 85 }, { key: "freshness", label: "تازگی داده", score: 85 }, { key: "digital", label: "حضور دیجیتال", score: 80 }], evidence: [{ fact: "میانه قیمت برند: ۶۱۵۰۰۰ تومان", source: "بازار فروشگاه‌های اینترنتی", url: "" }, { fact: "سایت رسمی فعال", source: "وب", url: "" }], candidates: [{ name: "غرفه خوانسار", kind: "غرفه / فروشنده", detail: "باسلام", url: "https://basalam.com/khansar", score: 100 }] } },
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
  let feeOv = {};
  let alertCfg = { telegramChat: "", hasToken: false, smsAccount: "", smsTo: "" };
  let fxRows = [{ date: "2026-10-11", usdToman: 105000, gold18Toman: 650000 }, { date: "2026-09-20", usdToman: 90000, gold18Toman: 600000 }];
  const logs = {};
  let logSeq = 0;
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
    watch_list: () => (empty ? [] : [{ id: 1, name: "عسل", req: { query: "عسل", platforms: ["torob"] }, ownPrice: 650000, alertPct: 5, enabled: true, lastRun: nowUtc(60), lastId: 7, median: 620000, min: 560000, basis: "per_kg", alert: "میانه قیمت ۵٪ بالا رفت", error: "", mySellerName: "هامیستا", myProductUrl: "", cost: 480000, minMarginPct: 10, alertChannel: "telegram", buybox: { status: "lost", myPrice: 650000, winnerSeller: "عسل‌سرای خوانسار", winnerPrice: 590000, winnerPlatform: "digikala", floor: 560000, suggested: 588000, advice: "برای گرفتن بای‌باکس قیمت را روی ۵۸۸٬۰۰۰ تومان بگذارید." }, history: [["2026-09-20", 600000, 540000], ["2026-09-27", 610000, 550000], ["2026-10-04", 615000, 552000], ["2026-10-11", 620000, 560000]], swing: { points: 4, changePct: 1.6, change7: 1.6, change30: 3.3, low: 540000, high: 620000, volatility: 1.2, direction: "up" } }, { id: 2, name: "زعفران سرگل", req: { query: "زعفران سرگل", platforms: [] }, ownPrice: 0, alertPct: 5, enabled: true, lastRun: "", lastId: 0, median: 0, min: 0, basis: "unit", alert: "", error: "", history: [], swing: { points: 0, changePct: 0, change7: 0, change30: 0, low: 0, high: 0, volatility: 0, direction: "flat" } }]),
    watch_time: () => "09:00",
    watch_save: () => 1,
    watch_delete: () => null,
    watch_run: async () => ({ message: "۲ کالا بررسی شد", error: false }),
    catalog_list: () => (empty ? [] : [{ id: 1, name: "عسل خوانسار ۱ کیلویی", sku: "H-1", brand: "خوانسار", ownPrice: 650000, cost: 480000, matched: 3, confidence: "high", min: 590000, median: 615000, max: 640000, srp: 630000, position: 82, matches: [{ platform: "torob", title: "عسل گون خوانسار ۱ کیلویی", price: 590000, seller: "عسل‌سرای خوانسار", url: "https://torob.com/p/0/" }], runId: 7, note: "قیمت شما ۶٪ بالاتر از میانه است.", checkedAt: nowUtc(60), query: "عسل خوانسار ۱ کیلویی", mySellerName: "هامیستا", myProductUrl: "", minMarginPct: 10, alertChannel: "", buybox: { status: "lost", myPrice: 650000, winnerSeller: "رقیب", winnerPrice: 400000, winnerPlatform: "digikala", floor: 520000, suggested: 0, advice: "رقابت زیان‌ده — کاهش قیمت توصیه نمی‌شود" } }, { id: 2, name: "زعفران نیم مثقال", sku: "", brand: "", ownPrice: 0, cost: 0, matched: 0, confidence: "", min: 0, median: 0, max: 0, srp: 0, position: -1, matches: [], runId: 0, note: "", checkedAt: "", query: "زعفران مثقال" }]),
    catalog_import: () => ({ added: 2, updated: 0, skipped: 0, columns: ["نام", "قیمت"] }),
    catalog_delete: () => null,
    catalog_check: async () => ({ message: "۲ کالا بررسی شد" }),
    market_login: () => null,
    wait_skip: () => null,
    product_list: async (a) => {
      await sleep(200);
      const items = [
        { title: "عسل گون طبیعی یک کیلویی", price: 450000, image: "", url: "https://basalam.com/asal_shop/product/1", seller: "عسل شاپ", sellerUrl: "https://basalam.com/asal_shop", brand: "عسل شاپ", rating: 4.6, reviews: 38, sales: 120, date: "2026-10-01 10:00:00", location: "اصفهان", availability: "موجود", description: "", views: 0, link: "", source: "basalam.com" },
        { title: "عسل کنار نیم کیلویی", price: 220000, image: "", url: "https://basalam.com/asal_shop/product/2", seller: "عسل شاپ", sellerUrl: "https://basalam.com/asal_shop", brand: "", rating: null, reviews: 0, sales: 0, date: "", location: "", availability: "", description: "", views: 0, link: "", source: "basalam.com" },
      ];
      return { url: a.req.url, scope: "فقط محصولات غرفه «asal_shop» در باسلام (نه کل باسلام)", items, pages: 2, stats: { count: 2, priced: 2, min: 220000, median: 335000, max: 450000, avg: 335000, sellers: [["عسل شاپ", 2]], brands: [["عسل شاپ", 1]], newest: "2026-10-01 10:00:00", oldest: "2026-10-01 10:00:00", perWeek: 0 }, notes: ["۳ مورد خارج از محدوده لینک کنار گذاشته شد."] };
    },
    product_list_export: () => "C:\\Users\\me\\Downloads\\HamistaSeller\\products-basalam-com.xlsx",
    brand_runs: () => (empty ? [] : [[3, 1, "خوانسار", 75, nowUtc(120)], [2, 2, "شهدینه", 61, nowUtc(200)], [1, 1, "خوانسار", 70, nowUtc(9000)]]),
    brand_run_get: () => mkBrands[0]?.report,
    city_photo: () => "",
    market_delete: () => null,
    brands: () => mkBrands,
    brand_save: (a) => a.brand.id || 9,
    brand_strategy: (a) => ({ ...MOCK_STRATEGY, inputs: a.inputs }),
    brand_delete: () => null,
    brand_analyze: (a) => mkBrands.find((b) => b.id === a.id)?.report || mkBrands[0].report,
    place_save: () => ({ id: 5000, duplicates: [] }),
    places_status: (a) => a.ids.length,
    places_tag: (a) => a.ids.length,
    places_delete: (a) => a.ids.length,
    transfer: (a) => a.ids.length,
    template_save: () => 2,
    // ---- 3.3 extras ----
    profit_compare: (a) => {
      if (a.setRates) feeOv = { ...a.setRates };
      const T = { digikala: ["دیجی‌کالا", 8], basalam: ["باسلام", 6], snappshop: ["اسنپ‌شاپ", 9], torob: ["ترب (تبلیغ کلیکی)", 0], site: ["فروش مستقیم (سایت اختصاصی)", 1.5] };
      const i = a.input;
      const rows = Object.entries(T).map(([id, [name, usual]]) => {
        const own = i.overrides?.[id] ?? feeOv[id];
        const pct = own ?? usual;
        const commission = Math.round((i.price * pct) / 100);
        const vat = Math.round((commission * (i.vatPct ?? 10)) / 100);
        const net = i.price - i.cost - commission - vat - (i.packCost || 0) - (i.adCost || 0);
        return { platform: id, name, group: "food", commissionPct: pct, estimated: own === undefined, price: i.price, commission, vat, pack: i.packCost || 0, ad: i.adCost || 0, net, marginPct: i.price ? Math.round((net * 1000) / i.price) / 10 : 0, floorPrice: Math.round((i.cost + (i.packCost || 0)) / (1 - (pct / 100) * 1.1)) };
      });
      rows.sort((x, y) => y.net - x.net);
      return { rows, group: "food" };
    },
    alert_config: (a) => {
      if (a.config) alertCfg = { ...alertCfg, ...a.config, hasToken: alertCfg.hasToken || !!a.token };
      return { ...alertCfg, smsAccounts: [["sms-1", "کاوه‌نگار"]] };
    },
    alert_test: () => null,
    catalog_track: () => null,
    catalog_add: (a) => ({ added: a.items.length, updated: 0, skipped: 0, columns: [] }),
    export_woocommerce_csv: () => "C:\\Users\\me\\Downloads\\HamistaSeller\\woocommerce.csv",
    export_basalam_xlsx: () => "C:\\Users\\me\\Downloads\\HamistaSeller\\basalam.xlsx",
    fx_rates: (a) => {
      if (a.set) fxRows = [...fxRows.filter((r) => r.date !== a.set.date), a.set].sort((x, y) => y.date.localeCompare(x.date));
      return fxRows;
    },
    fx_adjust: (a) => {
      const rates = [...fxRows].reverse();
      const at = (d, k) => (rates.filter((r) => r.date <= d && r[k] > 0).pop() || rates.find((r) => r[k] > 0) || {})[k] || 0;
      const usd = a.points.map((p) => (at(p[0], "usdToman") ? p[1] / at(p[0], "usdToman") : 0));
      const gold = a.points.map((p) => (at(p[0], "gold18Toman") ? p[1] / at(p[0], "gold18Toman") : 0));
      const ch = (v) => (v[0] > 0 && v[v.length - 1] > 0 ? Math.round((v[v.length - 1] / v[0] - 1) * 1000) / 10 : null);
      const nom = a.points.length ? ch(a.points.map((p) => p[1])) : null;
      return { usd, gold, nominalPct: nom, usdPct: ch(usd), goldPct: ch(gold), rateDays: fxRows.length, latest: fxRows[0] || null, fetchError: "" };
    },
    place_logs: (a) => (logs[a.id] || []).slice().reverse(),
    place_log_add: (a) => {
      (logs[a.id] ||= []).push({ id: ++logSeq, placeId: a.id, kind: a.kind, note: a.note, createdAt: "2026-10-21 10:00:00" });
      if (a.next != null) { const p = places.find((x) => x.id === a.id); if (p) p.followUpAt = a.next; }
      return logSeq;
    },
    place_log_delete: (a) => { for (const k of Object.keys(logs)) logs[k] = logs[k].filter((l) => l.id !== a.id); return null; },
    follow_up_set: (a) => { const p = places.find((x) => x.id === a.id); if (p) p.followUpAt = a.at; return null; },
    follow_ups_due: () => (empty ? [] : [{ id: 1000, name: places[0].name, person: "علی امینی", phone: "09121000000", status: "contacted", followUpAt: "2026-10-19 10:00", overdueDays: 2, last: "قول داد فاکتور را بفرستد" }, { id: 1001, name: places[1].name, person: "", phone: "09121000037", status: "new", followUpAt: "2026-10-21 09:00", overdueDays: 0, last: "" }]),
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

// a real `strategy::build` output (src-tauri strategy.rs test brand)
var MOCK_STRATEGY = {"brand": "داراشکوه", "sector": "مواد غذایی، کشاورزی و ارگانیک", "model": "فروش آنلاین به مصرف‌کننده (بازارگاه + شبکه اجتماعی)", "stage": "نوپا (کمتر از ۲ سال)", "goal": "افزایش فروش", "summary": ["«داراشکوه» در حوزه مواد غذایی، کشاورزی و ارگانیک با مدل «فروش آنلاین به مصرف‌کننده (بازارگاه + شبکه اجتماعی)» و در مرحله «نوپا (کمتر از ۲ سال)» فعالیت می‌کند. هدف این برنامه افزایش فروش است؛ مخاطب اصلی «خانواده‌های سلامت‌محور شهری (۲۵ تا ۵۵ سال)» در سراسر ایران.", "جایگاه قیمتی: میان‌رده (هم‌سطح بازار). تمایز باید از راه کیفیت، خدمات و داستان برند ساخته شود، نه جنگ قیمت.", "اقتصاد واحد: قیمت ۴۵۰٬۰۰۰ تومان، سود هر واحد پس از بهای تمام‌شده و هزینه کانال ۱۲۱٬۵۰۰ تومان (۲۷٪). نقطه سربه‌سر ماهانه با بودجه تبلیغات و هزینه ثابت: ۴۲ واحد.", "سه اقدام فوری بر پایه تحلیل: نام برند را در عنوان محصولات بیاورید."], "positioning": [{"key": "statement", "title": "بیانیه جایگاه‌یابی", "items": ["برای خانواده‌های سلامت‌محور شهری (۲۵ تا ۵۵ سال) که به دنبال عسل مطمئن هستند، «داراشکوه» برندی است که کیفیت قابل اتکا با قیمت منصفانه؛ برخلاف عرضه‌های بی‌نام و نشان بازار، منشأ، کیفیت و پاسخ‌گویی خود را شفاف نشان می‌دهد."]}, {"key": "value", "title": "ارزش پیشنهادی", "items": ["کارکردی: عسل با کیفیت ثابت و مشخصات شفاف", "احساسی: آرامش خیال از انتخاب درست و اصیل", "اجتماعی: هدیه‌ای که به خریدار اعتبار می‌دهد", "کیفیت قابل اتکا با قیمت منصفانه"]}, {"key": "archetype", "title": "کهن‌الگوی برند", "items": ["مراقب (Caregiver)", "خوراکی سالم یعنی مراقبت از خانواده؛ اعتماد، اصالت و گرما محور پیام است."]}, {"key": "tone", "title": "لحن و صدای برند", "items": ["گرم، حرفه‌ای و قابل اعتماد", "آموزشی و پاسخ‌گو", "طراحی تمیز با یک رنگ شاخص ثابت"]}, {"key": "taglines", "title": "ایده شعار (برای آزمون با مشتری)", "items": ["داراشکوه؛ اصالتی که می‌شود چشید و دید.", "داراشکوه، انتخاب مطمئن عسل.", "از مبدأ تا خانه شما؛ داراشکوه."]}], "canvas": [{"key": "partners", "title": "شرکای کلیدی", "items": ["زنبوردار/کشاورز طرف قرارداد و تأمین پایدار مواد اولیه", "آزمایشگاه معتمد برای آزمون کیفیت (مثلاً آزمون ساکارز/HMF عسل)", "چاپخانه و تأمین‌کننده بسته‌بندی بهداشتی", "شرکت‌های پست و پیک سرد/گرم", "پخش‌کننده‌ها و عمده‌فروش‌ها"]}, {"key": "activities", "title": "فعالیت‌های کلیدی", "items": ["تأمین پایدار و کنترل کیفیت هر سری", "تولید محتوا و مدیریت شبکه‌های اجتماعی", "مدیریت غرفه‌ها و موجودی در کانال‌ها", "خدمات مشتری، پیگیری پس از فروش و جمع‌آوری نظر", "تحلیل قیمت رقبا و پایش روزانه بازار"]}, {"key": "resources", "title": "منابع کلیدی", "items": ["منبع تأمین قابل ردیابی و داستان مبدأ", "مجوزهای بهداشتی و استاندارد", "برگه آزمایش کیفیت", "انبار با دما و رطوبت کنترل‌شده", "نام و هویت برند «داراشکوه»", "داده مشتریان و بازار (هامیستا سلر)"]}, {"key": "value", "title": "ارزش پیشنهادی", "items": ["کیفیت قابل اتکا با قیمت منصفانه", "عسل اصیل با کیفیت ثابت", "خرید آسان و پاسخ‌گویی سریع"]}, {"key": "relations", "title": "ارتباط با مشتری", "items": ["پاسخ‌گویی سریع در دایرکت/پیام‌رسان (زیر ۱ ساعت در ساعات کاری)", "پیگیری بعد از تحویل و درخواست نظر", "باشگاه مشتریان: تخفیف خرید دوم و معرفی دوست", "جامعه وفادار در کانال تلگرام/ایتا"]}, {"key": "channels", "title": "کانال‌ها", "items": ["غرفه باسلام و فروشگاه دیجی‌کالا", "سایت اختصاصی با اینماد", "اینستاگرام و فروش دایرکت", "سوپرمارکت‌های آنلاین (اسنپ‌مارکت، اکالا)", "پخش‌کننده‌های منطقه‌ای و بازار عمده", "هدایای سازمانی (B2B)"]}, {"key": "segments", "title": "بخش‌های مشتری", "items": ["خانواده‌های سلامت‌محور شهری (۲۵ تا ۵۵ سال)", "خریداران هدیه مناسبتی (یلدا، نوروز، سازمانی)", "فروشگاه‌های ارگانیک و هایپرمارکت‌ها", "کافه‌ها، شیرینی‌فروشی‌ها و رستوران‌ها (خرید عمده)"]}, {"key": "costs", "title": "ساختار هزینه", "items": ["بهای تمام‌شده هر واحد: ۲۹۲٬۵۰۰ تومان", "تبلیغات: ۵٬۰۰۰٬۰۰۰ تومان در ماه", "هزینه‌های ثابت (اجاره، حقوق، ابزارها)", "کمیسیون بازارگاه‌ها، درگاه پرداخت، بسته‌بندی و ارسال"]}, {"key": "revenue", "title": "جریان‌های درآمدی", "items": ["فروش تکی آنلاین (بازارگاه و سایت)", "بسته‌های هدیه و ترکیبی با حاشیه بالاتر", "اشتراک/خرید دوره‌ای محصولات مصرفی", "فروش عمده و سازمانی (هدایای مناسبتی)"]}], "businessModel": [{"key": "type", "title": "نوع مدل", "items": ["فروش آنلاین به مصرف‌کننده (بازارگاه + شبکه اجتماعی)", "موتور رشد پیشنهادی: بازارگاه‌ها برای فروش امروز + سایت و جامعه برای سود و وفاداری فردا"]}, {"key": "revenue", "title": "جریان‌های درآمدی", "items": ["فروش تکی آنلاین (بازارگاه و سایت)", "بسته‌های هدیه و ترکیبی با حاشیه بالاتر", "اشتراک/خرید دوره‌ای محصولات مصرفی", "فروش عمده و سازمانی (هدایای مناسبتی)"]}, {"key": "pricing", "title": "استراتژی قیمت", "items": ["قیمت نزدیک میانه بازار با تمایز در کیفیت و خدمت", "قیمت‌گذاری پلکانی اندازه‌ها (کوچک/متوسط/خانواده)", "تخفیف فقط هدفمند: خرید دوم، مناسبت‌ها، باشگاه مشتریان"]}, {"key": "moat", "title": "مزیت پایدار و دفاع در برابر رقبا", "items": ["داستان و منشأ اختصاصی که قابل کپی نیست", "نظرهای واقعی و امتیاز بالا در کانال‌ها (سرمایه اعتماد)", "داده و رابطه مستقیم با مشتری (باشگاه، کانال، شماره‌ها)", "علامت تجاری ثبت‌شده و بسته‌بندی شناخته‌شده"]}], "plan": [{"key": "market", "title": "تحلیل بازار و مخاطب", "items": ["حوزه: مواد غذایی، کشاورزی و ارگانیک؛ مخاطب: خانواده‌های سلامت‌محور شهری (۲۵ تا ۵۵ سال)؛ منطقه: سراسر ایران."]}, {"key": "competition", "title": "رقبا و تهدیدها", "items": ["رقبا هنوز سنجیده نشده‌اند: «دسته کالا» را در فرم برند بنویسید و دوباره تحلیل کنید تا سهم و قیمت رقبا بیاید."]}, {"key": "operations", "title": "عملیات و زنجیره تأمین", "items": ["کنترل کیفیت هر سری تولید با چک‌لیست و نمونه‌برداری", "ظرفیت ماهانه تولید/تأمین را مشخص کنید تا رشد با کمبود موجودی متوقف نشود.", "ارسال ۲۴ تا ۴۸ ساعته با بسته‌بندی ایمن و کارت تشکر داخل بسته", "موجودی یکپارچه برای همه کانال‌ها (جلوگیری از فروش بیش از موجودی)"]}, {"key": "licences", "title": "مجوزها و الزامات", "items": ["پروانه ساخت/بهره‌برداری و پروانه بهداشتی (سازمان غذا و دارو)", "کد IRC روی بسته", "نشان استاندارد ملی (در صورت اجباری بودن)", "اینماد برای سایت", "گواهی حلال/ارگانیک برای صادرات"]}, {"key": "team", "title": "تیم و نقش‌ها", "items": ["مالک: محصول، تأمین و فروش", "تولید محتوا (پاره‌وقت یا برون‌سپاری)", "پاسخ‌گویی و ارسال (پاره‌وقت)"]}], "branding": [{"key": "identity", "title": "هویت بصری و کلامی", "items": ["لوگو ساده و خوانا در اندازه کوچک (آیکون پیج و برچسب)", "یک رنگ شاخص + یک رنگ مکمل، ثابت در همه جا", "فونت فارسی حرفه‌ای و یکسان (مثل یکان‌بخ/ایران‌سنس)", "ثبت علامت تجاری در اداره مالکیت معنوی", "نام برند را در عنوان همه محصولات بیاورید (الان در کمتر از نیمی از عنوان‌هاست)."]}, {"key": "packaging", "title": "بسته‌بندی و تجربه باز کردن بسته", "items": ["بسته‌بندی متناسب با جایگاه قیمتی (پریمیوم: جعبه/شیشه و برچسب برجسته)", "کارت تشکر با QR به کانال و درخواست نظر", "اطلاعات شفاف: مبدأ، تاریخ تولید، مجوزها و روش مصرف", "نسخه هدیه برای مناسبت‌ها"]}, {"key": "digital", "title": "حضور دیجیتال", "items": ["سایت اختصاصی با دامنه برند، صفحه «درباره ما» با داستان و اینماد", "پیج اینستاگرام با همین نام، بیو با لینک خرید و هایلایت‌های اعتماد", "عکس محصول یکدست (زمینه، نور، زاویه) در همه کانال‌ها", "یک صفحه فرود برای کمپین‌ها با کد تخفیف"]}, {"key": "trust", "title": "نشانه‌های اعتماد", "items": ["پروانه ساخت/بهره‌برداری و پروانه بهداشتی (سازمان غذا و دارو)", "کد IRC روی بسته", "نشان استاندارد ملی (در صورت اجباری بودن)", "اینماد برای سایت", "گواهی حلال/ارگانیک برای صادرات", "نمایش نظر و امتیاز مشتریان واقعی", "ضمانت بازگشت وجه/تعویض روشن"]}, {"key": "story", "title": "داستان برند", "items": ["از کجا شروع شد، چرا «داراشکوه» و چه قولی به مشتری می‌دهد؛ در یک بند کوتاه، ثابت در سایت، بیو و بسته.", "چهره بنیان‌گذار یا تولیدکننده را نشان دهید؛ برند بی‌چهره اعتماد کمتری می‌سازد."]}], "ads": [{"name": "اینستاگرام (محتوا + تبلیغ در پیج‌ها)", "share": 26, "budget": 1300000, "why": "بیشترین زمان حضور خریدار ایرانی؛ جای ساختن اعتماد با تصویر، ویدیو و نظر مشتری.", "kpi": "نرخ تعامل بالای ۳٪، دایرکت خرید هفتگی، هزینه هر سفارش", "actions": ["هفته‌ای ۳ ریلز + استوری روزانه طبق تقویم محتوا", "هایلایت‌های ثابت: محصولات، نظر مشتری، نحوه خرید، مجوزها", "لینک خرید (غرفه/سایت) در بیو با UTM برای سنجش"]}, {"name": "تبلیغات داخل بازارگاه‌ها (دیجی‌کالا، باسلام، ترب)", "share": 25, "budget": 1250000, "why": "خریدار آماده خرید آنجاست؛ دیده‌شدن در صفحه اول جستجو مستقیماً فروش می‌آورد.", "kpi": "رتبه در جستجوی کلمه اصلی، ROAS بالای ۳", "actions": ["عنوان‌ها با الگوی «نوع + ویژگی + اندازه + برند» و عکس اول با زمینه سفید", "تبلیغ کالای پرفروش در جستجوی کلمه اصلی دسته", "ثبت در ترب برای مقایسه قیمت و بازدید رایگان"]}, {"name": "اینفلوئنسر و تولیدکننده محتوا", "share": 18, "budget": 900000, "why": "اعتماد قرض‌گرفته؛ برای برند تازه سریع‌ترین راه دیده‌شدن.", "kpi": "کد تخفیف اختصاصی هر اینفلوئنسر و فروش هر کد", "actions": ["۵ تا ۱۰ میکرواینفلوئنسر (۱۰ تا ۱۰۰ هزار) مرتبط با حوزه به‌جای یک چهره گران", "ارسال محصول برای تجربه واقعی قبل از قرارداد", "پرداخت ترکیبی: ثابت کم + درصد فروش کد اختصاصی"]}, {"name": "باشگاه مشتریان و خرید تکراری", "share": 15, "budget": 750000, "why": "جذب مشتری تازه ۵ برابر گران‌تر از نگه‌داشتن مشتری فعلی است.", "kpi": "نرخ خرید دوباره بالای ۲۵٪، تعداد نظر ثبت‌شده", "actions": ["پیام پیگیری بعد از تحویل و درخواست نظر", "تخفیف خرید دوم و برنامه معرفی دوست (هر دو طرف هدیه)", "بسته اشتراکی/دوره‌ای برای محصولات مصرفی"]}, {"name": "سئو و تبلیغات جستجو (گوگل، یکتانت)", "share": 9, "budget": 450000, "why": "خریدار با نیت مشخص جستجو می‌کند؛ ماندگارترین ورودی ارزان در بلندمدت.", "kpi": "کلیک ارگانیک ماهانه، رتبه ۱۰ کلمه اصلی، هزینه هر سفارش از تبلیغ کلیکی", "actions": ["صفحه هر محصول با توضیح کامل، سؤالات متداول و داده ساخت‌یافته (Schema)", "۴ مقاله راهنمای خرید در ماه برای کلمات پرجستجو", "ثبت در گوگل‌مپ و بلد/نشان با عکس و ساعت کاری"]}, {"name": "تلگرام، ایتا، بله و پیامک هدفمند", "share": 7, "budget": 350000, "why": "ارزان، مستقیم و قابل تکرار؛ برای اطلاع‌رسانی تخفیف و محصول جدید به مخاطب گرم.", "kpi": "نرخ کلیک پیامک بالای ۲٪، رشد اعضای کانال", "actions": ["کانال رسمی با محتوای انحصاری (کد تخفیف، پیش‌فروش)", "پیامک فقط به مشتریان قبلی و با لینک لغو عضویت", "تبلیغ در کانال‌های هم‌حوزه با کد تخفیف اختصاصی"]}], "content": ["ویدیوی کوتاه از مبدأ (مزرعه/کندو) و فرایند برداشت", "تست کیفیت جلوی دوربین و نشان دادن برگه آزمایش", "دستور پخت و کاربرد روزمره محصول", "مقایسه اصل و تقلبی (آموزشی، بدون تخریب رقیب)", "نظر و ویدیوی مشتری واقعی"], "seasons": ["یلدا (آذر): هدیه و بسته‌های ویژه", "نوروز (اسفند): پرفروش‌ترین ماه؛ بسته هدیه و سفارش سازمانی", "رمضان: محصولات سحری و افطاری", "شروع پاییز و زمستان: تقویت ایمنی و محصولات گرم", "محرم: سفارش نذری عمده"], "roadmap": [{"title": "پایه‌گذاری", "window": "روز ۱ تا ۳۰", "goal": "برند آماده دیده‌شدن و فروش باشد", "tasks": ["نام برند را در عنوان محصولات بیاورید.", "راه‌اندازی سایت ساده با اینماد (یا دست‌کم صفحه فروشگاه رسمی)", "تکمیل هویت: لوگو، رنگ، قالب عکس و بیو در همه کانال‌ها", "تقویم محتوای ۳۰ روزه و ۹ پست پایه (معرفی، داستان، اعتماد)"], "kpis": ["عنوان و عکس همه محصولات اصلاح‌شده", "۳۰ پست/استوری منتشرشده", "۱۰ نظر تازه از مشتریان"]}, {"title": "جذب و آزمون", "window": "ماه ۲ و ۳", "goal": "کانال و پیام سودآور پیدا شود", "tasks": ["آزمون تبلیغات با ۵٬۰۰۰٬۰۰۰ تومان در کانال‌های پیشنهادی (هر کانال جدا با کد/لینک قابل سنجش)", "همکاری با ۵ میکرواینفلوئنسر و سنجش فروش هر کد", "ورود به یک کانال فروش تازه که رقبا در آن کم‌اند", "راه‌اندازی باشگاه مشتریان و پیام پیگیری بعد از خرید"], "kpis": ["هزینه هر سفارش (CAC) کمتر از ۳۰٪ سود هر سفارش", "نرخ تبدیل غرفه/سایت بالای ۲٪", "رسیدن به نقطه سربه‌سر ماهانه"]}, {"title": "رشد", "window": "ماه ۴ تا ۶", "goal": "مقیاس دادن به کانال‌های سودآور", "tasks": ["دو برابر کردن بودجه کانال برنده و قطع کانال‌های زیان‌ده", "گسترش سبد: اندازه‌های تازه، بسته‌های هدیه و ترکیبی", "آماده‌سازی کمپین فصلی (یلدا (آذر): هدیه و بسته‌های ویژه)", "پایش روزانه قیمت رقبا و تنظیم قیمت"], "kpis": ["رشد ماهانه فروش ۱۰٪+", "نرخ خرید دوباره ۲۰٪+", "امتیاز میانگین ۴٫۵+"]}, {"title": "توسعه و تثبیت", "window": "ماه ۷ تا ۱۲", "goal": "برند شناخته‌شده با سود پایدار", "tasks": ["ورود به فروش سازمانی برای مناسبت‌ها", "محصول/خط تازه بر پایه نظر مشتریان", "کمپین برندسازی فصلی با اینفلوئنسر محوری", "بررسی صادرات یا نمایندگی استانی"], "kpis": ["سهم عرضه برند در دسته (در «تحلیل محصول») دو برابر", "۳۰٪ فروش از مشتریان تکراری", "سود خالص مثبت ماهانه"]}], "finance": {"price": 450000, "unitCost": 292500, "channelFee": 36000, "unitMargin": 121500, "marginPct": 27.0, "fixed": 0, "budget": 5000000, "breakEvenUnits": 42, "months": [{"month": 1, "units": 19, "revenue": 8550000, "gross": 2308500, "marketing": 5000000, "fixed": 0, "profit": -2691500, "cumulative": -2691500}, {"month": 2, "units": 21, "revenue": 9450000, "gross": 2551500, "marketing": 5000000, "fixed": 0, "profit": -2448500, "cumulative": -5140000}, {"month": 3, "units": 23, "revenue": 10350000, "gross": 2794500, "marketing": 5000000, "fixed": 0, "profit": -2205500, "cumulative": -7345500}, {"month": 4, "units": 26, "revenue": 11700000, "gross": 3159000, "marketing": 5000000, "fixed": 0, "profit": -1841000, "cumulative": -9186500}, {"month": 5, "units": 29, "revenue": 13050000, "gross": 3523500, "marketing": 5000000, "fixed": 0, "profit": -1476500, "cumulative": -10663000}, {"month": 6, "units": 33, "revenue": 14850000, "gross": 4009500, "marketing": 5000000, "fixed": 0, "profit": -990500, "cumulative": -11653500}, {"month": 7, "units": 37, "revenue": 16650000, "gross": 4495500, "marketing": 5000000, "fixed": 0, "profit": -504500, "cumulative": -12158000}, {"month": 8, "units": 42, "revenue": 18900000, "gross": 5103000, "marketing": 5000000, "fixed": 0, "profit": 103000, "cumulative": -12055000}, {"month": 9, "units": 47, "revenue": 21150000, "gross": 5710500, "marketing": 5000000, "fixed": 0, "profit": 710500, "cumulative": -11344500}, {"month": 10, "units": 52, "revenue": 23400000, "gross": 6318000, "marketing": 5000000, "fixed": 0, "profit": 1318000, "cumulative": -10026500}, {"month": 11, "units": 59, "revenue": 26550000, "gross": 7168500, "marketing": 5000000, "fixed": 0, "profit": 2168500, "cumulative": -7858000}, {"month": 12, "units": 66, "revenue": 29700000, "gross": 8019000, "marketing": 5000000, "fixed": 0, "profit": 3019000, "cumulative": -4839000}], "basis": ["قیمت فروش: میانه قیمت فروشگاه خود برند.", "بهای تمام‌شده: فرض حاشیه ناخالص معمول این حوزه (۳۵٪)؛ برای دقت، بهای تمام‌شده واقعی را وارد کنید.", "هزینه کانال (کمیسیون بازارگاه، درگاه، بسته‌بندی): میانگین ۸٪ قیمت فروش.", "سناریوی پایه: شروع از ۴۵٪ نقطه سربه‌سر و رشد ماهانه ۱۲٪ (متناسب با مرحله کسب‌وکار)."]}, "risks": [["وابستگی به یک کانال فروش", "حداقل سه کانال فعال و ساختن سایت/کانال مستقیم برای داده مشتری"], ["جنگ قیمت رقبا", "تمایز در کیفیت، داستان و خدمت؛ پایش روزانه قیمت و پرهیز از تخفیف عمومی"], ["نوسان قیمت مواد اولیه و تورم", "قرارداد تأمین بلندمدت، بازنگری ماهانه قیمت و بهای تمام‌شده"], ["کمبود موجودی در فصل اوج", "پیش‌خرید مواد و پیش‌فروش قبل از مناسبت‌ها"]], "kpis": [["فروش ماهانه (تعداد و مبلغ)", "بالای ۴۲ واحد (سربه‌سر)"], ["هزینه جذب هر مشتری (CAC)", "کمتر از ۳۶٬۴۵۰ تومان"], ["نرخ خرید دوباره", "۲۵٪ و بیشتر"], ["امتیاز و تعداد نظر", "میانگین ۴٫۵+ و ۲۰ نظر تازه در ماه"], ["سهم عرضه برند در دسته", "دیده‌شدن در صفحه اول جستجوی دسته"], ["نرخ تعامل اینستاگرام", "بالای ۳٪"]], "evidence": ["فروشگاه برند: ۶ محصول، قیمت ۳۰۰٬۰۰۰ تومان تا ۷۰۰٬۰۰۰ تومان."], "inputs": {"sector": "", "model": "", "stage": "", "goal": "", "audience": "", "usp": "", "region": "", "budget": 5000000, "price": 0, "unitCost": 0, "fixedCost": 0, "capacity": 0}, "prompt": "تو یک مشاور ارشد استراتژی، برندینگ و رشد کسب‌وکار در بازار ایران هستی. برای برند «داراشکوه» (حوزه مواد غذایی، کشاورزی و ارگانیک، مدل فروش آنلاین به مصرف‌کننده (بازارگاه + شبکه اجتماعی)، مرحله نوپا (کمتر از ۲ سال)، هدف افزایش فروش) یک بیزینس پلن کامل و حرفه‌ای در سطح جهانی بنویس: خلاصه مدیریتی، تحلیل بازار، رقبا، بوم کسب‌وکار، مدل درآمدی، استراتژی برند، برنامه بازاریابی و تبلیغات با بودجه، نقشه راه ۱۲ ماهه، پیش‌بینی مالی و ریسک‌ها. داده‌های واقعی:\n- فروشگاه برند: ۶ محصول، قیمت ۳۰۰٬۰۰۰ تومان تا ۷۰۰٬۰۰۰ تومان.\n- «داراشکوه» در حوزه مواد غذایی، کشاورزی و ارگانیک با مدل «فروش آنلاین به مصرف‌کننده (بازارگاه + شبکه اجتماعی)» و در مرحله «نوپا (کمتر از ۲ سال)» فعالیت می‌کند. هدف این برنامه افزایش فروش است؛ مخاطب اصلی «خانواده‌های سلامت‌محور شهری (۲۵ تا ۵۵ سال)» در سراسر ایران.\n- جایگاه قیمتی: میان‌رده (هم‌سطح بازار). تمایز باید از راه کیفیت، خدمات و داستان برند ساخته شود، نه جنگ قیمت.\n- اقتصاد واحد: قیمت ۴۵۰٬۰۰۰ تومان، سود هر واحد پس از بهای تمام‌شده و هزینه کانال ۱۲۱٬۵۰۰ تومان (۲۷٪). نقطه سربه‌سر ماهانه با بودجه تبلیغات و هزینه ثابت: ۴۲ واحد.\n- سه اقدام فوری بر پایه تحلیل: نام برند را در عنوان محصولات بیاورید.\nپیشنهادها باید عملیاتی، قابل سنجش و متناسب با کانال‌های ایرانی (باسلام، دیجی‌کالا، ترب، اینستاگرام، تلگرام/ایتا) باشد."};

MOCK_STRATEGY.brandbook = {"archetype": "مراقب (Caregiver)", "palette": [{"role": "اصلی", "hex": "#B8860B", "usage": "دکمه‌ها و عنوان‌ها"}, {"role": "ثانویه", "hex": "#6B4E16", "usage": "متن روی زمینه روشن"}, {"role": "تأکید", "hex": "#E9A23B", "usage": "برچسب‌ها و تخفیف"}, {"role": "زمینه", "hex": "#FFF8EC", "usage": "پس‌زمینه صفحات"}, {"role": "متن", "hex": "#2B2118", "usage": "متن اصلی"}], "headingFont": "Vazirmatn", "bodyFont": "Yekan Bakh", "fontNote": "عنوان‌ها ضخیم و فشرده، متن اصلی ساده و خوانا؛ در بسته‌بندی از وزن میان‌رده استفاده کنید.", "tone": [{"left": "رسمی", "right": "صمیمی", "value": 62}, {"left": "جدی", "right": "شوخ‌طبع", "value": 30}, {"left": "ساده", "right": "تخصصی", "value": 45}]};
Object.assign(MOCK_STRATEGY.finance, { vat: 3600, pack: 18000, returns: 5400, feePct: 8, vatPct: 10, returnPct: 3 });
