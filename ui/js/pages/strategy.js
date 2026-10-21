// «استراتژی و بیزینس پلن» of a brand: a few answers from the owner (sector, model, stage, goal,
// budget, price, cost …) + the brand's live analysis → executive summary, business plan, business
// model canvas, business model, positioning & branding, advertising mix with budget, a 12-month growth
// roadmap, unit economics with a 12-month projection, risks and KPIs. Printable as a PDF, and can be
// handed to an AI assistant for longer prose.
import { h, icon, invoke, fa, clear, safe, input, field, select, modal, copy } from "../core.js";
import { askGoogle } from "./market.js";

const SECTORS = [
  ["", "تشخیص خودکار از دسته کالا"],
  ["food", "مواد غذایی، کشاورزی و ارگانیک"],
  ["fashion", "پوشاک، کیف و کفش و اکسسوری"],
  ["beauty", "آرایشی، بهداشتی و مراقبت پوست"],
  ["home", "لوازم خانگی، دکوراسیون و مبلمان"],
  ["electronics", "کالای دیجیتال و الکترونیک"],
  ["handicraft", "صنایع دستی، هنری و هدیه"],
  ["health", "سلامت، مکمل و گیاهان دارویی"],
  ["industrial", "صنعتی، ساختمانی و تجهیزات (B2B)"],
  ["services", "خدمات (کافه، رستوران، آموزش، زیبایی و …)"],
  ["general", "کالای مصرفی عمومی"],
];
const MODELS = [
  ["", "خودکار"],
  ["b2c_online", "فروش آنلاین به مصرف‌کننده"],
  ["d2c", "برند مستقیم به مصرف‌کننده (سایت اختصاصی)"],
  ["b2b", "فروش عمده و سازمانی (B2B)"],
  ["manufacturer", "تولیدکننده با فروش چندکاناله"],
  ["retail", "فروشگاه حضوری + آنلاین"],
  ["service", "ارائه خدمات"],
];
const STAGES = [["", "خودکار"], ["idea", "ایده و پیش از راه‌اندازی"], ["startup", "نوپا (کمتر از ۲ سال)"], ["growth", "در حال رشد"], ["established", "تثبیت‌شده"]];
const GOALS = [["", "افزایش فروش"], ["awareness", "شناخته‌شدن برند"], ["launch", "عرضه محصول/برند جدید"], ["wholesale", "ورود به فروش عمده و سازمانی"], ["export", "صادرات"]];
const TABS = [
  ["summary", "خلاصه مدیریتی"],
  ["canvas", "بوم کسب‌وکار"],
  ["model", "مدل کسب‌وکار"],
  ["plan", "بیزینس پلن"],
  ["brand", "جایگاه و برندینگ"],
  ["ads", "تبلیغات و محتوا"],
  ["roadmap", "نقشه رشد ۱۲ ماهه"],
  ["finance", "پیش‌بینی مالی"],
  ["risks", "ریسک‌ها و شاخص‌ها"],
];

const toman = (n) => `${fa(n || 0)} تومان`;
/** «۲۰٬۰۰۰٬۰۰۰», "20,000,000" or "۲۰ میلیون" → 20000000. */
const num = (s) => {
  const t = String(s || "").replace(/[۰-۹]/g, (d) => "۰۱۲۳۴۵۶۷۸۹".indexOf(d)).replace(/[٠-٩]/g, (d) => "٠١٢٣٤٥٦٧٨٩".indexOf(d));
  const v = Number(t.replace(/[^\d.]/g, "")) || 0;
  return Math.round(/میلیون/.test(t) ? v * 1e6 : /هزار/.test(t) ? v * 1e3 : v);
};

/** The card under a brand's analysis: what the plan holds, and the button that builds it. */
export function strategyCard(b) {
  const out = h("div", { class: "stack" });
  const start = () => inputsModal(b, (st) => show(out, b, st));
  const card = h(
    "div",
    { class: "card st-hero" },
    h("div", { class: "ic-box lg" }, icon("sparkle")),
    h(
      "div",
      { class: "grow stack tight" },
      h("h2", {}, "استراتژی و بیزینس پلن"),
      h("p", { class: "small muted" }, `برای «${b.name}» بیزینس پلن، بوم کسب‌وکار، مدل درآمدی، پیشنهاد برندینگ، برنامه تبلیغات با بودجه، نقشه راه رشد ۱۲ ماهه و پیش‌بینی مالی ساخته می‌شود؛ بر پایه حوزه کسب‌وکار، پاسخ‌های شما و داده‌های واقعی تحلیل برند.`),
    ),
    h("button", { class: "btn accent", type: "button", onclick: start }, icon("sparkle", "sm"), b.plan ? "به‌روزرسانی برنامه" : "ساخت برنامه"),
  );
  // answered before: rebuild silently so the plan is there on every visit
  if (b.plan) safe(invoke("brand_strategy", { id: b.id, inputs: b.plan })).then((st) => st && show(out, b, st));
  return h("div", { class: "stack" }, card, out);
}

