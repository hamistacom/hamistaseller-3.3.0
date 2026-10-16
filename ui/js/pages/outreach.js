// Campaigns: one message to a group of your contacts, sent one of two ways —
//   * SMS, automatically, through your SMS panel (Settings → SMS panel);
//   * WhatsApp / Telegram / Eitaa / Bale with your own account, one chat at a time in the chat assistant.
import { h, icon, invoke, fa, clear, safe, select, field, input, toggle, toast, modal, confirmBox, bar, jdate, newChannel } from "../core.js";
import { app, bankOptions, go } from "../state.js";
import { onJob, runJob } from "../jobs.js";
import { ACCOUNT_KINDS, MESSENGERS, PLACEHOLDERS } from "../data.js";
import { STATUS } from "../core.js";

let off = null;
let seq = 0; // bumped by every tab switch and by leave()

const CAMPAIGN_STATUS = { done: ["پایان یافته", "ok"], running: ["در حال ارسال", "accent"], cancelled: ["متوقف شده", "warn"], interrupted: ["نیمه‌کاره", "warn"], failed: ["متوقف با خطا", "bad"] };

/** Persian SMS: 70 characters in one part, 67 per part after that. */
const smsParts = (n) => (n <= 70 ? 1 : Math.ceil(n / 67));

export async function render(root, params) {
  const tabs = [
    ["new", "کمپین جدید"],
    ["history", "سابقه کمپین‌ها"],
  ];
  let tab = params.tab === "history" ? "history" : "new";
  const body = h("div", { class: "stack" });
  const tabBar = h("div", { class: "tabs" });
  const draw = async () => {
    const my = ++seq;
    off?.();
    off = null;
    clear(tabBar, tabs.map(([k, l]) => h("button", { class: k === tab ? "on" : "", type: "button", onclick: () => { tab = k; draw(); } }, l)));
    const box = h("div", { class: "stack" });
    const unsub = tab === "new" ? await campaignForm(box) : await historyTab(box);
    if (my !== seq) return unsub?.(); // the user switched tabs meanwhile
    off = unsub || null;
    clear(body, box);
  };
  root.append(
    h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "camp-intro" },
        h("div", { class: "grow" }, h("b", {}, "کمپین یعنی یک پیام برای گروهی از مخاطبان شما."), h("span", { class: "small muted" }, " مخاطبان را از بانک انتخاب کنید، متن را بنویسید و روش ارسال را انتخاب کنید؛ نام هر کسب‌وکار و شهرش خودکار در متن قرار می‌گیرد.")),
        tabBar,
      ),
      body,
    ),
  );
  await draw();
}

export function leave() {
  seq++;
  off?.();
  off = null;
}

// ---------------- new campaign ----------------

