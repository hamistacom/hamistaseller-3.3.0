// «تحلیل محصول»: one product search across Iranian shops and classifieds → market prices, a
// suggested selling price (seller mode) or the best place to buy (buyer mode), brands, sales
// channels, related searches, price history and every offer with its photo.
import { h, icon, invoke, fa, clear, safe, input, field, toast, modal, newChannel, jdate, ago, confirmBox, select, copy, skipWaitButton } from "../core.js";
import { refreshLists, go } from "../state.js";
import { runJob } from "../jobs.js";
import { hero, photoCards, productPhoto } from "../photos.js";
import { PROVINCES } from "../data.js";

// id, name, monogram, has a sign-in page, is a classifieds site (asking prices of public ads)
const PLATFORMS = [
  ["web", "وب", "و", false, false],
  ["torob", "ترب", "ت", false, false],
  ["digikala", "دیجی‌کالا", "د", true, false],
  ["basalam", "باسلام", "ب", true, false],
  ["emalls", "ایمالز", "ا", false, false],
  ["divar", "دیوار", "د", true, true],
  ["sheypoor", "شیپور", "ش", true, true],
];
const PNAME = Object.fromEntries(PLATFORMS.map((p) => [p[0], p[1]]));
const PMONO = Object.fromEntries(PLATFORMS.map((p) => [p[0], p[2]]));
const CLASSIFIED = new Set(PLATFORMS.filter((p) => p[4]).map((p) => p[0]));
// how a buyer actually gets the product on each platform
const METHOD = {
  web: "سایت خود برند و فروشگاه‌های معتبر (ترجیحاً دارای اینماد)",
  torob: "فروشگاه اینترنتی",
  emalls: "فروشگاه اینترنتی",
  digikala: "آنلاین با ضمانت",
  basalam: "مستقیم از تولیدکننده",
  divar: "حضوری / توافقی",
  sheypoor: "حضوری / توافقی",
};
const ORIGIN = {
  torob: "https://torob.com",
  digikala: "https://www.digikala.com",
  basalam: "https://basalam.com",
  emalls: "https://emalls.ir",
  divar: "https://divar.ir",
  sheypoor: "https://www.sheypoor.com",
};
const link = (o, url = o.url) => (url.startsWith("http") ? url : `${ORIGIN[o.platform] || ORIGIN.torob}${url}`);

/** ✓ «لینک دقیق» when the backend proved the URL is that product's page, else ⚠ «تأییدنشده». */
export function linkBadge(status) {
  if (status === "exact") return h("span", { class: "link-badge exact", title: "لینک مستقیم همین کالا، تأییدشده" }, "✓ لینک دقیق");
  if (status === "unverified") return h("span", { class: "link-badge unverified", title: "لینک این منبع تأیید نشده؛ ممکن است صفحه دیگری باز شود" }, "⚠ تأییدنشده");
  return null;
}

/** Data-confidence pill (green ≥ 75, amber ≥ 45, else red). */
export function confBadge(score, label = "اعتماد داده") {
  const n = Math.round(score || 0);
  const cls = n >= 75 ? "ok" : n >= 45 ? "warn" : "bad";
  return h("span", { class: `pill ${cls}`, title: `${label} بر پایه پوشش منابع، حجم داده و تطبیق هویت` }, `${label}: ${fa(n)}٪`);
}
const SORTS = [
  ["relevance", "مرتبط‌ترین"],
  ["popular", "پربازدیدترین"],
  ["bestselling", "پرفروش‌ترین"],
  ["reviews", "بیشترین نظر"],
  ["cheap", "ارزان‌ترین"],
  ["expensive", "گران‌ترین"],
  ["newest", "جدیدترین / بروزترین"],
];
const LIMITS = [30, 60, 120, 240, 500];
const POPULAR = ["عسل", "زعفران", "موبایل", "پوشاک", "مبلمان", "خشکبار", "لوازم خانگی", "قطعات خودرو", "تجهیزات کافه", "چای", "برنج", "روغن"];
const remember = (k, v) => {
  try {
    if (v === undefined) return localStorage.getItem(k);
    localStorage.setItem(k, v);
  } catch {}
  return null;
};
export const toman = (n) => `${fa(n)} تومان`;
/** Persian / Arabic digits and thousands separators → a number (0 when empty). */
export const money = (v) =>
  Number(
    String(v || "")
      .replace(/[۰-۹]/g, (d) => "۰۱۲۳۴۵۶۷۸۹".indexOf(d))
      .replace(/[٠-٩]/g, (d) => "٠١٢٣٤٥٦٧٨٩".indexOf(d))
      .replace(/[^\d]/g, ""),
  ) || 0;

const BAD_BRAND_WORDS = new Set([
  "چند", "استاندارد", "طبیعی", "اصل", "اصلی", "ارگانیک", "خالص", "ممتاز", "درجه", "یک", "دو", "سه", "چهل", "گیاه", "کوهستان", "کوهی", "وحشی", "گون", "کنار", "آویشن", "موم", "تازه", "اعلا", "سوپر", "نگین", "سرگل", "پوشال", "خانگی", "محلی", "سنتی", "عمده", "فله", "تضمینی", "ویژه", "جدید",
]);
const cleanBrandList = (list) =>
  [...(list || [])]
    .filter((b) => b?.name && !BAD_BRAND_WORDS.has(b.name.trim()))
    .sort((a, b) => (b.count || 0) - (a.count || 0) || a.name.localeCompare(b.name, "fa"));

/** The platform's monogram badge (colour per platform; not the platforms' official logos). */
export const pbadge = (id, withName = false) =>
  h("span", { class: `pb p-${id}`, title: PNAME[id] || id }, h("i", { "aria-hidden": "true" }, PMONO[id] || "•"), withName ? h("span", {}, PNAME[id] || id) : null);

/** Segmented control. */
function seg(options, value, onchange, cls = "") {
  const box = h("div", { class: `seg ${cls}`.trim(), role: "tablist" });
  const draw = () =>
    clear(
      box,
      options.map(([v, label, ic]) =>
        h(
          "button",
          {
            type: "button",
            class: v === value ? "on" : "",
            "aria-selected": v === value ? "true" : "false",
            onclick: () => {
              value = v;
              draw();
              onchange(v);
            },
          },
          ic ? icon(ic, "sm") : null,
          label,
        ),
      ),
    );
  draw();
  box.value = () => value;
  return box;
}

// ---------------- small motion charts (CSP-safe: sizes set through CSSOM) ----------------

/** Horizontal share bars that grow in. rows = [{name, pct, count, median}] */
function shareBars(rows, { max = 8, unit } = {}) {
  const top = rows.slice(0, max);
  const rest = rows.slice(max);
  if (rest.length) {
    top.push({
      name: `${fa(rest.length)} مورد دیگر`,
      pct: Math.round(rest.reduce((a, r) => a + r.pct, 0) * 10) / 10,
      count: rest.reduce((a, r) => a + r.count, 0),
      median: 0,
      rest: true,
    });
  }
  const peak = Math.max(1, ...top.map((r) => r.pct));
  return h(
    "div",
    { class: "mbars" },
    top.map((r, i) => {
      const fill = h("i", { class: `t${i % 6} ${r.rest ? "rest" : ""}`.trim() });
      requestAnimationFrame(() => requestAnimationFrame(() => (fill.style.width = `${(r.pct / peak) * 100}%`)));
      return h(
        "div",
        { class: "mbar", title: r.median ? `میانه قیمت: ${toman(r.median)}` : "" },
        h("span", { class: "n" }, r.name),
        h("span", { class: "track" }, fill),
        h("b", {}, `${fa(r.pct)}٪`),
        h("span", { class: "c" }, unit ? `${fa(r.count)} ${unit}` : ""),
      );
    }),
  );
}

/** Vertical tier histogram. */
function tiers(rows) {
  const peak = Math.max(1, ...rows.map((r) => r.count));
  return h(
    "div",
    { class: "tiers" },
    rows.map((r, i) => {
      const col = h("i", { class: `t${i}` });
      requestAnimationFrame(() => requestAnimationFrame(() => (col.style.height = `${Math.max(4, (r.count / peak) * 100)}%`)));
      return h(
        "div",
        { class: "tier" },
        h("b", {}, `${fa(r.pct)}٪`),
        h("span", { class: "col" }, col),
        h("span", { class: "n" }, r.name),
        h("span", { class: "c" }, r.median ? `از ${toman(r.median)}` : ""),
      );
    }),
  );
}

const NS = "http://www.w3.org/2000/svg";
function svg(tag, attrs = {}, ...kids) {
  const el = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  kids.flat().forEach((k) => k && el.append(k instanceof Node ? k : document.createTextNode(String(k))));
  return el;
}

