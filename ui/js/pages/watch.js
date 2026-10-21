// «پایش روزانه قیمت» (watched products, re-analysed every day) and «کالاهای من» (the seller's own
// catalogue from Excel/CSV, each item identified in the market by brand, size and model).
import { h, icon, invoke, fa, clear, safe, input, field, toast, modal, confirmBox, ago, pickFile, fileToBase64, select } from "../core.js";
import { go } from "../state.js";
import { runJob, onJob } from "../jobs.js";
import { pbadge, sparkline, changePill, toman, money, watchModal } from "./market.js";
import { alertModal, buyboxLine, profitModal, trackFields } from "./profit.js";

const remember = (k) => {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
};
const VOL = (v) => (v >= 12 ? ["زیاد", "bad"] : v >= 5 ? ["متوسط", "warn"] : ["کم", "ok"]);
const DIR = { up: ["▲ رو به افزایش", "up"], down: ["▼ رو به کاهش", "down"], flat: ["— ثابت", ""] };

/** Re-runs when the job this page started has ended. */
function afterJob(fn) {
  let seen = false;
  const off = onJob((j) => {
    if (j.running) seen = true;
    else if (seen) {
      off();
      fn();
    }
  });
}

// ---------------- watchlist ----------------

export async function render(root, params, actions) {
  const [items, at] = await Promise.all([invoke("watch_list"), invoke("watch_time", { at: null })]);
  const reload = () => {
    clear(root);
    clear(actions);
    render(root, params, actions);
  };
  const time = input({ type: "time", value: at, dir: "ltr" });
  time.classList.add("sm");
  time.addEventListener("change", () => safe(invoke("watch_time", { at: time.value }), "ساعت پایش ذخیره شد"));
  const runAll = h("button", { class: "btn primary", type: "button", disabled: !items.some((w) => w.enabled) }, icon("refresh", "sm"), "بررسی همه الان");
  runAll.addEventListener("click", () => {
    runJob("watch", "پایش قیمت", "watch_run", { ids: [] });
    afterJob(reload);
  });
  actions.append(h("button", { class: "btn", type: "button", title: "تلگرام یا پیامک برای وقتی بای‌باکس از دست می‌رود", onclick: alertModal }, icon("bell", "sm"), "هشدار تلگرام / پیامک"), h("label", { class: "row tight-row small muted" }, icon("clock", "sm"), "پایش خودکار هر روز ساعت", time), runAll);

  if (!items.length) {
    root.append(
      h(
        "div",
        { class: "card" },
        h(
          "div",
          { class: "mkt-empty" },
          h("div", { class: "ic-box lg" }, icon("bell")),
          h("h2", {}, "قیمت کالاهای مهم را هر روز خودکار بپایید"),
          h("p", { class: "muted" }, "در «تحلیل محصول» یک کالا را تحلیل کنید و «افزودن به پایش روزانه» را بزنید. از آن به بعد:"),
          h(
            "ol",
            { class: "how cols" },
            h("li", {}, h("b", {}, "هر روز خودکار"), h("span", {}, "در ساعت پایش (برنامه باز یا در سینی ویندوز) همه کالاها دوباره تحلیل می‌شوند")),
            h("li", {}, h("b", {}, "نوسان و روند"), h("span", {}, "تغییر روزانه، ۷ و ۳۰ روزه، میزان نوسان و جهت روند")),
            h("li", {}, h("b", {}, "هشدار"), h("span", {}, "افت یا جهش قیمت، و وقتی قیمت شما از بازار عقب یا جلو می‌افتد")),
            h("li", {}, h("b", {}, "نمودار هر منبع"), h("span", {}, "روند قیمت ترب، دیجی‌کالا، باسلام و… جدا از هم")),
          ),
          h("button", { class: "btn primary", type: "button", onclick: () => go("market") }, icon("trend", "sm"), "رفتن به تحلیل محصول"),
        ),
      ),
    );
    return;
  }

  const alerts = items.filter((w) => w.alert);
  const up = items.filter((w) => w.swing.changePct > 0).length;
  const down = items.filter((w) => w.swing.changePct < 0).length;
  const kpi = (t, v, d, cls = "") => h("div", { class: `card kpi3 ${cls}`.trim() }, h("span", { class: "t" }, t), h("div", { class: "v" }, h("b", {}, v)), h("span", { class: "d" }, d));

  const tbody = h(
    "tbody",
    {},
    items.map((w) => {
      const [vl, vc] = VOL(w.swing.volatility);
      const [dl, dc] = DIR[w.swing.direction] || DIR.flat;
      const perKg = w.basis === "per_kg";
      const own = w.ownPrice && w.median && !perKg ? Math.round(((w.ownPrice - w.median) / w.median) * 1000) / 10 : null;
      return h(
        "tr",
        { class: w.enabled ? "" : "dim" },
        h("td", {}, h("div", { class: "name-cell" }, h("b", { class: "n" }, w.name), h("span", { class: "c" }, [w.req.platforms?.length ? `${fa(w.req.platforms.length)} منبع` : "", perKg ? "قیمت هر کیلو/لیتر" : ""].filter(Boolean).join(" · ")))),
        h("td", {}, sparkline(w.history.map((p) => p[1]))),
        h("td", { class: "num" }, w.median ? fa(w.median) : "—", w.min ? h("div", { class: "x small" }, `کمترین ${fa(w.min)}`) : null),
        h("td", {}, changePill(w.swing.changePct)),
        h("td", {}, changePill(w.swing.change7)),
        h("td", {}, changePill(w.swing.change30)),
        h("td", {}, w.swing.points >= 3 ? h("span", { class: `pill ${vc}`, title: `ضریب تغییرات میانه‌های روزانه: ${fa(w.swing.volatility)}٪` }, vl) : h("span", { class: "x small" }, "داده کم"), w.swing.points >= 3 ? h("div", { class: `small trend-t ${dc}` }, dl) : null),
        h("td", {}, w.ownPrice ? [h("span", { class: "num" }, fa(w.ownPrice)), own !== null ? h("div", { class: `small ${own > 0 ? "warn-t" : "ok-t"}` }, own > 0 ? `${fa(own)}٪ بالای میانه` : `${fa(Math.abs(own))}٪ زیر میانه`) : null] : h("span", { class: "x" }, "—")),
        h("td", {}, buyboxLine(w.buybox)),
        h("td", { class: "small" }, w.lastRun ? ago(w.lastRun) : "هنوز نه", w.error ? h("div", { class: "x small warn-t", title: w.error }, "خطا در آخرین بررسی") : null),
        h(
          "td",
          {},
          h(
            "div",
            { class: "row tight-row" },
            w.lastId ? h("button", { class: "btn ghost sm icon-only", type: "button", title: "آخرین تحلیل", onclick: () => go("market", { run: w.lastId }) }, icon("chart", "sm")) : null,
            h("button", { class: "btn ghost sm icon-only", type: "button", title: "بررسی همین الان", onclick: () => { runJob("watch", `پایش «${w.name}»`, "watch_run", { ids: [w.id] }); afterJob(reload); } }, icon("refresh", "sm")),
            h("button", { class: "btn ghost sm icon-only", type: "button", title: "ویرایش", onclick: () => editWatch(w, reload) }, icon("edit", "sm")),
            h("button", { class: "btn ghost sm icon-only", type: "button", title: "حذف", onclick: async () => { if (await confirmBox(`«${w.name}» از پایش حذف شود؟ تحلیل‌های ذخیره‌شده‌اش می‌مانند.`, { ok: "حذف", danger: true })) { await safe(invoke("watch_delete", { id: w.id })); reload(); } } }, icon("trash", "sm")),
          ),
        ),
      );
    }),
  );

  root.append(
    h(
      "div",
      { class: "stack" },
      h("div", { class: "kpi-row" }, kpi("کالای در پایش", fa(items.length), `${fa(items.filter((w) => w.enabled).length)} فعال`), kpi("گران‌تر شد", fa(up), "از بررسی قبل"), kpi("ارزان‌تر شد", fa(down), "از بررسی قبل"), kpi("هشدار", fa(alerts.length), "تغییر مهم یا فاصله از قیمت شما", alerts.length ? "hero" : "")),
      alerts.length
        ? h(
            "div",
            { class: "card stack tight alert-card" },
            h("div", { class: "card-h" }, h("h2", {}, icon("alert", "sm"), "هشدارهای آخرین بررسی")),
            alerts.map((w) => h("div", { class: "alert-row" }, h("b", {}, w.name), h("span", { class: "small" }, w.alert), w.lastId ? h("button", { class: "btn ghost sm", type: "button", onclick: () => go("market", { run: w.lastId }) }, "جزئیات") : null)),
          )
        : null,
      h(
        "div",
        { class: "card flush" },
        h("div", { class: "table-wrap" }, h("table", { class: "t watch-t" }, h("thead", {}, h("tr", {}, ["کالا", "روند", "میانه امروز", "از قبل", "۷ روز", "۳۰ روز", "نوسان", "قیمت شما", "بای‌باکس", "آخرین بررسی", ""].map((t) => h("th", {}, t)))), tbody)),
        h("p", { class: "small muted pad" }, "قیمت‌ها به تومان است؛ اگر کالا در بسته‌های مختلف عرضه شود، به ازای هر کیلوگرم یا لیتر. «نوسان» میزان بالا و پایین رفتن قیمت در روزهای ثبت‌شده است."),
      ),
    ),
  );
}