async function campaignForm(root) {
  const [accounts, templates] = await Promise.all([invoke("accounts"), invoke("templates")]);
  const panels = accounts.filter((a) => ACCOUNT_KINDS[a.kind]);
  let method = panels.length ? "sms" : "messenger";
  let messenger = "whatsapp";

  // 1. audience
  let handoff = app.handoff;
  app.handoff = null;
  const bank = select(bankOptions(true), app.lists[0]?.id || 0);
  const status = select([["", "همه وضعیت‌ها"], ...Object.entries(STATUS).map(([k, v]) => [k, v.label])], "new");
  const audienceBox = h("div", { class: "stack tight" });
  let recipients = 0;
  const filter = () => (handoff ? { ...handoff.filter, channel: "mobile" } : { listId: Number(bank.value), status: status.value, channel: "mobile" });
  const recount = async () => {
    const r = await invoke("places", { filter: filter(), offset: 0, limit: 1 }).catch(() => null);
    recipients = r ? r.total : 0;
    summary();
    preview();
  };
  const drawAudience = () =>
    clear(
      audienceBox,
      handoff
        ? h("div", { class: "row" }, h("span", { class: "chip" }, icon("filter", "sm"), handoff.label, h("button", { type: "button", "aria-label": "حذف", onclick: () => { handoff = null; drawAudience(); recount(); } }, icon("x", "sm"))))
        : h("div", { class: "grid-2" }, field("بانک", bank), field("وضعیت پیگیری", status)),
      h("span", { class: "hint" }, "فقط کسب‌وکارهای دارای موبایل شمرده می‌شوند. لیست «عدم ارسال» همیشه کنار گذاشته می‌شود."),
    );
  bank.addEventListener("change", recount);
  status.addEventListener("change", recount);
  drawAudience();

  // 2. how
  const account = panels.length ? select(panels.map((a) => [a.id, `${a.name} — ${fa(a.perMinute)} پیامک در دقیقه`]), panels[0].id) : null;
  const acc = () => panels.find((a) => a.id === account?.value);
  account?.addEventListener("change", () => summary());
  const optout = toggle("افزودن «لغو۱۱» به انتهای پیامک", true, { onchange: () => preview() });
  const recent = select([[0, "ارسال دوباره مجاز است"], [3, "رد کسانی که ۳ روز اخیر پیام گرفته‌اند"], [7, "رد کسانی که ۷ روز اخیر پیام گرفته‌اند"], [30, "رد کسانی که ۳۰ روز اخیر پیام گرفته‌اند"]], 7);
  const every = toggle("ارسال به همه موبایل‌های هر کسب‌وکار (نه فقط اولی)", false);
  const testTo = input({ dir: "ltr", placeholder: "0912…" });
  const testBtn = h("button", { class: "btn sm", type: "button" }, icon("send", "sm"), "ارسال آزمایشی");
  const methodBox = h("div", { class: "stack" });
  const methodCards = h("div", { class: "grid-2 methods" });
  const drawMethod = () => {
    const card = (key, ic, title, desc) =>
      h("button", { type: "button", class: `method ${method === key ? "on" : ""}`, onclick: () => { method = key; drawMethod(); summary(); preview(); } }, h("span", { class: "ic-box" }, icon(ic)), h("span", { class: "grow" }, h("b", {}, title), h("span", {}, desc)), h("span", { class: "tick" }, method === key ? icon("check", "sm") : null));
    clear(
      methodCards,
      card("sms", "mobile", "پیامک خودکار", "از پنل پیامک شما، بدون دخالت؛ مناسب تعداد زیاد"),
      card("messenger", "chat", "پیام‌رسان با حساب خودتان", "واتساپ، تلگرام، ایتا یا بله؛ هر پیام را خودتان ارسال می‌کنید"),
    );
    if (method === "sms") {
      clear(
        methodBox,
        account
          ? [
              field("پنل پیامک", account),
              h("div", { class: "grid-2" }, field("پیام تکراری", recent), h("div", { class: "stack tight opts" }, optout.el, every.el)),
              h("div", { class: "row test-row" }, field("آزمایش روی یک شماره", testTo), testBtn),
            ]
          : h("div", { class: "callout" }, icon("alert", "sm"), h("div", { class: "small grow" }, "هنوز پنل پیامکی وصل نکرده‌اید. پنل خود (کاوه‌نگار، ملی‌پیامک، فراز اس‌ام‌اس و…) را یک بار اضافه کنید."), h("button", { class: "btn sm primary", type: "button", onclick: () => go("settings", { tab: "accounts" }) }, icon("plus", "sm"), "افزودن پنل")),
      );
    } else {
      clear(
        methodBox,
        h("div", { class: "app-pick" }, MESSENGERS.map((m) => h("button", { type: "button", class: `k-${m.key} ${m.key === messenger ? "on" : ""}`, onclick: () => { messenger = m.key; drawMethod(); summary(); } }, icon(m.icon, "lg"), m.label))),
        h("div", { class: "small muted" }, MESSENGERS.find((m) => m.key === messenger).note),
        h("div", { class: "small faint" }, "با «ساخت صف» به دستیار گفتگو می‌روید؛ گفتگوی هر نفر با متن آماده باز می‌شود و «ارسال» را خودتان می‌زنید. حساب شما در معرض مسدود شدن قرار نمی‌گیرد."),
      );
    }
    go_.replaceChildren(...(method === "sms" ? [icon("play", "sm"), "شروع ارسال پیامک"] : [icon("chat", "sm"), "ساخت صف در دستیار گفتگو"]));
    go_.disabled = method === "sms" && !account;
  };

  // 3. message
  const msg = h("textarea", { class: "input", rows: 4, placeholder: "سلام {person}، از تیم … با شما تماس می‌گیریم درباره {name} در {city}…" });
  const tplSel = select([["", "بدون قالب"], ...templates.map((t) => [t.id, t.name])], "");
  tplSel.addEventListener("change", () => {
    const t = templates.find((x) => String(x.id) === tplSel.value);
    if (t) {
      msg.value = t.body;
      preview();
    }
  });
  const insert = (ph) => {
    msg.setRangeText(ph, msg.selectionStart, msg.selectionEnd, "end");
    msg.focus();
    preview();
  };
  const counter = h("span", { class: "small muted" });
  const pv = h("div", { class: "preview" });
  let pvTimer = 0;
  let parts = 1;
  function preview() {
    clearTimeout(pvTimer);
    pvTimer = setTimeout(async () => {
      const r = await invoke("places", { filter: filter(), offset: 0, limit: 1 }).catch(() => null);
      const sample = r?.rows[0];
      const text = msg.value.trim();
      const footer = method === "sms" && optout.input.checked ? "\nلغو۱۱" : "";
      const rendered = text && sample ? await invoke("template_preview", { placeId: sample.id, text }).catch(() => text) : text;
      clear(pv, rendered ? `${rendered}${footer}` : h("span", { class: "faint" }, "پیش‌نمایش پیام برای اولین گیرنده اینجا نمایش داده می‌شود."), sample ? h("span", { class: "meta" }, `نمونه برای: ${sample.name}`) : null);
      const n = `${rendered}${footer}`.length;
      parts = smsParts(n);
      counter.textContent = method === "sms" ? `${fa(n)} حرف · ${fa(parts)} پیامک برای هر گیرنده` : `${fa(n)} حرف`;
      summary();
    }, 250);
  }
  msg.addEventListener("input", preview);
  const saveTpl = h("button", { class: "btn ghost sm", type: "button" }, icon("plus", "sm"), "ذخیره قالب");
  saveTpl.addEventListener("click", () => {
    if (!msg.value.trim()) return toast("ابتدا متن پیام را بنویسید.", "bad");
    const name = input({ placeholder: "مثلاً معرفی خدمات" });
    modal({
      title: "ذخیره قالب",
      body: field("نام قالب", name),
      actions: [{ label: "انصراف" }, { label: "ذخیره", kind: "primary", onClick: async () => (await safe(invoke("template_save", { template: { id: 0, channel: method === "sms" ? "sms" : "assist", name: name.value, subject: "", body: msg.value } }), "قالب ذخیره شد")) !== undefined || false }],
    });
    name.focus();
  });

  const req = (extra = {}) => ({ filter: filter(), accountId: account?.value || "", subject: "", message: msg.value, optoutFooter: optout.input.checked, skipRecentDays: Number(recent.value), everyNumber: every.input.checked, ...extra });
  testBtn.addEventListener("click", async () => {
    if (!testTo.value.trim()) return toast("شماره آزمایشی را وارد کنید.", "bad");
    if (!msg.value.trim()) return toast("متن پیام را بنویسید.", "bad");
    testBtn.disabled = true;
    await safe(invoke("campaign_run", { req: req({ testTo: testTo.value.trim() }), onEvent: newChannel() }), (r) => r.message);
    testBtn.disabled = false;
  });
  const go_ = h("button", { class: "btn accent", type: "button" });
  go_.addEventListener("click", async () => {
    if (!msg.value.trim() && method === "sms") return toast("متن پیام را بنویسید.", "bad");
    if (!recipients) return toast("در این انتخاب مخاطب دارای موبایل نیست.", "bad");
    if (method === "messenger") {
      app.handoff = { filter: filter(), label: handoff?.label || `${fa(recipients)} مخاطب کمپین`, messenger, text: msg.value, autostart: true };
      return go("assist");
    }
    const a = acc();
    if (!(await confirmBox(`ارسال پیامک به ${fa(recipients)} کسب‌وکار از «${a.name}» شروع شود؟ حدود ${fa(recipients * parts)} پیامک، با سرعت ${fa(a.perMinute)} در دقیقه.`, { ok: "شروع ارسال", title: "شروع کمپین" }))) return;
    runJob("campaign", `کمپین پیامکی «${a.name}»`, "campaign_run", { req: req() });
  });

  // summary / live progress
  const side = h("div", { class: "card stack camp-side" });
  let running = null;
  function summary() {
    if (running) return;
    const a = acc();
    const minutes = method === "sms" && a ? Math.ceil(recipients / Math.max(1, a.perMinute)) : 0;
    const label = method === "sms" ? "پیامک خودکار" : `${MESSENGERS.find((m) => m.key === messenger).label} (دستیار)`;
    clear(
      side,
      h("div", { class: "card-h" }, h("h2", {}, "خلاصه کمپین")),
      h(
        "div",
        { class: "grid-2" },
        h("div", { class: "stat" }, h("b", {}, fa(recipients)), h("span", {}, "گیرنده دارای موبایل")),
        h("div", { class: "stat" }, h("b", {}, method === "sms" ? fa(recipients * parts) : "—"), h("span", {}, method === "sms" ? "پیامک تقریبی" : "ارسال دستی")),
      ),
      h("dl", { class: "kv compact" }, [
        [h("dt", {}, "روش"), h("dd", {}, label)],
        method === "sms" ? [h("dt", {}, "زمان تقریبی"), h("dd", {}, minutes ? `${fa(minutes)} دقیقه` : "—")] : null,
        method === "sms" ? [h("dt", {}, "هر گیرنده"), h("dd", {}, `${fa(parts)} پیامک`)] : null,
      ]),
      h(
        "ol",
        { class: "how" },
        method === "sms"
          ? [
              h("li", {}, h("b", {}, "ارسال از پنل شما"), h("span", {}, "پیامک‌ها با سرعت تعیین‌شده پنل یکی‌یکی می‌روند")),
              h("li", {}, h("b", {}, "رعایت لغو و عدم ارسال"), h("span", {}, "شماره‌های لیست «عدم ارسال» و پیام‌گرفته‌های اخیر رد می‌شوند")),
              h("li", {}, h("b", {}, "توقف ایمن"), h("span", {}, "اگر ۵ ارسال پشت‌سرهم خطای پنل بدهد، کمپین خودکار متوقف می‌شود")),
            ]
          : [
              h("li", {}, h("b", {}, "صف در دستیار گفتگو"), h("span", {}, "همین مخاطبان و متن در صف قرار می‌گیرند")),
              h("li", {}, h("b", {}, "باز شدن گفتگو"), h("span", {}, "با حساب خودتان، داخل برنامه")),
              h("li", {}, h("b", {}, "ثبت نتیجه"), h("span", {}, "ارسال‌ها و شماره‌های بدون حساب ثبت می‌شوند")),
            ],
      ),
    );
  }

  const form = h(
    "div",
    { class: "stack" },
    h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("span", { class: "step-n" }, "۱"), h("h2", {}, "مخاطبان")), audienceBox),
    h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("span", { class: "step-n" }, "۲"), h("h2", {}, "روش ارسال")), methodCards, methodBox),
    h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("span", { class: "step-n" }, "۳"), h("h2", {}, "متن پیام"), tplSel, saveTpl), msg, h("div", { class: "row between" }, h("div", { class: "chips ph-chips" }, PLACEHOLDERS.map(([ph, label]) => h("span", { class: "chip", title: ph, onclick: () => insert(ph) }, `+ ${label}`))), counter), pv, h("div", { class: "row end" }, go_)),
  );
  drawMethod();
  root.append(h("div", { class: "side-grid" }, form, side));
  recount();

  return onJob((j) => {
    go_.disabled = j.running || (method === "sms" && !account);
    if (j.kind !== "campaign" || (!j.running && !j.result)) {
      running = null;
      summary();
      return;
    }
    running = j;
    const s = j.sending || { sent: 0, failed: 0, skipped: 0, total: 0 };
    const done = s.sent + s.failed + s.skipped;
    clear(
      side,
      h("div", { class: "card-h" }, h("h2", {}, j.title), j.running ? h("span", { class: "pill accent" }, h("span", { class: "dot live" }), "در حال ارسال") : h("span", { class: `pill ${j.result?.error ? "bad" : "ok"}` }, j.result?.error ? "متوقف شد" : "پایان")),
      h("div", { class: "grid-2" }, h("div", { class: "stat" }, h("b", {}, fa(s.sent)), h("span", {}, "ارسال موفق")), h("div", { class: "stat" }, h("b", {}, fa(s.failed)), h("span", {}, "ناموفق")), h("div", { class: "stat" }, h("b", {}, fa(s.skipped)), h("span", {}, "ردشده")), h("div", { class: "stat" }, h("b", {}, fa(s.total)), h("span", {}, "کل"))),
      bar(s.total ? done / s.total : 0, "ok"),
      h("div", { class: "small muted" }, j.running ? j.status : j.result?.message),
      h("div", { class: "log" }, j.logs.slice(-120).map((l) => h("div", { class: l.level }, l.message))),
      !j.running ? h("button", { class: "btn sm", type: "button", onclick: () => { running = null; summary(); } }, "کمپین تازه") : null,
    );
  });
}