/** Price trend (median and lowest) over earlier searches of the same product. */
function trend(points) {
  const W = 1000,
    H = 200,
    L = 10,
    R = 10,
    T = 16,
    B = 28;
  const vals = points.flatMap((p) => [p[1], p[2]]).filter((v) => v > 0);
  const lo = Math.min(...vals) * 0.95,
    hi = Math.max(...vals) * 1.05;
  const x = (i) => L + (points.length === 1 ? (W - L - R) / 2 : (i * (W - L - R)) / (points.length - 1));
  const y = (v) => T + (H - T - B) * (1 - (v - lo) / Math.max(1, hi - lo));
  const line = (k, cls) => {
    const d = points.map((p, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(p[k]).toFixed(1)}`).join(" ");
    const path = svg("path", { d, class: `ln ${cls}` });
    return [
      path,
      ...points.map((p, i) =>
        svg("circle", { cx: x(i).toFixed(1), cy: y(p[k]).toFixed(1), r: 3.5, class: `pt ${cls}` }, svg("title", {}, `${jdate(p[0])}: ${toman(p[k])}`)),
      ),
    ];
  };
  const grid = [0, 0.5, 1].map((f) => svg("line", { x1: L, x2: W - R, y1: T + f * (H - T - B), y2: T + f * (H - T - B), class: "gr" }));
  const labels = points.map((p, i) =>
    points.length <= 8 || i % Math.ceil(points.length / 8) === 0
      ? svg(
          "text",
          {
            x: x(i),
            y: H - 6,
            class: "lb",
            "text-anchor": i === 0 ? "end" : i === points.length - 1 ? "start" : "middle",
          },
          jdate(p[0]).replace(/\s*[\d۰-۹]{4}$/, ""),
        )
      : null,
  );
  return svg("svg", { viewBox: `0 0 ${W} ${H}`, class: "trend-chart", role: "img", "aria-label": "روند قیمت" }, grid, line(1, "md"), line(2, "mn"), labels);
}

/** One line per source over the dates they were searched: series = [{name, cls, points: [[date, value]]}]. */
export function sourceChart(series) {
  const W = 1000,
    H = 220,
    L = 10,
    R = 10,
    T = 16,
    B = 28;
  const dates = [...new Set(series.flatMap((s) => s.points.map((p) => p[0])))].sort();
  const vals = series.flatMap((s) => s.points.map((p) => p[1])).filter((v) => v > 0);
  if (!dates.length || !vals.length) return null;
  const lo = Math.min(...vals) * 0.95,
    hi = Math.max(...vals) * 1.05;
  const x = (d) => {
    const i = dates.indexOf(d);
    return L + (dates.length === 1 ? (W - L - R) / 2 : (i * (W - L - R)) / (dates.length - 1));
  };
  const y = (v) => T + (H - T - B) * (1 - (v - lo) / Math.max(1, hi - lo));
  const grid = [0, 0.5, 1].map((f) => svg("line", { x1: L, x2: W - R, y1: T + f * (H - T - B), y2: T + f * (H - T - B), class: "gr" }));
  const lines = series.map((s) => [
    s.points.length > 1
      ? svg("path", {
          d: s.points.map((p, i) => `${i ? "L" : "M"}${x(p[0]).toFixed(1)},${y(p[1]).toFixed(1)}`).join(" "),
          class: `ln ${s.cls}`,
        })
      : null,
    ...s.points.map((p) =>
      svg("circle", { cx: x(p[0]).toFixed(1), cy: y(p[1]).toFixed(1), r: 3.5, class: `pt ${s.cls}` }, svg("title", {}, `${s.name} · ${jdate(p[0])}: ${toman(p[1])}`)),
    ),
  ]);
  const labels = dates.map((d, i) =>
    dates.length <= 8 || i % Math.ceil(dates.length / 8) === 0
      ? svg(
          "text",
          {
            x: x(d),
            y: H - 6,
            class: "lb",
            "text-anchor": i === 0 ? "end" : i === dates.length - 1 ? "start" : "middle",
          },
          jdate(d).replace(/\s*[\d۰-۹]{4}$/, ""),
        )
      : null,
  );
  return svg("svg", { viewBox: `0 0 ${W} ${H}`, class: "trend-chart multi", role: "img", "aria-label": "قیمت هر منبع" }, grid, lines, labels);
}

/** A tiny price line for tables. */
export function sparkline(values) {
  const v = values.filter((x) => x > 0);
  if (v.length < 2) return h("span", { class: "x small" }, v.length ? "۱ نقطه" : "—");
  const W = 120,
    H = 32,
    lo = Math.min(...v),
    hi = Math.max(...v);
  const pts = v.map((p, i) => `${((i * (W - 4)) / (v.length - 1) + 2).toFixed(1)},${(H - 3 - ((p - lo) / Math.max(1, hi - lo)) * (H - 6)).toFixed(1)}`);
  return svg(
    "svg",
    {
      viewBox: `0 0 ${W} ${H}`,
      class: `spark ${v[v.length - 1] >= v[0] ? "up" : "down"}`,
      preserveAspectRatio: "none",
      role: "img",
      "aria-label": "روند قیمت",
    },
    svg("polyline", { points: pts.join(" ") }),
  );
}

/** Price change as a coloured pill: ▲ 4٪ / ▼ 2٪ / —. */
export function changePill(pct) {
  if (pct === null || pct === undefined || !Number.isFinite(pct) || pct === 0) return h("span", { class: "x" }, "—");
  return h("span", { class: `trend ${pct > 0 ? "up" : "down"}` }, `${pct > 0 ? "▲" : "▼"} ${fa(Math.abs(pct))}٪`);
}

const PID = Object.fromEntries(PLATFORMS.map((p) => [p[1], p[0]])); // display name → id

/** Counts a figure up from 0 (motion KPI). */
function countUp(el, to, fmt) {
  const t0 = performance.now();
  const step = (t) => {
    const k = Math.min(1, (t - t0) / 700);
    el.textContent = fmt(Math.round(to * (1 - Math.pow(1 - k, 3))));
    if (k < 1) requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}

// ---------------- brand / company / shop combobox (no alphabet headers; sorted by market credibility) ----------------

function brandBox(value = "") {
  const inp = h("input", {
    class: "input",
    placeholder: "همه برندها و شرکت‌ها — یا انتخاب کنید",
    value,
    autocomplete: "off",
    role: "combobox",
    "aria-expanded": "false",
    "aria-autocomplete": "list",
  });
  const pop = h("div", { class: "combo-pop", role: "listbox", hidden: true });
  const wrap = h("div", { class: "combo" }, inp, pop);
  let list = [];
  let active = -1;
  let typed = false;
  const close = () => {
    pop.hidden = true;
    inp.setAttribute("aria-expanded", "false");
  };
  const draw = () => {
    const f = typed ? inp.value.trim() : "";
    const rows = list.filter((b) => !f || b.name.includes(f));
    active = Math.min(active, rows.length - 1);
    if (!list.length) {
      clear(pop, h("div", { class: "combo-empty" }, "بعد از اولین تحلیل، برندها، شرکت‌ها و فروشگاه‌های معتبر این کالا اینجا فهرست می‌شوند."));
    } else {
      clear(
        pop,
        h(
          "button",
          { type: "button", class: `combo-row all ${!inp.value.trim() ? "on" : ""}`.trim(), onclick: () => pick("") },
          h("b", {}, "همه برندها و فروشگاه‌ها"),
          h("span", { class: "muted small" }, `${fa(list.length)} مورد معتبر`),
        ),
        rows.map((b, i) =>
          h(
            "button",
            { type: "button", role: "option", class: `combo-row ${i === active ? "act" : ""}`.trim(), onclick: () => pick(b.name) },
            h("b", {}, b.name),
            h("span", { class: "combo-pl" }, (b.platforms || []).map((p) => pbadge(p))),
            h("span", { class: "muted small" }, b.count ? `${fa(b.count)} کالا${b.median ? ` · میانه ${fa(b.median)}` : ""}` : "برند شناخته‌شده این صنف"),
          ),
        ),
        f && !list.some((b) => b.name === f)
          ? h("button", { type: "button", class: "combo-row custom", onclick: () => pick(f) }, icon("plus", "sm"), h("b", {}, `جستجوی برند یا فروشگاه «${f}»`))
          : null,
      );
    }
    pop.hidden = false;
    inp.setAttribute("aria-expanded", "true");
  };
  const pick = (v) => {
    inp.value = v;
    close();
    wrap.dispatchEvent(new CustomEvent("pick", { detail: v }));
  };
  inp.addEventListener("focus", () => {
    typed = false;
    draw();
  });
  inp.addEventListener("input", () => {
    typed = true;
    active = -1;
    draw();
  });
  inp.addEventListener("keydown", (e) => {
    const rows = pop.querySelectorAll(".combo-row:not(.all)");
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.max(0, Math.min(rows.length - 1, active + (e.key === "ArrowDown" ? 1 : -1)));
      draw();
      pop.querySelector(".act")?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Escape") close();
    else if (e.key === "Enter" && !pop.hidden && active >= 0) {
      e.preventDefault();
      e.stopPropagation();
      rows[active]?.click();
    }
  });
  document.addEventListener("mousedown", (e) => !wrap.contains(e.target) && close());
  return {
    el: wrap,
    input: inp,
    get value() {
      return inp.value.trim();
    },
    set value(v) {
      inp.value = v;
    },
    setList(l) {
      list = cleanBrandList(l);
    },
  };
}

// ---------------- page ----------------

export async function render(root, params, actions) {
  let mode = remember("mktMode") || "seller";
  let platforms = new Set((remember("mktPlatforms") || "web,torob,digikala,basalam,emalls").split(",").filter((p) => PNAME[p]));
  if (remember("mktWeb") === "1") platforms.add("web"); // the old separate «وب» switch
  let exact = remember("mktExact") !== "0";
  // «منبع دلخواه»: one site / booth / channel; every field then comes from that source only
  const site = input({ class: "input ltr", placeholder: "https://… سایت، غرفه باسلام یا کانال تلگرام (اختیاری)", value: remember("mktSite") || "" });
  const siteHint = h("span", { class: "small muted" });
  const syncSite = () => {
    siteHint.textContent = site.value.trim() ? "همه اطلاعات (محصول، قیمت، فروشنده، برند…) فقط از همین منبع خوانده می‌شود؛ منابع دیگر کنار می‌روند." : "خالی = منابع انتخاب‌شده در پایین.";
    plat.classList.toggle("muted-off", !!site.value.trim());
  };
  site.addEventListener("input", syncSite);
  const q = input({ placeholder: "نام دقیق کالا؛ چند کالا با «+» یا «یا»، حذف با «-» (مثلاً: عسل یک کیلویی + شهد آزادی -موم)", value: params.q || remember("mktQuery") || "" });
  const brand = brandBox(params.brand || "");
  const margin = input({ type: "number", min: 0, max: 300, dir: "ltr", value: remember("mktMargin") || 25 });
  const minP = input({ inputmode: "numeric", dir: "ltr", placeholder: "از", value: remember("mktMin") || "" });
  const maxP = input({ inputmode: "numeric", dir: "ltr", placeholder: "تا", value: remember("mktMax") || "" });
  for (const el of [minP, maxP]) el.addEventListener("blur", () => (el.value = money(el.value) ? fa(money(el.value)) : ""));
  const limit = select(LIMITS.map((n) => [n, `${fa(n)} نتیجه`]), Number(remember("mktLimit")) || 60);
  const sort = select(SORTS, remember("mktSort") || "relevance");
  const match = seg([["exact", "عبارت دقیق"], ["broad", "همه کلمات"]], exact ? "exact" : "broad", (v) => (exact = v === "exact"));
  match.title = "«عبارت دقیق»: فقط همان کالا (چوب پینگ پنگ بدون میز). «همه کلمات»: هر عنوانی که همه کلمه‌ها را داشته باشد.";
  // region
  const prov = select([["", "همه ایران"], ...Object.keys(PROVINCES).map((p) => [p, p])], remember("mktProvince") || "");
  const city = h("select", { class: "input" });
  const fillCity = (keep) => {
    const list = PROVINCES[prov.value] || [];
    clear(city, [["", prov.value ? "همه شهرهای استان" : "شهر"], ...list.map((c) => [c, c])].map(([v, l]) => h("option", { value: v, selected: v === keep }, l)));
    city.disabled = !prov.value;
  };
  fillCity(remember("mktCity") || "");
  prov.addEventListener("change", () => fillCity(""));
  const localOnly = h("input", { type: "checkbox", checked: remember("mktLocal") === "1" });
  const district = input({ placeholder: "مثلاً ونک، سعادت‌آباد", value: remember("mktDistrict") || "" });
  const regionHint = h("span", { class: "small muted" });
  const regionSum = h("span", { class: "chip" });
  const syncRegion = () => {
    const where = [district.value.trim(), city.value || prov.value].filter(Boolean).join("، ");
    regionHint.textContent = where
      ? `دیوار و شیپور در «${city.value || prov.value || "کل ایران"}» جستجو می‌شوند${localOnly.checked ? `؛ فقط فروشندگان «${district.value.trim() || city.value || prov.value}» نگه داشته می‌شوند` : ""}.`
      : "آگهی‌های دیوار و شیپور در کل ایران جستجو می‌شوند.";
    regionSum.textContent = where || "همه ایران";
  };
  [prov, city, localOnly].forEach((el) => el.addEventListener("change", syncRegion));
  district.addEventListener("input", syncRegion);
  syncRegion();
  // sources
  const plat = h("div", { class: "src-chips" });
  const drawPlat = () =>
    clear(
      plat,
      PLATFORMS.map(([id, name, , login, cls]) =>
        h(
          "span",
          { class: `src-chip ${platforms.has(id) ? "on" : ""}`.trim() },
          h(
            "button",
            {
              type: "button",
              "aria-pressed": platforms.has(id) ? "true" : "false",
              title: id === "web" ? "سایت خود برندها و فروشگاه‌های معتبر (اول اینماد و داده ساخت‌یافته)؛ می‌تواند تنها منبع باشد" : cls ? "قیمت آگهی‌های عمومی (اغلب دست‌دوم یا تولید خانگی)؛ عنوان، قیمت، عکس، شهر و زمان آگهی خوانده می‌شود" : METHOD[id],
              onclick: () => {
                platforms.has(id) && platforms.size > 1 ? platforms.delete(id) : platforms.add(id);
                drawPlat();
              },
            },
            pbadge(id),
            h("span", {}, name),
            platforms.has(id) ? icon("check", "sm") : null,
          ),
          login
            ? h(
                "button",
                {
                  type: "button",
                  class: "src-login",
                  title: `ورود به حساب ${name} در پنجره مرورگر (برای دیدن نتایج کامل‌تر)`,
                  onclick: () => signIn(id, name),
                },
                icon("user", "sm"),
              )
            : null,
        ),
      ),
    );
  drawPlat();
  syncSite();
  const signIn = async (id, name) => {
    toast(`پنجره ورود ${name} باز شد؛ وارد حساب شوید و پنجره را ببندید. ورود شما برای جستجوهای بعدی می‌ماند.`, "ok");
    await safe(invoke("market_login", { platform: id }));
  };
  const go_ = h("button", { class: "btn primary big", type: "button" }, icon("trend", "sm"), "تحلیل");
  const progress = h("div", { class: "mkt-progress", hidden: true });
  const out = h("div", { class: "stack" });
  const marginField = field("حاشیه سود هدف (٪)", margin);
  const modeSeg = seg(
    [
      ["seller", "فروشنده هستم", "store"],
      ["buyer", "خریدار هستم", "bag"],
      ["wholesale", "خریدار عمده / B2B", "layers"],
    ],
    mode,
    (v) => {
      mode = v;
      remember("mktMode", v);
      marginField.hidden = v !== "seller";
      if (current) show(current);
    },
    "mode-seg",
  );
  marginField.hidden = mode !== "seller";
  let current = null;

  const run = async () => {
    const query = q.value.trim();
    if (query.length < 2) return toast("نام کالا را بنویسید.", "bad");
    const lo = money(minP.value),
      hi = money(maxP.value);
    if (hi && lo > hi) return toast("در محدوده قیمت، «از» بزرگ‌تر از «تا» است.", "bad");
    remember("mktQuery", query);
    remember("mktMargin", margin.value);
    remember("mktPlatforms", [...platforms].join(","));
    remember("mktSite", site.value.trim());
    remember("mktMin", minP.value);
    remember("mktMax", maxP.value);
    remember("mktLimit", limit.value);
    remember("mktSort", sort.value);
    remember("mktExact", exact ? "1" : "0");
    remember("mktProvince", prov.value);
    remember("mktCity", city.value);
    remember("mktLocal", localOnly.checked ? "1" : "0");
    remember("mktDistrict", district.value.trim());
    go_.disabled = true;
    go_.setAttribute("aria-busy", "true");
    clear(go_, h("span", { class: "spin-dot" }), "در حال تحلیل…");
    progress.hidden = false;
    const line = h("span", { class: "small" }, "در حال آماده‌سازی…");
    const track = h("div", { class: "bar" }, h("i"));
    const log = h("div", { class: "small muted mkt-log" });
    const stop = h("button", { class: "btn sm danger", type: "button", onclick: (e) => { e.currentTarget.disabled = true; safe(invoke("job_stop", {})); } }, icon("stop", "sm"), "توقف");
    const waitBox = h("div", { class: "callout compact row", hidden: true });
    clear(progress, h("div", { class: "row between" }, line, stop), waitBox, track, log);
    const ch = newChannel();
    ch.onmessage = (e) => {
      if (e.type === "status") {
        line.textContent = e.message;
        waitBox.hidden = true;
      }
      if (e.type === "progress") track.firstChild.style.width = `${(100 * e.checked) / Math.max(1, e.target)}%`;
      if (e.type === "log") log.append(h("div", { class: e.level }, e.message));
      if (e.type === "captcha") {
        // never stuck on one source (e.g. Torob's «I'm not a robot»): skip it and go on with the rest
        clear(waitBox, icon("alert", "sm"), h("span", { class: "small grow" }, e.message), skipWaitButton());
        waitBox.hidden = false;
        toast(e.message, "ok");
      }
    };
    const req = {
      query,
      brand: brand.value,
      platforms: [...platforms],
      targetMargin: Number(margin.value) || 25,
      limit: Number(limit.value) || 60,
      sort: sort.value,
      exact,
      minPrice: lo,
      maxPrice: hi,
      province: prov.value,
      city: city.value,
      district: district.value.trim(),
      localOnly: localOnly.checked,
      mode,
      web: platforms.has("web") || mode === "wholesale",
      site: site.value.trim(),
    };
    remember("mktWeb", "");
    const rep = await safe(invoke("market_search", { req, onEvent: ch }));
    go_.disabled = false;
    go_.removeAttribute("aria-busy");
    clear(go_, icon("trend", "sm"), "تحلیل");
    progress.hidden = true;
    if (rep) show(rep);
  };
  go_.addEventListener("click", run);
  q.addEventListener("keydown", (e) => e.key === "Enter" && run());
  brand.input.addEventListener("keydown", (e) => e.key === "Enter" && !e.defaultPrevented && run());
  brand.el.addEventListener("pick", () => q.value.trim() && current && run());

  function show(r) {
    current = r;
    q.value = r.query;
    brand.value = r.brand || "";
    brand.setList(r.brandList);
    clear(
      out,
      report(r, {
        mode,
        search: (query, b = "") => {
          q.value = query;
          brand.value = b;
          run();
        },
      }),
    );
  }

  root.append(
    h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "card stack mkt-search" },
        h("div", { class: "row between wrap" }, modeSeg, h("span", { class: "small muted" }, "")),
        h("div", { class: "mkt-row" }, field("کالا (کلمه کلیدی دقیق)", q), field("برند / شرکت / غرفه", brand.el), marginField, h("div", { class: "field go" }, h("label", {}, " "), go_)),
        h(
          "div",
          { class: "mkt-row2" },
          field("محدوده قیمت حدودی بازار (تومان)", h("div", { class: "range2" }, minP, h("span", { class: "muted" }, "تا"), maxP), "قیمت‌های بیرون این بازه (قدیمی یا اشتباه) در تحلیل نمی‌آیند."),
          field("تعداد نتایج هر منبع", limit),
          field("ترتیب نتایج در منبع", sort),
          field("تطبیق کلمه", match),
        ),
        h(
          "details",
          { class: "mkt-more", open: !!(prov.value || district.value) },
          h("summary", {}, icon("map", "sm"), "منطقه (برای دیوار، شیپور و فروشندگان محلی)", regionSum),
          h("div", { class: "mkt-row2" }, field("استان", prov), field("شهر", city), field("محله / منطقه", district), h("label", { class: "check" }, localOnly, h("span", {}, "فقط فروشندگان همین منطقه"))),
          regionHint,
        ),
        h("div", { class: "row wrap" }, h("span", { class: "label-t" }, "منابع:"), plat),
        h("div", { class: "mkt-site" }, field("منبع دلخواه (لینک)", site), siteHint),
        progress,
      ),
      out,
    ),
  );
  actions.append(h("button", { class: "btn", type: "button", onclick: () => go("market-runs") }, icon("history", "sm"), "تحلیل‌های قبلی"));
  if (params.run) {
    const r = await safe(invoke("market_get", { id: Number(params.run) }));
    if (r) show(r);
  } else {
    const trends = await invoke("market_trends").catch(() => []);
    clear(
      out,
      emptyState(trends, (p) => {
        q.value = p;
        brand.value = "";
        run();
      }),
    );
  }
}

function emptyState(trends, pick) {
  const month = trends.length
    ? h(
        "div",
        { class: "card flush" },
        h("div", { class: "card-h pad" }, h("h2", {}, "بازار این ماه"), h("span", { class: "small muted" }, "از تحلیل‌های ۳۰ روز اخیر شما: پرتکرارترین کالاها و تغییر قیمتشان")),
        h(
          "div",
          { class: "table-wrap" },
          h(
            "table",
            { class: "t" },
            h("thead", {}, h("tr", {}, ["کالا", "دفعات تحلیل", "میانه اول ماه", "میانه اخیر", "تغییر", "میانه هر پلتفرم", ""].map((t) => h("th", {}, t)))),
            h(
              "tbody",
              {},
              trends.map((t) => {
                const ch = t.firstMedian ? Math.round(((t.lastMedian - t.firstMedian) / t.firstMedian) * 1000) / 10 : 0;
                return h(
                  "tr",
                  { class: "click", onclick: () => go("market", { run: t.lastId }) },
                  h("td", {}, h("b", {}, t.query)),
                  h("td", {}, fa(t.searches)),
                  h("td", { class: "num" }, fa(t.firstMedian)),
                  h("td", { class: "num" }, fa(t.lastMedian)),
                  h("td", {}, ch ? h("span", { class: `trend ${ch > 0 ? "up" : "down"}` }, `${ch > 0 ? "▲" : "▼"} ${fa(Math.abs(ch))}٪`) : h("span", { class: "x" }, "—")),
                  h("td", {}, h("div", { class: "row wrap tight-row" }, (t.channels || []).slice(0, 4).map((c) => h("span", { class: "pill" }, `${c.name}: ${fa(c.median)}`)))),
                  h(
                    "td",
                    {},
                    h(
                      "button",
                      {
                        class: "btn ghost sm",
                        type: "button",
                        onclick: (e) => {
                          e.stopPropagation();
                          pick(t.query);
                        },
                      },
                      icon("refresh", "sm"),
                      "به‌روزرسانی",
                    ),
                  ),
                );
              }),
            ),
          ),
        ),
      )
    : null;
  return h(
    "div",
    { class: "stack" },
    month,
    h(
      "div",
      { class: "card" },
      h("div", { class: "card-h" }, h("h2", {}, "شروع سریع"), h("span", { class: "muted" }, "یک دسته را بزنید؛ با «پرفروش‌ترین» در «ترتیب نتایج»، پرفروش‌های ماه هر پلتفرم را ببینید")),
      photoCards(POPULAR.filter((p) => productPhoto(p)).map((p) => ({ title: p, tag: "تحلیل", photo: productPhoto(p, true), onclick: () => pick(p) }))),
    ),
    h(
      "div",
      { class: "card" },
      h(
        "div",
        { class: "mkt-empty" },
        h("div", { class: "ic-box lg" }, icon("trend")),
        h("h2", {}, "قیمت روز بازار هر کالا را دقیق ببینید"),
        h("p", { class: "muted" }, "نام دقیق کالا را بنویسید. قیمت‌ها از ترب، دیجی‌کالا، باسلام و ایمالز (و اگر بخواهید آگهی‌های دیوار و شیپور) صفحه به صفحه جمع می‌شوند:"),
        h(
          "ol",
          { class: "how cols" },
          h("li", {}, h("b", {}, "فقط همان کالا"), h("span", {}, "«چوب پینگ پنگ» با «میز پینگ پنگ» قاطی نمی‌شود؛ املای دیگر («کیسه فریز») هم پیدا می‌شود")),
          h("li", {}, h("b", {}, "قیمت معتبر"), h("span", {}, "با محدوده قیمت شما و حذف آماری قیمت‌های پرت")),
          h("li", {}, h("b", {}, "فروشنده یا خریدار"), h("span", {}, "قیمت پیشنهادی فروش و رقبا، یا بهترین جای خرید")),
          h("li", {}, h("b", {}, "برندها، شرکت‌ها و کانال‌ها"), h("span", {}, "فهرست برندها و غرفه‌های معتبر، سهم هر پلتفرم، عکس و تاریخ بروزرسانی هر پیشنهاد")),
        ),
      ),
    ),
  );
}

/** The full analysis of one search (also used for saved searches). */
export function report(r, { search, mode = "seller" }) {
  const s = r.stats;
  const perKg = s.basis === "per_kg";
  const unitLabel = perKg ? "هر کیلوگرم / لیتر" : "هر عدد";
  const basis = (o) => (perKg ? o.unitPrice || 0 : o.price);
  const inRange = (o) => basis(o) >= s.min && basis(o) <= s.max;
  const kpi = (title, value, sub, cls = "") => {
    const b = h("b", {}, "۰");
    countUp(b, value, (v) => fa(v));
    return h("div", { class: `card kpi3 ${cls}`.trim() }, h("span", { class: "t" }, title), h("div", { class: "v" }, b, h("small", {}, "تومان")), h("span", { class: "d" }, sub));
  };
  const a = r.advice || {};
  const statusLine = (r.platforms || []).map(([p, n, err]) =>
    h("span", { class: `pill ${err ? "warn" : n ? "ok" : ""}`.trim(), title: err || "" }, pbadge(p), `${err ? "خطا" : fa(n)}`),
  );
  const filtered = [
    r.dropped ? `${fa(r.dropped)} نتیجه درباره کالای دیگر` : "",
    r.outOfRange ? `${fa(r.outOfRange)} قیمت خارج از محدوده شما` : "",
    s.outliers ? `${fa(s.outliers)} قیمت پرت` : "",
  ].filter(Boolean);

  // ---------- buyer view: who to buy from ----------
  const valid = r.offers.filter((o) => basis(o) > 0 && inRange(o));
  const cheapest = [...valid].sort((x, y) => basis(x) - basis(y))[0];
  const bestValue = valid[0];
  const trusted = [...valid].filter((o) => (o.reviews || 0) >= 10 && o.rating).sort((x, y) => y.rating - x.rating || y.reviews - x.reviews)[0];
  const local = valid.find((o) => CLASSIFIED.has(o.platform));
  const offerLine = (o) => (o ? `${o.seller || PNAME[o.platform]} · ${PNAME[o.platform] || o.platform}` : "—");

  const kpis =
    mode === "buyer"
      ? h(
          "div",
          { class: "kpi-row" },
          kpi("ارزان‌ترین پیشنهاد معتبر", cheapest ? basis(cheapest) : 0, offerLine(cheapest), "hero"),
          kpi("بهترین ارزش خرید", bestValue ? basis(bestValue) : 0, `${offerLine(bestValue)} · امتیاز ${fa(bestValue?.value || 0)}`),
          kpi("معتبرترین فروشنده", trusted ? basis(trusted) : 0, trusted ? `${offerLine(trusted)} · ${fa(trusted.rating)} از ۵ (${fa(trusted.reviews)} نظر)` : "فروشنده با ۱۰+ نظر پیدا نشد"),
          kpi("قیمت معمول بازار (میانه)", s.median, `${unitLabel} · ۵۰٪ عرضه‌ها ارزان‌تر`),
        )
      : h(
          "div",
          { class: "kpi-row" },
          kpi("کمترین نرخ بازار", s.min, `کف قیمت معتبر · ${unitLabel}`),
          kpi("میانگین وزنی قیمت", s.weighted, `میانه ${fa(s.median)} · وزن بیشتر برای فروشنده معتبر`),
          kpi("بالاترین نرخ بازار", s.max, `سقف قیمت معتبر · ۷۵٪ عرضه‌ها زیر ${fa(s.p75)}`),
          kpi("قیمت پیشنهادی فروش", r.srp, `با حاشیه هدف ${fa(r.targetMargin)}٪ و محدوده رقابتی بازار`, "hero"),
        );
  const advice =
    mode === "buyer"
      ? h(
          "div",
          { class: "advice" },
          h("span", { class: "ic-box" }, icon("target")),
          h(
            "div",
            { class: "grow stack tight" },
            h("b", {}, bestValue ? `بهترین خرید: «${bestValue.title}» از ${offerLine(bestValue)} با ${toman(bestValue.price)}.` : "پیشنهاد معتبری پیدا نشد."),
            cheapest && cheapest !== bestValue ? h("span", { class: "small" }, `ارزان‌ترین: ${offerLine(cheapest)} با ${toman(cheapest.price)} (${METHOD[cheapest.platform] || ""}).`) : null,
            local ? h("span", { class: "small" }, `خرید حضوری و سریع: «${local.title}» در ${PNAME[local.platform]}${local.location ? ` (${local.location})` : ""} با ${toman(local.price)}.`) : null,
            h("span", { class: "small muted" }, `• قیمتی پایین‌تر از ${toman(s.p25)} یعنی بسیار ارزان؛ اصالت کالا و اعتبار فروشنده را بررسی کنید.`),
          ),
          bestValue?.url
            ? h("button", { class: "btn tonal sm", type: "button", onclick: () => safe(invoke("open_url", { url: link(bestValue) })) }, icon("external", "sm"), "صفحه خرید")
            : null,
        )
      : r.offers[0]
        ? h(
            "div",
            { class: "advice" },
            h("span", { class: "ic-box" }, icon("target")),
            h(
              "div",
              { class: "grow stack tight" },
              h("b", {}, `از «${a.buyFrom || r.offers[0].seller || PNAME[r.offers[0].platform]}» با ${toman(a.buyPrice)} بخرید؛ در «${a.sellOn}» با ${toman(a.sellPrice)} بفروشید.`),
              h("span", { class: "small" }, `حاشیه سود با قیمت پیشنهادی: ${fa(a.marginPct)}٪ · اگر روی میانه بازار بفروشید: ${fa(a.marketMarginPct)}٪`),
              (a.notes || []).map((n) => h("span", { class: "small muted" }, `• ${n}`)),
            ),
            a.buyUrl
              ? h("button", { class: "btn tonal sm", type: "button", onclick: () => safe(invoke("open_url", { url: link({ platform: r.offers[0].platform, url: a.buyUrl }) })) }, icon("external", "sm"), "صفحه فروشنده")
              : null,
          )
        : null;

  // ---------- tabs ----------
  const cleanedBrands = cleanBrandList(r.brandList);
  const brandsN = cleanedBrands.filter((b) => b.count).length;
  const tabs = [
    ["summary", "خلاصه", "chart"],
    ["products", `محصولات (${fa(r.offers.length)})`, "store"],
    ...(r.groups?.length ? [["same", `کالاهای یکسان (${fa(r.groups.length)})`, "layers"]] : []),
    ["brands", `برندها و شرکت‌ها (${fa(brandsN)})`, "award"],
    ["sellers", `فروشندگان (${fa(new Set(r.offers.map((o) => o.seller || o.platform)).size)})`, "store"],
    ["channels", `کانال‌های فروش (${fa(r.channels.length)})`, "trend"],
    ...(r.suppliers?.length ? [["suppliers", `عمده‌فروشان (${fa(r.suppliers.length)})`, "layers"]] : []),
    ...(r.web?.length ? [["web", `منابع وب (${fa(r.web.length)})`, "globe"]] : []),
    ["history", "روند قیمت", "history"],
  ];
  const body = h("div", { class: "stack" });
  const tabBar = seg(tabs, "summary", (v) => draw(v), "rep-tabs");
  const draw = (t) => clear(body, { summary, products, same, brands, sellers, channels, suppliers, web: webTab, history }[t]());

  function summary() {
    const validBrandShares = (r.brands || []).filter((b) => !BAD_BRAND_WORDS.has(b.name.trim()));
    return h(
      "div",
      { class: "stack" },
      kpis,
      advice,
      r.related?.length || r.subcategories?.length
        ? h(
            "div",
            { class: "card stack tight chips-card" },
            r.related?.length
              ? h("div", { class: "row wrap" }, h("span", { class: "label-t" }, "جستجوهای پرطرفدار مرتبط"), h("div", { class: "chips" }, r.related.map((c) => h("button", { type: "button", class: "chip click assist", onclick: () => search(c) }, icon("search", "sm"), c))))
              : null,
            r.subcategories?.length
              ? h("div", { class: "row wrap" }, h("span", { class: "label-t" }, "انواع همین کالا"), h("div", { class: "chips" }, r.subcategories.map((c) => h("button", { type: "button", class: "chip click", onclick: () => search(c) }, c))))
              : null,
          )
        : null,
      h(
        "div",
        { class: "mkt-grid" },
        h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "سهم برندها و فروشگاه‌ها"), h("span", { class: "small muted" }, `${fa(validBrandShares.length)} مورد`)), shareBars(validBrandShares, { unit: "کالا" })),
        h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "سهم کانال‌های فروش")), shareBars(r.channels, { unit: "کالا" })),
        h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "توزیع قیمت")), tiers(r.tiers), h("span", { class: "small muted" }, "ارزان‌ترین ۲۵٪ تا گران‌ترین ۲۵٪ عرضه‌ها")),
      ),
    );
  }

  function products() {
    const tbody = h("tbody");
    let pf = "";
    let sort = mode === "buyer" ? "low" : "value";
    let find = "";
    const fil = h(
      "select",
      { class: "input sm pill-select", onchange: (e) => { pf = e.target.value; rows(); } },
      h("option", { value: "" }, "همه کانال‌ها"),
      [...new Set(r.offers.map((o) => o.platform))].map((p) => h("option", { value: p }, PNAME[p] || p)),
    );
    const srt = select(
      [
        ["value", "بیشترین ارزش خرید"],
        ["low", "ارزان‌ترین"],
        ["high", "گران‌ترین"],
        ["rating", "بیشترین امتیاز"],
        ["reviews", "بیشترین نظر"],
        ["sales", "پرفروش‌ترین"],
        ["newest", "بروزترین قیمت"],
      ],
      sort,
      { onchange: (e) => { sort = e.target.value; rows(); } },
    );
    srt.classList.add("sm", "pill-select");
    const box = input({ placeholder: "جستجو در نتایج…" });
    box.classList.add("sm");
    box.addEventListener("input", () => {
      find = box.value.trim();
      rows();
    });
    const cmp = {
      low: (x, y) => basis(x) - basis(y),
      high: (x, y) => basis(y) - basis(x),
      rating: (x, y) => (y.rating || 0) - (x.rating || 0),
      reviews: (x, y) => (y.reviews || 0) - (x.reviews || 0),
      sales: (x, y) => (y.sales || 0) - (x.sales || 0) || (y.reviews || 0) - (x.reviews || 0),
      newest: (x, y) => (y.updated || "").localeCompare(x.updated || ""),
      value: (x, y) => y.value - x.value,
    };
    function rows() {
      let list = r.offers.filter(
        (o) => (!pf || o.platform === pf) && (!find || o.title.includes(find) || (o.seller || "").includes(find) || (o.brand || "").includes(find)),
      );
      list = [...list].sort(cmp[sort]);
      clear(
        tbody,
        list.slice(0, 500).map((o, i) => {
          const vb = h("i");
          vb.style.width = `${o.value}%`;
          const img = o.image ? h("img", { src: o.image, alt: "", loading: "lazy", decoding: "async", referrerpolicy: "no-referrer" }) : icon("store");
          if (o.image) img.addEventListener("error", () => img.replaceWith(icon("store")));
          return h(
            "tr",
            {},
            h("td", { class: "rn" }, fa(i + 1)),
            h("td", {}, h("span", { class: "thumb" }, img)),
            h(
              "td",
              { class: "pt" },
              h(
                "div",
                { class: "name-cell" },
                h("span", { class: "n full", title: o.title }, o.title),
                h("span", { class: "c" }, [o.brand && !BAD_BRAND_WORDS.has(o.brand.trim()) ? o.brand : "", o.shops ? `در ${fa(o.shops)} فروشگاه` : ""].filter(Boolean).join(" · "), o.url ? linkBadge(o.link) : null),
              ),
            ),
            h("td", {}, pbadge(o.platform, true), h("div", { class: "x small" }, METHOD[o.platform] || "")),
            h(
              "td",
              { class: "clip" },
              o.sellerUrl
                ? h("button", { class: "seller-link", type: "button", title: "صفحه فروشنده / غرفه", onclick: () => safe(invoke("open_url", { url: o.sellerUrl })) }, o.seller || "—")
                : o.seller || "—",
              o.location ? h("div", { class: "x small" }, o.location) : null,
            ),
            h(
              "td",
              {},
              o.rating ? h("span", { class: "rate" }, icon("award", "sm"), fa(Math.round(o.rating * 10) / 10)) : h("span", { class: "x" }, "—"),
              o.reviews ? h("span", { class: "x" }, ` (${fa(o.reviews)})`) : null,
              o.sales ? h("div", { class: "x small" }, `${fa(o.sales)} فروش`) : null,
            ),
            h("td", { class: "num" }, fa(o.price), !inRange(o) ? h("div", { class: "x small warn-t" }, "خارج از محدوده معتبر") : null),
            h("td", { class: "num muted" }, o.unitPrice ? fa(o.unitPrice) : "—"),
            h("td", { class: "small" }, o.updated ? ago(o.updated) : h("span", { class: "x" }, "—")),
            h("td", {}, h("div", { class: "vscore" }, h("span", { class: "vb" }, vb), h("b", {}, fa(o.value)))),
            h(
              "td",
              {},
              o.url
                ? h(
                    "button",
                    { class: "btn ghost sm icon-only", type: "button", title: "باز کردن صفحه کالا", onclick: () => safe(invoke("open_url", { url: link(o) })) },
                    icon("external", "sm"),
                  )
                : null,
            ),
          );
        }),
      );
    }
    rows();
    let sellers = [...new Set(r.offers.map((o) => o.seller).filter((x) => x && !Object.values(PNAME).includes(x)))];
    if (!sellers.length) {
      sellers = [...new Set(r.offers.map((o) => o.brand || o.title).filter((x) => x && !Object.values(PNAME).includes(x) && !BAD_BRAND_WORDS.has(x.trim())))].slice(0, 30);
    }
    const seed = h(
      "button",
      { class: "btn sm", type: "button", disabled: !sellers.length, title: sellers.length ? "" : "در این نتایج نام فروشنده ثبت نشده است" },
      icon("bank", "sm"),
      `${fa(sellers.length)} فروشنده به بانک`,
    );
    seed.addEventListener("click", () => seedModal(r.query, sellers));
    return h(
      "div",
      { class: "card flush" },
      h("div", { class: "card-h pad wrap" }, h("h2", {}, "پیشنهادهای قیمت"), box, fil, srt, seed),
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          { class: "t mkt-t" },
          h(
            "thead",
            {},
            h("tr", {}, ["#", "", "کالا", "کانال / روش خرید", "فروشنده", "امتیاز", "قیمت (تومان)", "هر کیلو", "بروزرسانی", "ارزش خرید", ""].map((t) => h("th", {}, t))),
          ),
          tbody,
        ),
      ),
      h(
        "p",
        { class: "small muted pad" },
        "«ارزش خرید» از ۱۰۰: ۵۵٪ مزیت قیمتی، ۳۵٪ امتیاز فروشنده، ۱۰٪ تعداد نظرها؛ قیمت‌های غیرعادی نصف امتیاز می‌گیرند. «بروزرسانی» زمانی است که منبع اعلام کرده (اگر اعلام کرده باشد).",
      ),
    );
  }

  function brands() {
    const list = cleanedBrands;
    const box = input({ placeholder: `جستجو در ${fa(list.length)} برند، شرکت و غرفه…` });
    const grid = h("div", { class: "brand-az" });
    const drawB = () => {
      const f = box.value.trim();
      clear(
        grid,
        list
          .filter((b) => !f || b.name.includes(f))
          .map((b) =>
            h(
              "div",
              { class: `az-row ${b.count ? "" : "dim"}`.trim() },
              h("b", {}, b.name),
              h("span", { class: "combo-pl" }, (b.platforms || []).map((p) => pbadge(p))),
              h("span", { class: "small muted" }, b.count ? `${fa(b.count)} کالا · از ${fa(b.min)} · میانه ${fa(b.median)}` : "برند شناخته‌شده این صنف"),
              h(
                "span",
                { class: "row tight-row" },
                h("button", { class: "btn ghost sm", type: "button", onclick: () => search(r.query, b.name) }, icon("search", "sm"), "فقط این برند / غرفه"),
                h("button", { class: "btn ghost sm", type: "button", title: "افزودن به «تحلیل برند»", onclick: () => addBrand(b.name, r.query) }, icon("award", "sm"), "تحلیل برند"),
              ),
            ),
          ),
      );
    };
    box.addEventListener("input", drawB);
    drawB();
    return h(
      "div",
      { class: "card stack" },
      h("div", { class: "card-h" }, h("h2", {}, "برندها، شرکت‌ها و غرفه‌های معتبر این بازار"), h("span", { class: "small muted" }, "به ترتیب بیشترین حضور و اعتبار در بازار")),
      box,
      grid,
    );
  }

  function channels() {
    const by = {};
    for (const o of r.offers) (by[o.platform] ||= []).push(o);
    const cards = Object.entries(by)
      .sort((x, y) => y[1].length - x[1].length)
      .map(([p, os]) => {
        const ps = os
          .map(basis)
          .filter((v) => v > 0)
          .sort((x, y) => x - y);
        const sellers = {};
        os.forEach((o) => o.seller && (sellers[o.seller] = (sellers[o.seller] || 0) + 1));
        const top = Object.entries(sellers)
          .sort((x, y) => y[1] - x[1])
          .slice(0, 5);
        const best = [...os].sort((x, y) => y.value - x.value)[0];
        const med = ps[Math.floor(ps.length / 2)] || 0;
        return h(
          "div",
          { class: "card stack ch-card" },
          h("div", { class: "row between" }, pbadge(p, true), h("span", { class: "small muted" }, METHOD[p] || "")),
          h(
            "div",
            { class: "ch-kpis" },
            h("div", {}, h("span", {}, "پیشنهاد"), h("b", {}, fa(os.length))),
            h("div", {}, h("span", {}, "فروشنده"), h("b", {}, fa(Object.keys(sellers).length))),
            h("div", {}, h("span", {}, "کمترین"), h("b", {}, fa(ps[0] || 0))),
            h("div", {}, h("span", {}, "میانه"), h("b", {}, fa(med))),
          ),
          s.median && med ? h("span", { class: "small" }, `سطح قیمت: ${fa(Math.round((med * 100) / s.median))} (۱۰۰ = میانه کل بازار)`) : null,
          best ? h("span", { class: "small muted" }, `بهترین پیشنهاد: ${best.seller || "—"} با ${toman(best.price)}`) : null,
          top.length ? h("div", { class: "chips" }, top.map(([n, c]) => h("span", { class: "chip" }, `${n} (${fa(c)})`))) : null,
        );
      });
    return h("div", { class: "ch-grid" }, cards);
  }

  function same() {
    const box = input({ placeholder: "جستجو در کالاها…" });
    const list = h("div", { class: "same-list" });
    const drawS = () => {
      const f = box.value.trim();
      clear(
        list,
        r.groups
          .filter((g) => !f || g.title.includes(f) || (g.brand || "").includes(f))
          .slice(0, 120)
          .map((g) => {
            const img = g.image ? h("img", { src: g.image, alt: "", loading: "lazy", referrerpolicy: "no-referrer" }) : icon("store");
            if (g.image) img.addEventListener("error", () => img.replaceWith(icon("store")));
            const cheapest = r.offers[g.offers[0]];
            return h(
              "div",
              { class: "card same" },
              h(
                "div",
                { class: "same-h" },
                h("span", { class: "thumb" }, img),
                h(
                  "div",
                  { class: "stack tight grow" },
                  h("b", { class: "same-t" }, g.title),
                  h(
                    "span",
                    { class: "small muted" },
                    [
                      g.brand && !BAD_BRAND_WORDS.has(g.brand.trim()) ? g.brand : "",
                      `${fa(g.offers.length)} فروشنده در ${fa(g.platforms.length)} کانال`,
                      g.spreadPct ? `اختلاف قیمت ${fa(g.spreadPct)}٪` : "",
                    ]
                      .filter(Boolean)
                      .join(" · "),
                  ),
                ),
                h("div", { class: "same-k" }, h("span", { class: "small muted" }, "ارزان‌ترین"), h("b", {}, toman(g.min))),
              ),
              h(
                "div",
                { class: "same-rows" },
                g.offers.slice(0, 8).map((i) => {
                  const o = r.offers[i];
                  return h(
                    "div",
                    { class: `same-r ${o === cheapest ? "best" : ""}`.trim() },
                    pbadge(o.platform, true),
                    h("span", { class: "clip small" }, o.seller || "—"),
                    h(
                      "span",
                      { class: "same-bar" },
                      (() => {
                        const b = h("i");
                        b.style.width = `${g.max > g.min ? 8 + ((o.price - g.min) / (g.max - g.min)) * 92 : 100}%`;
                        return b;
                      })(),
                    ),
                    h("b", { class: "num" }, fa(o.price)),
                    o.url
                      ? h("button", { class: "btn ghost sm icon-only", type: "button", title: "صفحه کالا", onclick: () => safe(invoke("open_url", { url: link(o) })) }, icon("external", "sm"))
                      : h("span"),
                  );
                }),
              ),
            );
          }),
      );
    };
    box.addEventListener("input", drawS);
    drawS();
    return h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "card row wrap between" },
        h(
          "div",
          { class: "stack tight" },
          h("b", {}, "یک کالا، قیمت هر فروشگاه"),
          h(
            "span",
            { class: "small muted" },
            "آگهی‌های یک کالا با نام‌های مختلف یکی شده‌اند. برند، وزن یا حجم، تعداد در بسته و مدل (مثل A55، ۲۵۶ گیگ، Pro) باید یکی باشند؛ عسل ۵۰۰ گرمی با یک کیلویی یا آیفون ۱۵ با ۱۵ پرو یکی نمی‌شود.",
          ),
        ),
        box,
      ),
      list,
    );
  }

  function sellers() {
    const by = new Map();
    for (const o of r.offers) {
      const name = o.seller || PNAME[o.platform] || o.platform;
      const s = by.get(name) || by.set(name, { name, platforms: new Set(), places: new Set(), offers: [] }).get(name);
      s.platforms.add(o.platform);
      if (o.location) s.places.add(o.location);
      s.offers.push(o);
    }
    const all = [...by.values()].map((s) => ({ ...s, min: Math.min(...s.offers.map((o) => o.price)) })).sort((a, b) => b.offers.length - a.offers.length || a.min - b.min);
    const places = [...new Set(r.offers.map((o) => o.location).filter(Boolean))].sort((a, b) => a.localeCompare(b, "fa"));
    const box = input({ placeholder: "جستجوی فروشنده…" });
    const where = select([["", `همه مناطق (${fa(places.length)})`], ...places.map((p) => [p, p])], "");
    const tbody = h("tbody");
    let shown = all;
    const drawT = () => {
      const f = box.value.trim();
      shown = all.filter((s) => (!f || s.name.includes(f)) && (!where.value || s.places.has(where.value)));
      clear(
        tbody,
        shown.map((s) =>
          h(
            "tr",
            {},
            h("td", {}, h("b", {}, s.name)),
            h("td", {}, h("div", { class: "row tight wrap" }, [...s.platforms].map((p) => pbadge(p, true)))),
            h("td", { class: "small" }, [...s.places].join("، ") || h("span", { class: "x" }, "—")),
            h("td", { class: "num" }, fa(s.offers.length)),
            h("td", { class: "num" }, fa(s.min)),
            h(
              "td",
              {},
              h(
                "div",
                { class: "row tight wrap" },
                s.offers.slice(0, 4).map((o) =>
                  h(
                    "button",
                    {
                      class: "btn ghost sm icon-only",
                      type: "button",
                      title: `${o.title} · ${fa(o.price)} تومان`,
                      onclick: () => safe(invoke("open_url", { url: link(o) })),
                    },
                    icon("external", "sm"),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
    };
    box.addEventListener("input", drawT);
    where.addEventListener("change", drawT);
    drawT();
    const links = () => shown.flatMap((s) => s.offers.map((o) => `${s.name}\t${o.location || ""}\t${o.price}\t${link(o)}`)).join("\n");
    const named = () => {
      const list = shown.map((s) => s.name).filter((n) => !Object.values(PNAME).includes(n));
      return list.length ? list : [...new Set(r.offers.map((o) => o.brand || o.title).filter((x) => x && !Object.values(PNAME).includes(x) && !BAD_BRAND_WORDS.has(x.trim())))].slice(0, 30);
    };
    return h(
      "div",
      { class: "card flush" },
      h(
        "div",
        { class: "card-h pad wrap" },
        h(
          "div",
          { class: "stack tight" },
          h("h2", {}, "فروشندگان و لینک آگهی‌ها"),
          h("span", { class: "small muted" }, "برای دیوار و شیپور منطقه را بالای صفحه انتخاب کنید."),
        ),
        box,
        where,
        h("button", { class: "btn sm", type: "button", onclick: () => copy(links(), "لینک‌ها کپی شد؛ در اکسل بچسبانید") }, icon("copy", "sm"), "کپی لینک‌ها"),
        h(
          "button",
          { class: "btn sm", type: "button", onclick: () => (named().length ? seedModal(r.query, named()) : toast("در این فهرست نام فروشگاه ثبت نشده است")) },
          icon("bank", "sm"),
          "به بانک مشتریان",
        ),
      ),
      h("div", { class: "table-wrap" }, h("table", { class: "t" }, h("thead", {}, h("tr", {}, ["فروشنده", "کانال", "منطقه", "آگهی", "کمترین قیمت", "لینک"].map((t) => h("th", {}, t)))), tbody)),
    );
  }

  function suppliers() {
    const list = r.suppliers || [];
    if (!list.length) return h("div", { class: "card" }, h("div", { class: "mkt-empty" }, h("div", { class: "ic-box lg" }, icon("layers")), h("h2", {}, "تأمین‌کننده‌ای پیدا نشد"), h("p", { class: "muted" }, "در حالت «خریدار عمده / B2B» دوباره تحلیل کنید تا تولیدکننده‌ها، عمده‌فروش‌ها و پخش‌کننده‌ها جستجو شوند.")));
    return h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "advice" },
        h("span", { class: "ic-box" }, icon("layers")),
        h("div", { class: "grow stack tight" }, h("b", {}, `${fa(list.length)} تأمین‌کننده B2B شناسایی شد.`), h("span", { class: "small muted" }, "فقط اطلاعات عمومی کسب‌وکار؛ هر مورد با نوع، اعتماد داده و لینک مستقیم. «درخواست استعلام» یک متن آماده می‌سازد (ارسال خودکار نمی‌شود).")),
        h("button", { class: "btn tonal sm", type: "button", onclick: () => rfqModal(r.query) }, icon("send", "sm"), "ساخت درخواست استعلام"),
      ),
      h(
        "div",
        { class: "sup-grid" },
        list.map((sp) =>
          h(
            "div",
            { class: "card stack sup-card" },
            h("div", { class: "row between" }, h("b", {}, sp.name || sp.source), h("span", { class: "pill" }, sp.kind)),
            h("div", { class: "sup-meta" }, [
              sp.city ? `${sp.city}` : "",
              sp.moq ? `حداقل سفارش: ${sp.moq}` : "",
              sp.phone ? `تماس: ${sp.phone}` : "",
              sp.email || "",
            ].filter(Boolean).map((x) => h("span", { class: "small muted" }, x))),
            sp.terms?.length ? h("div", { class: "chips" }, sp.terms.slice(0, 6).map((t) => h("span", { class: "chip" }, t))) : null,
            h(
              "div",
              { class: "row between" },
              confBadge(sp.confidence, "اعتماد داده"),
              h("button", { class: "btn ghost sm", type: "button", onclick: () => safe(invoke("open_url", { url: sp.url || sp.website })) }, icon("external", "sm"), "صفحه"),
            ),
          ),
        ),
      ),
    );
  }

  function webTab() {
    const list = r.web || [];
    if (!list.length) return h("div", { class: "card" }, h("div", { class: "mkt-empty" }, icon("globe"), h("h2", {}, "منبع وبی یافت نشد")));
    return h(
      "div",
      { class: "card flush" },
      h("div", { class: "card-h pad" }, h("h2", {}, "منابع وب (بهترین نتایج)"), h("span", { class: "small muted" }, "حداکثر ۳ نتیجه ارگانیک معتبر، با داده ساخت‌یافته صفحه")),
      h(
        "div",
        { class: "web-list" },
        list.map((w) =>
          h(
            "div",
            { class: "card web-row" },
            h(
              "div",
              { class: "row between wrap" },
              h("div", { class: "stack tight" }, h("b", {}, w.productName || w.title), h("span", { class: "small muted ltr" }, w.domain)),
              h("span", { class: "pill" }, w.pageType),
            ),
            h(
              "div",
              { class: "row wrap tight-row" },
              w.price ? h("span", { class: "chip" }, toman(w.price)) : null,
              w.availability ? h("span", { class: "chip" }, w.availability) : null,
              w.brand ? h("span", { class: "chip" }, w.brand) : null,
              h("span", { class: "small muted" }, `ارتباط ${fa(w.relevance)}٪`),
              confBadge(w.confidence, "اعتماد"),
            ),
            h(
              "div",
              { class: "row" },
              h("button", { class: "btn ghost sm", type: "button", onclick: () => safe(invoke("open_url", { url: w.productUrl || w.url })) }, icon("external", "sm"), w.productUrl ? "صفحه محصول" : "باز کردن"),
            ),
          ),
        ),
      ),
    );
  }

  function history() {
    const hs = r.history || [];
    const first = hs[0]?.[1],
      last = hs[hs.length - 1]?.[1];
    const lows = hs.map((p) => p[2] || p[1]).filter(Boolean);
    const bySource = {};
    for (const [d, name, med] of r.sourceHistory || []) (bySource[name] ||= []).push([d, med]);
    const series = Object.entries(bySource).map(([name, points]) => ({ name, cls: `p-${PID[name] || "x"}`, points }));
    const chart = series.some((s) => s.points.length) ? sourceChart(series) : null;
    return h(
      "div",
      { class: "stack" },
      hs.length >= 2
        ? h(
            "div",
            { class: "kpi-row" },
            h(
              "div",
              { class: "card kpi3" },
              h("span", { class: "t" }, "تغییر میانه از اولین تحلیل"),
              h("div", { class: "v" }, changePill(first ? Math.round(((last - first) / first) * 1000) / 10 : 0)),
              h("span", { class: "d" }, `${jdate(hs[0][0])} تا ${jdate(hs[hs.length - 1][0])}`),
            ),
            h("div", { class: "card kpi3" }, h("span", { class: "t" }, "کمترین قیمت دیده‌شده"), h("div", { class: "v" }, h("b", {}, fa(Math.min(...lows))), h("small", {}, "تومان")), h("span", { class: "d" }, unitLabel)),
            h("div", { class: "card kpi3" }, h("span", { class: "t" }, "بیشترین میانه"), h("div", { class: "v" }, h("b", {}, fa(Math.max(...hs.map((p) => p[1])))), h("small", {}, "تومان")), h("span", { class: "d" }, unitLabel)),
            h("div", { class: "card kpi3" }, h("span", { class: "t" }, "تعداد روزهای ثبت‌شده"), h("div", { class: "v" }, h("b", {}, fa(hs.length))), h("span", { class: "d" }, "با پایش روزانه، هر روز خودکار اضافه می‌شود")),
          )
        : null,
      h(
        "div",
        { class: "card stack" },
        h("div", { class: "card-h" }, h("h2", {}, "روند قیمت کل بازار"), h("span", { class: "legend" }, h("span", {}, h("i", { class: "m" }), "میانه"), h("span", {}, h("i", { class: "p" }), "کمترین"))),
        hs.length >= 2
          ? trend(hs)
          : h("p", { class: "small muted" }, "هر روز که این کالا تحلیل شود، یک نقطه به روند اضافه می‌شود. با «پایش روزانه» این کار خودکار انجام می‌شود."),
      ),
      chart
        ? h(
            "div",
            { class: "card stack" },
            h("div", { class: "card-h" }, h("h2", {}, "میانه قیمت هر منبع"), h("span", { class: "legend src" }, series.map((s) => h("span", {}, h("i", { class: s.cls }), s.name)))),
            chart,
            h("span", { class: "small muted" }, "هر نقطه آخرین تحلیل همان روز است؛ نشانگر را روی نقطه نگه دارید تا قیمت را ببینید."),
          )
        : null,
    );
  }

  const title = `${r.query}${r.brand ? ` — برند ${r.brand}` : ""}`;
  const sub = `${fa(s.count)} پیشنهاد معتبر · ${fa(r.channels.length)} کانال · ${fa(brandsN)} برند و غرفه · قیمت‌ها: ${unitLabel}${r.createdAt ? ` · ${ago(r.createdAt)}` : ""}`;
  const p = productPhoto(r.query);
  const head = p
    ? hero({ kicker: mode === "buyer" ? "از کجا بخرم؟" : "تحلیل محصول برای فروشنده", title, sub, photo: p, small: true, action: h("div", { class: "row wrap" }, statusLine) })
    : h(
        "div",
        { class: "card rep-head" },
        h("div", { class: "stack tight grow" }, h("span", { class: "hp-k" }, mode === "buyer" ? "از کجا بخرم؟" : "تحلیل محصول برای فروشنده"), h("h2", {}, title), h("span", { class: "small muted" }, sub)),
        h("div", { class: "row wrap" }, statusLine),
      );
  const tools = h(
    "div",
    { class: "row wrap rep-tools" },
    h(
      "button",
      {
        class: "btn sm",
        type: "button",
        title: "این کالا هر روز خودکار دوباره تحلیل شود و تغییر قیمت هشدار داده شود",
        onclick: () => watchModal(r),
      },
      icon("bell", "sm"),
      "افزودن به پایش روزانه",
    ),
    h(
      "button",
      {
        class: "btn sm",
        type: "button",
        title: "خلاصه این تحلیل با یک درخواست آماده در هوش مصنوعی گوگل (بیرون از برنامه) باز می‌شود",
        onclick: () => askGoogle(reportPrompt(r, mode)),
      },
      icon("sparkle", "sm"),
      "پرسش از هوش مصنوعی گوگل",
    ),
  );
  const el = h(
    "div",
    { class: "stack" },
    head,
    tools,
    filtered.length ? h("p", { class: "small muted filt" }, icon("info", "sm"), `کنار گذاشته شد: ${filtered.join("، ")}.`) : null,
    h("div", { class: "rep-shell" }, tabBar, body),
  );
  draw("summary");
  return el;
}

/** Adds the analysed product (same sources, range and mode) to the daily watchlist. */
export function watchModal(r, query = r.query, brand = r.brand || "") {
  const own = input({ inputmode: "numeric", dir: "ltr", placeholder: "اختیاری" });
  own.addEventListener("blur", () => (own.value = money(own.value) ? fa(money(own.value)) : ""));
  const pct = input({ type: "number", min: 1, max: 100, dir: "ltr", value: 5 });
  modal({
    title: "افزودن به پایش روزانه",
    sub: `«${[query, brand].filter(Boolean).join(" — ")}» هر روز در ساعت پایش دوباره تحلیل می‌شود؛ روند قیمت هر منبع خودکار ساخته می‌شود و تغییر مهم هشدار داده می‌شود.`,
    body: [
      field("قیمت فروش شما (تومان)", own, "اگر بدهید، وقتی قیمت شما از میانه بازار بالاتر برود یا از ارزان‌ترین رقیب پایین‌تر باشد خبر می‌دهیم."),
      field("آستانه هشدار (٪)", pct, "تغییر میانه یا کمترین قیمت بیشتر از این درصد، هشدار می‌شود."),
    ],
    actions: [
      { label: "انصراف" },
      {
        label: "افزودن",
        kind: "primary",
        onClick: async () => {
          const req = { ...(r.req || {}), query, brand };
          const id = await safe(
            invoke("watch_save", { item: { req, ownPrice: money(own.value), alertPct: Number(pct.value) || 5, enabled: true } }),
            "به پایش روزانه اضافه شد",
          );
          if (id === undefined) return false;
        },
      },
    ],
  });
}

/** A ready question for Google's AI from one analysis (seller or buyer). */
export function reportPrompt(r, mode = "seller") {
  const s = r.stats || {};
  const unit = s.basis === "per_kg" ? "هر کیلوگرم/لیتر" : "هر عدد";
  const ch = (r.channels || [])
    .slice(0, 5)
    .map((c) => `${c.name} (${c.count} پیشنهاد، میانه ${c.median})`)
    .join("، ");
  const br = (r.brands || [])
    .filter((b) => b.name !== "سایر / بدون برند" && !BAD_BRAND_WORDS.has(b.name.trim()))
    .slice(0, 6)
    .map((b) => `${b.name} ${b.pct}٪`)
    .join("، ");
  const data = `کالا: «${r.query}»${r.brand ? ` برند ${r.brand}` : ""}. بازار ایران امروز (تومان، ${unit}): کمترین ${s.min}، میانه ${s.median}، بیشترین ${s.max}، ${s.count} پیشنهاد معتبر. کانال‌ها: ${ch}. سهم برندها: ${br || "نامشخص"}.`;
  return mode === "buyer"
    ? `${data}\nمن خریدار هستم. با این داده‌ها بگو منصفانه‌ترین قیمت خرید چقدر است، از چه کانالی بخرم، چه نشانه‌هایی از کالای تقلبی یا قیمت غیرواقعی را بررسی کنم و بهترین زمان خرید چیست. پاسخ کوتاه و فارسی.`
    : `${data}\nمن فروشنده این کالا هستم و قیمت پیشنهادی سیستم ${r.srp} تومان است. با این داده‌ها: ۱) استراتژی قیمت‌گذاری و جایگاه مناسب، ۲) مزیت رقابتی در برابر برندهای اصلی، ۳) بهترین کانال فروش و ۴) سه ایده تبلیغ و فروش برای بازار ایران پیشنهاد بده. پاسخ کوتاه، کاربردی و فارسی.`;
}

/** Opens Google's AI in the browser with the question (also copied, for Gemini). */
export async function askGoogle(prompt) {
  await copy(prompt, "متن درخواست کپی شد");
  const url = prompt.length <= 1800 ? `https://www.google.com/search?udm=50&q=${encodeURIComponent(prompt)}` : "https://gemini.google.com/app";
  await safe(invoke("open_url", { url }));
}