function editWatch(w, done) {
  const name = input({ value: w.name });
  const own = input({ inputmode: "numeric", dir: "ltr", value: w.ownPrice ? fa(w.ownPrice) : "" });
  const pct = input({ type: "number", min: 1, max: 100, dir: "ltr", value: w.alertPct || 5 });
  const on = h("input", { type: "checkbox", checked: w.enabled });
  const cost = input({ inputmode: "numeric", dir: "ltr", value: w.cost ? fa(w.cost) : "", placeholder: "برای کف سود ریپرایسر" });
  const track = trackFields(w);
  modal({
    title: "ویرایش پایش",
    body: [field("نام", name), field("قیمت فروش شما (تومان)", own), field("بهای خرید (تومان)", cost), field("آستانه هشدار (٪)", pct), track.el, h("label", { class: "check" }, on, h("span", {}, "پایش روزانه فعال باشد"))],
    actions: [
      { label: "انصراف" },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () => {
          const id = await safe(invoke("watch_save", { item: { ...w, name: name.value, ownPrice: money(own.value), cost: money(cost.value), alertPct: Number(pct.value) || 5, enabled: on.checked, ...track.value(), history: [], swing: {} } }), "ذخیره شد");
          if (id === undefined) return false;
          done();
        },
      },
    ],
  });
}

