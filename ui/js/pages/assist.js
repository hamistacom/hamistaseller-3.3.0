// Chat assistant: you log in once to your own WhatsApp / Telegram / Eitaa / Bale (their web clients,
// inside the app); the app opens each extracted number's chat in turn with the text ready. You press Send.
import { h, icon, invoke, fa, clear, safe, select, field, toast, copy, confirmBox, toggle, appsOf, appBadges, STATUS } from "../core.js";
import { app, bankOptions } from "../state.js";
import { MESSENGERS, PLACEHOLDERS } from "../data.js";
import { openCard } from "./card.js";

// survives page switches. pick = the picker's live choice; messenger = the one the queue was built for
const q = { pick: "whatsapp", messenger: "whatsapp", items: [], i: 0, text: "", done: 0, none: 0, label: "" };

function target(p, messenger, skipNone) {
  if (messenger === "telegram") {
    const tg = p.contacts.find((c) => c.kind === "telegram");
    if (tg) return { to: tg.value, contact: tg };
  }
  // the first mobile not already known to lack this messenger
  const m = p.contacts.find((c) => c.kind === "mobile" && !(skipNone && appsOf(c)[messenger] === false));
  return m ? { to: m.value, contact: m } : null;
}

export async function render(root) {
  let handoff = app.handoff;
  app.handoff = null;
  if (handoff?.messenger) q.pick = handoff.messenger;
  const label = () => MESSENGERS.find((m) => m.key === q.pick).label;

  // ---- messenger + your account ----
  const picker = h("div", { class: "app-pick" });
  const note = h("div", { class: "small muted" });
  const account = h("div", { class: "row wrap acct" });
  const drawPicker = () => {
    clear(picker, MESSENGERS.map((m) => h("button", { class: `k-${m.key} ${m.key === q.pick ? "on" : ""}`, type: "button", onclick: () => { q.pick = m.key; drawPicker(); } }, icon(m.icon, "lg"), m.label)));
    note.textContent = MESSENGERS.find((m) => m.key === q.pick).note;
    clear(
      account,
      h("span", { class: "small grow" }, `حساب ${label()} شما: یک بار وارد شوید؛ ورود روی همین کامپیوتر می‌ماند.`),
      h("button", { class: "btn sm", type: "button", onclick: () => safe(invoke("assist_session", { messenger: q.pick, action: "open" })) }, icon("user", "sm"), "ورود / تغییر حساب"),
      h(
        "button",
        {
          class: "btn sm ghost danger",
          type: "button",
          onclick: async () => {
            if (!(await confirmBox(`از حساب ${label()} خارج شوید و ورود ذخیره‌شده پاک شود؟ دفعه بعد دوباره باید وارد شوید.`, { ok: "خروج", danger: true }))) return;
            await safe(invoke("assist_session", { messenger: q.pick, action: "reset" }), `از ${label()} خارج شدید`);
          },
        },
        icon("logout", "sm"),
        "خروج کامل",
      ),
    );
  };
  drawPicker();

  // ---- who ----
  const bank = select(bankOptions(true), app.lists[0]?.id || 0);
  const status = select([["", "همه وضعیت‌ها"], ...Object.entries(STATUS).map(([k, v]) => [k, v.label])], "new");
  const skipNone = toggle("رد شماره‌هایی که قبلاً در این پیام‌رسان «حساب ندارد» خورده‌اند", true);
  const audience = h("div", {});
  const drawAudience = () =>
    clear(
      audience,
      handoff
        ? h("span", { class: "chip" }, icon("filter", "sm"), handoff.label, h("button", { type: "button", "aria-label": "حذف", onclick: () => { handoff = null; drawAudience(); } }, icon("x", "sm")))
        : h("div", { class: "grid-2" }, field("بانک", bank), field("وضعیت", status)),
    );
  drawAudience();
  const msg = h("textarea", { class: "input", rows: 4, placeholder: "سلام {person}، وقت بخیر…" }, handoff?.text ?? q.text);
  const chips = h("div", { class: "chips ph-chips" }, PLACEHOLDERS.map(([ph, lbl]) => h("span", { class: "chip", onclick: () => { msg.setRangeText(ph, msg.selectionStart, msg.selectionEnd, "end"); msg.focus(); } }, `+ ${lbl}`)));

  const build = h("button", { class: "btn primary", type: "button" }, icon("play", "sm"), "ساخت صف گفتگو");
  const buildQueue = async () => {
    build.disabled = true;
    const messenger = q.pick;
    const channel = messenger === "telegram" ? "" : "mobile";
    const filter = handoff ? { ...handoff.filter, channel } : { listId: Number(bank.value), status: status.value, channel };
    const rows = [];
    const optouts = await safe(invoke("optouts"));
    for (let off = 0; off < 5000; off += 500) {
      const r = await safe(invoke("places", { filter, offset: off, limit: 500 }));
      if (!r) break;
      rows.push(...r.rows);
      if (rows.length >= r.total) break;
    }
    build.disabled = false;
    if (!optouts) return;
    const blocked = new Set(optouts);
    Object.assign(q, { messenger, i: 0, done: 0, none: 0, text: msg.value, label: label() });
    q.items = rows.map((p) => ({ p, t: target(p, messenger, skipNone.input.checked) })).filter((x) => x.t && !blocked.has(x.t.to));
    if (!q.items.length) return toast("در این انتخاب شماره‌ای برای این پیام‌رسان نیست.", "bad");
    toast(`${fa(q.items.length)} نفر در صف ${q.label}`, "ok");
    drawCurrent();
  };
  build.addEventListener("click", buildQueue);

  // ---- the current person ----
  const current = h("div", { class: "card assist-card" });
  async function drawCurrent() {
    if (!q.items.length) {
      clear(
        current,
        h("div", { class: "card-h" }, h("h2", {}, "صف گفتگو")),
        h(
          "ol",
          { class: "how" },
          h("li", {}, h("b", {}, "وارد حساب خود شوید"), h("span", {}, "«ورود / تغییر حساب» پنجره پیام‌رسان را باز می‌کند")),
          h("li", {}, h("b", {}, "مخاطبان و متن را انتخاب کنید"), h("span", {}, "و «ساخت صف گفتگو» را بزنید")),
          h("li", {}, h("b", {}, "یکی‌یکی ارسال کنید"), h("span", {}, "گفتگوی هر نفر باز و متن آماده می‌شود؛ «ارسال» را خودتان می‌زنید")),
          h("li", {}, h("b", {}, "نتیجه ثبت می‌شود"), h("span", {}, "حساب داشتن یا نداشتن هر شماره در این پیام‌رسان کنار شماره نمایش داده می‌شود")),
        ),
      );
      return;
    }
    if (q.i >= q.items.length) {
      clear(current, h("div", { class: "empty" }, icon("check", "lg"), h("b", {}, "صف تمام شد"), h("span", { class: "small" }, `${fa(q.done)} ارسال ثبت شد · ${fa(q.none)} شماره بدون حساب ${q.label}`), h("button", { class: "btn", type: "button", onclick: () => { q.items = []; drawCurrent(); } }, "صف جدید")));
      return;
    }
    const { p, t } = q.items[q.i];
    const text = q.text.trim() ? await invoke("template_preview", { placeId: p.id, text: q.text }).catch(() => q.text) : "";
    const open = h("button", { class: "btn accent block", type: "button" }, icon("external", "sm"), `باز کردن گفتگو در ${q.label}`);
    open.addEventListener("click", async () => {
      open.disabled = true;
      // copy while this window still has focus (the messenger window takes it next)
      const copyText = q.messenger === "whatsapp" ? "" : ["eitaa", "bale"].includes(q.messenger) ? t.to : text;
      const copied = copyText ? await navigator.clipboard.writeText(copyText).then(() => true, () => false) : false;
      const prefilled = await safe(invoke("assist_open", { messenger: q.messenger, to: t.to, text }));
      open.disabled = false;
      if (prefilled === undefined || prefilled) return;
      if (["eitaa", "bale"].includes(q.messenger)) toast(copied ? `شماره کپی شد؛ در جستجوی ${q.label} بچسبانید. بعد متن را از دکمه «کپی متن» بردارید.` : `شماره ${t.to} را در ${q.label} جستجو کنید.`);
      else if (text) toast(copied ? "متن کپی شد؛ در گفتگو بچسبانید (Ctrl+V)." : "کپی خودکار متن ممکن نشد؛ متن را از پیش‌نمایش کپی کنید.", copied ? "" : "bad");
    });
    const step = (kind) => async () => {
      if (kind === "sent") {
        await safe(invoke("assist_done", { placeId: p.id, contactId: t.contact.id, messenger: q.messenger, recipient: t.to }));
        q.done++;
      } else if (kind === "none") {
        await safe(invoke("contact_app", { contactId: t.contact.id, messenger: q.messenger, has: false }));
        q.none++;
      }
      q.i++;
      drawCurrent();
    };
    clear(
      current,
      h("div", { class: "row between" }, h("span", { class: "pill info" }, `${fa(q.i + 1)} از ${fa(q.items.length)}`), h("span", { class: "small muted" }, `${fa(q.done)} ارسال · ${fa(q.none)} بدون حساب`)),
      h(
        "div",
        { class: "assist-who" },
        h("div", { class: "avatar" }, (p.name.trim()[0] || "؟").toUpperCase()),
        h(
          "div",
          { class: "grow stack tight" },
          h("button", { class: "link cc-name", type: "button", onclick: () => openCard(p.id) }, p.name),
          p.contactPerson ? h("span", { class: "cc-person" }, icon("user", "sm"), p.contactPerson, p.contactRole ? ` — ${p.contactRole}` : "") : null,
          h("span", { class: "row small" }, h("span", { class: "ltr" }, t.to), appBadges(t.contact), h("button", { class: "btn ghost sm icon-only", type: "button", title: "کپی شماره", onclick: () => copy(t.to) }, icon("copy", "sm"))),
        ),
      ),
      text ? h("div", { class: "preview" }, text, h("button", { class: "btn ghost sm", type: "button", onclick: () => copy(text, "متن کپی شد") }, icon("copy", "sm"), "کپی متن")) : h("div", { class: "small faint" }, "بدون متن آماده — فقط گفتگو باز می‌شود."),
      open,
      h(
        "div",
        { class: "grid-3" },
        h("button", { class: "btn primary", type: "button", onclick: step("sent") }, icon("check", "sm"), "ارسال کردم"),
        h("button", { class: "btn", type: "button", title: `این شماره در ${q.label} حساب ندارد`, onclick: step("none") }, icon("x", "sm"), "حساب ندارد"),
        h("button", { class: "btn ghost", type: "button", onclick: step("skip") }, icon("skip", "sm"), "رد کردن"),
      ),
    );
  }

  root.append(
    h(
      "div",
      { class: "side-grid" },
      h(
        "div",
        { class: "stack" },
        h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "پیام‌رسان و حساب شما")), picker, note, account),
        h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "مخاطبان")), audience, skipNone.el),
        h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "متن آماده")), msg, chips, h("div", { class: "row end" }, build)),
      ),
      current,
    ),
  );
  drawCurrent();
  if (handoff?.autostart) buildQueue();
}

export function leave() {}
