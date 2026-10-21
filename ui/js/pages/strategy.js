// «استراتژی و بیزینس پلن» of a brand: a few answers from the owner (sector, model, stage, goal,
// budget, price, cost …) + the brand's live analysis → executive summary, business plan, business
// model canvas, business model, positioning & branding, advertising mix with budget, a 12-month growth
// roadmap, unit economics with a 12-month projection, risks and KPIs. Printable as a PDF, and can be
// handed to an AI assistant for longer prose.
import { h, icon, invoke, fa, clear, safe, input, field, select, modal, copy, pickFile, toast } from "../core.js";
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
  ["brandbook", "برندبوک"],
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
  const pct = (v, ph) => input({ value: v ?? "", placeholder: ph, inputmode: "decimal", dir: "ltr" });
  const feePct = pct(p.channelFeePct, "خالی = نرخ معمول حوزه (۵ تا ۱۸٪)");
  const vatPct = pct(p.vatPct, "خالی = ۱۰٪");
  const packCost = input({ value: p.packCost ? fa(p.packCost) : "", placeholder: "خالی = سهم معمول حوزه", inputmode: "numeric" });
  const returnPct = pct(p.returnPct, "خالی = نرخ معمول حوزه (۲ تا ۸٪)");
  const advisor = input({ value: p.advisor || "", placeholder: "نام مشاور یا آژانس (روی جلد)" });
  const client = input({ value: p.client || "", placeholder: "نام مشتری (روی جلد)" });
  let logo = p.logo || "";
  const logoShow = h("div", { class: "row tight-row" });
  const drawLogo = () =>
    clear(
      logoShow,
      logo ? h("img", { class: "st-logo-prev", src: logo, alt: "لوگو" }) : h("span", { class: "small muted" }, "بدون لوگو"),
      h("button", { class: "btn sm", type: "button", onclick: async () => { const l = await pickLogo(); if (l) { logo = l; drawLogo(); } } }, icon("upload", "sm"), "انتخاب لوگو"),
      logo ? h("button", { class: "btn ghost sm", type: "button", onclick: () => { logo = ""; drawLogo(); } }, icon("x", "sm"), "حذف") : null,
    );
  drawLogo();
  const optNum = (el) => (el.value.trim() === "" ? null : Number(String(el.value).replace(/[۰-۹]/g, (d) => "۰۱۲۳۴۵۶۷۸۹".indexOf(d)).replace("٫", ".")) );
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
      h(
        "details",
        { class: "fold" },
        h("summary", {}, "کارمزد کانال، مالیات، مرجوعی و بسته‌بندی (اختیاری — برای محاسبه دقیق‌تر سود)"),
        h("div", { class: "grid-2" }, field("کارمزد کانال فروش (٪)", feePct, "دیجی‌کالا، باسلام، اسنپ‌شاپ…"), field("مالیات بر ارزش افزوده کارمزد (٪)", vatPct), field("بسته‌بندی و ارسال هر واحد (تومان)", packCost), field("نرخ مرجوعی (٪)", returnPct)),
      ),
      h(
        "details",
        { class: "fold" },
        h("summary", {}, "جلد گزارش (برای مشاوران و آژانس‌ها)"),
        h("div", { class: "grid-2" }, field("نام مشاور / آژانس", advisor), field("نام مشتری", client)),
        field("لوگو", logoShow, "کوچک‌شده و در همین برنامه ذخیره می‌شود؛ روی جلد گزارش و اسلایدها می‌آید."),
      ),
    ],
    actions: [
      { label: "انصراف" },
      {
        label: "ساخت برنامه",
        kind: "primary",
        icon: "sparkle",
        onClick: async () => {
          const inputs = { sector: sector.value, model: model.value, stage: stage.value, goal: goal.value, audience: audience.value.trim(), usp: usp.value.trim(), region: region.value.trim(), budget: num(budget.value), price: num(price.value), unitCost: num(cost.value), fixedCost: num(fixed.value), capacity: num(capacity.value), channelFeePct: optNum(feePct), vatPct: optNum(vatPct), packCost: num(packCost.value) || null, returnPct: optNum(returnPct), advisor: advisor.value.trim(), client: client.value.trim(), logo };
          const st = await safe(invoke("brand_strategy", { id: b.id, inputs }));
          if (!st) return false;
          b.plan = inputs;
          done(st);
        },
      },
    ],
  });
}