// ---------------- my products (catalogue) ----------------

const CONF = { high: ["دقیق", "ok"], medium: ["احتمالی", "warn"], none: ["پیدا نشد", "bad"], "": ["بررسی نشده", ""] };

export async function renderCatalog(root, params, actions) {
  const items = await invoke("catalog_list");
  const reload = () => {
    clear(root);
    clear(actions);
    renderCatalog(root, params, actions);
  };
  const picked = new Set();
  const platforms = (remember("mktPlatforms") || "torob,digikala,basalam,emalls").split(",").filter((p) => !["divar", "sheypoor"].includes(p));
  const check = (ids) => {
    runJob("catalog", ids.length === 1 ? "بررسی کالا در بازار" : `بررسی ${fa(ids.length || items.length)} کالا در بازار`, "catalog_check", { ids, platforms, margin: Number(remember("mktMargin")) || 25 });
    afterJob(reload);
  };
  actions.append(
    h("button", { class: "btn", type: "button", onclick: () => importFlow(reload) }, icon("upload", "sm"), "ورود فایل اکسل / CSV"),
    items.length ? h("button", { class: "btn primary", type: "button", onclick: () => check([]) }, icon("search", "sm"), "بررسی همه در بازار") : null,
  );

  if (!items.length) {
    root.append(
      h(
        "div",
        { class: "card" },
        h(
          "div",
          { class: "mkt-empty" },
          h("div", { class: "ic-box lg" }, icon("store")),
          h("h2", {}, "همه کالاهای خودتان را یک‌جا با بازار مقایسه کنید"),
          h("p", { class: "muted" }, "فایل اکسل یا CSV کالاهایتان را وارد کنید. ردیف اول عنوان ستون‌هاست:"),
          h(
            "ol",
            { class: "how cols" },
            h("li", {}, h("b", {}, "نام کالا (لازم)"), h("span", {}, "هرچه کامل‌تر بهتر: برند، مدل، وزن یا حجم، تعداد در بسته")),
            h("li", {}, h("b", {}, "قیمت فروش"), h("span", {}, "تومان؛ اگر ستون «ریال» باشد خودکار تبدیل می‌شود")),
            h("li", {}, h("b", {}, "قیمت خرید، برند، کد کالا"), h("span", {}, "اختیاری؛ با قیمت خرید، قیمت پیشنهادی با حاشیه سود شما حساب می‌شود")),
            h("li", {}, h("b", {}, "تشخیص دقیق"), h("span", {}, "فقط آگهی‌هایی که برند، اندازه و مدلشان با کالای شما یکی است حساب می‌شوند")),
          ),
          h("button", { class: "btn primary", type: "button", onclick: () => importFlow(reload) }, icon("upload", "sm"), "ورود فایل"),
        ),
      ),
    );
    return;
  }

  const checked = items.filter((i) => i.confidence);
  const found = items.filter((i) => i.confidence === "high" || i.confidence === "medium");
  const cheap = found.filter((i) => i.position >= 0 && i.position < 25).length;
  const dear = found.filter((i) => i.position > 75).length;
  const kpi = (t, v, d, cls = "") => h("div", { class: `card kpi3 ${cls}`.trim() }, h("span", { class: "t" }, t), h("div", { class: "v" }, h("b", {}, v)), h("span", { class: "d" }, d));

  let filter = "";
  const tbody = h("tbody");
  const del = h("button", { class: "btn sm", type: "button", disabled: true }, icon("trash", "sm"), "حذف انتخاب‌شده");
  const chk = h("button", { class: "btn sm", type: "button", disabled: true }, icon("search", "sm"), "بررسی انتخاب‌شده");
  const syncBulk = () => {
    del.disabled = chk.disabled = !picked.size;
  };
  del.addEventListener("click", async () => {
    if (await confirmBox(`${fa(picked.size)} کالا حذف شود؟`, { ok: "حذف", danger: true })) {
      await safe(invoke("catalog_delete", { ids: [...picked] }));
      reload();
    }
  });
  chk.addEventListener("click", () => check([...picked]));
  const show = select(
    [["", "همه کالاها"], ["dear", "گران‌تر از بازار"], ["cheap", "ارزان‌تر از بازار"], ["medium", "تشخیص احتمالی"], ["none", "پیدا نشده"], ["new", "بررسی نشده"]],
    "",
    { onchange: (e) => { filter = e.target.value; rows(); } },
  );
  show.classList.add("sm", "pill-select");
  const box = input({ placeholder: "جستجو در کالاها…" });
  box.classList.add("sm");
  box.addEventListener("input", () => rows());

  function rows() {
    const f = box.value.trim();
    const list = items.filter(
      (i) =>
        (!f || i.name.includes(f) || i.sku.includes(f) || i.brand.includes(f)) &&
        (!filter || (filter === "dear" && i.position > 75) || (filter === "cheap" && i.position >= 0 && i.position < 25) || (filter === "medium" && i.confidence === "medium") || (filter === "none" && i.confidence === "none") || (filter === "new" && !i.confidence)),
    );
    clear(
      tbody,
      list.map((i) => {
        const [cl, cc] = CONF[i.confidence] || CONF[""];
        const cb = h("input", { type: "checkbox", checked: picked.has(i.id), onchange: (e) => { e.target.checked ? picked.add(i.id) : picked.delete(i.id); syncBulk(); } });
        const pos = h("span", { class: "pos-bar" }, h("i"));
        if (i.position >= 0) pos.firstChild.style.insetInlineStart = `calc(${i.position}% - 5px)`;
        return h(
          "tr",
          {},
          h("td", {}, cb),
          h("td", {}, h("div", { class: "name-cell" }, h("b", { class: "n full" }, i.name), h("span", { class: "c" }, [i.brand, i.sku ? `کد ${i.sku}` : ""].filter(Boolean).join(" · ")))),
          h("td", { class: "num" }, i.ownPrice ? fa(i.ownPrice) : "—", i.cost ? h("div", { class: "x small" }, `خرید ${fa(i.cost)}`) : null),
          h("td", {}, h("span", { class: `pill ${cc}`.trim(), title: i.note || "" }, cl), i.matched ? h("button", { class: "btn ghost sm", type: "button", onclick: () => matchesModal(i) }, `${fa(i.matched)} آگهی`) : null),
          h("td", { class: "num" }, i.median ? [fa(i.median), h("div", { class: "x small" }, `${fa(i.min)} تا ${fa(i.max)}`)] : "—"),
          h("td", {}, i.position >= 0 ? h("div", { class: "stack tight" }, pos, h("span", { class: `small ${i.position > 75 ? "warn-t" : i.position < 25 ? "ok-t" : ""}`.trim() }, i.position > 75 ? "گران‌تر از بازار" : i.position < 25 ? "ارزان‌تر از بازار" : "رقابتی")) : h("span", { class: "x" }, "—")),
          h("td", { class: "num" }, i.srp ? h("b", {}, fa(i.srp)) : "—"),
          h("td", {}, buyboxLine(i.buybox)),
          h("td", { class: "small" }, i.checkedAt ? ago(i.checkedAt) : "—", i.note ? h("div", { class: "x small clip", title: i.note }, i.note) : null),
          h(
            "td",
            {},
            h(
              "div",
              { class: "row tight-row" },
              h("button", { class: "btn ghost sm icon-only", type: "button", title: "بررسی همین کالا", onclick: () => check([i.id]) }, icon("refresh", "sm")),
              i.runId ? h("button", { class: "btn ghost sm icon-only", type: "button", title: "تحلیل کامل بازار", onclick: () => go("market", { run: i.runId }) }, icon("chart", "sm")) : null,
              h("button", { class: "btn ghost sm icon-only", type: "button", title: "سود روی هر کانال فروش", onclick: () => profitModal({ name: i.name, price: i.ownPrice || i.srp, cost: i.cost, category: `${i.name} ${i.brand}` }) }, icon("chart", "sm")),
              h("button", { class: "btn ghost sm icon-only", type: "button", title: "ردیابی بای‌باکس و هشدار", onclick: () => trackModal(i, reload) }, icon("target", "sm")),
              h("button", { class: "btn ghost sm icon-only", type: "button", title: "افزودن به پایش روزانه", onclick: () => watchModal({ req: { exact: false, platforms }, query: i.query }, i.query, "") }, icon("bell", "sm")),
            ),
          ),
        );
      }),
    );
  }
  rows();

  root.append(
    h(
      "div",
      { class: "stack" },
      h("div", { class: "kpi-row" }, kpi("کالاهای من", fa(items.length), `${fa(checked.length)} بررسی‌شده`), kpi("شناسایی‌شده در بازار", fa(found.length), `${fa(found.filter((i) => i.confidence === "high").length)} با تطبیق دقیق`), kpi("گران‌تر از بازار", fa(dear), "قیمت شما بالای ۷۵٪ عرضه‌ها", dear ? "hero" : ""), kpi("جای افزایش قیمت", fa(cheap), "قیمت شما زیر ۲۵٪ عرضه‌ها")),
      h(
        "div",
        { class: "card flush" },
        h("div", { class: "card-h pad wrap" }, h("h2", {}, "کالاها و جایگاه قیمت"), box, show, chk, del),
        h("div", { class: "table-wrap" }, h("table", { class: "t cat-t" }, h("thead", {}, h("tr", {}, ["", "کالا", "قیمت شما", "تطبیق", "میانه بازار", "جایگاه قیمت شما", "قیمت پیشنهادی", "بای‌باکس", "آخرین بررسی", ""].map((t) => h("th", {}, t)))), tbody)),
        h("p", { class: "small muted pad" }, "«دقیق»: برند، اندازه، تعداد و مدل آگهی با کالای شما یکی است. «احتمالی»: بیشتر کلمه‌ها یکی است و هیچ مشخصه‌ای مغایر نیست؛ آگهی‌ها را با دکمه تعداد آگهی ببینید. قیمت‌های پرت (قدیمی یا اشتباه) کنار گذاشته می‌شوند."),
      ),
    ),
  );
}

