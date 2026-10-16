// Scheduled extractions + running in the background (Windows tray, start with Windows).
import { h, icon, invoke, fa, faDigits, clear, safe, select, field, input, toggle, tagInput, toast, modal, confirmBox, jdate, ago } from "../core.js";
import { app, bankOptions, refreshLists } from "../state.js";
import { runJob } from "../jobs.js";
import { areaPicker } from "../area.js";

// %w of SQLite: 0 = Sunday … 6 = Saturday; shown in the Persian week order
const WEEK = [
  [6, "شنبه"],
  [0, "یکشنبه"],
  [1, "دوشنبه"],
  [2, "سه‌شنبه"],
  [3, "چهارشنبه"],
  [4, "پنجشنبه"],
  [5, "جمعه"],
];
const dayName = (n) => WEEK.find(([v]) => v === n)?.[1] || "";
const SHORT = {
  maps: "گوگل‌مپ و وب",
  divar: "دیوار",
  sheypoor: "شیپور",
  neshan: "نشان",
  balad: "بلد",
  telegram: "تلگرام",
  instagram: "اینستاگرام",
};
const AREA_SOURCES = ["maps", "divar", "sheypoor", "neshan", "balad"];

function repeatText(s) {
  const t = faDigits(s.at);
  if (s.every === "daily") return `هر روز، ساعت ${t}`;
  if (s.every === "weekly") return `هر ${dayName(s.weekday)}، ساعت ${t}`;
  return `یک‌بار، ${jdate(s.onDate)} ساعت ${t}`;
}

/** "۳ ساعت دیگر" for a future local "YYYY-MM-DD HH:MM". */
function until(when) {
  const m = Math.round((new Date(when.replace(" ", "T")) - Date.now()) / 60000);
  if (m <= 1) return "به‌زودی";
  if (m < 60) return `${fa(m)} دقیقه دیگر`;
  if (m < 1440) return `${fa(Math.round(m / 60))} ساعت دیگر`;
  return `${fa(Math.round(m / 1440))} روز دیگر`;
}

const sourcesOf = (req) => (req.sources?.length ? req.sources : [req.source || "maps"]);