/** A picked image, shrunk to at most 220 px and kept as a small PNG data: URL (the page may only show data: images). */
async function pickLogo() {
  const file = await pickFile("image/png,image/jpeg,image/webp,image/svg+xml");
  if (!file) return "";
  if (file.size > 5 * 1024 * 1024) return toast("تصویر بزرگ‌تر از ۵ مگابایت است.", "bad"), "";
  const src = await new Promise((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(String(r.result));
    r.onerror = () => rej();
    r.readAsDataURL(file);
  }).catch(() => "");
  const img = new Image();
  const ok = await new Promise((res) => {
    img.onload = () => res(true);
    img.onerror = () => res(false);
    img.src = src;
  });
  if (!ok || !img.width) return toast("تصویر خوانده نشد.", "bad"), "";
  const k = Math.min(1, 220 / Math.max(img.width, img.height));
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(img.width * k));
  c.height = Math.max(1, Math.round(img.height * k));
  c.getContext("2d").drawImage(img, 0, 0, c.width, c.height);
  const url = c.toDataURL("image/png");
  return url.length < 250_000 ? url : (toast("لوگو هنوز بزرگ است؛ تصویر ساده‌تری انتخاب کنید.", "bad"), "");
}

/** Black or white, whichever reads on `hex`. */
const inkOn = (hex) => {
  const n = parseInt(String(hex).replace("#", ""), 16) || 0;
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? "#16181d" : "#ffffff";
};

/** Palette, fonts, tone of voice and archetype — one page a designer can start from. */
function brandbookPanel(st) {
  const bb = st.brandbook || {};
  const palette = h(
    "div",
    { class: "bb-palette" },
    (bb.palette || []).map((c) => {
      const chip = h("button", { class: "bb-chip", type: "button", title: "کپی کد رنگ", onclick: () => copy(c.hex, `${c.hex} کپی شد`) }, h("b", { class: "ltr" }, c.hex));
      chip.style.background = c.hex;
      chip.style.color = inkOn(c.hex);
      return h("div", { class: "bb-sw" }, chip, h("b", { class: "small" }, c.role), h("span", { class: "small muted" }, c.usage));
    }),
  );
  const sample = (font, text, cls) => {
    const el = h("div", { class: `bb-sample ${cls}` }, text);
    el.style.fontFamily = `"${font}", "Yekan Bakh", Tahoma, sans-serif`;
    return el;
  };
  const tone = (bb.tone || []).map((a) => {
    const dot = h("i", { class: "bb-dot" });
    dot.style.insetInlineStart = `calc(${Math.max(0, Math.min(100, a.value))}% - 7px)`;
    return h("div", { class: "bb-tone" }, h("span", { class: "small" }, a.left), h("span", { class: "bb-track" }, dot), h("span", { class: "small" }, a.right));
  });
  return [
    h("div", { class: "row wrap" }, h("span", { class: "label-t" }, "شخصیت برند"), h("span", { class: "pill info" }, bb.archetype || "—")),
    h("div", { class: "st-block" }, h("h3", {}, "پالت رنگ پیشنهادی"), palette),
    h(
      "div",
      { class: "grid-2" },
      h("div", { class: "st-block" }, h("h3", {}, "جفت فونت"), sample(bb.headingFont, "عنوان‌ها: هامیستا", "head"), h("span", { class: "small muted" }, bb.headingFont), sample(bb.bodyFont, "متن اصلی: کیفیتی که به آن اعتماد می‌کنید.", "body"), h("span", { class: "small muted" }, bb.bodyFont), h("p", { class: "small" }, bb.fontNote)),
      h("div", { class: "st-block" }, h("h3", {}, "ماتریس لحن برند"), h("div", { class: "stack" }, tone)),
    ),
  ];
}

// ---------------- pitch deck (16:9) ----------------