function inputsModal(b, done) {
  const p = b.plan || {};
  const sector = select(SECTORS, p.sector || "");
  const model = select(MODELS, p.model || "");
  const stage = select(STAGES, p.stage || "");
  const goal = select(GOALS, p.goal || "");
  const audience = input({ value: p.audience || "", placeholder: "مثلاً خانواده‌های شهری ۲۵ تا ۴۵ سال" });
  const usp = input({ value: p.usp || "", placeholder: "مثلاً عسل گون کوهستان با برگه آزمایش هر سری" });
  const region = input({ value: p.region || "", placeholder: "سراسر ایران" });
  const money = (v, ph) => input({ value: v ? fa(v) : "", placeholder: ph, inputmode: "numeric" });
  const budget = money(p.budget, "مثلاً ۲۰ میلیون");
  const price = money(p.price, "خالی = میانه بازار / فروشگاه شما");
  const cost = money(p.unitCost, "خالی = حاشیه معمول این حوزه");
  const fixed = money(p.fixedCost, "اجاره، حقوق، ابزارها");
  const capacity = money(p.capacity, "خالی = بدون محدودیت");
  modal({
    title: `استراتژی و بیزینس پلن «${b.name}»`,
    sub: "همه موارد اختیاری است؛ هرچه دقیق‌تر بنویسید، برنامه و اعداد دقیق‌تر می‌شود. پاسخ‌ها ذخیره می‌شوند.",
    wide: true,
    body: [
      h("div", { class: "grid-2" }, field("حوزه کسب‌وکار", sector), field("مدل کسب‌وکار", model), field("مرحله کسب‌وکار", stage), field("هدف اصلی ۱۲ ماه آینده", goal)),
      h("div", { class: "grid-2" }, field("مخاطب اصلی", audience), field("منطقه فروش", region)),
      field("مزیت و تمایز محصول", usp, "به زبان خودتان: چرا مشتری باید از شما بخرد؟"),
      h("div", { class: "grid-2" }, field("بودجه تبلیغات ماهانه (تومان)", budget), field("هزینه‌های ثابت ماهانه (تومان)", fixed), field("قیمت فروش هر واحد (تومان)", price), field("بهای تمام‌شده هر واحد (تومان)", cost)),
      field("ظرفیت تولید / تأمین در ماه (واحد)", capacity),
    ],
    actions: [
      { label: "انصراف" },
      {
        label: "ساخت برنامه",
        kind: "primary",
        icon: "sparkle",
        onClick: async () => {
          const inputs = { sector: sector.value, model: model.value, stage: stage.value, goal: goal.value, audience: audience.value.trim(), usp: usp.value.trim(), region: region.value.trim(), budget: num(budget.value), price: num(price.value), unitCost: num(cost.value), fixedCost: num(fixed.value), capacity: num(capacity.value) };
          const st = await safe(invoke("brand_strategy", { id: b.id, inputs }));
          if (!st) return false;
          b.plan = inputs;
          done(st);
        },
      },
    ],
  });
}

// ---------------- report ----------------

const list = (items, cls = "") => h("ul", { class: `st-list ${cls}`.trim() }, items.map((t) => h("li", {}, t)));
const blockCard = (bk) => h("div", { class: `st-block cv-${bk.key}` }, h("h3", {}, bk.title), list(bk.items));
const blocks = (arr) => h("div", { class: "st-blocks" }, arr.map(blockCard));
const kpi = (v, l, cls = "") => h("div", { class: `stat ${cls}`.trim() }, h("b", {}, v), h("span", {}, l));

