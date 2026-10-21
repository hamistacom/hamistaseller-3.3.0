// Dashboard: KPIs, 30-day intake, follow-up funnel, top keywords / cities, recent work.
import { h, icon, invoke, fa, bars, bar, jdate, ago, clear, select, STATUS, LOCALE } from "../core.js";
import { go } from "../state.js";
import { hero, cityPhoto, keywordPhoto, photoCards } from "../photos.js";
import { SUGGESTED_KEYWORDS } from "../data.js";
import { openCard, openEditor } from "./card.js";

const RUN_STATUS = { done: ["پایان یافته", "ok"], running: ["در حال اجرا", "accent"], cancelled: ["متوقف شده", "warn"], interrupted: ["نیمه‌کاره", "warn"], failed: ["ناموفق", "bad"] };
export { RUN_STATUS };

function rank(rows, onPick) {
  if (!rows.length) return h("div", { class: "empty small" }, "هنوز داده‌ای نیست");
  const max = Math.max(...rows.map((r) => r[1]), 1);
  return h(
    "div",
    { class: "rank" },
    rows.slice(0, 6).map(([name, n]) => h("div", { class: "r" }, h("button", { class: "link clip", type: "button", onclick: () => onPick(name) }, name), h("span", { class: "muted" }, fa(n)), bar(n / max))),
  );
}

const SHORT = { maps: "گوگل‌مپ و وب", divar: "دیوار", sheypoor: "شیپور", neshan: "نشان", balad: "بلد", telegram: "تلگرام", instagram: "اینستاگرام" };
const pct = (n, total) => (total ? Math.round((100 * n) / total) : 0);

/** "▲ ۱۲٪" chip comparing two periods; null when there is nothing to compare. */
function trendChip(now, before) {
  if (!now && !before) return null;
  if (!before) return h("span", { class: "trend up" }, icon("sparkle", "sm"), "تازه");
  const p = Math.round(((now - before) / before) * 100);
  return h("span", { class: `trend ${p >= 0 ? "up" : "down"}`, title: `${fa(now)} در برابر ${fa(before)}` }, p >= 0 ? "▲" : "▼", `${fa(Math.abs(p))}٪`);
}

function kpiCard(title, desc, value, chips, { ic, dark = false } = {}) {
  return h(
    "div",
    { class: `card kpi2 ${dark ? "dark" : ""}`.trim() },
    h("div", { class: "t" }, ic ? icon(ic) : null, h("span", { class: "grow" }, title)),
    h("div", { class: "d" }, desc),
    h("div", { class: "v" }, h("b", {}, fa(value)), chips),
  );
}

/** Diverging bars per source: all-time (mint, towards the start) vs last 30 days (pink). */
function sourceBars(rows, names) {
  if (!rows.length) return h("div", { class: "empty small" }, "هنوز مخاطبی ثبت نشده");
  const maxAll = Math.max(1, ...rows.map((r) => r[1]));
  const maxNew = Math.max(1, ...rows.map((r) => r[2]));
  return h(
    "div",
    { class: "dbars" },
    rows.slice(0, 7).map(([src, all, recent]) => {
      const a = h("i", { title: `${fa(all)} مخاطب از ابتدا` });
      a.style.width = `${Math.max(3, (100 * all) / maxAll)}%`;
      const b = h("i", { title: `${fa(recent)} مخاطب در ۳۰ روز اخیر` });
      b.style.width = `${recent ? Math.max(3, (100 * recent) / maxNew) : 0}%`;
      return h("div", { class: "r" }, h("span", { title: names[src] || src }, SHORT[src] || names[src] || (src === "manual" ? "ثبت دستی" : src === "import" ? "ورود فایل" : src === "megabank" ? "بانک جامع" : src || "نامشخص")), h("div", { class: "l" }, a), h("div", { class: "g" }, b));
    }),
  );
}

/** Follow-up as one segmented bar: untouched → in progress → interested → customer. */
function funnel(d) {
  const inProgress = Math.max(0, d.contacted - d.interested - d.customers);
  const segs = [
    ["پیگیری‌نشده", d.total - d.contacted, "s0"],
    ["در حال پیگیری", inProgress, "s1"],
    [STATUS.interested.label, d.interested, "s2"],
    [STATUS.customer.label, d.customers, "s3"],
  ];
  const box = h("div", { class: "segbar" });
  box.style.gridTemplateColumns = segs.map(([, n]) => `${Math.max(n, d.total / 40, 1)}fr`).join(" ");
  segs.forEach(([label, n]) => box.append(h("div", { class: "seg-l" }, label, h("b", {}, fa(n)), h("span", { class: "small faint" }, `${fa(pct(n, d.total))}٪`))));
  segs.forEach(([label, n, cls]) => box.append(h("div", { class: `seg-b ${cls}`, title: `${label}: ${fa(n)}` })));
  return box;
}

