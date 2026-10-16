// Admin: the customer registry (offline — needs no server) and offline licence issuing / renewal.
import { h, icon, invoke, fa, clear, safe, select, field, input, toast, modal, confirmBox, copy, jdate, ago } from "../core.js";
import { dateField, expiryPill } from "./admin-common.js";

export const PLANS = ["سالانه", "شش‌ماهه", "سه‌ماهه", "ماهانه", "آزمایشی"];
const PLAN_DAYS = { سالانه: 365, "شش‌ماهه": 183, "سه‌ماهه": 92, ماهانه: 31, آزمایشی: 7 };
const VIEWS = [
  ["all", "همه"],
  ["active", "فعال"],
  ["expiring", "رو به اتمام"],
  ["expired", "منقضی"],
  ["paused", "متوقف"],
];

/** Machine-code input that formats as XXXX-XXXX-XXXX-XXXX while typing. */
export function machineInput(value = "") {
  const m = input({ dir: "ltr", placeholder: "XXXX-XXXX-XXXX-XXXX", class: "input mono", value });
  m.addEventListener("input", () => {
    const raw = m.value.toUpperCase().replace(/[^0-9A-F]/g, "").slice(0, 16);
    m.value = raw.match(/.{1,4}/g)?.join("-") || "";
  });
  return m;
}

export function customerMessage(name, code, expires) {
  return `کد لایسنس هامیستا سلر برای «${name || "شما"}» (اعتبار تا ${jdate(expires)}):\n\n${code}\n\nدر برنامه: تنظیمات ← لایسنس ← کد آفلاین ← کد را بچسبانید و «فعال‌سازی» را بزنید.`;
}

/** Issue or renew an offline licence. `c` = existing customer (or {}) ; onDone(customerId). */
export function issueModal(c = {}, onDone) {
  const machine = machineInput(c.machine || "");
  const name = input({ value: c.name || "", placeholder: "نام مشتری یا شرکت" });
  const plan = select(PLANS, c.plan && PLANS.includes(c.plan) ? c.plan : PLANS[0]);
  const date = dateField("", { base: c.expires || "" });
  date.set(PLAN_DAYS[plan.value] || 365);
  plan.addEventListener("change", () => date.set(PLAN_DAYS[plan.value] || 365));
  const out = h("div", { class: "stack tight" });
  let issued = false;
  modal({
    title: c.id ? `تمدید / صدور لایسنس — ${c.name}` : "صدور لایسنس آفلاین",
    sub: c.expires ? `اعتبار فعلی تا ${jdate(c.expires)}؛ مدت‌های سریع از همان تاریخ اضافه می‌شوند.` : null,
    body: [
      h("div", { class: "grid-2" }, field("کد سیستم مشتری", machine, "مشتری این کد را از تنظیمات ← لایسنس می‌فرستد."), field("نام مشتری", name)),
      h("div", { class: "grid-2" }, field("نوع", plan), field("اعتبار تا (میلادی)", date.el)),
      out,
    ],
    actions: [
      { label: "بستن" },
      {
        label: "صدور لایسنس",
        kind: "primary",
        icon: "key",
        onClick: async () => {
          const r = await safe(invoke("admin_issue", { customerId: c.id || 0, machine: machine.value, customer: name.value, plan: plan.value, expires: date.input.value }));
          if (!r) return false;
          issued = true;
          const msg = customerMessage(name.value, r.code, date.input.value);
          copy(r.code, "لایسنس صادر و کپی شد");
          clear(
            out,
            h("div", { class: "callout ok" }, icon("check"), h("div", { class: "small" }, "لایسنس صادر و در کارت مشتری ثبت شد. آن را با تلگرام یا ایمیل برای مشتری بفرستید.")),
            h("div", { class: "code-box" }, r.code),
            h("div", { class: "row" }, h("button", { class: "btn sm", type: "button", onclick: () => copy(r.code, "کد کپی شد") }, icon("copy", "sm"), "کپی کد"), h("button", { class: "btn sm", type: "button", onclick: () => copy(msg, "پیام آماده کپی شد") }, icon("send", "sm"), "کپی پیام آماده")),
          );
          return false; // keep it open so the code can be copied
        },
      },
    ],
    onClose: () => issued && onDone?.(),
  });
}