/** The plan as slides: [{title, body}] — shown one by one and printed one per page. */
function deckSlides(st) {
  const f = st.finance || {};
  const inp = st.inputs || {};
  const stat = (v, l) => h("div", { class: "stat" }, h("b", {}, v), h("span", {}, l));
  const first = (blocks, n) => (blocks || []).slice(0, n).map(blockCard);
  const bb = st.brandbook || {};
  const swatches = h(
    "div",
    { class: "bb-palette deck" },
    (bb.palette || []).map((c) => {
      const chip = h("span", { class: "bb-chip" }, h("b", { class: "ltr" }, c.hex));
      chip.style.background = c.hex;
      chip.style.color = inkOn(c.hex);
      return h("div", { class: "bb-sw" }, chip, h("span", { class: "small" }, c.role));
    }),
  );
  return [
    {
      cover: true,
      title: st.brand,
      body: [
        inp.logo ? h("img", { class: "deck-logo", src: inp.logo, alt: "" }) : null,
        h("h1", {}, st.brand),
        h("p", {}, [st.sector, st.model, st.stage].filter(Boolean).join(" · ")),
        h("p", { class: "small" }, [inp.client ? `برای ${inp.client}` : "", inp.advisor ? `تهیه‌شده توسط ${inp.advisor}` : "", new Date().toLocaleDateString("fa-IR")].filter(Boolean).join(" — ")),
      ],
    },
    { title: "خلاصه مدیریتی", body: [h("div", { class: "grid-4" }, stat(toman(f.price), "قیمت هر واحد"), stat(toman(f.unitMargin), `سود هر واحد (${fa(f.marginPct)}٪)`), stat(`${fa(f.breakEvenUnits)} واحد`, "سربه‌سر ماهانه"), stat(toman(f.budget), "بودجه تبلیغات ماهانه")), h("div", { class: "st-prose" }, (st.summary || []).slice(0, 3).map((t) => h("p", {}, t)))] },
    { title: "جایگاه و پیشنهاد ارزش", body: [h("div", { class: "st-blocks" }, first(st.positioning, 4))] },
    { title: "بوم کسب‌وکار", body: [h("div", { class: "st-canvas" }, (st.canvas || []).map(blockCard))] },
    { title: "مدل کسب‌وکار و بازار", body: [h("div", { class: "st-blocks" }, [...first(st.businessModel, 2), ...first(st.plan, 2)])] },
    { title: "هویت بصری", body: [swatches, h("div", { class: "grid-2" }, h("p", {}, `فونت عنوان: ${bb.headingFont || "—"}`), h("p", {}, `فونت متن: ${bb.bodyFont || "—"}`)), h("p", { class: "small" }, `شخصیت برند: ${bb.archetype || "—"}`)] },
    {
      title: "تبلیغات و بودجه",
      body: [
        h(
          "div",
          { class: "stack" },
          (st.ads || []).slice(0, 6).map((a) => {
            const i = h("i");
            i.style.width = `${a.share}%`;
            return h("div", { class: "deck-bar" }, h("b", {}, a.name), h("span", { class: "track" }, i), h("span", {}, `${fa(a.share)}٪ · ${toman(a.budget)}`));
          }),
        ),
      ],
    },
    { title: "نقشه رشد ۱۲ ماهه", body: [h("ol", { class: "deck-road" }, (st.roadmap || []).map((p) => h("li", {}, h("b", {}, p.title), h("span", { class: "pill info" }, p.window), h("span", { class: "small" }, p.goal))))] },
    {
      title: "پیش‌بینی مالی",
      body: [
        h("div", { class: "grid-4" }, stat(toman(f.price), "قیمت فروش"), stat(toman(f.unitCost), "بهای تمام‌شده"), stat(toman((f.channelFee || 0) + (f.vat || 0)), "کارمزد کانال + ارزش افزوده"), stat(toman(f.unitMargin), "سود هر واحد")),
        h("div", { class: "grid-4" }, stat(toman(f.pack), "بسته‌بندی و ارسال"), stat(toman(f.returns), "هزینه مرجوعی"), stat(toman(f.fixed), "هزینه ثابت ماهانه"), stat(toman((f.months || []).at(-1)?.cumulative), "سود انباشته ماه ۱۲")),
      ],
    },
    { title: "ریسک‌ها و شاخص‌ها", body: [h("div", { class: "grid-2" }, h("div", { class: "st-block" }, h("h3", {}, "ریسک‌ها"), list((st.risks || []).slice(0, 4).map(([r, m]) => `${r}: ${m}`), "small")), h("div", { class: "st-block" }, h("h3", {}, "شاخص‌های کلیدی"), list((st.kpis || []).slice(0, 5).map(([k, t]) => `${k}: ${t}`), "small")))] },
  ];
}