function show(out, b, st) {
  const f = st.finance || {};
  const months = f.months || [];
  const last = months[months.length - 1] || {};
  const paybackAt = months.find((m) => m.cumulative >= 0 && m.month > 1 && months[m.month - 2]?.cumulative < 0)?.month;
  const panels = {
    summary: [
      h("div", { class: "grid-4" }, kpi(toman(f.price), "قیمت هر واحد"), kpi(toman(f.unitMargin), `سود هر واحد (${fa(f.marginPct)}٪)`), kpi(`${fa(f.breakEvenUnits)} واحد`, "سربه‌سر ماهانه"), kpi(toman(f.budget), "بودجه تبلیغات ماهانه")),
      h("div", { class: "st-prose" }, st.summary.map((t) => h("p", {}, t))),
      st.evidence?.length ? h("div", { class: "st-evidence" }, h("span", { class: "label-t" }, "بر پایه داده‌های واقعی:"), list(st.evidence, "small")) : h("p", { class: "small muted" }, "برند هنوز تحلیل نشده است؛ برنامه بر پایه حوزه و پاسخ‌های شماست. پس از «تحلیل»، داده‌های بازار، فروشگاه و رقبا هم وارد برنامه می‌شود."),
    ],
    canvas: [h("p", { class: "small muted" }, "بوم کسب‌وکار ۹ بخشی (Business Model Canvas) — برای جلسه، چاپ یا ارائه به سرمایه‌گذار."), h("div", { class: "st-canvas" }, st.canvas.map(blockCard))],
    model: [blocks(st.businessModel)],
    plan: [blocks(st.plan)],
    brand: [blocks(st.positioning), blocks(st.branding)],
    ads: [
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          { class: "t st-ads" },
          h("thead", {}, h("tr", {}, ["کانال", "سهم بودجه", "بودجه ماهانه", "چرا", "شاخص موفقیت"].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            st.ads.map((a) => {
              const bar = h("i");
              requestAnimationFrame(() => (bar.style.width = `${a.share}%`));
              return h(
                "tr",
                {},
                h("td", {}, h("b", {}, a.name), list(a.actions, "small")),
                h("td", {}, h("div", { class: "st-share" }, h("span", { class: "track" }, bar), h("b", {}, `${fa(a.share)}٪`))),
                h("td", { class: "nowrap" }, toman(a.budget)),
                h("td", { class: "small" }, a.why),
                h("td", { class: "small muted" }, a.kpi),
              );
            }),
          ),
        ),
      ),
      h("div", { class: "grid-2" }, h("div", { class: "st-block" }, h("h3", {}, "ایده‌های محتوا"), list(st.content)), h("div", { class: "st-block" }, h("h3", {}, "تقویم فصلی و مناسبت‌ها"), list(st.seasons))),
    ],
    roadmap: [
      h(
        "ol",
        { class: "st-road" },
        st.roadmap.map((p, i) =>
          h(
            "li",
            {},
            h("span", { class: "st-step" }, fa(i + 1)),
            h(
              "div",
              { class: "stack tight grow" },
              h("div", { class: "row between wrap" }, h("h3", {}, p.title), h("span", { class: "pill info" }, p.window)),
              h("p", { class: "small muted" }, `هدف: ${p.goal}`),
              h("div", { class: "grid-2" }, h("div", {}, h("span", { class: "label-t" }, "کارها"), list(p.tasks)), h("div", {}, h("span", { class: "label-t" }, "شاخص پایان مرحله"), list(p.kpis, "small"))),
            ),
          ),
        ),
      ),
    ],
    finance: [
      h("div", { class: "grid-4" }, kpi(toman(f.price), "قیمت فروش"), kpi(toman(f.unitCost), "بهای تمام‌شده"), kpi(toman(f.channelFee), "کارمزد کانال / ارسال"), kpi(toman(f.unitMargin), "سود هر واحد", "ok")),
      h(
        "div",
        { class: "grid-4" },
        kpi(toman(f.fixed), "هزینه ثابت ماهانه"),
        kpi(`${fa(f.breakEvenUnits)} واحد`, "سربه‌سر ماهانه"),
        kpi(toman(last.revenue), "درآمد ماه ۱۲"),
        kpi(paybackAt ? `ماه ${fa(paybackAt)}` : last.cumulative >= 0 ? "از ابتدا" : "پس از ۱۲ ماه", "بازگشت سرمایه تبلیغات"),
      ),
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          { class: "t st-fin" },
          h("thead", {}, h("tr", {}, ["ماه", "فروش (واحد)", "درآمد", "سود ناخالص", "تبلیغات", "هزینه ثابت", "سود ماه", "انباشته"].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            months.map((m) =>
              h(
                "tr",
                {},
                h("td", {}, fa(m.month)),
                h("td", {}, fa(m.units)),
                h("td", {}, fa(m.revenue)),
                h("td", {}, fa(m.gross)),
                h("td", {}, fa(m.marketing)),
                h("td", {}, fa(m.fixed)),
                h("td", { class: m.profit < 0 ? "neg" : "pos" }, fa(m.profit)),
                h("td", { class: m.cumulative < 0 ? "neg" : "pos" }, fa(m.cumulative)),
              ),
            ),
          ),
        ),
      ),
      h("div", { class: "st-evidence" }, h("span", { class: "label-t" }, "فرض‌ها (مبالغ به تومان):"), list(f.basis || [], "small")),
    ],
    risks: [
      h("div", { class: "table-wrap" }, h("table", { class: "t" }, h("thead", {}, h("tr", {}, h("th", {}, "ریسک"), h("th", {}, "راه کنترل"))), h("tbody", {}, st.risks.map(([r, m]) => h("tr", {}, h("td", {}, h("b", {}, r)), h("td", { class: "small" }, m)))))),
      h("div", { class: "table-wrap" }, h("table", { class: "t" }, h("thead", {}, h("tr", {}, h("th", {}, "شاخص کلیدی (KPI)"), h("th", {}, "هدف"))), h("tbody", {}, st.kpis.map(([k, t]) => h("tr", {}, h("td", {}, k), h("td", { class: "small" }, t)))))),
    ],
  };
  let tab = "summary";
  const sections = TABS.map(([k, t]) => h("section", { class: "st-panel", "data-k": k, hidden: k !== tab }, h("h2", { class: "st-print-h" }, t), panels[k]));
  const bar = h("div", { class: "tabs st-tabs", role: "tablist" });
  const drawTabs = () => {
    clear(bar, TABS.map(([k, t]) => h("button", { class: k === tab ? "on" : "", type: "button", role: "tab", onclick: () => { tab = k; drawTabs(); } }, t)));
    sections.forEach((s) => (s.hidden = s.dataset.k !== tab));
  };
  drawTabs();
  // the app shell scrolls inside fixed panes: print a copy of the plan as the only thing on the page
  const print = () => {
    const copy_ = out.querySelector(".st-report").cloneNode(true);
    copy_.querySelectorAll(".st-panel").forEach((x) => (x.hidden = false));
    const sheet = h("div", { class: "print-sheet" }, copy_);
    document.body.append(sheet);
    document.body.classList.add("printing");
    addEventListener("afterprint", () => { document.body.classList.remove("printing"); sheet.remove(); }, { once: true });
    window.print();
  };
  clear(
    out,
    h(
      "div",
      { class: "card st-report" },
      h(
        "div",
        { class: "st-cover" },
        h("div", { class: "grow stack tight" }, h("span", { class: "label-t" }, "استراتژی و بیزینس پلن"), h("h1", {}, st.brand), h("span", { class: "small" }, [st.sector, st.model, st.stage, `هدف: ${st.goal}`].join(" · "))),
        h("span", { class: "small muted" }, new Date().toLocaleDateString("fa-IR")),
      ),
      h(
        "div",
        { class: "row between wrap st-tools" },
        bar,
        h(
          "div",
          { class: "row" },
          h("button", { class: "btn sm", type: "button", onclick: () => inputsModal(b, (s2) => show(out, b, s2)) }, icon("edit", "sm"), "ویرایش پاسخ‌ها"),
          h("button", { class: "btn sm", type: "button", onclick: () => copy(st.prompt, "خلاصه برنامه کپی شد") }, icon("copy", "sm"), "کپی"),
          h("button", { class: "btn sm tonal", type: "button", title: "متن کامل برنامه برای دستیار هوش مصنوعی کپی و باز می‌شود", onclick: () => askGoogle(st.prompt) }, icon("sparkle", "sm"), "نگارش کامل با هوش مصنوعی"),
          h("button", { class: "btn sm primary", type: "button", onclick: print }, icon("download", "sm"), "چاپ / PDF"),
        ),
      ),
      sections,
      h("p", { class: "small faint st-foot" }, "تهیه‌شده با هامیستا سلر — اعداد بر پایه داده‌های تحلیل و فرض‌های همین برنامه است؛ با تغییر پاسخ‌ها دوباره ساخته می‌شود."),
    ),
  );
}