// ---------------- history ----------------

export async function historyTab(root) {
  const list = await invoke("campaigns");
  if (!list.length) {
    root.append(h("div", { class: "card" }, h("div", { class: "empty" }, icon("history", "lg"), "هنوز کمپینی اجرا نشده است.")));
    return;
  }
  root.append(
    h(
      "div",
      { class: "card flush" },
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          { class: "t" },
          h("thead", {}, h("tr", {}, ["تاریخ", "حساب", "بانک", "پیام", "موفق", "ناموفق", "ردشده", "وضعیت"].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            list.map((c) => {
              const [label, tone] = CAMPAIGN_STATUS[c.status] || [c.status, ""];
              return h(
                "tr",
                {},
                h("td", { class: "small" }, jdate(c.createdAt, true)),
                h("td", {}, h("div", { class: "name-cell" }, h("span", {}, c.accountName), h("span", { class: "c" }, ACCOUNT_KINDS[c.channel]?.label || "حساب قدیمی"))),
                h("td", { class: "clip" }, c.listName || "همه بانک‌ها"),
                h("td", { class: "clip small", title: c.message }, c.subject || c.message),
                h("td", {}, fa(c.sent)),
                h("td", {}, fa(c.failed)),
                h("td", {}, fa(c.skipped)),
                h("td", { title: c.error }, h("span", { class: `pill ${tone}` }, label)),
              );
            }),
          ),
        ),
      ),
    ),
  );
}


