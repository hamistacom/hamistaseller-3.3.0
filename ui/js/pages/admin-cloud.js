// Admin pages that live on the server: online users, plans, mega-bank, notifications/updates, logs.
// Each page first checks the connection (needServer) — offline, it explains and links to «اتصال سرور».
import { h, icon, invoke, fa, clear, safe, select, field, input, toggle, toast, modal, confirmBox, copy, jdate, ago, utc, bar } from "../core.js";
import { api, needServer, dateField, expiryPill, limitText } from "./admin-common.js";

const STATUS = { active: ["فعال", "ok"], suspended: ["تعلیق", "bad"] };

async function plansList() {
  return (await api("GET", "/plans")).items || [];
}

function randomPass() {
  const a = new Uint8Array(10);
  crypto.getRandomValues(a);
  return [...a].map((b) => "abcdefghjkmnpqrstuvwxyz23456789"[b % 31]).join("");
}

// ======================= online users =======================

function userModal(u, plans, onDone) {
  const isNew = !u.id;
  const username = input({ dir: "ltr", value: u.username || "", disabled: !isNew, autocomplete: "off" });
  const password = input({ dir: "ltr", value: isNew ? randomPass() : "", placeholder: isNew ? "" : "خالی = بدون تغییر", autocomplete: "new-password" });
  const name = input({ value: u.customerName || "" });
  const plan = select([["", "— بدون پلن —"], ...plans.map((p) => [p.id, p.name])], u.planId ?? plans[0]?.id ?? "");
  const date = dateField(u.expires || "", { base: u.expires || "" });
  if (isNew) date.set(365);
  const status = select([["active", "فعال"], ["suspended", "تعلیق"]], u.status || "active");
  const note = h("textarea", { class: "input", rows: 2 }, u.note || "");
  const gen = h("button", { class: "btn sm ghost", type: "button", onclick: () => (password.value = randomPass()) }, icon("refresh", "sm"), "رمز تصادفی");
  modal({
    title: isNew ? "کاربر آنلاین جدید" : `ویرایش ${u.username}`,
    wide: true,
    body: [
      h("div", { class: "grid-3" }, field("نام کاربری", username, isNew ? "حداقل ۳ حرف، انگلیسی" : null), field(isNew ? "رمز عبور" : "رمز جدید", h("div", { class: "stack tight" }, password, gen)), field("نام مشتری", name)),
      h("div", { class: "grid-3" }, field("پلن", plan), field("اعتبار اشتراک تا", date.el), isNew ? h("div") : field("وضعیت", status)),
      field("یادداشت", note),
    ],
    actions: [
      { label: "انصراف" },
      {
        label: isNew ? "ساخت کاربر" : "ذخیره",
        kind: "primary",
        onClick: async () => {
          const planId = plan.value ? Number(plan.value) : null;
          let ok;
          if (isNew) {
            ok = await safe(api("POST", "/users", { username: username.value.trim(), password: password.value, customerName: name.value, planId, expires: date.input.value, note: note.value }), "کاربر ساخته شد");
            if (ok) {
              const msg = `حساب هامیستا سلر شما آماده است:\nنام کاربری: ${username.value.trim()}\nرمز عبور: ${password.value}\n\nدر برنامه: تنظیمات ← لایسنس ← حساب آنلاین ← ورود.`;
              copy(msg, "مشخصات ورود برای ارسال به مشتری کپی شد");
            }
          } else {
            const patch = { customerName: name.value, planId, expires: date.input.value, status: status.value, note: note.value };
            if (password.value) patch.newPassword = password.value;
            ok = await safe(api("PUT", `/users/${u.id}`, patch), "ذخیره شد");
          }
          if (!ok) return false;
          onDone?.();
        },
      },
    ],
  });
}