/** The single most useful next step, from the numbers. */
function insight(d, names) {
  const best = [...d.sources].sort((a, b) => b[2] - a[2])[0];
  const mobile = pct(d.mobile, d.total);
  if (d.interestedWaiting) return { tag: `${fa(d.interestedWaiting)} منتظر پیگیری`, text: `${fa(d.interestedWaiting)} مشتری علاقه‌مند بیش از ۳ روز است پیگیری نشده‌اند؛ امروز با آن‌ها تماس بگیرید.`, go: () => go("contacts", { status: "interested" }) };
  if (d.contacted < d.total / 2 && d.mobile) return { tag: `${fa(mobile)}٪ موبایل‌دار`, text: `${fa(d.total - d.contacted)} مخاطب هنوز پیگیری نشده‌اند. یک کمپین پیامکی برای ${fa(d.mobile)} مخاطب موبایل‌دار بفرستید.`, go: () => go("outreach") };
  if (best && best[2]) {
    const label = SHORT[best[0]] || names[best[0]] || best[0];
    return { tag: label, text: `بیشترین مخاطب ۳۰ روز اخیر از «${label}» آمده (${fa(best[2])} مخاطب). همین منبع را با کلمات تازه تکرار کنید.`, go: () => go("scan") };
  }
  return { tag: "بانک خود را گسترش دهید", text: "با انتخاب چند منبع با هم (گوگل‌مپ، دیوار، شیپور، بلد، نشان) در «استخراج»، کسب‌وکارهای بیشتری پیدا کنید.", go: () => go("scan") };
}

/** «پیگیری‌های امروز و معوق»: businesses whose reminder day has come, the most overdue first. */
function dueCard(due, reload) {
  return h(
    "div",
    { class: `card stack dash-due ${due.some((x) => x.overdueDays > 0) ? "late" : ""}`.trim() },
    h("div", { class: "card-h" }, icon("clock", "sm"), h("h2", {}, "پیگیری‌های امروز و معوق"), due.length ? h("span", { class: `pill ${due.some((x) => x.overdueDays > 0) ? "bad" : "info"}` }, fa(due.length)) : null),
    due.length
      ? h(
          "div",
          { class: "feed" },
          due.slice(0, 8).map((x) =>
            h(
              "div",
              { class: "it" },
              h("div", { class: "grow" }, h("button", { class: "link clip", type: "button", onclick: () => openCard(x.id, { onChange: reload }) }, h("b", {}, x.name)), h("span", {}, [x.person, x.phone, x.last && `آخرین: ${x.last}`].filter(Boolean).join(" · "))),
              x.overdueDays > 0 ? h("span", { class: "pill bad" }, `${fa(x.overdueDays)} روز معوق`) : h("span", { class: "pill info" }, "امروز"),
            ),
          ),
        )
      : h("div", { class: "empty small" }, "پیگیری سررسیدشده‌ای نیست. در کارت مشتری «پیگیری بعدی» را بگذارید تا اینجا و با اعلان ویندوز یادآوری شود."),
  );
}