function editModal(c, onDone) {
  const name = input({ value: c.name || "" });
  const phone = input({ value: c.phone || "", dir: "ltr" });
  const email = input({ value: c.email || "", dir: "ltr" });
  const machine = machineInput(c.machine || "");
  const plan = input({ value: c.plan || "" });
  const date = dateField(c.expires || "", { base: c.expires || "" });
  const status = select([["active", "فعال"], ["paused", "متوقف"]], c.status || "active");
  const cloud = input({ value: c.cloudUser || "", dir: "ltr", placeholder: "اگر حساب آنلاین هم دارد" });
  const note = h("textarea", { class: "input", rows: 2 }, c.note || "");
  modal({
    title: c.id ? "ویرایش مشتری" : "مشتری جدید",
    wide: true,
    body: [
      h("div", { class: "grid-3" }, field("نام مشتری", name), field("تلفن", phone), field("ایمیل", email)),
      h("div", { class: "grid-3" }, field("کد سیستم", machine), field("نوع اشتراک", plan), field("وضعیت", status)),
      h("div", { class: "grid-2" }, field("اعتبار تا", date.el, "فقط برای یادآوری؛ اعتبار واقعی را لایسنس صادرشده تعیین می‌کند."), field("نام کاربری آنلاین", cloud)),
      field("یادداشت", note),
    ],
    actions: [
      { label: "انصراف" },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () => {
          const id = await safe(
            invoke("admin_customer_save", {
              customer: { ...c, name: name.value, phone: phone.value, email: email.value, machine: machine.value, plan: plan.value, expires: date.input.value, status: status.value, cloudUser: cloud.value, note: note.value },
            }),
            "ذخیره شد",
          );
          if (id == null) return false;
          onDone?.(id);
        },
      },
    ],
  });
}

async function historyModal(c) {
  const list = await safe(invoke("admin_licences", { customerId: c.id, limit: 200 }));
  if (!list) return;
  modal({
    title: `سابقه لایسنس‌ها — ${c.name}`,
    wide: true,
    body: list.length
      ? h(
          "div",
          { class: "feed" },
          list.map((l) =>
            h(
              "div",
              { class: "it" },
              icon("key", "sm"),
              h("div", { class: "grow" }, h("b", {}, `${l.plan || "—"} · تا ${jdate(l.expires)}`), h("span", { class: "mono" }, `${l.machine} · صادرشده ${jdate(l.issuedAt, true)}`)),
              h("button", { class: "btn sm", type: "button", onclick: () => copy(l.code, "کد کپی شد") }, icon("copy", "sm"), "کپی کد"),
            ),
          ),
        )
      : h("div", { class: "empty small" }, "هنوز لایسنسی برای این مشتری صادر نشده"),
  });
}