async function devicesModal(u) {
  const body = h("div", { class: "feed" });
  const draw = async () => {
    const r = await safe(api("GET", `/users/${u.id}/devices`));
    if (!r) return;
    clear(
      body,
      r.items.length
        ? r.items.map((d) =>
            h(
              "div",
              { class: "it" },
              icon("cpu", "sm"),
              h("div", { class: "grow" }, h("b", {}, d.name || "بدون نام", d.revoked ? h("span", { class: "pill bad" }, "مسدود") : null), h("span", { class: "mono" }, `${d.machineId} · v${d.appVersion || "?"} · ${d.lastIp || ""}`), h("span", {}, `آخرین اتصال ${ago(utc(d.lastSeen))}`)),
              d.revoked
                ? null
                : h(
                    "button",
                    {
                      class: "btn sm danger",
                      type: "button",
                      onclick: async () => {
                        if (!(await confirmBox("این دستگاه مسدود شود؟ در اولین همگام‌سازی (حداکثر چند ساعت) از کار می‌افتد و یک جای خالی برای دستگاه دیگر باز می‌شود.", { ok: "مسدود کن", danger: true }))) return;
                        if (await safe(api("POST", `/devices/${d.id}/revoke`), "دستگاه مسدود شد")) draw();
                      },
                    },
                    "مسدود",
                  ),
            ),
          )
        : h("div", { class: "empty small" }, "هنوز با هیچ دستگاهی وارد نشده است"),
    );
  };
  modal({ title: `دستگاه‌های ${u.username}`, wide: true, body });
  await draw();
}