async function addBrand(name, category) {
  const id = await safe(invoke("brand_save", { brand: { name, category, own: false } }), `«${name}» به تحلیل برند اضافه شد`);
  if (id !== undefined) go("brands");
}

/** Suppliers → a new bank, then their phone numbers are looked up by exact name. */
/** «درخواست استعلام قیمت» (RFQ): the user fills what they know, gets a ready-to-copy message. */
function rfqModal(product) {
  const f = {
    product: input({ value: product || "" }),
    quantity: input({ placeholder: "مثلاً ۵۰ کیلوگرم یا ۱۰ کارتن" }),
    city: input({ placeholder: "شهر مقصد" }),
    packaging: input({ placeholder: "نوع بسته‌بندی (اختیاری)" }),
    payment: input({ placeholder: "شرایط پرداخت (اختیاری)" }),
    leadTime: input({ placeholder: "زمان تأمین (اختیاری)" }),
    targetPrice: input({ placeholder: "قیمت هدف (اختیاری)" }),
  };
  const out = h("textarea", { class: "input ai-q", readonly: true, placeholder: "متن استعلام اینجا ساخته می‌شود…" });
  const build = async () => {
    const text = await safe(invoke("market_rfq", { req: { product: f.product.value, quantity: f.quantity.value, city: f.city.value, packaging: f.packaging.value, payment: f.payment.value, leadTime: f.leadTime.value, targetPrice: f.targetPrice.value } }));
    if (text) out.value = text;
  };
  modal({
    title: "درخواست استعلام قیمت (RFQ)",
    sub: "یک متن حرفه‌ای برای فرستادن به تأمین‌کننده؛ چیزی خودکار ارسال نمی‌شود.",
    wide: true,
    body: [
      h("div", { class: "grid-2" }, field("کالا", f.product), field("تعداد / مقدار", f.quantity), field("شهر مقصد", f.city), field("بسته‌بندی", f.packaging), field("شرایط پرداخت", f.payment), field("زمان تأمین", f.leadTime), field("قیمت هدف", f.targetPrice)),
      field("متن آماده", out),
    ],
    actions: [
      { label: "ساخت متن", kind: "primary", icon: "refresh", onClick: async () => { await build(); return false; } },
      { label: "کپی", icon: "copy", onClick: () => out.value && copy(out.value) },
      { label: "بستن" },
    ],
  });
  build();
}