export async function render(root, _params, actions) {
  actions.append(
    h("button", { class: "btn", type: "button", onclick: () => openEditor({}, { onChange: () => go("dashboard") }) }, icon("plus", "sm"), "افزودن مخاطب"),
    h("button", { class: "btn primary", type: "button", onclick: () => go("scan") }, icon("play", "sm"), "استخراج جدید"),
  );
  const [d, recent, runs, acts, srcList, due] = await Promise.all([
    invoke("dashboard"),
    invoke("places", { filter: { listId: 0 }, offset: 0, limit: 7 }),
    invoke("runs"),
    invoke("activity", { limit: 8 }),
    invoke("sources_list").catch(() => []),
    invoke("follow_ups_due").catch(() => []),
  ]);
  const names = Object.fromEntries(srcList.map((x) => [x.id, x.name]));

  // kit «Banner»: the latest extraction, over a photo of that city
  const last = runs.find((r) => r.province) || null;
  const banner = hero({
    kicker: last ? `آخرین استخراج · ${ago(last.startedAt)}` : "شروع کار",
    title: last ? `«${last.keyword}» در ${last.city || last.province}` : "اولین بانک مشتریان را بسازید",
    sub: last ? `${fa(last.found)} مخاطب از ${fa(last.checked)} کسب‌وکار بررسی‌شده · ${last.listName}` : "استان، شهر و حوزه کاری را انتخاب کنید؛ اطلاعات عمومی کسب‌وکارها جمع می‌شود.",
    action: h("button", { class: "btn primary", type: "button", onclick: () => go("scan") }, icon("play", "sm"), last ? "استخراج جدید" : "شروع استخراج"),
  });
  if (last) cityPhoto(last.province, last.city).then((p) => banner.update({ photo: p }));
  else banner.update({ photo: keywordPhoto("کافه") });
  // kit «Featured Course»: one click → the extraction form with that keyword
  const quick = h(
    "div",
    { class: "card" },
    h("div", { class: "card-h" }, h("h2", {}, "شروع سریع"), h("button", { class: "btn ghost sm", type: "button", onclick: () => go("scan") }, "همه حوزه‌ها")),
    photoCards(SUGGESTED_KEYWORDS.slice(0, 6).map((k) => ({ title: k, tag: "استخراج", photo: keywordPhoto(k, true), onclick: () => go("scan", { kw: k }) }))),
  );

  if (!d.total) {
    root.append(
      banner,
      quick,
      h(
        "div",
        { class: "card gate-card" },
        h(
          "div",
          { class: "empty" },
          icon("sparkle", "lg"),
          h("h2", {}, "به هامیستا سلر خوش آمدید"),
          h("p", { class: "muted" }, "اولین بانک مشتریان را با یک استخراج یا وارد کردن فایل Excel بسازید."),
          h("div", { class: "row" }, h("button", { class: "btn primary", type: "button", onclick: () => go("scan") }, icon("play", "sm"), "شروع استخراج"), h("button", { class: "btn", type: "button", onclick: () => go("contacts", { import: 1 }) }, icon("upload", "sm"), "ورود از Excel / CSV")),
        ),
      ),
    );
    return;
  }

  // ---------- trend chart: weekly (12 weeks) or daily (30 days) ----------
  let mode = "weekly";
  const chartBox = h("div");
  const drawChart = () => {
    const rows = mode === "weekly" ? d.weekly : d.daily;
    const data = rows.map(([day, n], i) => {
      const prev = i ? rows[i - 1][1] : 0;
      const diff = i && prev ? Math.round(((n - prev) / prev) * 100) : null;
      return {
        label: mode === "weekly" ? `هفته ${jdate(day)}` : jdate(day),
        short: mode === "weekly" ? new Date(`${day}T00:00:00`).toLocaleDateString(LOCALE, { day: "numeric", month: "short" }) : i % 3 === 0 || i === rows.length - 1 ? new Date(`${day}T00:00:00`).toLocaleDateString(LOCALE, { day: "numeric" }) : "",
        value: n,
        tip: diff == null ? "" : `${diff >= 0 ? "▲" : "▼"} ${fa(Math.abs(diff))}٪ نسبت به ${mode === "weekly" ? "هفته" : "روز"} قبل`,
      };
    });
    clear(chartBox, bars(data, { unit: "مخاطب" }));
  };
  const modeSel = select([["weekly", "هفتگی"], ["daily", "روزانه"]], mode, { class: "input sm pill-select", "aria-label": "بازه نمودار" });
  modeSel.addEventListener("change", () => {
    mode = modeSel.value;
    drawChart();
  });
  drawChart();

  const ins = insight(d, names);
  const mobilePct = pct(d.mobile, d.total);
  const growth = d.prevMonth ? Math.round(((d.month - d.prevMonth) / d.prevMonth) * 100) : null;

  root.append(
    h(
      "div",
      { class: "stack" },
      banner,
      h(
        "div",
        { class: "dash-grid" },
        kpiCard("کل مخاطبان", "در مقایسه با ۳۰ روز قبل", d.total, [trendChip(d.month, d.prevMonth), d.month ? h("span", { class: "pill ok" }, `+${fa(d.month)} این ماه`) : null], { ic: "database" }),
        kpiCard("پیام‌های موفق", "۷ روز اخیر در مقایسه با هفته قبل", d.sentWeek, [trendChip(d.sentWeek, d.sentPrevWeek), h("span", { class: "pill" }, `${fa(d.contacted)} مخاطب پیگیری‌شده`)], { ic: "send" }),
        h(
          "div",
          { class: "card stack dash-sources" },
          h("div", { class: "card-h" }, h("h2", {}, "سهم منابع"), h("button", { class: "link small", type: "button", onclick: () => go("scan") }, "استخراج")),
          h("p", { class: "small muted" }, "از کدام منبع چند مخاطب (دارای راه ارتباطی) به دست آمده است."),
          h("div", { class: "legend" }, h("span", {}, h("i", { class: "m" }), "از ابتدا"), h("span", {}, h("i", { class: "p" }), "۳۰ روز اخیر")),
          sourceBars(d.sources, names),
        ),
        h(
          "div",
          { class: "card stack dash-funnel" },
          h("div", { class: "card-h" }, h("span", { class: "ic-box" }, icon("filter", "sm")), h("div", { class: "grow" }, h("span", { class: "small muted" }, "وضعیت پیگیری"), h("h2", {}, `${fa(pct(d.customers, d.total))}٪ تبدیل به مشتری`)), h("button", { class: "btn sm", type: "button", onclick: () => go("contacts", { view: "stats" }) }, "تحلیل")),
          funnel(d),
        ),
        h(
          "div",
          { class: "card stack dash-chart" },
          h("div", { class: "card-h" }, h("div", { class: "grow" }, h("h2", {}, "روند ورود مخاطب"), h("p", { class: "small muted" }, "تعداد کسب‌وکار دارای راه ارتباطی که به بانک‌ها اضافه شده است.")), modeSel),
          chartBox,
        ),
        h(
          "div",
          { class: "card glow dash-glow" },
          growth != null ? h("span", { class: "tag r" }, `${growth >= 0 ? "+" : "−"}${fa(Math.abs(growth))}٪ رشد ماهانه`) : null,
          h("span", { class: "tag" }, `${fa(mobilePct)}٪ موبایل‌دار`),
          h("button", { class: "go", type: "button", "aria-label": "انجام پیشنهاد", onclick: ins.go }, icon("move", "sm")),
          h("h2", {}, "بینش هوشمند"),
          h("p", {}, ins.text),
        ),
      ),
      dueCard(due, () => go("dashboard")),
      h(
        "div",
        { class: "grid-3" },
        h("div", { class: "card" }, h("div", { class: "card-h" }, h("h2", {}, "کلمات کلیدی برتر")), rank(d.topKeywords, (keyword) => go("contacts", { keyword }))),
        h("div", { class: "card" }, h("div", { class: "card-h" }, h("h2", {}, "شهرهای برتر")), rank(d.topCities, (city) => go("contacts", { city }))),
        h(
          "div",
          { class: "card" },
          h("div", { class: "card-h" }, h("h2", {}, "رویدادهای اخیر"), h("button", { class: "link small", type: "button", onclick: () => go("history", { tab: "activity" }) }, "همه")),
          acts.length ? h("div", { class: "feed" }, acts.map((a) => h("div", { class: "it" }, h("div", { class: "grow" }, h("span", {}, a.text), h("span", {}, ago(a.at)))))) : h("div", { class: "empty small" }, "رویدادی نیست"),
        ),
      ),
      h(
        "div",
        { class: "grid-2" },
        h(
          "div",
          { class: "card" },
          h("div", { class: "card-h" }, h("h2", {}, "آخرین مخاطبان"), h("button", { class: "link small", type: "button", onclick: () => go("contacts") }, "مشاهده همه")),
          h(
            "div",
            { class: "feed" },
            recent.rows.map((p) => {
              const first = p.contacts[0];
              return h(
                "div",
                { class: "it" },
                h("div", { class: "avatar small-av" }, p.name.trim()[0] || "؟"),
                h("div", { class: "grow" }, h("button", { class: "link clip", type: "button", onclick: () => openCard(p.id, { onChange: () => go("dashboard") }) }, h("b", {}, p.name)), h("span", {}, [p.contactPerson && `${p.contactPerson}${p.contactRole ? ` (${p.contactRole})` : ""}`, p.category, p.city, SHORT[p.source] || names[p.source]].filter(Boolean).join(" · "))),
                first ? h("span", { class: first.kind === "email" ? "ltr small" : "small" }, first.value.startsWith("http") ? "" : first.value) : null,
              );
            }),
          ),
        ),
        h(
          "div",
          { class: "card" },
          h("div", { class: "card-h" }, h("h2", {}, "آخرین استخراج‌ها"), h("button", { class: "link small", type: "button", onclick: () => go("history") }, "تاریخچه")),
          runs.length
            ? h(
                "div",
                { class: "feed" },
                runs.slice(0, 6).map((r) => {
                  const [label, tone] = RUN_STATUS[r.status] || [r.status, ""];
                  return h("div", { class: "it" }, h("div", { class: "grow" }, h("b", {}, `${r.keyword} — ${r.city || r.province || "همه مناطق"}`), h("span", {}, `${fa(r.found)} مخاطب جدید از ${fa(r.checked)} مورد بررسی‌شده · ${ago(r.startedAt)}`)), h("span", { class: `pill ${tone}` }, label));
                }),
              )
            : h("div", { class: "empty small" }, "هنوز استخراجی انجام نشده"),
        ),
      ),
      quick,
    ),
  );
}

export function leave() {}
export { clear };