export async function renderUsers(root, params, actions) {
  if (!(await needServer(root))) return;
  const plans = await plansList();
  const q = input({ class: "input search-in", placeholder: "نام کاربری یا نام مشتری…", value: params.q || "" });
  const body = h("div", { class: "card flush" });
  actions.append(h("button", { class: "btn accent", type: "button", onclick: () => userModal({}, plans, draw) }, icon("plus", "sm"), "کاربر جدید"));
  let seq = 0;
  async function draw() {
    const my = ++seq;
    const term = encodeURIComponent(q.value.trim()).replace(/[!'()*~]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
    const r = await safe(api("GET", `/users?q=${term}`));
    if (my !== seq || !r) return;
    if (!r.items.length) {
      clear(body, h("div", { class: "empty" }, icon("users", "lg"), q.value ? "کاربری پیدا نشد." : plans.length ? "هنوز کاربر آنلاینی نساخته‌اید." : "اول از «پلن‌ها» یک پلن بسازید، بعد کاربر.", h("button", { class: "btn accent sm", type: "button", onclick: () => userModal({}, plans, draw) }, "کاربر جدید")));
      return;
    }
    clear(
      body,
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          { class: "t" },
          h("colgroup", {}, h("col"), h("col", { class: "c-status" }), h("col", { class: "c-status" }), h("col", { class: "c-status" }), h("col", { class: "c-act3" })),
          h("thead", {}, h("tr", {}, ["کاربر", "پلن", "وضعیت", "اعتبار", ""].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            r.items.map((u) => {
              const [st, tone] = STATUS[u.status] || [u.status, ""];
              const flip = u.status === "active" ? "suspended" : "active";
              return h(
                "tr",
                {},
                h("td", {}, h("div", { class: "name-cell" }, h("span", { class: "n ltr", onclick: () => userModal(u, plans, draw) }, u.username), h("span", { class: "c" }, [u.customerName, `${fa(u.devices)} دستگاه`, `ساخت ${jdate(utc(u.createdAt))}`].filter(Boolean).join(" · ")))),
                h("td", {}, u.plan || h("span", { class: "faint" }, "—")),
                h("td", {}, h("span", { class: `pill ${tone}` }, st)),
                h("td", {}, expiryPill(u.expires)),
                h(
                  "td",
                  {},
                  h(
                    "div",
                    { class: "row" },
                    h("button", { class: "btn sm icon-only", type: "button", title: "ویرایش و تمدید", "aria-label": "ویرایش", onclick: () => userModal(u, plans, draw) }, icon("edit", "sm")),
                    h("button", { class: "btn sm icon-only", type: "button", title: "دستگاه‌ها", "aria-label": "دستگاه‌ها", onclick: () => devicesModal(u) }, icon("cpu", "sm")),
                    h(
                      "button",
                      {
                        class: `btn sm icon-only ${flip === "suspended" ? "danger" : ""}`.trim(),
                        type: "button",
                        title: flip === "suspended" ? "تعلیق" : "فعال‌سازی دوباره",
                        "aria-label": "تغییر وضعیت",
                        onclick: async () => {
                          if (flip === "suspended" && !(await confirmBox(`حساب «${u.username}» تعلیق شود؟ برنامه مشتری در اولین همگام‌سازی متوقف می‌شود.`, { ok: "تعلیق", danger: true }))) return;
                          if (await safe(api("PUT", `/users/${u.id}`, { status: flip }), flip === "active" ? "فعال شد" : "تعلیق شد")) draw();
                        },
                      },
                      icon(flip === "suspended" ? "stop" : "play", "sm"),
                    ),
                  ),
                ),
              );
            }),
          ),
        ),
      ),
    );
  }
  let t;
  q.addEventListener("input", () => {
    clearTimeout(t);
    t = setTimeout(draw, 300);
  });
  root.append(h("div", { class: "stack" }, h("div", { class: "card" }, h("div", { class: "filters" }, q)), body));
  await draw();
}

// ======================= plans =======================

async function planModal(p, onDone) {
  const sources = (await invoke("sources_list")).filter((s) => s.kind === "scan");
  const chosen = new Set(String(p.sources || "").split(",").map((s) => s.trim()).filter(Boolean));
  const name = input({ value: p.name || "" });
  const devices = input({ type: "number", min: 1, value: p.maxDevices ?? 1, dir: "ltr" });
  const scan = input({ type: "number", min: 0, value: p.scanQuota ?? 0, dir: "ltr" });
  const send = input({ type: "number", min: 0, value: p.sendQuota ?? 0, dir: "ltr" });
  const mega = toggle("دسترسی به بانک جامع", !!p.megabank);
  const note = input({ value: p.note || "" });
  const boxes = sources.map((s) => {
    const t = toggle(`${s.name}${s.ready ? "" : " (به‌زودی)"}`, chosen.has(s.id));
    t.input.dataset.id = s.id;
    return t;
  });
  modal({
    title: p.id ? `ویرایش پلن «${p.name}»` : "پلن جدید",
    wide: true,
    body: [
      h("div", { class: "grid-2" }, field("نام پلن", name), field("یادداشت", note)),
      h("div", { class: "grid-3" }, field("حداکثر دستگاه", devices), field("سهمیه استخراج ماهانه", scan, "تعداد استخراج؛ ۰ = نامحدود"), field("سهمیه ارسال ماهانه", send, "تعداد پیام موفق؛ ۰ = نامحدود")),
      field("منابع مجاز", h("div", { class: "chips" }, boxes.map((b) => b.el)), "هیچ‌کدام انتخاب نشود = همه منابع"),
      mega.el,
    ],
    actions: [
      { label: "انصراف" },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () => {
          const srcs = boxes.filter((b) => b.input.checked).map((b) => b.input.dataset.id).join(",");
          const ok = await safe(
            api("POST", "/plans", { id: p.id || 0, name: name.value, maxDevices: Math.max(1, Number(devices.value) || 1), scanQuota: Math.max(0, Number(scan.value) || 0), sendQuota: Math.max(0, Number(send.value) || 0), sources: srcs, megabank: mega.input.checked, note: note.value }),
            "پلن ذخیره شد",
          );
          if (!ok) return false;
          onDone?.();
        },
      },
    ],
  });
}