const slideEl = (sl, i, n) => h("section", { class: `slide ${sl.cover ? "cover" : ""}`.trim() }, sl.cover ? null : h("h2", {}, sl.title), h("div", { class: "slide-b" }, sl.body), h("span", { class: "slide-n small" }, `${fa(i + 1)} / ${fa(n)}`));

/** Slide mode: one 16:9 slide at a time (arrow keys / buttons) and a print of all of them, one per page. */
function deckModal(st) {
  const slides = deckSlides(st);
  let i = 0;
  const stage = h("div", { class: "deck-stage" });
  const draw = () => clear(stage, slideEl(slides[i], i, slides.length));
  const move = (d) => {
    i = Math.max(0, Math.min(slides.length - 1, i + d));
    draw();
  };
  const printAll = () => {
    const sheet = h("div", { class: "print-sheet deck" }, slides.map((s, k) => slideEl(s, k, slides.length)));
    document.body.append(sheet);
    document.body.classList.add("printing");
    addEventListener("afterprint", () => { document.body.classList.remove("printing"); sheet.remove(); }, { once: true });
    window.print();
  };
  const dlg = modal({
    title: "حالت اسلاید (۱۶:۹)",
    sub: "با کلیدهای جهت‌دار جابه‌جا شوید؛ «چاپ / PDF» همه اسلایدها را هر کدام در یک صفحه می‌دهد.",
    wide: true,
    body: [stage, h("div", { class: "row between" }, h("button", { class: "btn sm", type: "button", onclick: () => move(1) }, "اسلاید بعد"), h("button", { class: "btn sm", type: "button", onclick: () => move(-1) }, "اسلاید قبل"))],
    actions: [{ label: "بستن" }, { label: "چاپ / PDF اسلایدها", kind: "primary", icon: "download", onClick: () => (printAll(), false) }],
  });
  dlg.el.addEventListener("keydown", (e) => {
    if (e.key === "ArrowLeft") move(1); // RTL: left = next
    if (e.key === "ArrowRight") move(-1);
  });
  draw();
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
    brandbook: brandbookPanel(st),
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
      h("div", { class: "grid-4" }, kpi(toman(f.price), "قیمت فروش"), kpi(toman(f.unitCost), "بهای تمام‌شده"), kpi(toman(f.channelFee), `کارمزد کانال (${fa(f.feePct)}٪)`), kpi(toman(f.vat), `مالیات ارزش افزوده کارمزد (${fa(f.vatPct)}٪)`)),
      h("div", { class: "grid-4" }, kpi(toman(f.pack), "بسته‌بندی و ارسال"), kpi(toman(f.returns), `هزینه مرجوعی (${fa(f.returnPct)}٪)`), kpi(toman(f.unitMargin), "سود خالص هر واحد", "ok"), kpi(`${fa(f.marginPct)}٪`, "حاشیه سود خالص")),
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
        st.inputs?.logo ? h("img", { class: "st-cover-logo", src: st.inputs.logo, alt: "" }) : null,
        h("div", { class: "grow stack tight" }, h("span", { class: "label-t" }, "استراتژی و بیزینس پلن"), h("h1", {}, st.brand), h("span", { class: "small" }, [st.sector, st.model, st.stage, `هدف: ${st.goal}`].join(" · ")), st.inputs?.client || st.inputs?.advisor ? h("span", { class: "small" }, [st.inputs.client ? `برای ${st.inputs.client}` : "", st.inputs.advisor ? `تهیه‌شده توسط ${st.inputs.advisor}` : ""].filter(Boolean).join(" — ")) : null),
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
          h("button", { class: "btn sm tonal", type: "button", title: "همین برنامه به شکل اسلایدهای ۱۶:۹ برای ارائه", onclick: () => deckModal(st) }, icon("layers", "sm"), "حالت اسلاید"),
          h("button", { class: "btn sm primary", type: "button", onclick: print }, icon("download", "sm"), "چاپ / PDF"),
        ),
      ),
      sections,
      h("p", { class: "small faint st-foot" }, st.inputs?.advisor ? `تهیه‌شده توسط ${st.inputs.advisor}${st.inputs.client ? ` برای ${st.inputs.client}` : ""} — اعداد بر پایه داده‌های تحلیل و فرض‌های همین برنامه است.` : "تهیه‌شده با هامیستا سلر — اعداد بر پایه داده‌های تحلیل و فرض‌های همین برنامه است؛ با تغییر پاسخ‌ها دوباره ساخته می‌شود."),
    ),
  );
}