function seedModal(query, sellers) {
  const name = input({ value: `تأمین‌کنندگان ${query}` });
  const picked = new Set(sellers.slice(0, 40));
  const list = h("div", { class: "seed-list" });
  const draw = () =>
    clear(
      list,
      sellers.map((s) => {
        const cb = h("input", { type: "checkbox", checked: picked.has(s), onchange: (e) => (e.target.checked ? picked.add(s) : picked.delete(s)) });
        return h("label", { class: "check" }, cb, h("span", {}, s));
      }),
    );
  draw();
  modal({
    title: "افزودن فروشندگان به بانک",
    sub: "برای هر فروشنده، صفحه کسب‌وکارش با نام دقیق جستجو می‌شود و اگر شماره عمومی داشت در بانک ذخیره می‌شود.",
    wide: true,
    body: [field("نام بانک", name), h("span", { class: "label-t" }, `${fa(sellers.length)} فروشنده`), list],
    actions: [
      { label: "انصراف" },
      {
        label: "ساخت بانک و شروع",
        kind: "primary",
        onClick: async () => {
          const names = [...picked];
          if (!names.length) {
            toast("حداقل یک فروشنده انتخاب کنید.", "bad");
            return false;
          }
          const listId = await safe(invoke("list_create", { name: name.value.trim() || `تأمین‌کنندگان ${query}`, color: "teal" }), "بانک ساخته شد");
          if (listId === undefined) return false;
          await refreshLists();
          runJob("scan", `یافتن شماره ${fa(names.length)} فروشنده`, "scan_start", {
            req: { listId, keywords: [query], lookups: names, limit: names.length, source: "maps", enrich: true },
          });
          toast("یافتن شماره‌ها در پس‌زمینه شروع شد؛ نتیجه در «بانک‌ها و مخاطبان».", "ok");
        },
      },
    ],
  });
}