function editor(s, srcList, onDone) {
  const scanSources = srcList.filter((x) => x.kind === "scan" && x.ready && x.enabled !== false && x.allowed);
  const req = s.req || {};
  const picked = new Set(sourcesOf(req).filter((id) => scanSources.some((x) => x.id === id)));
  if (!picked.size && scanSources[0]) picked.add(scanSources[0].id);
  const name = input({ value: s.name || "", placeholder: "مثلاً «کافه‌های تهران — هر شب»" });
  const bank = select(bankOptions(), req.listId || app.lists[0]?.id);
  const kws = tagInput(req.keywords || [], { placeholder: "کلمه را بنویسید و Enter بزنید" });
  const area = areaPicker({ province: req.province || "", city: req.city || "", district: req.district || "" });
  const limit = input({ type: "number", min: 1, max: 500, dir: "ltr", value: req.limit || 30 });
  const every = select([["daily", "هر روز"], ["weekly", "هر هفته"], ["once", "یک‌بار"]], s.every || "daily");
  const at = input({ type: "time", dir: "ltr", value: s.at || "09:00" });
  const weekday = select(WEEK, s.weekday ?? 6);
  const onDate = input({ type: "date", dir: "ltr", value: s.onDate || "" });
  const jal = h("span", { class: "small muted" });
  const showJal = () => (jal.textContent = onDate.value ? `= ${jdate(onDate.value)}` : "");
  onDate.addEventListener("change", showJal);
  showJal();
  const dayBox = field("روز هفته", weekday);
  const dateBox = field("تاریخ (میلادی)", h("div", { class: "stack tight" }, onDate, jal));
  const sync = () => {
    dayBox.hidden = every.value !== "weekly";
    dateBox.hidden = every.value !== "once";
  };
  every.addEventListener("change", sync);
  sync();
  const chips = h("div", { class: "chips" });
  const drawChips = () => {
    area.setNeeded([...picked].some((id) => AREA_SOURCES.includes(id)));
    clear(
      chips,
      scanSources.map((x) =>
        h(
          "button",
          {
            type: "button",
            class: `chip click ${picked.has(x.id) ? "on" : ""}`.trim(),
            "aria-pressed": picked.has(x.id) ? "true" : "false",
            onclick: () => {
              if (picked.has(x.id) && picked.size > 1) picked.delete(x.id);
              else picked.add(x.id);
              drawChips();
            },
          },
          picked.has(x.id) ? icon("check", "sm") : icon(x.icon, "sm"),
          SHORT[x.id] || x.name,
        ),
      ),
    );
  };
  drawChips();
  modal({
    title: s.id ? "ویرایش زمان‌بندی" : "زمان‌بندی تازه",
    sub: "در زمان مقرر، اگر برنامه باز یا در سینی ویندوز باشد، استخراج خودکار اجرا می‌شود.",
    wide: true,
    body: [
      h("div", { class: "grid-2" }, field("نام", name), field("بانک مقصد", bank)),
      scanSources.length > 1 ? field("منابع", chips, "چند منبع پشت‌سرهم اجرا می‌شوند.") : null,
      field("کلمات کلیدی", kws.el),
      area.el,
      h("div", { class: "grid-4" }, field("تعداد برای هر کلمه", limit), field("تکرار", every), field("ساعت", at), dayBox, dateBox),
    ],
    actions: [
      { label: "انصراف" },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () => {
          const list = scanSources.filter((x) => picked.has(x.id)).map((x) => x.id);
          const r = { listId: Number(bank.value), keywords: kws.values(), ...area.value(), limit: Number(limit.value) || 30, enrich: true };
          if (list.length === 1) r.source = list[0];
          else r.sources = list;
          area.setNeeded(list.some((id) => AREA_SOURCES.includes(id)));
          const bad = area.error();
          if (bad) {
            toast(bad, "bad");
            return false;
          }
          const id = await safe(
            invoke("schedule_save", { schedule: { id: s.id || 0, name: name.value, req: r, every: every.value, at: at.value, weekday: Number(weekday.value), onDate: onDate.value } }),
            "زمان‌بندی ذخیره شد",
          );
          if (id === undefined) return false;
          onDone();
        },
      },
    ],
  });
}

async function backgroundCard(root) {
  const p = await invoke("background_get").catch(() => null);
  if (!p) return;
  const tray = toggle("با بستن پنجره، برنامه در سینی ویندوز (کنار ساعت) بماند", p.tray);
  const auto = toggle("اجرای خودکار همراه روشن شدن ویندوز (در سینی)", p.autostart);
  const save = async () => {
    const r = await safe(invoke("background_set", { prefs: { tray: tray.input.checked, autostart: auto.input.checked } }), "ذخیره شد");
    if (!r) {
      tray.input.checked = p.tray;
      auto.input.checked = p.autostart;
    } else Object.assign(p, r);
  };
  tray.input.addEventListener("change", save);
  auto.input.addEventListener("change", save);
  root.append(
    h(
      "div",
      { class: "card stack" },
      h("div", { class: "card-h" }, h("span", { class: "ic-box" }, icon("clock", "sm")), h("h2", {}, "اجرا در پس‌زمینه")),
      h(
        "p",
        { class: "small muted" },
        "زمان‌بندی‌ها، استخراج در حال اجرا و همگام‌سازی حساب آنلاین فقط وقتی کار می‌کنند که برنامه باز یا در سینی ویندوز باشد. از منوی آیکون سینی می‌توانید برنامه را نمایش دهید، عملیات را متوقف کنید یا کامل خارج شوید.",
      ),
      tray.el,
      auto.el,
    ),
  );
}

