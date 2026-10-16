// Admin: offline licence issuing (records the customer card too) and the signing key + backups.
import { h, icon, invoke, clear, safe, select, field, input, modal, confirmBox, copy, jdate, ago } from "../core.js";
import { backups } from "./settings.js";
import { dateField } from "./admin-common.js";
import { PLANS, machineInput, customerMessage, issueModal } from "./admin-customers.js";
import { go } from "../state.js";

const PLAN_DAYS = { سالانه: 365, "شش‌ماهه": 183, "سه‌ماهه": 92, ماهانه: 31, آزمایشی: 7 };

export async function render(root) {
  const key = await safe(invoke("license_key_info"));
  if (!key) return;
  const machine = machineInput();
  const customer = input({ placeholder: "نام مشتری یا شرکت" });
  const plan = select(PLANS, PLANS[0]);
  const date = dateField();
  date.set(365);
  plan.addEventListener("change", () => date.set(PLAN_DAYS[plan.value] || 365));
  const out = h("div", { class: "stack tight" });
  const issue = h("button", { class: "btn primary", type: "button" }, icon("key", "sm"), "صدور لایسنس");
  issue.addEventListener("click", async () => {
    issue.disabled = true;
    const r = await safe(invoke("admin_issue", { customerId: 0, machine: machine.value, customer: customer.value, plan: plan.value, expires: date.input.value }));
    issue.disabled = false;
    if (!r) return;
    const message = customerMessage(customer.value, r.code, date.input.value);
    copy(r.code, "کد لایسنس کپی شد");
    clear(
      out,
      h("div", { class: "callout ok" }, icon("check"), h("div", { class: "small" }, "لایسنس صادر، کپی و در «مشتریان» ثبت شد. آن را با تلگرام یا ایمیل برای مشتری بفرستید.")),
      h("div", { class: "code-box" }, r.code),
      h(
        "div",
        { class: "row wrap" },
        h("button", { class: "btn sm", type: "button", onclick: () => copy(r.code, "کد کپی شد") }, icon("copy", "sm"), "کپی کد"),
        h("button", { class: "btn sm", type: "button", onclick: () => copy(message, "پیام آماده کپی شد") }, icon("send", "sm"), "کپی پیام آماده برای مشتری"),
        h("button", { class: "btn sm ghost", type: "button", onclick: () => go("customers", { q: machine.value }) }, icon("users", "sm"), "کارت مشتری"),
      ),
    );
    drawLog();
  });

  // renew an existing customer quickly
  const find = input({ placeholder: "جستجوی مشتری برای تمدید…" });
  const found = h("div", { class: "feed" });
  let t;
  find.addEventListener("input", () => {
    clearTimeout(t);
    t = setTimeout(async () => {
      const q = find.value.trim();
      if (!q) return clear(found);
      const list = await safe(invoke("admin_customers", { q, view: "all" }));
      if (!list) return;
      clear(
        found,
        list.slice(0, 6).map((c) =>
          h(
            "div",
            { class: "it" },
            h("div", { class: "grow" }, h("b", {}, c.name), h("span", { class: "mono" }, `${c.machine || "—"} · ${c.expires ? `تا ${jdate(c.expires)}` : "بدون تاریخ"}`)),
            h("button", { class: "btn sm", type: "button", onclick: () => issueModal(c, () => { find.value = ""; clear(found); drawLog(); }) }, icon("key", "sm"), "تمدید"),
          ),
        ),
        list.length ? null : h("div", { class: "empty small" }, "پیدا نشد"),
      );
    }, 250);
  });

  const log = h("div", { class: "feed" });
  const drawLog = async () => {
    const list = await safe(invoke("admin_licences", { customerId: 0, limit: 20 }));
    if (!list) return;
    clear(
      log,
      list.length
        ? list.map((l) => h("div", { class: "it" }, h("div", { class: "grow" }, h("b", {}, `${l.customer || "—"} · ${l.plan}`), h("span", {}, `تا ${jdate(l.expires)} · ${ago(l.issuedAt)}`)), h("button", { class: "btn sm ghost icon-only", type: "button", title: "کپی کد", "aria-label": "کپی کد", onclick: () => copy(l.code, "کد کپی شد") }, icon("copy", "sm"))))
        : h("div", { class: "empty small" }, "هنوز لایسنسی صادر نشده"),
    );
  };
  drawLog();

  root.append(
    h(
      "div",
      { class: "side-grid issuer-grid" },
      h(
        "div",
        { class: "stack" },
        key.embeddedMatches ? null : keyWarning(),
        h(
          "div",
          { class: "card stack" },
          h("div", { class: "card-h" }, h("h2", {}, "صدور لایسنس آفلاین")),
          h("div", { class: "grid-2" }, field("کد سیستم مشتری", machine, "مشتری این کد را از تنظیمات ← لایسنس برایتان می‌فرستد."), field("نام مشتری", customer)),
          h("div", { class: "grid-2" }, field("نوع", plan), field("اعتبار تا (میلادی)", date.el)),
          h("div", { class: "row end" }, issue),
          out,
        ),
        h("div", { class: "card stack" }, h("h2", {}, "تمدید برای مشتری قبلی"), find, found),
      ),
      h("div", { class: "card stack" }, h("h2", {}, "آخرین لایسنس‌ها"), log),
    ),
  );
}