/** Free trial: one per PC, on a plan the admin picks. */
async function trialCard(plans) {
  const t = (await safe(api("GET", "/trial"))) || { enabled: false, days: 3, used: 0 };
  const on = toggle("دوره آزمایشی رایگان برای سیستم‌های تازه", !!t.enabled);
  const plan = select([["", "— پلن —"], ...plans.map((p) => [p.id, p.name])], t.planId ?? "");
  const days = input({ type: "number", min: 1, max: 60, dir: "ltr", value: t.days || 3 });
  const save = h("button", { class: "btn sm", type: "button" }, icon("check", "sm"), "ذخیره");
  save.addEventListener("click", async () => {
    save.disabled = true;
    await safe(api("PUT", "/trial", { enabled: on.input.checked, planId: plan.value ? Number(plan.value) : null, days: Number(days.value) || 3 }), "تنظیمات دوره آزمایشی ذخیره شد");
    save.disabled = false;
  });
  return h(
    "div",
    { class: "card stack span-3" },
    h("div", { class: "card-h" }, h("span", { class: "ic-box" }, icon("sparkle", "sm")), h("h2", {}, "دوره آزمایشی"), h("span", { class: "pill info" }, `${fa(t.used || 0)} سیستم تاکنون استفاده کرده`)),
    h("p", { class: "small muted" }, "مشتری بدون حساب، از «تنظیمات ← لایسنس ← دوره آزمایشی رایگان» با نام و موبایل یک حساب کوتاه‌مدت می‌گیرد. هر سیستم فقط یک بار. حساب‌های آزمایشی با نام trial- در «کاربران آنلاین» دیده می‌شوند و قابل تمدید یا تبدیل به اشتراک‌اند."),
    h("div", { class: "row wrap" }, on.el, h("div", { class: "grow" }), field("پلن آزمایشی", plan), field("مدت (روز)", days), save),
  );
}

export async function renderPlans(root, _params, actions) {
  if (!(await needServer(root))) return;
  const grid = h("div", { class: "grid-3" });
  actions.append(h("button", { class: "btn accent", type: "button", onclick: () => planModal({}, draw) }, icon("plus", "sm"), "پلن جدید"));
  const names = Object.fromEntries((await invoke("sources_list")).map((s) => [s.id, s.name]));
  async function draw() {
    const plans = await safe(plansList());
    if (!plans) return;
    if (!plans.length) {
      clear(grid, h("div", { class: "card span-3" }, h("div", { class: "empty" }, icon("layers", "lg"), "هنوز پلنی ندارید. مثلاً «ماهانه — ۱ دستگاه — ۳۰ استخراج».", h("button", { class: "btn accent sm", type: "button", onclick: () => planModal({}, draw) }, "پلن جدید"))));
      return;
    }
    clear(
      grid,
      plans.map((p) =>
        h(
          "div",
          { class: "card stack" },
          h("div", { class: "card-h" }, h("h2", {}, p.name), h("span", { class: "pill info" }, `${fa(p.users)} کاربر`)),
          h(
            "dl",
            { class: "kv" },
            h("dt", {}, "دستگاه"),
            h("dd", {}, fa(p.maxDevices)),
            h("dt", {}, "استخراج / ماه"),
            h("dd", {}, limitText(p.scanQuota)),
            h("dt", {}, "ارسال / ماه"),
            h("dd", {}, limitText(p.sendQuota)),
            h("dt", {}, "منابع"),
            h("dd", {}, p.sources ? p.sources.split(",").map((s) => names[s.trim()] || s).join("، ") : "همه"),
            h("dt", {}, "بانک جامع"),
            h("dd", {}, p.megabank ? h("span", { class: "pill ok" }, "دارد") : "ندارد"),
          ),
          p.note ? h("p", { class: "small muted" }, p.note) : null,
          h(
            "div",
            { class: "row end" },
            h("button", { class: "btn sm", type: "button", onclick: () => planModal(p, draw) }, icon("edit", "sm"), "ویرایش"),
            h(
              "button",
              {
                class: "btn sm ghost danger",
                type: "button",
                onclick: async () => {
                  if (!(await confirmBox(`پلن «${p.name}» حذف شود؟`, { ok: "حذف", danger: true }))) return;
                  if (await safe(api("DELETE", `/plans/${p.id}`), "حذف شد")) draw();
                },
              },
              icon("trash", "sm"),
            ),
          ),
        ),
      ),
    );
  }
  root.append(grid);
  await draw();
  const plans = await safe(plansList());
  if (plans) root.prepend(h("div", { class: "grid-3 trial-row" }, await trialCard(plans)));
}

