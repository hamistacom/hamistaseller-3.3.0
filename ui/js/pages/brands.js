// «تحلیل برند»: our brand and competitors — brand health index on four pillars (market presence,
// perceived quality, digital presence, social), price positioning, a positioning map, and actions.
import { h, icon, invoke, fa, clear, safe, input, field, toggle, toast, modal, confirmBox, ago, newChannel } from "../core.js";
import { pbadge, confBadge } from "./market.js";
import { go } from "../state.js";

const PILLARS = [
  ["presence", "حضور در بازار", "سهم از عرضه‌ها و تعداد پلتفرم‌ها در جستجوهای «محصول و قیمت»"],
  ["quality", "کیفیت درک‌شده", "امتیاز و تعداد نظر خریداران در فروشگاه‌ها"],
  ["digital", "حضور دیجیتال", "سایت رسمی: در دسترس، امن، قابل جستجو، راه تماس، سرعت"],
  ["social", "شبکه اجتماعی", "اندازه پیج عمومی اینستاگرام"],
];

const PNAME = { torob: "ترب", digikala: "دیجی‌کالا", basalam: "باسلام", emalls: "ایمالز", divar: "دیوار", sheypoor: "شیپور" };
const NS = "http://www.w3.org/2000/svg";
function svg(tag, attrs = {}, ...kids) {
  const el = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  kids.flat().forEach((k) => k && el.append(k instanceof Node ? k : document.createTextNode(String(k))));
  return el;
}

/** Circular score (0–100) drawn with a conic gradient whose angle is set through CSSOM. */
function ring(v, cls = "") {
  const el = h("div", { class: `ring ${cls}`.trim(), role: "img", "aria-label": `امتیاز ${fa(v)} از ۱۰۰` }, h("b", {}, v ? fa(v) : "—"), h("span", {}, "از ۱۰۰"));
  requestAnimationFrame(() => el.style.setProperty("--p", `${v || 0}`));
  return el;
}

function pillarBars(rep) {
  return h(
    "div",
    { class: "pbars" },
    PILLARS.map(([k, label, tip]) => {
      const v = rep?.[k];
      const fill = h("i", { class: v == null ? "na" : v >= 70 ? "good" : v >= 45 ? "mid" : "low" });
      requestAnimationFrame(() => requestAnimationFrame(() => (fill.style.width = `${v ?? 0}%`)));
      return h("div", { class: "pbar", title: tip }, h("span", {}, label), h("span", { class: "track" }, fill), h("b", {}, v == null ? "بدون داده" : fa(v)));
    }),
  );
}

/** Price index (x) vs buyers' rating (y): where each brand sits. */
function positioning(brands) {
  const pts = brands.filter((b) => b.report?.market?.priceIndex && b.report?.market?.rating);
  if (pts.length < 1) return h("p", { class: "small muted" }, "برای نقشه جایگاه، برندها باید در جستجوهای «محصول و قیمت» قیمت و امتیاز داشته باشند.");
  const W = 520, H = 300, L = 44, R = 16, T = 16, B = 40;
  const xs = pts.map((b) => b.report.market.priceIndex);
  const x0 = Math.min(60, ...xs) - 5, x1 = Math.max(150, ...xs) + 5;
  const y0 = 3, y1 = 5;
  const x = (v) => L + ((v - x0) / (x1 - x0)) * (W - L - R);
  const y = (v) => T + (1 - (Math.max(y0, Math.min(y1, v)) - y0) / (y1 - y0)) * (H - T - B);
  const quad = [
    [x0, 100, 4, y1, "ارزش بالا"],
    [100, x1, 4, y1, "پریمیوم موجه"],
    [x0, 100, y0, 4, "ارزان"],
    [100, x1, y0, 4, "گران بی‌پشتوانه"],
  ].map(([a, b, c, d, label]) => [svg("rect", { x: x(a), y: y(d), width: x(b) - x(a), height: y(c) - y(d), class: "quad" }), svg("text", { x: x(a) + 8, y: y(d) + 16, class: "ql" }, label)]);
  const axes = [
    svg("line", { x1: x(100), x2: x(100), y1: T, y2: H - B, class: "mid" }),
    svg("line", { x1: L, x2: W - R, y1: y(4), y2: y(4), class: "mid" }),
    svg("text", { x: (W + L) / 2, y: H - 8, class: "ax", "text-anchor": "middle" }, "شاخص قیمت (۱۰۰ = میانه بازار)"),
    svg("text", { x: 12, y: (H - B) / 2, class: "ax", transform: `rotate(-90 12 ${(H - B) / 2})`, "text-anchor": "middle" }, "امتیاز خریداران"),
    ...[x0, 100, x1].map((v) => svg("text", { x: x(v), y: H - B + 16, class: "tk", "text-anchor": "middle" }, fa(Math.round(v)))),
    ...[3, 4, 5].map((v) => svg("text", { x: L - 8, y: y(v) + 4, class: "tk", "text-anchor": "end" }, fa(v))),
  ];
  const dots = pts.map((b) => {
    const m = b.report.market;
    return svg("g", { class: `dot ${b.own ? "own" : ""}`.trim() }, svg("circle", { cx: x(m.priceIndex), cy: y(m.rating), r: b.own ? 8 : 6 }, svg("title", {}, `${b.name}: شاخص قیمت ${fa(m.priceIndex)} · امتیاز ${fa(m.rating)}`)), svg("text", { x: x(m.priceIndex), y: y(m.rating) - 12, "text-anchor": "middle" }, b.name));
  });
  return svg("svg", { viewBox: `0 0 ${W} ${H}`, class: "posmap", role: "img", "aria-label": "نقشه جایگاه برندها" }, quad, axes, dots);
}