function keyWarning() {
  return h(
    "div",
    { class: "callout" },
    icon("alert"),
    h("div", { class: "small grow" }, "کلید عمومی این پنل هنوز در فایل license.pub نسخه مشتری نیست؛ لایسنس‌ها تا آن موقع در برنامه مشتری تأیید نمی‌شوند."),
    h("button", { class: "btn sm", type: "button", onclick: () => go("keys") }, "کلید و پشتیبان"),
  );
}

// ---------------- signing key + backups ----------------

export async function renderKeys(root) {
  const key = await safe(invoke("license_key_info"));
  if (!key) return;
  const box = h("div"); // backups() redraws into its own container
  root.append(
    h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "card stack license-card" },
        h("h2", {}, "کلید امضای لایسنس"),
        h("p", { class: "small muted" }, "همه لایسنس‌ها — کد آفلاین و حساب آنلاین — با همین یک کلید امضا می‌شوند. کلید خصوصی فقط روی این کامپیوتر (و اگر سرور دارید، روی سرور) است."),
        h("span", { class: "label-t" }, "کلید عمومی (برای فایل src-tauri/license.pub نسخه مشتری)"),
        h("div", { class: "code-box" }, key.publicKey),
        h("div", { class: "row wrap" }, h("button", { class: "btn sm", type: "button", onclick: () => copy(key.publicKey, "کلید عمومی کپی شد") }, icon("copy", "sm"), "کپی کلید عمومی")),
        key.embeddedMatches
          ? h("div", { class: "callout ok" }, icon("check"), h("div", { class: "small" }, "این کلید با فایل license.pub این build یکی است."))
          : h("div", { class: "callout" }, icon("alert"), h("div", { class: "small" }, "این کلید هنوز در src-tauri/license.pub نیست. آن را جایگزین کنید و نسخه مشتری را دوباره build کنید؛ وگرنه لایسنس‌ها در نسخه مشتری تأیید نمی‌شوند.")),
        h(
          "div",
          { class: "row wrap" },
          h("button", { class: "btn sm", type: "button", onclick: exportKey }, icon("download", "sm"), "پشتیبان کلید خصوصی"),
          h("button", { class: "btn sm", type: "button", onclick: importKey }, icon("upload", "sm"), "ورود کلید خصوصی"),
        ),
      ),
      box,
    ),
  );
  await backups(box);
}

export async function exportKey() {
  if (!(await confirmBox("کلید خصوصی را فقط جایی امن و خارج از این کامپیوتر نگه دارید. هر کس آن را داشته باشد می‌تواند لایسنس بسازد.", { ok: "نمایش کلید", title: "پشتیبان کلید خصوصی" }))) return;
  const seed = await safe(invoke("license_export_key"));
  if (!seed) return;
  modal({ title: "کلید خصوصی", body: [h("div", { class: "code-box" }, seed), h("p", { class: "small muted" }, "این متن ۶۴ کاراکتری را در مدیر رمز یا فایل رمزدار نگه دارید. برای سرور، همین متن در HAMISTA_SIGNING_SEED قرار می‌گیرد.")], actions: [{ label: "کپی", icon: "copy", onClick: () => (copy(seed, "کلید خصوصی کپی شد"), false) }, { label: "بستن", kind: "primary" }] });
}

function importKey() {
  const seed = h("textarea", { class: "input mono", rows: 2, dir: "ltr", placeholder: "64 hex characters" });
  modal({
    title: "ورود کلید خصوصی",
    body: [h("div", { class: "callout" }, icon("alert"), h("div", { class: "small" }, "کلید فعلی این کامپیوتر جایگزین می‌شود. لایسنس‌هایی که با کلید فعلی صادر شده‌اند فقط با همان کلید عمومی تأیید می‌شوند.")), field("کلید خصوصی", seed)],
    actions: [
      { label: "انصراف" },
      {
        label: "جایگزینی کلید",
        kind: "danger fill",
        onClick: async () => {
          const k = await safe(invoke("license_import_key", { seed: seed.value }), "کلید جایگزین شد");
          if (!k) return false;
          location.reload();
        },
      },
    ],
  });
}