export async function render(root, params, actions) {
  let view = params.view || "all";
  const q = input({ class: "input search-in", placeholder: "نام، تلفن، ایمیل، کد سیستم…", value: params.q || "" });
  const chips = h("div", { class: "chips" });
  const body = h("div", { class: "card flush" });

  actions.append(
    h("button", { class: "btn", type: "button", onclick: exportCsv }, icon("download", "sm"), "خروجی CSV"),
    h("button", { class: "btn", type: "button", onclick: () => editModal({}, draw) }, icon("plus", "sm"), "مشتری جدید"),
    h("button", { class: "btn accent", type: "button", onclick: () => issueModal({}, draw) }, icon("key", "sm"), "صدور لایسنس"),
  );

  let seq = 0;
  async function draw() {
    const my = ++seq;
    const r = await safe(Promise.all([invoke("admin_customers", { q: q.value, view }), invoke("admin_stats")]));
    if (my !== seq || !r) return;
    const [list, st] = r;
    const counts = { all: st.customers, active: st.active, expiring: st.expiring, expired: st.expired };
    clear(chips, VIEWS.map(([k, l]) => h("button", { type: "button", class: `chip click ${k === view ? "on" : ""}`.trim(), onclick: () => { view = k; draw(); } }, l, counts[k] != null ? ` (${fa(counts[k])})` : "")));
    if (!list.length) {
      clear(
        body,
        h("div", { class: "empty" }, icon("users", "lg"), q.value || view !== "all" ? "مشتری‌ای با این فیلتر نیست." : "هنوز مشتری‌ای ثبت نشده. با «صدور لایسنس» کارت مشتری خودکار ساخته می‌شود.", h("button", { class: "btn accent sm", type: "button", onclick: () => issueModal({}, draw) }, icon("key", "sm"), "صدور اولین لایسنس")),
      );
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
          h("colgroup", {}, h("col"), h("col", { class: "c-machine" }), h("col", { class: "c-status" }), h("col", { class: "c-status" }), h("col", { class: "c-act3" })),
          h("thead", {}, h("tr", {}, ["مشتری", "کد سیستم", "اشتراک", "اعتبار", ""].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            list.map((c) =>
              h(
                "tr",
                {},
                h(
                  "td",
                  {},
                  h(
                    "div",
                    { class: "name-cell" },
                    h("span", { class: "n", onclick: () => editModal(c, draw) }, c.name, c.status === "paused" ? h("span", { class: "pill warn" }, "متوقف") : null),
                    h("span", { class: "c" }, [c.phone, c.email, c.cloudUser && `آنلاین: ${c.cloudUser}`].filter(Boolean).join(" · ") || `ثبت ${ago(c.createdAt)}`),
                  ),
                ),
                h("td", {}, c.machine ? h("span", { class: "mono small" }, c.machine) : h("span", { class: "faint" }, "—")),
                h("td", {}, h("div", { class: "stack tight" }, h("span", {}, c.plan || "—"), h("span", { class: "small faint" }, `${fa(c.licenses)} لایسنس`))),
                h("td", {}, expiryPill(c.expires)),
                h(
                  "td",
                  {},
                  h(
                    "div",
                    { class: "row" },
                    h("button", { class: "btn sm icon-only", type: "button", title: "تمدید / صدور لایسنس", "aria-label": "تمدید", onclick: () => issueModal(c, draw) }, icon("key", "sm")),
                    h("button", { class: "btn sm icon-only", type: "button", title: "سابقه لایسنس‌ها", "aria-label": "سابقه", onclick: () => historyModal(c) }, icon("history", "sm")),
                    h("button", { class: "btn sm icon-only", type: "button", title: "ویرایش", "aria-label": "ویرایش", onclick: () => editModal(c, draw) }, icon("edit", "sm")),
                    h("button", { class: "btn sm icon-only danger", type: "button", title: "حذف", "aria-label": "حذف", onclick: () => remove(c) }, icon("trash", "sm")),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  async function remove(c) {
    if (!(await confirmBox(`کارت «${c.name}» حذف شود؟ لایسنس‌های صادرشده معتبر می‌مانند و در سابقه کلی باقی‌اند.`, { ok: "حذف", danger: true }))) return;
    if ((await safe(invoke("admin_customer_delete", { id: c.id }), "حذف شد")) !== undefined) draw();
  }

  async function exportCsv() {
    const path = await safe(invoke("admin_export", { kind: "customers", filter: null }));
    if (path) toast(`خروجی در ${path} ذخیره شد.`, "ok");
  }

  let t;
  q.addEventListener("input", () => {
    clearTimeout(t);
    t = setTimeout(draw, 250);
  });
  root.append(h("div", { class: "stack" }, h("div", { class: "card stack" }, h("div", { class: "filters" }, q), chips), body));
  await draw();
}