// ---------------- saved analyses ----------------

export async function renderRuns(root) {
  const rows = await invoke("market_runs");
  if (!rows.length) {
    root.append(
      h(
        "div",
        { class: "card" },
        h(
          "div",
          { class: "empty" },
          icon("trend", "lg"),
          h("b", {}, "هنوز تحلیلی ذخیره نشده"),
          h("button", { class: "btn primary", type: "button", onclick: () => go("market") }, "اولین تحلیل محصول"),
        ),
      ),
    );
    return;
  }
  root.append(
    runsTable(rows, () => {
      clear(root);
      renderRuns(root);
    }),
  );
}

/** The saved product analyses as a table (also used on the History page). */
export function runsTable(rows, reload) {
  const tbody = h(
    "tbody",
    {},
    rows.map(([id, query, brand, offers, median, at]) =>
      h(
        "tr",
        { class: "click", onclick: (e) => !e.target.closest("button") && go("market", { run: id }) },
        h("td", {}, h("b", {}, query), brand ? h("span", { class: "x" }, ` — ${brand}`) : null),
        h("td", {}, fa(offers)),
        h("td", { class: "num" }, toman(median)),
        h("td", { class: "small" }, jdate(at, true)),
        h(
          "td",
          {},
          h(
            "button",
            {
              class: "btn ghost sm icon-only",
              type: "button",
              title: "حذف",
              onclick: async () => {
                if (await confirmBox(`تحلیل «${query}» حذف شود؟`, { ok: "حذف", danger: true })) {
                  await safe(invoke("market_delete", { id }));
                  reload();
                }
              },
            },
            icon("trash", "sm"),
          ),
        ),
      ),
    ),
  );
  return h(
    "div",
    { class: "card flush" },
    h("div", { class: "table-wrap" }, h("table", { class: "t" }, h("thead", {}, h("tr", {}, ["کالا", "پیشنهاد قیمت", "میانه", "تاریخ", ""].map((t) => h("th", {}, t)))), tbody)),
  );
}

export function leave() {}