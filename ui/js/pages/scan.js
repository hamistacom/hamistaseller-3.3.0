// Scan: form on one side, live progress on the other. The job keeps running if you leave the page.
import { hero, cityPhoto, keywordPhoto, photo } from "../photos.js";
import { h, icon, invoke, fa, clear, safe, select, field, input, toggle, tagInput, toast, modal, bar, ago, COLORS, skipWaitButton } from "../core.js";
import { app, bankOptions, refreshLists, go } from "../state.js";
import { job, onJob, runJob } from "../jobs.js";
import { SUGGESTED_KEYWORDS } from "../data.js";
import { areaPicker } from "../area.js";
import { openCard } from "./card.js";

let off = null;

const remember = (k, v) => {
  try {
    if (v === undefined) return localStorage.getItem(k);
    localStorage.setItem(k, v);
  } catch {}
  return null;
};

export function newBankModal(onDone) {
  const name = input({ placeholder: "مثلاً کافه‌های تهران" });
  let color = "navy";
  const sw = h(
    "div",
    { class: "chips" },
    COLORS.map((c) =>
      h(
        "span",
        {
          class: `chip click ${c === color ? "on" : ""}`,
          onclick: (e) => {
            color = c;
            [...sw.children].forEach((x) => x.classList.remove("on"));
            e.currentTarget.classList.add("on");
          },
        },
        h("span", { class: `sw c-${c} bank-sw` }),
      ),
    ),
  );
  modal({
    title: "بانک جدید",
    body: [field("نام بانک", name), field("رنگ", sw)],
    actions: [
      { label: "انصراف" },
      {
        label: "ساخت بانک",
        kind: "primary",
        onClick: async () => {
          const id = await safe(invoke("list_create", { name: name.value, color }), "بانک ساخته شد");
          if (id === undefined) return false;
          await refreshLists();
          onDone?.(id);
        },
      },
    ],
  });
  name.focus();
}

// per-source form rules (registry ids from sources.rs)
const SOURCE_UI = {
  maps: {
    area: true,
    enrich: true,
    browserOpts: true,
    login: false,
    hint: null,
    kw: "کلمات کلیدی (حوزه کاری)",
    kwHint: "هر کلمه جداگانه جستجو می‌شود؛ مثلاً «کافه» و «کافی‌شاپ».",
  },
  divar: {
    area: true,
    enrich: true,
    browserOpts: true,
    login: true,
    hint: "استخراج شماره تماس آگهی‌ها و فروشگاه‌های دیوار بر اساس شهر و منطقه. اگر قبلاً وارد حساب دیوار نشده باشید، پنجره مرورگر خودکار برای ورود یک‌باره باز می‌شود و سپس در پس‌زمینه ادامه می‌دهد.",
    kw: "کلمات کلیدی در دیوار",
    kwHint: "مثلاً «پخش عمده»، «مبلمان»، «لوازم یدکی»، «عسل».",
  },
  sheypoor: {
    area: true,
    enrich: true,
    browserOpts: true,
    login: true,
    hint: "استخراج خودکار شماره تماس فروشگاه‌ها و آگهی‌دهندگان شیپور بر اساس شهر و منطقه؛ در صورت نیاز به ورود، پنجره مرورگر خودکار باز می‌شود.",
    kw: "کلمات کلیدی در شیپور",
    kwHint: "مثلاً «تجهیزات صنعتی»، «پوشاک عمده»، «املاک».",
  },
  neshan: {
    area: true,
    enrich: true,
    browserOpts: true,
    login: false,
    needs: "apiKey",
    hint: "نشان فهرست کسب‌وکارهای منطقه را می‌دهد و شماره هر کدام در گوگل‌مپ پیدا می‌شود. کلید API نشان لازم است.",
    kw: "کلمات کلیدی",
    kwHint: "مثلاً «کافه»، «آرایشگاه».",
  },
  balad: {
    area: true,
    enrich: false,
    browserOpts: true,
    login: false,
    hint: "صفحه‌های عمومی کسب‌وکارها در بلد از طریق جستجوی وب پیدا و خوانده می‌شوند. در صورت نیاز به تأیید «ربات نیستم»، پنجره مرورگر خودکار باز می‌شود.",
    kw: "کلمات کلیدی",
    kwHint: "مثلاً «کافه»، «دندانپزشکی».",
  },
  telegram: {
    area: false,
    enrich: false,
    browserOpts: false,
    login: true,
    hint: "کانال‌ها و گروه‌های عمومی کسب‌وکارها: نام، لینک، تعداد عضو و هر راه تماس توضیحات (شماره، ایمیل، سایت و آیدی ادمین/سفارش به‌صورت لینک). حساب‌های شخصی کنار گذاشته می‌شوند. برای خواندن کانال‌ها ورود لازم نیست؛ اگر تلگرام در شبکه شما فیلتر است VPN/پروکسی سیستم را روشن کنید.",
    kw: "موضوع کانال",
    kwHint: "مثلاً «لوازم خانگی»، «پوشاک عمده».",
  },
  instagram: {
    area: false,
    enrich: false,
    browserOpts: true,
    login: true,
    hint: "پیج‌های کسب‌وکاری با اطلاعات تماس عمومی بیو (شماره، ایمیل، سایت، واتساپ و آیدی ادمین). می‌توانید با دکمه زیر یک بار در مرورگر وارد اینستاگرام شوید یا از تنظیمات ← ماژول‌ها Graph API را وصل کنید.",
    kw: "کلمه یا هشتگ",
    kwHint: "مثلاً «کافه» یا «#مبلمان».",
  },
};