function brandModal(b, done) {
  const name = input({ value: b?.name || "", placeholder: "مثلاً عسل خوانسار" });
  const site = input({ value: b?.website || "", dir: "ltr", placeholder: "example.ir" });
  const ig = input({ value: b?.instagram || "", dir: "ltr", placeholder: "@brand" });
  const cat = input({ value: b?.category || "", placeholder: "همان کالایی که در «محصول و قیمت» جستجو می‌کنید؛ مثلاً عسل" });
  const own = toggle("این برند ماست", !!b?.own);
  const note = input({ value: b?.note || "", placeholder: "اختیاری" });
  modal({
    title: b?.id ? "ویرایش برند" : "افزودن برند",
    wide: true,
    body: [h("div", { class: "grid-2" }, field("نام برند", name), field("دسته کالا", cat)), h("div", { class: "grid-2" }, field("سایت رسمی", site, "خالی بماند، از روی نام پیدا می‌شود"), field("اینستاگرام", ig, "خالی بماند، از روی نام پیدا می‌شود")), field("یادداشت", note), own.el],
    actions: [
      { label: "انصراف" },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () => {
          const id = await safe(invoke("brand_save", { brand: { id: b?.id || 0, name: name.value, website: site.value, instagram: ig.value, category: cat.value, note: note.value, own: own.input.checked } }), "ذخیره شد");
          if (id === undefined) return false;
          done(id);
        },
      },
    ],
  });
  name.focus();
}