/** Buy-box tracking of one catalogue item. */
function trackModal(i, done) {
  const t = trackFields(i);
  modal({
    title: "ردیابی بای‌باکس",
    sub: `«${i.name}» — با «بررسی در بازار» جایگاه شما بین فروشندگان همین کالا سنجیده و قیمت پیشنهادی ریپرایسر نشان داده می‌شود.`,
    body: [t.el, h("button", { class: "btn sm", type: "button", onclick: alertModal }, icon("bell", "sm"), "تنظیمات هشدار تلگرام / پیامک")],
    actions: [
      { label: "انصراف" },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () => {
          const v = t.value();
          const ok = await safe(invoke("catalog_track", { id: i.id, seller: v.mySellerName, url: v.myProductUrl, margin: v.minMarginPct, channel: v.alertChannel }), "ذخیره شد");
          if (ok === undefined) return false;
          done();
        },
      },
    ],
  });
}

function matchesModal(i) {
  modal({
    title: `آگهی‌های شناسایی‌شده: ${i.name}`,
    sub: "این آگهی‌ها همان کالای شما تشخیص داده شدند و قیمت بازار از آن‌ها حساب شد.",
    wide: true,
    body: h(
      "div",
      { class: "same-rows" },
      i.matches.map((m) =>
        h(
          "div",
          { class: "same-r wide" },
          pbadge(m.platform, true),
          h("span", { class: "clip small", title: m.title }, m.title),
          h("span", { class: "clip small muted" }, m.seller || "—"),
          h("b", { class: "num" }, toman(m.price)),
          m.url ? h("button", { class: "btn ghost sm icon-only", type: "button", title: "صفحه کالا", onclick: () => safe(invoke("open_url", { url: m.url.startsWith("http") ? m.url : `https://torob.com${m.url}` })) }, icon("external", "sm")) : h("span"),
        ),
      ),
    ),
    actions: [{ label: "بستن" }],
  });
}

function importFlow(done) {
  modal({
    title: "ورود کالاهای من",
    sub: "اکسل (xlsx) یا CSV. ستون‌های «نام کالا»، «قیمت فروش»، «قیمت خرید»، «برند» و «کد کالا» خودکار شناخته می‌شوند؛ ورود دوباره همان فایل قیمت‌ها را به‌روز می‌کند.",
    body: [],
    actions: [
      { label: "انصراف" },
      {
        label: "انتخاب فایل",
        kind: "primary",
        icon: "upload",
        onClick: async () => {
          const file = await pickFile(".xlsx,.xls,.xlsm,.ods,.csv,.txt,.tsv");
          if (!file) return false;
          if (file.size > 20 * 1024 * 1024) return toast("فایل بزرگ‌تر از ۲۰ مگابایت است.", "bad"), false;
          const rep = await safe(invoke("catalog_import", { fileName: file.name, data: await fileToBase64(file) }));
          if (!rep) return false;
          toast(`${fa(rep.added)} کالا اضافه و ${fa(rep.updated)} کالا به‌روز شد.`, "ok");
          done();
        },
      },
    ],
  });
}