export async function render(root, params = {}) {
  const [settings, srcList] = await Promise.all([invoke("settings_get"), invoke("sources_list").catch(() => [])]);
  // modules switched off in Settings → Modules are not offered at all
  const scanSources = srcList.filter((x) => x.kind === "scan" && x.ready && x.enabled !== false);
  const offCount = srcList.filter((x) => x.kind === "scan" && x.enabled === false).length;
  // one or several sources; several run one after another in the same job
  const usable = (id) => scanSources.some((x) => x.id === id && x.allowed);
  let selected = new Set(String(params.source || remember("scanSources") || "maps").split(",").filter(usable));
  if (!selected.size) selected.add(usable("maps") ? "maps" : scanSources.find((x) => x.allowed)?.id || "maps");
  const chosen = () => scanSources.filter((x) => selected.has(x.id)); // registry order
  const uiOf = (id) => SOURCE_UI[id] || SOURCE_UI.maps;
  const s = { concurrency: 3, headless: true, expandAreas: true, delayMs: 400, ...settings };

  // ---------- form ----------
  const lastBank = Number(remember("scanBank")) || app.lists[0]?.id;
  const bank = select(bankOptions(), lastBank);
  const newBank = h(
    "button",
    {
      class: "btn sm",
      type: "button",
      title: "ساخت بانک تازه برای این استخراج",
      onclick: () => newBankModal((id) => clear(bank, bankOptions().map(([v, l]) => h("option", { value: v, selected: v === id }, l)))),
    },
    icon("plus", "sm"),
    "بانک جدید",
  );
  const kws = tagInput(params.kw ? [params.kw] : [], { placeholder: "کلمه را بنویسید و Enter بزنید", suggestions: SUGGESTED_KEYWORDS });
  const kwField = field("کلمات کلیدی", kws.el, "");
  const srcPick = h("div", { class: "stack tight" });
  const srcHint = h("div");
  const srcCount = h("span", { class: "small muted" });
  const area = areaPicker({ province: remember("scanProvince") || "", city: remember("scanCity") || "", district: remember("scanDistrict") || "" });
  const limit = input({ type: "number", min: 1, max: 2000, value: remember("scanLimit") || 50, dir: "ltr" });
  const enrich = toggle("خواندن سایت کسب‌وکار", remember("scanEnrich") !== "0");
  enrich.el.title = "ایمیل، شبکه‌های اجتماعی و نام مدیر از سایت هر کسب‌وکار خوانده می‌شود (کندتر).";
  const save = (patch) => safe(invoke("settings_save", { patch }));
  const tabs = select([1, 2, 3, 4, 5, 6, 8].map((n) => [n, `${fa(n)} تب`]), s.concurrency, { onchange: (e) => save({ concurrency: Number(e.target.value) }) });
  const visible = toggle("نمایش پنجره مرورگر هنگام استخراج", !s.headless, { onchange: (e) => save({ headless: !e.target.checked }) });
  const expand = toggle("جستجو در محله‌های اطراف وقتی نتیجه کم است", s.expandAreas, { onchange: (e) => save({ expandAreas: e.target.checked }) });

  const openWebLogin = async (srcObj) => {
    if (job.running) return toast("ابتدا عملیات در حال اجرا را متوقف کنید.", "bad");
    const listId = Number(bank.value) || app.lists[0]?.id || 1;
    const where = area.value();
    const req = {
      listId,
      source: srcObj.id,
      keywords: ["__login__"],
      province: where.province || "تهران",
      city: where.city || "تهران",
      district: "",
      areas: [],
      limit: 1,
      enrich: false,
    };
    await runJob("scan", `ورود به حساب ${srcObj.name}`, "scan_start", { req });
  };

  const start = h("button", { class: "btn accent block", type: "button" }, icon("play", "sm"), "شروع استخراج");
  start.addEventListener("click", async () => {
    const list = chosen();
    if (!list.length) return toast("حداقل یک منبع انتخاب کنید.", "bad");
    const where = area.value();
    const req = {
      listId: Number(bank.value),
      keywords: kws.values(),
      ...where,
      limit: Number(limit.value) || 50,
      enrich: list.some((x) => uiOf(x.id).enrich) ? enrich.input.checked : false,
    };
    if (list.length === 1) req.source = list[0].id;
    else req.sources = list.map((x) => x.id);
    if (!req.keywords.length) return toast("حداقل یک کلمه کلیدی وارد کنید.", "bad");
    const bad = area.error();
    if (bad) return toast(bad, "bad");
    const missing = list.find((x) => uiOf(x.id).needs && !x.config?.[uiOf(x.id).needs]);
    if (missing) return toast(`ابتدا تنظیمات «${missing.name}» را در «تنظیمات ← ماژول‌ها» کامل کنید.`, "bad");
    remember("scanSources", list.map((x) => x.id).join(","));
    remember("scanBank", req.listId);
    remember("scanProvince", where.province);
    remember("scanCity", where.city);
    remember("scanDistrict", where.district);
    remember("scanLimit", req.limit);
    remember("scanEnrich", req.enrich ? "1" : "0");
    await runJob("scan", list.length === 1 ? `استخراج از ${list[0].name}` : `استخراج از ${fa(list.length)} منبع`, "scan_start", { req });
    refreshLists();
  });

  const adv = h(
    "details",
    { class: "adv" },
    h("summary", { class: "muted small" }, "تنظیمات پیشرفته مرورگر"),
    h("div", { class: "stack tight adv-body" }, h("div", { class: "row" }, h("span", { class: "small grow" }, "تب‌های هم‌زمان"), tabs), visible.el, expand.el),
  );
  const SHORT = {
    maps: "گوگل‌مپ و وب",
    divar: "دیوار",
    sheypoor: "شیپور",
    neshan: "نقشه نشان",
    balad: "نقشه بلد",
    telegram: "کانال تلگرام",
    instagram: "پیج اینستاگرام",
  };
  function drawSource() {
    const list = chosen();
    const allowed = scanSources.filter((x) => x.allowed);
    const all = allowed.length > 1 && allowed.every((x) => selected.has(x.id));
    const card = (x) => {
      const on = selected.has(x.id);
      const need = uiOf(x.id).needs && !x.config?.[uiOf(x.id).needs];
      return h(
        "button",
        {
          type: "button",
          class: `src-card ${on ? "on" : ""}`.trim(),
          disabled: !x.allowed,
          "aria-pressed": on ? "true" : "false",
          title: x.allowed ? x.desc : "این منبع در اشتراک شما فعال نیست",
          onclick: () => {
            if (on) {
              if (selected.size > 1) selected.delete(x.id);
              else toast("حداقل یک منبع باید انتخاب بماند.");
            } else selected.add(x.id);
            drawSource();
          },
        },
        h("span", { class: `ic c-${x.id}` }, icon(x.icon, "sm")),
        h(
          "span",
          { class: "src-t" },
          h("b", {}, SHORT[x.id] || x.name),
          h(
            "span",
            { class: "n" },
            !x.allowed
              ? "خارج از اشتراک"
              : need
                ? h("span", { class: "warn" }, "نیاز به کلید API")
                : x.places
                  ? `${fa(x.places)} مخاطب${x.recent ? ` · ${fa(x.recent)} در ۳۰ روز` : ""}`
                  : "هنوز استفاده نشده",
          ),
        ),
        h("span", { class: "tick" }, on ? icon("check", "sm") : null),
      );
    };
    srcCount.textContent = list.length > 1 ? `${fa(list.length)} منبع · پشت‌سرهم` : `${fa(list.length)} منبع`;
    clear(
      srcPick,
      h(
        "div",
        { class: "row between" },
        h("span", { class: "label-t" }, "منابع استخراج"),
        allowed.length > 1
          ? h(
              "button",
              {
                type: "button",
                class: "link small",
                onclick: () => {
                  selected = all ? new Set([allowed[0].id]) : new Set(allowed.map((x) => x.id));
                  drawSource();
                },
              },
              all ? "فقط یک منبع" : "انتخاب همه",
            )
          : null,
      ),
      h("div", { class: "src-grid" }, scanSources.map(card)),
      offCount ? h("button", { type: "button", class: "link small", onclick: () => go("settings", { tab: "modules" }) }, `${fa(offCount)} ماژول خاموش است — تنظیمات ماژول‌ها`) : null,
    );
    const hints = list.filter((x) => uiOf(x.id).hint);
    const missing = list.filter((x) => uiOf(x.id).needs && !x.config?.[uiOf(x.id).needs]);
    const loginNeeded = list.filter((x) => uiOf(x.id).login);
    clear(
      srcHint,
      hints.length || list.length > 1 || loginNeeded.length
        ? h(
            "div",
            { class: `callout compact ${missing.length ? "" : "info"}`.trim() },
            icon(missing.length ? "alert" : "info", "sm"),
            h(
              "div",
              { class: "small grow stack tight" },
              list.length > 1 ? h("span", {}, "منابع پشت‌سرهم اجرا می‌شوند و نتایج در همان بانک ذخیره می‌شود؛ تکراری‌ها خودکار رد می‌شوند.") : null,
              hints.map((x) => h("span", {}, list.length > 1 ? h("b", {}, `${SHORT[x.id] || x.name}: `) : null, uiOf(x.id).hint)),
              missing.length ? h("b", {}, `تنظیمات ${missing.map((x) => `«${x.name}»`).join("، ")} هنوز کامل نشده است.`) : null,
              loginNeeded.length
                ? h(
                    "div",
                    { class: "row wrap" },
                    loginNeeded.map((ls) =>
                      h(
                        "button",
                        { class: "btn sm tonal", type: "button", onclick: () => openWebLogin(ls) },
                        icon("user", "sm"),
                        `ورود به حساب ${SHORT[ls.id] || ls.name} در مرورگر`,
                      ),
                    ),
                  )
                : null,
            ),
            missing.length ? h("button", { class: "btn sm", type: "button", onclick: () => go("settings", { tab: "modules" }) }, "تنظیمات") : null,
          )
        : null,
    );
    const one = list.length === 1 ? uiOf(list[0].id) : null;
    kwField.querySelector("label").textContent = one ? one.kw : "کلمات کلیدی یا موضوع";
    const hint = kwField.querySelector(".hint") || kwField.appendChild(h("span", { class: "hint" }));
    hint.textContent = one ? one.kwHint : "هر کلمه در همه منابع انتخاب‌شده جستجو می‌شود.";
    area.setNeeded(list.some((x) => uiOf(x.id).area));
    enrich.el.hidden = !list.some((x) => uiOf(x.id).enrich);
    adv.hidden = !list.some((x) => uiOf(x.id).browserOpts);
  }

  // one finder: no picker at all, just the form; several: a source list on top of the first column
  const multi = scanSources.length > 1;
  const form = h(
    "div",
    { class: "card stack scan-card" },
    h("div", { class: "card-h" }, h("h2", {}, "استخراج جدید"), multi ? srcCount : null),
    h(
      "div",
      { class: "scan-form" },
      h("div", { class: "stack" }, multi ? srcPick : null, multi ? srcHint : null, field("بانک مقصد", h("div", { class: "row tight-row bank-row" }, bank, newBank)), kwField),
      h(
        "div",
        { class: "stack" },
        area.el,
        h("div", { class: "row limit-row" }, field("تعداد برای هر کلمه", limit), enrich.el),
        adv,
        start,
        h("span", { class: "hint" }, "کسب‌وکارهایی که قبلاً در همین بانک هستند دوباره بررسی نمی‌شوند."),
      ),
    ),
  );
  drawSource();

  // ---------- live panel ----------
  const live = h("div", { class: "card stack" });
  const unfinished = async () => (await invoke("runs").catch(() => [])).filter((r) => r.resume && r.status !== "done" && r.status !== "running");
  let resumable = await unfinished();
  let wasRunning = job.running;

  const draw = (j) => {
    start.disabled = j.running;
    if (wasRunning && !j.running) {
      unfinished().then((r) => {
        resumable = r;
      });
    } // a run just ended: its row may now be resumable or done
    wasRunning = j.running;
    const p = j.progress;
    if (j.kind !== "scan" || (!j.running && !j.result)) {
      clear(
        live,
        h("div", { class: "card-h" }, h("h2", {}, "وضعیت"), h("span", { class: "pill" }, "آماده")),
        h(
          "ol",
          { class: "how" },
          h("li", {}, h("b", {}, "جستجوی کسب‌وکارها و آگهی‌ها"), h("span", {}, "در محدوده انتخابی، برای هر کلمه کلیدی در منابع منتخب")),
          h("li", {}, h("b", {}, "خواندن مشخصات و شماره تماس"), h("span", {}, "نام، موبایل و تلفن ثابت، آدرس، سایت و لینک آگهی")),
          h("li", {}, h("b", {}, "تکمیل از سایت کسب‌وکار"), h("span", {}, "ایمیل، شبکه‌های اجتماعی و نام مدیر (اختیاری)")),
          h("li", {}, h("b", {}, "پاک‌سازی و ذخیره"), h("span", {}, "شماره‌ها استاندارد و تکراری‌ها رد می‌شوند")),
        ),
        h(
          "div",
          { class: "small muted" },
          "کار در پس‌زمینه انجام می‌شود و می‌توانید به صفحه‌های دیگر بروید. اگر پلتفرمی نیاز به ورود به حساب یا تأیید «ربات نیستم» داشته باشد، پنجره مرورگر خودکار باز می‌شود؛ پس از تأیید، استخراج خودش ادامه پیدا می‌کند.",
        ),
        resumable.length
          ? h(
              "div",
              { class: "stack tight" },
              h("h3", {}, "استخراج‌های نیمه‌تمام"),
              h(
                "div",
                { class: "feed" },
                resumable.slice(0, 5).map((r) =>
                  h(
                    "div",
                    { class: "it" },
                    h("div", { class: "grow" }, h("b", {}, `${r.keyword} — ${r.city || r.province}`), h("span", {}, `${fa(r.found)} مخاطب از ${fa(r.checked)} مکان · ${r.listName || ""} · ${ago(r.startedAt)}`)),
                    h("button", { class: "btn sm", type: "button", onclick: () => runJob("scan", "ادامه استخراج", "scan_resume", { runId: r.id }).then(refreshLists) }, icon("play", "sm"), "ادامه"),
                  ),
                ),
              ),
            )
          : null,
      );
      return;
    }
    const logBox = h("div", { class: "log card-in" }, j.logs.slice(-200).map((l) => h("div", { class: l.level }, l.message)));
    clear(
      live,
      h(
        "div",
        { class: "card-h" },
        h("h2", {}, j.running ? j.title : "نتیجه"),
        j.running
          ? h("span", { class: "pill accent" }, h("span", { class: "dot live" }), "در حال اجرا")
          : h("span", { class: `pill ${j.result?.error ? "bad" : "ok"}` }, j.result?.error ? "با خطا" : "پایان"),
      ),
      j.captcha ? h("div", { class: "callout" }, icon("alert"), h("div", { class: "grow" }, h("b", {}, "منتظر ورود یا تأیید شما در مرورگر"), h("div", { class: "small" }, j.captcha)), skipWaitButton()) : null,
      h(
        "div",
        { class: "grid-4" },
        h("div", { class: "stat" }, h("b", {}, fa(p?.collected)), h("span", {}, "در صف")),
        h("div", { class: "stat" }, h("b", {}, fa(p?.checked)), h("span", {}, "بررسی‌شده")),
        h("div", { class: "stat" }, h("b", {}, fa(p?.found)), h("span", {}, "مخاطب جدید")),
        h("div", { class: "stat" }, h("b", {}, fa(p?.target)), h("span", {}, "هدف این کلمه")),
      ),
      bar(p?.target ? p.checked / p.target : 0),
      h("div", { class: "small muted" }, p ? `«${p.keyword}» — ${j.status}` : j.status),
      h(
        "div",
        { class: "grid-2" },
        h("div", { class: "stack tight" }, h("h3", {}, "گزارش زنده"), logBox),
        h(
          "div",
          { class: "stack tight" },
          h("h3", {}, "مخاطبان تازه"),
          h(
            "div",
            { class: "feed log" },
            j.found.length
              ? j.found.slice(0, 30).map((pl) =>
                  h(
                    "div",
                    { class: "it" },
                    h(
                      "div",
                      { class: "grow" },
                      h("button", { class: "link clip", type: "button", onclick: () => openCard(pl.id) }, h("b", {}, pl.name)),
                      h("span", {}, [pl.contactPerson, pl.contacts[0]?.value].filter(Boolean).join(" · ")),
                    ),
                  ),
                )
              : h("div", { class: "empty small" }, "هنوز موردی ثبت نشده"),
          ),
        ),
      ),
      !j.running ? h("div", { class: "row end" }, h("button", { class: "btn primary", type: "button", onclick: () => go("contacts", { list: remember("scanBank") }) }, icon("bank", "sm"), "مشاهده در بانک")) : null,
    );
    logBox.scrollTop = logBox.scrollHeight;
  };

  // banner: a photo of the area being searched, following the form as it changes
  const banner = hero({ kicker: "استخراج", small: true });
  let bt = 0;
  const syncBanner = () => {
    clearTimeout(bt);
    bt = setTimeout(async () => {
      const v = area.value();
      const place = v.district || v.city || v.province;
      const words = kws.values();
      const what = words.length ? `«${words.slice(0, 2).join("» و «")}»${words.length > 2 ? ` و ${fa(words.length - 2)} کلمه دیگر` : ""}` : "کسب‌وکارها";
      banner.update({
        kicker: v.province ? (v.city && v.city !== v.province ? `${v.province} · ${v.city}` : v.province) : "استخراج",
        title: place ? `${what} در ${place}` : "کجا دنبال مشتری بگردیم؟",
        sub: place
          ? v.city || v.district
            ? "اطلاعات عمومی کسب‌وکارهای این محدوده جمع و تکراری‌ها حذف می‌شوند."
            : `${fa(v.areas.length)} شهر استان یکی‌یکی جستجو می‌شوند.`
          : "استان و شهر را انتخاب کنید و کلمه کلیدی بنویسید.",
        photo: v.province ? await cityPhoto(v.province, v.district ? "" : v.city) : keywordPhoto(words[0] || "") || photo("p-isfahan"),
      });
    }, 200);
  };
  form.addEventListener("change", syncBanner);
  form.addEventListener("input", syncBanner);
  form.addEventListener("click", syncBanner);
  syncBanner();
  root.append(banner, h("div", { class: "scan-grid" }, form, live));
  off = onJob(draw);
}

export function leave() {
  off?.();
  off = null;
}