export async function render(root, _params, actions) {
  const list = h("div", { class: "stack" });
  let srcList = [];
  const draw = async () => {
    const [items, sl] = await Promise.all([invoke("schedules"), invoke("sources_list").catch(() => [])]);
    srcList = sl;
    const names = Object.fromEntries(sl.map((x) => [x.id, SHORT[x.id] || x.name]));
    if (!items.length) {
      clear(
        list,
        h(
          "div",
          { class: "card" },
          h(
            "div",
            { class: "empty" },
            icon("clock", "lg"),
            h("h2", {}, "هنوز زمان‌بندی ندارید"),
            h("p", { class: "muted" }, "مثلاً هر شب ساعت ۲ بامداد از گوگل‌مپ، دیوار و نشان «کافه» در تهران را استخراج کند؛ صبح نتیجه در بانک است."),
            h("button", { class: "btn primary", type: "button", onclick: () => editor({}, srcList, draw) }, icon("plus", "sm"), "زمان‌بندی تازه"),
          ),
        ),
      );
      return;
    }
    clear(
      list,
      items.map((s) => {
        const sw = toggle("", s.enabled, { "aria-label": s.enabled ? "خاموش کردن" : "روشن کردن" });
        sw.input.addEventListener("change", async () => {
          if ((await safe(invoke("schedule_toggle", { id: s.id, on: sw.input.checked }))) === undefined) sw.input.checked = !sw.input.checked;
          draw();
        });
        const r = s.req;
        return h(
          "div",
          { class: `card sched ${s.enabled ? "" : "off"}`.trim() },
          h(
            "div",
            { class: "row wrap" },
            h("span", { class: "ic-box" }, icon("clock", "sm")),
            h("div", { class: "grow" }, h("b", {}, s.name), h("div", { class: "small muted" }, repeatText(s))),
            s.enabled && s.nextRun ? h("span", { class: "pill info", title: jdate(s.nextRun, true) }, `اجرای بعدی: ${until(s.nextRun)}`) : h("span", { class: "pill" }, "خاموش"),
            sw.el,
          ),
          h(
            "dl",
            { class: "kv" },
            sl.filter((x) => x.kind === "scan").length > 1 ? [h("dt", {}, "منابع"), h("dd", {}, sourcesOf(r).map((id) => names[id] || id).join("، "))] : null,
            h("dt", {}, "جستجو"),
            h(
              "dd",
              {},
              `${(r.keywords || []).join("، ")} — ${r.district ? `${r.district}${r.city ? `، ${r.city}` : ""}` : r.city || (r.province ? `همه شهرهای ${r.province}` : "همه مناطق")} — ${fa(r.limit)} مورد برای هر کلمه`,
            ),
            h("dt", {}, "بانک"),
            h("dd", {}, app.lists.find((l) => l.id === r.listId)?.name || "—"),
            h("dt", {}, "آخرین اجرا"),
            h("dd", {}, s.lastRun ? `${ago(s.lastRun)} — ${s.lastResult}` : "هنوز اجرا نشده"),
          ),
          h(
            "div",
            { class: "row end" },
            h("button", { class: "btn sm", type: "button", onclick: () => runJob("scan", `زمان‌بندی: ${s.name}`, "scan_start", { req: r }).then(refreshLists) }, icon("play", "sm"), "اجرا همین حالا"),
            h("button", { class: "btn sm", type: "button", onclick: () => editor(s, srcList, draw) }, icon("edit", "sm"), "ویرایش"),
            h(
              "button",
              {
                class: "btn sm ghost danger",
                type: "button",
                onclick: async () => {
                  if (!(await confirmBox(`زمان‌بندی «${s.name}» حذف شود؟ داده‌های استخراج‌شده می‌مانند.`, { ok: "حذف", danger: true }))) return;
                  if ((await safe(invoke("schedule_delete", { id: s.id }), "حذف شد")) !== undefined) draw();
                },
              },
              icon("trash", "sm"),
            ),
          ),
        );
      }),
    );
  };
  actions.append(h("button", { class: "btn primary", type: "button", onclick: () => editor({}, srcList, draw) }, icon("plus", "sm"), "زمان‌بندی تازه"));
  const side = h("div", { class: "stack" });
  root.append(h("div", { class: "side-grid" }, list, side));
  await Promise.all([draw(), backgroundCard(side)]);
}