// ======================= mega-bank =======================

export async function renderMega(root, _params, actions) {
  if (!(await needServer(root))) return;
  const PAGE = 50;
  const f = { q: "", province: "", city: "", category: "", source: "" };
  const q = input({ class: "input search-in", placeholder: "نام، شماره، ایمیل…" });
  const chips = h("div", { class: "chips" });
  const facets = h("div", { class: "grid-4" });
  const body = h("div");
  const pager = h("div", { class: "pager" });
  let offset = 0;
  const srcNames = Object.fromEntries((await invoke("sources_list")).map((s) => [s.id, s.name]));
  actions.append(
    h(
      "button",
      {
        class: "btn",
        type: "button",
        onclick: async (e) => {
          e.currentTarget.disabled = true;
          const path = await safe(invoke("admin_export", { kind: "megabank", filter: { ...f, q: q.value.trim() } }));
          e.currentTarget.disabled = false;
          if (path) toast(`خروجی در ${path} ذخیره شد.`, "ok");
        },
      },
      icon("download", "sm"),
      "خروجی CSV (فیلتر فعلی)",
    ),
  );

  const facet = (title, key, items, label = (v) => v) =>
    h(
      "div",
      { class: "card stack tight facet" },
      h("h3", {}, title),
      items.length
        ? h(
            "div",
            { class: "rank" },
            items.slice(0, 8).map(([v, n]) => h("div", { class: "r" }, h("button", { class: "link clip", type: "button", onclick: () => { f[key] = v; load(0); } }, label(v)), h("span", { class: "muted" }, fa(n)), bar(n / Math.max(1, items[0][1])))),
          )
        : h("div", { class: "empty small" }, "—"),
    );

  let seq = 0;
  async function load(off = 0) {
    const my = ++seq;
    offset = Math.max(0, off);
    const s = { ...f, q: q.value.trim(), offset, limit: PAGE };
    const [r, fc] = await Promise.all([safe(api("POST", "/megabank/search", s)), safe(api("POST", "/megabank/facets", s))]);
    if (my !== seq || !r || !fc) return;
    clear(
      chips,
      Object.entries(f)
        .filter(([k, v]) => v && k !== "q")
        .map(([k, v]) => h("span", { class: "chip" }, k === "source" ? srcNames[v] || v : v, h("button", { type: "button", "aria-label": "حذف فیلتر", onclick: () => { f[k] = ""; load(0); } }, icon("x", "sm")))),
    );
    clear(facets, facet("استان", "province", fc.provinces), facet("شهر", "city", fc.cities), facet("حوزه کاری", "category", fc.categories), facet("منبع", "source", fc.sources, (v) => srcNames[v] || v));
    if (!r.rows.length) {
      clear(body, h("div", { class: "empty" }, icon("database", "lg"), r.total ? "" : "ابربانک هنوز خالی است؛ با استخراج‌های مشتریان آنلاین پر می‌شود."));
      clear(pager);
      return;
    }
    clear(
      body,
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          { class: "t" },
          h("colgroup", {}, h("col"), h("col", { class: "c-phone" }), h("col"), h("col", { class: "c-status" }), h("col", { class: "c-status" })),
          h("thead", {}, h("tr", {}, ["کسب‌وکار", "شماره‌ها", "منطقه و حوزه", "منبع", "اعتبار"].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            r.rows.map((x) => {
              const nums = [...x.mobiles.split(","), ...x.landlines.split(",")].filter(Boolean);
              const socials = [x.website, x.emails, x.instagram, x.telegram].filter(Boolean).join(" · ");
              return h(
                "tr",
                {},
                h("td", {}, h("div", { class: "name-cell" }, h("span", { class: "n" }, x.name || "—"), h("span", { class: "c" }, socials || x.address || ""))),
                h("td", {}, h("div", { class: "phones ltr" }, nums.slice(0, 2).map((n) => h("span", {}, n)), nums.length > 2 ? h("span", { class: "x" }, `+${fa(nums.length - 2)}`) : null)),
                h("td", {}, h("div", { class: "stack tight" }, h("span", { class: "clip" }, [x.province, x.city].filter(Boolean).join("، ") || "—"), h("span", { class: "small faint clip" }, x.category))),
                h("td", {}, srcNames[x.source] || x.source || "—"),
                h("td", {}, h("div", { class: "stack tight" }, h("span", { class: "pill info" }, `${fa(x.reportCount)} گزارش`), h("span", { class: "small faint" }, ago(utc(x.lastSeen))))),
              );
            }),
          ),
        ),
      ),
    );
    clear(
      pager,
      h("span", {}, `${fa(offset + 1)}–${fa(offset + r.rows.length)} از ${fa(r.total)}`),
      h("div", { class: "row" }, h("button", { class: "btn sm", type: "button", disabled: offset === 0, onclick: () => load(offset - PAGE) }, "قبلی"), h("button", { class: "btn sm", type: "button", disabled: offset + PAGE >= r.total, onclick: () => load(offset + PAGE) }, "بعدی")),
    );
  }
  q.addEventListener("keydown", (e) => e.key === "Enter" && load(0));
  root.append(
    h(
      "div",
      { class: "stack" },
      h("div", { class: "card stack" }, h("div", { class: "filters" }, q, h("button", { class: "btn", type: "button", onclick: () => load(0) }, icon("search", "sm"), "جستجو")), chips),
      facets,
      h("div", { class: "card flush" }, body, pager),
      h("p", { class: "small faint" }, "ابربانک فقط اطلاعات عمومی کسب‌وکارهاست که برنامه‌های مشتریان از منابع عمومی پیدا کرده‌اند؛ یادداشت‌ها، برچسب‌ها و داده‌های خصوصی مشتریان در آن نیست."),
    ),
  );
  await load(0);
}