export async function render(root, params, actions) {
  let brands = await invoke("brands");
  let selected = Number(params.id) || brands.find((b) => b.own)?.id || brands[0]?.id || 0;
  const body = h("div", { class: "stack" });
  const reload = async (id) => {
    brands = await invoke("brands");
    if (id) selected = id;
    draw();
  };
  // analysing searches the web and the marketplaces: a live progress line under the header with Continue/Skip & Stop buttons
  const progress = h("div", { class: "mkt-progress", hidden: true });
  const analyze = async (ids, btn) => {
    if (btn) btn.disabled = true;
    progress.hidden = false;
    for (const id of ids) {
      const b = brands.find((x) => x.id === id);
      const line = h("span", { class: "small grow" }, `تحلیل «${b?.name}»: پیدا کردن سایت، اینستاگرام و محصولات در فروشگاه‌ها…`);
      const skipBtn = h(
        "button",
        {
          class: "btn sm tonal",
          type: "button",
          hidden: true,
          title: "عبور از مرحله تأیید «ربات نیستم» و ادامه خودکار روی بقیه منابع",
          onclick: () => {
            skipBtn.disabled = true;
            line.textContent = `«${b?.name}»: در حال عبور از این مرحله و ادامه…`;
            invoke("browser_detect", { custom: "__skip_wait__" }).catch(() => {});
          },
        },
        icon("skip", "sm"),
        "رد کردن این مرحله و ادامه",
      );
      const stopBtn = h(
        "button",
        {
          class: "btn sm ghost danger",
          type: "button",
          onclick: () => invoke("job_stop").catch(() => {}),
        },
        icon("stop", "sm"),
        "توقف",
      );
      clear(
        progress,
        h("div", { class: "row between wrap" }, line, h("div", { class: "row tight-row" }, skipBtn, stopBtn)),
        h("div", { class: "bar indeterminate" }, h("i")),
      );
      const ch = newChannel();
      ch.onmessage = (e) => {
        if (e.type === "status") {
          line.textContent = `«${b?.name}»: ${e.message}`;
          skipBtn.hidden = true;
          skipBtn.disabled = false;
        } else if (e.type === "captcha") {
          line.textContent = `«${b?.name}»: ${e.message}`;
          skipBtn.hidden = false;
          skipBtn.disabled = false;
        }
      };
      await safe(invoke("brand_analyze", { id, onEvent: ch }));
    }
    progress.hidden = true;
    if (btn) btn.disabled = false;
    reload();
  };
  root.append(progress);
  const analyzeAll = h("button", { class: "btn tonal", type: "button" }, icon("refresh", "sm"), "تحلیل همه");
  analyzeAll.addEventListener("click", () => analyze(brands.map((b) => b.id), analyzeAll));
  actions.append(analyzeAll, h("button", { class: "btn primary", type: "button", onclick: () => brandModal(null, reload) }, icon("plus", "sm"), "افزودن برند"));

  function draw() {
    analyzeAll.hidden = !brands.length;
    if (!brands.length) {
      clear(
        body,
        h(
          "div",
          { class: "card" },
          h(
            "div",
            { class: "mkt-empty" },
            h("div", { class: "ic-box lg" }, icon("award")),
            h("h2", {}, "برند خود را کنار رقبا بسنجید"),
            h("p", { class: "muted" }, "شاخص سلامت برند از چهار ستون ساخته می‌شود و برای هر ضعف، اقدام مشخص پیشنهاد می‌شود:"),
            h("ol", { class: "how cols" }, PILLARS.map(([, t, d]) => h("li", {}, h("b", {}, t), h("span", {}, d)))),
            h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: () => brandModal({ own: true }, reload) }, icon("plus", "sm"), "افزودن برند ما"), h("button", { class: "btn", type: "button", onclick: () => brandModal(null, reload) }, icon("plus", "sm"), "افزودن رقیب")),
          ),
        ),
      );
      return;
    }
    const cur = brands.find((b) => b.id === selected) || brands[0];
    const analyzed = brands.filter((b) => b.report);
    const own = brands.find((b) => b.own && b.report);
    clear(
      body,
      h(
        "div",
        { class: "brand-grid" },
        brands.map((b) => {
          const r = b.report;
          return h(
            "div",
            { class: `card brand-card ${b.id === cur.id ? "on" : ""} ${b.own ? "own" : ""}`.trim(), onclick: (e) => !e.target.closest("button") && ((selected = b.id), draw()) },
            h("div", { class: "row" }, ring(r?.overall), h("div", { class: "grow stack tight" }, h("b", { class: "bn" }, b.name), h("span", { class: "small muted" }, [b.own ? "برند ما" : "رقیب", b.category, r?.market?.tier].filter(Boolean).join(" · ")), h("span", { class: "small faint" }, b.analyzedAt ? `تحلیل ${ago(b.analyzedAt)}` : "هنوز تحلیل نشده"))),
            pillarBars(r),
            h(
              "div",
              { class: "row end" },
              h("button", { class: "btn ghost sm icon-only", type: "button", title: "حذف", onclick: async () => { if (await confirmBox(`«${b.name}» حذف شود؟`, { ok: "حذف", danger: true })) { await safe(invoke("brand_delete", { id: b.id })); reload(); } } }, icon("trash", "sm")),
              h("button", { class: "btn ghost sm icon-only", type: "button", title: "ویرایش", onclick: () => brandModal(b, reload) }, icon("edit", "sm")),
              h("button", { class: "btn tonal sm", type: "button", onclick: (e) => analyze([b.id], e.currentTarget) }, icon("refresh", "sm"), "تحلیل"),
            ),
          );
        }),
      ),
      analyzed.length >= 2
        ? h(
            "div",
            { class: "mkt-grid two" },
            h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "نقشه جایگاه")), positioning(brands)),
            h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "مقایسه ستون‌ها")), compare(analyzed)),
          )
        : null,
      detail(cur, own),
    );
  }

  root.append(body);
  draw();
}