// ======================= notifications, version, scenarios, opt-outs =======================

export async function renderNotify(root) {
  if (!(await needServer(root))) return;
  const plans = await plansList();

  // --- notifications
  const title = input({ placeholder: "عنوان اعلان" });
  const text = h("textarea", { class: "input", rows: 3, placeholder: "متن (اختیاری)" });
  const audience = select([["all", "همه مشتریان"], ...plans.map((p) => [`plan:${p.id}`, `پلن ${p.name}`])], "all");
  const list = h("div", { class: "feed" });
  const drawList = async () => {
    const r = await safe(api("GET", "/notifications"));
    if (!r) return;
    const aud = Object.fromEntries([["all", "همه"], ...plans.map((p) => [`plan:${p.id}`, `پلن ${p.name}`])]);
    clear(list, r.items.length ? r.items.slice(0, 20).map((n) => h("div", { class: "it" }, icon("bell", "sm"), h("div", { class: "grow" }, h("b", {}, n.title), h("span", {}, `${aud[n.audience] || n.audience} · ${ago(utc(n.createdAt))}`)))) : h("div", { class: "empty small" }, "هنوز اعلانی نفرستاده‌اید"));
  };
  const sendBtn = h("button", { class: "btn primary", type: "button" }, icon("send", "sm"), "ارسال اعلان");
  sendBtn.addEventListener("click", async () => {
    if (!title.value.trim()) return toast("عنوان اعلان را بنویسید.", "bad");
    sendBtn.disabled = true;
    const ok = await safe(api("POST", "/notifications", { title: title.value.trim(), body: text.value.trim(), audience: audience.value }), "اعلان ثبت شد؛ در اولین بازشدن برنامه‌ها نمایش داده می‌شود");
    sendBtn.disabled = false;
    if (ok) {
      title.value = text.value = "";
      drawList();
    }
  });

  // --- version
  const v = (await safe(api("GET", "/version"))) || {};
  const ver = input({ dir: "ltr", value: v.version || "", placeholder: "2.1.0" });
  const url = input({ dir: "ltr", value: v.url || "", placeholder: "https://hamista.ir/download" });
  const notes = input({ value: v.notes || "", placeholder: "تغییرات این نسخه" });
  const mandatory = toggle("به‌روزرسانی اجباری (بنر قرمز)", !!v.mandatory);
  const saveVer = h("button", { class: "btn", type: "button" }, icon("check", "sm"), "انتشار");
  saveVer.addEventListener("click", async () => {
    if (!/^\d+\.\d+\.\d+$/.test(ver.value.trim())) return toast("شماره نسخه به شکل 2.1.0 باشد.", "bad");
    saveVer.disabled = true;
    await safe(api("PUT", "/version", { version: ver.value.trim(), url: url.value.trim(), notes: notes.value.trim(), mandatory: mandatory.input.checked }), "نسخه منتشر شد");
    saveVer.disabled = false;
  });

  // --- scenarios (remote scanner fixes)
  const scen = (await safe(api("GET", "/scenarios"))) || { items: [] };
  const cur = scen.items.find((s) => s.source === "maps");
  const body = h("textarea", { class: "input mono", rows: 6, dir: "ltr", placeholder: '{"listJs": "...", "placeJs": "...", "baladJs": "...", "searchUrl": "https://www.bing.com/search?q={q}&first={first}", "searchJs": "..."}' }, cur ? JSON.stringify(cur.body, null, 2) : "");
  const saveScen = h("button", { class: "btn", type: "button" }, icon("upload", "sm"), "انتشار سناریو");
  saveScen.addEventListener("click", async () => {
    let parsed;
    try {
      parsed = JSON.parse(body.value || "{}");
    } catch {
      return toast("متن سناریو JSON معتبر نیست.", "bad");
    }
    if (!(await confirmBox("سناریوی جدید در اولین همگام‌سازی روی همه برنامه‌های مشتری اعمال می‌شود. فقط سناریوی آزموده‌شده منتشر کنید.", { ok: "انتشار" }))) return;
    const r = await safe(api("PUT", "/scenarios/maps", { body: parsed }));
    if (r) toast(`سناریو نسخه ${fa(r.version)} منتشر شد.`, "ok");
  });

  // --- shared opt-outs
  const opt = h("textarea", { class: "input mono", rows: 3, dir: "ltr", placeholder: "09120000000\nname@example.com" });
  const optCount = h("span", { class: "small muted" });
  const drawOpt = async () => {
    const r = await safe(api("GET", "/optouts"));
    if (r) optCount.textContent = `${fa(r.items.length)} مورد در لیست مشترک`;
  };
  const saveOpt = h("button", { class: "btn", type: "button" }, icon("plus", "sm"), "افزودن");
  saveOpt.addEventListener("click", async () => {
    const values = opt.value.split(/[\s,،]+/).map((s) => s.trim()).filter(Boolean);
    if (!values.length) return;
    if (await safe(api("POST", "/optouts", { values }), "به لیست عدم ارسال اضافه شد")) {
      opt.value = "";
      drawOpt();
    }
  });

  root.append(
    h(
      "div",
      { class: "grid-2" },
      h("div", { class: "card stack" }, h("h2", {}, "اعلان به مشتریان"), title, text, h("div", { class: "row" }, h("span", { class: "grow" }, audience), sendBtn), h("h3", {}, "اعلان‌های اخیر"), list),
      h(
        "div",
        { class: "stack" },
        h("div", { class: "card stack" }, h("h2", {}, "نسخه جدید برنامه"), h("p", { class: "small muted" }, "برنامه‌های مشتری آنلاین، نسخه جدیدتر را با بنر «دریافت نسخه جدید» نشان می‌دهند."), h("div", { class: "grid-2" }, field("شماره نسخه", ver), field("لینک دانلود", url)), field("توضیح", notes), h("div", { class: "row" }, h("span", { class: "grow" }, mandatory.el), saveVer)),
        h(
          "div",
          { class: "card stack" },
          h("div", { class: "card-h" }, h("h2", {}, "سناریوی استخراج (گوگل‌مپ، بلد، جستجوی وب)"), cur ? h("span", { class: "pill info" }, `نسخه ${fa(cur.version)}`) : h("span", { class: "pill" }, "پیش‌فرض برنامه")),
          h("p", { class: "small muted" }, "اگر گوگل‌مپ، بلد یا موتور جستجو ظاهر صفحه‌هایش را عوض کند، اسکریپت‌های جایگزین را اینجا منتشر کنید تا برنامه‌ها بدون نصب نسخه جدید درست شوند. کلیدها: listJs، scrollJs، consentJs، placeJs (گوگل‌مپ)، baladJs (بلد)، searchUrl و searchJs (جستجوی وب). خالی گذاشتن هر کلید = اسکریپت داخلی برنامه."),
          body,
          h("div", { class: "row end" }, saveScen),
        ),
        h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "لیست عدم ارسال مشترک"), optCount), opt, h("div", { class: "row end" }, saveOpt)),
      ),
    ),
  );
  await Promise.all([drawList(), drawOpt()]);
}