function compare(list) {
  const best = Object.fromEntries(PILLARS.map(([k]) => [k, Math.max(...list.map((b) => b.report[k] ?? -1))]));
  return h(
    "div",
    { class: "table-wrap" },
    h(
      "table",
      { class: "t cmp" },
      h("thead", {}, h("tr", {}, h("th", {}, "برند"), PILLARS.map(([, t]) => h("th", {}, t)), h("th", {}, "شاخص قیمت"), h("th", {}, "کل"))),
      h(
        "tbody",
        {},
        list.map((b) =>
          h(
            "tr",
            { class: b.own ? "own" : "" },
            h("td", {}, h("b", {}, b.name)),
            PILLARS.map(([k]) => h("td", { class: b.report[k] != null && b.report[k] === best[k] ? "best" : "" }, b.report[k] == null ? "—" : fa(b.report[k]))),
            h("td", {}, b.report.market?.priceIndex ? fa(b.report.market.priceIndex) : "—"),
            h("td", {}, h("b", {}, fa(b.report.overall))),
          ),
        ),
      ),
    ),
  );
}

function detail(b, own) {
  const r = b.report;
  if (!r) return h("div", { class: "card" }, h("div", { class: "empty" }, icon("award", "lg"), h("b", {}, `«${b.name}» هنوز تحلیل نشده است`), h("span", { class: "small" }, "«تحلیل» را روی کارت برند بزنید.")));
  const m = r.market || {};
  const s = r.site || {};
  const check = (ok, label) => h("div", { class: `chk ${ok ? "ok" : "no"}` }, icon(ok ? "check" : "x", "sm"), label);
  const gaps =
    b.own || !own
      ? null
      : PILLARS.filter(([k]) => r[k] != null && own.report[k] != null).map(([k, t]) => {
          const d = Math.round(own.report[k] - r[k]);
          return h("div", { class: `gap ${d >= 0 ? "up" : "down"}` }, h("span", {}, t), h("b", {}, `${d >= 0 ? "+" : "−"}${fa(Math.abs(d))}`));
        });
  return h(
    "div",
    { class: "stack" },
    h(
      "div",
      { class: "row between wrap" },
      h("h2", {}, `جزئیات «${b.name}»`),
      r.confidence != null ? confBadge(r.confidence, "اعتماد داده") : null,
    ),
    // ambiguous name: show the entities we actually found instead of guessing
    r.candidates?.length
      ? h(
          "div",
          { class: "card stack" },
          h("div", { class: "card-h" }, h("h2", {}, "احتمالاً منظور شما یکی از این‌هاست؟"), h("span", { class: "small muted" }, "نام این برند چند هویت نزدیک دارد؛ برای دقت بیشتر، نام دقیق‌تر یا سایت/اینستاگرام را در ویرایش برند وارد کنید.")),
          h(
            "div",
            { class: "cand-grid" },
            r.candidates.map((c) =>
              h(
                "div",
                { class: "cand-card" },
                h("div", { class: "row between" }, h("b", {}, c.name), h("span", { class: "pill" }, c.kind)),
                h("span", { class: "small muted" }, c.detail),
                h(
                  "div",
                  { class: "row" },
                  c.url ? h("button", { class: "btn ghost sm", type: "button", onclick: () => safe(invoke("open_url", { url: c.url })) }, icon("external", "sm"), "مشاهده") : null,
                ),
              ),
            ),
          ),
        )
      : null,
    r.signals?.length
      ? h(
          "details",
          { class: "card stack tight" },
          h("summary", {}, icon("info", "sm"), `اعتماد داده: ${fa(r.confidence || 0)}٪ — بر چه پایه‌ای؟`),
          h("div", { class: "pbars" }, r.signals.map((sg) => h("div", { class: "pbar" }, h("span", {}, sg.label), h("span", { class: "track" }, (() => { const f = h("i", { class: sg.score >= 70 ? "good" : sg.score >= 45 ? "mid" : "low" }); requestAnimationFrame(() => (f.style.width = `${sg.score}%`)); return f; })()), h("b", {}, fa(sg.score))))),
          r.evidence?.length ? h("div", { class: "stack tight" }, h("span", { class: "label-t" }, "منبع داده‌ها:"), r.evidence.map((e) => h("p", { class: "small muted" }, `• ${e.fact} — ${e.source}`, e.url ? h("button", { class: "seller-link", type: "button", onclick: () => safe(invoke("open_url", { url: e.url })) }, " مشاهده") : null))) : null,
        )
      : null,
    h(
      "div",
      { class: "mkt-grid" },
      h(
        "div",
        { class: "card stack" },
        h("div", { class: "card-h" }, h("h2", {}, "بازار و قیمت")),
        h(
          "dl",
          { class: "kv compact" },
          [
            ["عرضه در جستجوها", m.searches ? `${fa(m.offers)} از ${fa(m.searches)} جستجو` : "جستجویی انجام نشده"],
            ["سهم از عرضه‌ها", m.searches ? `${fa(m.sharePct)}٪` : "—"],
            ["پلتفرم‌ها", m.platforms?.length ? m.platforms.map((p) => PNAME[p] || p).join("، ") : "—"],
            ["شاخص قیمت", m.priceIndex ? `${fa(m.priceIndex)} (${m.tier})` : "—"],
            ["امتیاز خریداران", m.rating ? `${fa(m.rating)} از ۵ · ${fa(m.reviews)} نظر` : "—"],
          ].map(([k, v]) => [h("dt", {}, k), h("dd", {}, v)]),
        ),
        !m.searches ? h("button", { class: "btn sm", type: "button", onclick: () => go("market", { q: b.category || b.name }) }, icon("trend", "sm"), "جستجوی قیمت این دسته") : null,
      ),
      h(
        "div",
        { class: "card stack" },
        h("div", { class: "card-h" }, h("h2", {}, "سایت و شبکه اجتماعی"), r.foundSite || r.foundInstagram ? h("span", { class: "pill ok" }, "پیدا شده خودکار") : null),
        b.website
          ? h("div", { class: "chks" }, check(s.ok, "در دسترس"), check(s.https, "HTTPS امن"), check(!!s.title, "عنوان صفحه"), check(!!s.description, "توضیحات متا"), check(s.schema, "داده ساخت‌یافته"), check(s.contact, "راه تماس"), check(s.ok && s.ms < 1500, s.ms ? `سرعت ${fa(Math.round(s.ms / 100) / 10)} ثانیه` : "سرعت"), check((s.socials || []).length > 0, "لینک شبکه‌ها"))
          : h("p", { class: "small muted" }, "سایتی ثبت نشده است."),
        h("p", { class: "small" }, b.instagram ? (r.insta?.followers != null ? `اینستاگرام @${b.instagram}: ${fa(r.insta.followers)} دنبال‌کننده · ${fa(r.insta.posts || 0)} پست` : `اطلاعات عمومی @${b.instagram} خوانده نشد.`) : "اینستاگرامی ثبت نشده است."),
      ),
      gaps?.length ? h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, `برند ما در برابر «${b.name}»`)), h("div", { class: "gaps" }, gaps), h("span", { class: "small muted" }, "عدد مثبت یعنی برند ما در آن ستون جلوتر است.")) : null,
    ),
    mix(r),
    channelsAndProducts(r),
    h(
      "div",
      { class: "swot-grid" },
      h("div", { class: "card stack swot ok" }, h("div", { class: "card-h" }, icon("check"), h("h2", {}, "قوت‌ها (S)")), r.strengths.length ? r.strengths.map((t) => h("p", {}, t)) : h("p", { class: "muted small" }, "هنوز نقطه قوت برجسته‌ای در داده‌ها دیده نشد.")),
      h("div", { class: "card stack swot bad" }, h("div", { class: "card-h" }, icon("alert"), h("h2", {}, "ضعف‌ها (W)")), r.weaknesses.length ? r.weaknesses.map((t) => h("p", {}, t)) : h("p", { class: "muted small" }, "ضعف مهمی دیده نشد.")),
      h("div", { class: "card stack swot opp" }, h("div", { class: "card-h" }, icon("trend"), h("h2", {}, "فرصت‌ها (O)")), (r.opportunities || []).length ? r.opportunities.map((t) => h("p", {}, t)) : h("p", { class: "muted small" }, "فرصت مشخصی از داده‌های بازار دیده نشد.")),
      h("div", { class: "card stack swot thr" }, h("div", { class: "card-h" }, icon("shield"), h("h2", {}, "تهدیدها (T)")), (r.threats || []).length ? r.threats.map((t) => h("p", {}, t)) : h("p", { class: "muted small" }, "تهدید جدی دیده نشد.")),
    ),
    h("div", { class: "card stack swot act" }, h("div", { class: "card-h" }, icon("target"), h("h2", {}, "اقدام‌های پیشنهادی")), h("ol", { class: "acts" }, r.actions.map((t) => h("li", {}, t)))),
  );
}