// ======================= logs =======================

const KINDS = { login: "ورود", scan: "استخراج", campaign: "ارسال", ingest: "ارسال به ابربانک", quota: "سقف سهمیه", admin: "مدیر", admin_fail: "ورود ناموفق مدیر" };

export async function renderLogs(root) {
  if (!(await needServer(root))) return;
  let kind = "";
  const chips = h("div", { class: "chips" });
  const body = h("div", { class: "card flush" });
  let seq = 0;
  async function draw() {
    const my = ++seq;
    clear(chips, [["", "همه"], ...Object.entries(KINDS)].map(([k, l]) => h("button", { type: "button", class: `chip click ${k === kind ? "on" : ""}`.trim(), onclick: () => { kind = k; draw(); } }, l)));
    const r = await safe(api("GET", `/logs?limit=500${kind ? `&kind=${kind}` : ""}`));
    if (my !== seq || !r) return;
    clear(
      body,
      r.items.length
        ? h(
            "div",
            { class: "table-wrap" },
            h(
              "table",
              { class: "t" },
              h("colgroup", {}, h("col", { class: "c-status" }), h("col", { class: "c-status" }), h("col", { class: "c-status" }), h("col"), h("col", { class: "c-status" })),
              h("thead", {}, h("tr", {}, ["زمان", "کاربر", "رویداد", "جزئیات", "IP"].map((t) => h("th", {}, t)))),
              h(
                "tbody",
                {},
                r.items.map((a) =>
                  h(
                    "tr",
                    {},
                    h("td", { class: "small", title: jdate(utc(a.at), true) }, ago(utc(a.at))),
                    h("td", { class: "ltr clip" }, a.username || "—"),
                    h("td", {}, h("span", { class: `pill ${a.kind === "admin_fail" || a.kind === "quota" ? "warn" : ""}`.trim() }, KINDS[a.kind] || a.kind)),
                    h("td", { class: "small clip ltr" }, a.detail),
                    h("td", { class: "small mono" }, a.ip || ""),
                  ),
                ),
              ),
            ),
          )
        : h("div", { class: "empty" }, icon("history", "lg"), "رویدادی ثبت نشده"),
    );
  }
  root.append(h("div", { class: "stack" }, chips, body));
  await draw();
}