const MIX = { product: ["محصول", "store"], price: ["قیمت", "trend"], place: ["توزیع و کانال", "map"], promotion: ["ترویج و دیجیتال", "send"] };

/** Marketing mix (4P): one tile per P with its score and findings. */
function mix(r) {
  if (!r.mix?.length) return null;
  return h(
    "div",
    { class: "card stack" },
    h("div", { class: "card-h" }, h("h2", {}, "آمیخته بازاریابی (4P)"), h("span", { class: "small muted" }, "امتیاز هر بخش از ۱۰۰، از داده‌های بازار، سایت و شبکه اجتماعی")),
    h(
      "div",
      { class: "mix-grid" },
      r.mix.map((x) => {
        const [t, ic] = MIX[x.key] || [x.key, "info"];
        const fill = h("i");
        requestAnimationFrame(() => requestAnimationFrame(() => (fill.style.width = `${x.score ?? 0}%`)));
        return h("div", { class: "mix" }, h("div", { class: "row between" }, h("b", {}, icon(ic, "sm"), t), h("span", { class: "mix-s" }, x.score != null ? fa(x.score) : "—")), h("span", { class: "mix-bar" }, fill), x.notes.map((n) => h("span", { class: "small muted" }, n)));
      }),
    ),
  );
}

/** Where the brand is sold, its products as they are listed now, and its competitors. */
function channelsAndProducts(r) {
  if (!r.channels?.length && !r.products?.length && !r.competitors?.length) return null;
  return h(
    "div",
    { class: "mkt-grid" },
    r.channels?.length
      ? h(
          "div",
          { class: "card stack" },
          h("div", { class: "card-h" }, h("h2", {}, "کانال‌های فروش برند"), h("span", { class: "small muted" }, `${fa(r.sellers || 0)} فروشنده · ${fa(r.priceMin || 0)} تا ${fa(r.priceMax || 0)} تومان`)),
          h(
            "table",
            { class: "t" },
            h("thead", {}, h("tr", {}, ["کانال", "کالا", "فروشنده", "کمترین", "میانه"].map((t) => h("th", {}, t)))),
            h("tbody", {}, r.channels.map((c) => h("tr", {}, h("td", {}, pbadge(c.platform, true)), h("td", {}, fa(c.offers)), h("td", {}, fa(c.sellers)), h("td", { class: "num" }, fa(c.min)), h("td", { class: "num" }, fa(c.median))))),
          ),
          r.topSellers?.length ? h("div", { class: "chips" }, r.topSellers.map(([n, c]) => h("span", { class: "chip" }, `${n} (${fa(c)})`))) : null,
        )
      : null,
    r.products?.length
      ? h(
          "div",
          { class: "card stack" },
          h("div", { class: "card-h" }, h("h2", {}, "محصولات برند در بازار")),
          h(
            "div",
            { class: "prod-list" },
            r.products.slice(0, 8).map((o) => {
              const img = o.image ? h("img", { src: o.image, alt: "", loading: "lazy", referrerpolicy: "no-referrer" }) : icon("store");
              return h("div", { class: "prod" }, h("span", { class: "thumb" }, img), h("div", { class: "grow" }, h("div", { class: "small" }, o.title), h("span", { class: "x small" }, `${fa(o.price)} تومان · `, pbadge(o.platform))));
            }),
          ),
          r.lines?.length ? h("div", { class: "row wrap" }, h("span", { class: "label-t" }, "خطوط محصول"), h("div", { class: "chips" }, r.lines.map((l) => h("span", { class: "chip" }, l)))) : null,
        )
      : null,
    r.competitors?.length
      ? h(
          "div",
          { class: "card stack" },
          h("div", { class: "card-h" }, h("h2", {}, "رقبای اصلی در همین بازار")),
          r.competitors.map((c) => h("div", { class: "row between comp" }, h("b", {}, c.name), h("span", { class: "small muted" }, `${fa(c.pct)}٪ عرضه · میانه ${fa(c.median)}`))),
        )
      : null,
  );
}

export function leave() {}