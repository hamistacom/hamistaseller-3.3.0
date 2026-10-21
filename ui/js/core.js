// Shared UI helpers: IPC, safe DOM building, formatting, toasts, modals, icons.
// Scraped data (names, addresses…) is only ever inserted as text — never as HTML.

import { EN, tr } from "./i18n.js";

const T = window.__TAURI__;
export const invoke = (cmd, args) => T.core.invoke(cmd, args);
export const newChannel = () => new T.core.Channel();
/** App-wide backend events (scheduled jobs). Returns a promise of the unlisten function. */
export const listen = (name, fn) => (T.event ? T.event.listen(name, (e) => fn(e.payload)) : Promise.resolve(() => {}));

// ---------------- DOM ----------------

const PROPS = new Set(["value", "checked", "disabled", "selected", "hidden", "readOnly", "multiple", "indeterminate"]);

/** h("div", {class: "x", onclick}, child, "text", [more]) */
export function h(tag, props, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props || {})) {
    if (v == null || v === false) continue;
    if (k === "class") el.className = v;
    else if (k === "html") el.innerHTML = v; // static markup only (icons)
    else if (k.startsWith("on") && typeof v === "function") el.addEventListener(k.slice(2), v);
    else if (PROPS.has(k)) el[k] = v;
    else el.setAttribute(k, v === true ? "" : v);
  }
  append(el, kids);
  return el;
}

function append(el, kids) {
  for (const c of kids.flat(Infinity)) {
    if (c == null || c === false) continue;
    el.append(c instanceof Node ? c : String(c));
  }
}

export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
export function clear(el, ...kids) {
  el.replaceChildren();
  append(el, kids);
  return el;
}

// ---------------- icons (stroke, 24×24) ----------------

const P = {
  trend: '<path d="M3 17l6-6 4 4 8-8"/><path d="M14 7h7v7"/>',
  bag: '<path d="M6 7h12l1 13H5L6 7Z"/><path d="M9 10V6a3 3 0 0 1 6 0v4"/>',
  award: '<circle cx="12" cy="9" r="6"/><path d="m8.5 14-1.5 7 4.5-2.5 4.5 2.5-1.5-7"/>',
  store: '<path d="M4 10v10h16V10"/><path d="M3 10l2-6h14l2 6"/><path d="M9 20v-6h6v6"/>',
  chart: '<path d="M4 20V10M10 20V4M16 20v-8M2 20h20"/>',
  target: '<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="5"/><circle cx="12" cy="12" r="1"/>',
  dashboard: '<rect x="3" y="3" width="7" height="9" rx="2"/><rect x="14" y="3" width="7" height="5" rx="2"/><rect x="14" y="12" width="7" height="9" rx="2"/><rect x="3" y="16" width="7" height="5" rx="2"/>',
  scan: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/><path d="M11 8v6M8 11h6"/>',
  bank: '<ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5"/><path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"/>',
  send: '<path d="m22 2-7 20-4-9-9-4Z"/><path d="M22 2 11 13"/>',
  chat: '<path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.4A8 8 0 1 1 21 12Z"/><path d="M8 11h8M8 14h5"/>',
  history: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l3 2"/>',
  settings: '<path d="M12.2 2h-.4a2 2 0 0 0-2 2v.2a2 2 0 0 1-1 1.7l-.4.3a2 2 0 0 1-2 0l-.2-.1a2 2 0 0 0-2.7.7l-.2.4a2 2 0 0 0 .7 2.7l.2.1a2 2 0 0 1 1 1.7v.6a2 2 0 0 1-1 1.7l-.2.1a2 2 0 0 0-.7 2.7l.2.4a2 2 0 0 0 2.7.7l.2-.1a2 2 0 0 1 2 0l.4.3a2 2 0 0 1 1 1.7v.2a2 2 0 0 0 2 2h.4a2 2 0 0 0 2-2v-.2a2 2 0 0 1 1-1.7l.4-.3a2 2 0 0 1 2 0l.2.1a2 2 0 0 0 2.7-.7l.2-.4a2 2 0 0 0-.7-2.7l-.2-.1a2 2 0 0 1-1-1.7v-.6a2 2 0 0 1 1-1.7l.2-.1a2 2 0 0 0 .7-2.7l-.2-.4a2 2 0 0 0-2.7-.7l-.2.1a2 2 0 0 1-2 0l-.4-.3a2 2 0 0 1-1-1.7V4a2 2 0 0 0-2-2Z"/><circle cx="12" cy="12" r="3"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  play: '<path d="M7 4v16l13-8Z"/>',
  stop: '<rect x="6" y="6" width="12" height="12" rx="2"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  x: '<path d="M18 6 6 18M6 6l12 12"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  copy: '<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1"/>',
  edit: '<path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/>',
  trash: '<path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6"/>',
  more: '<circle cx="12" cy="5" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="12" cy="19" r="1"/>',
  phone: '<path d="M22 16.9v3a2 2 0 0 1-2.2 2 19.8 19.8 0 0 1-8.6-3.1 19.5 19.5 0 0 1-6-6A19.8 19.8 0 0 1 2.1 4.2 2 2 0 0 1 4.1 2h3a2 2 0 0 1 2 1.7c.1.9.4 1.8.7 2.7a2 2 0 0 1-.5 2.1L8 9.8a16 16 0 0 0 6 6l1.3-1.3a2 2 0 0 1 2.1-.4c.9.3 1.8.6 2.7.7a2 2 0 0 1 1.7 2Z"/>',
  mobile: '<rect x="6" y="2" width="12" height="20" rx="3"/><path d="M11 18h2"/>',
  mail: '<rect x="2" y="4" width="20" height="16" rx="3"/><path d="m22 7-10 6L2 7"/>',
  globe: '<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15 15 0 0 1 0 20 15 15 0 0 1 0-20Z"/>',
  instagram: '<rect x="2" y="2" width="20" height="20" rx="6"/><circle cx="12" cy="12" r="4"/><circle cx="17.5" cy="6.5" r=".6"/>',
  telegram: '<path d="m22 3-20 8 7 2.5M22 3l-4 18-9-7.5M22 3 9 13.5V20l3.5-3.5"/>',
  whatsapp: '<path d="M3 21l1.7-5A9 9 0 1 1 8 19.4Z"/><path d="M9 9.5c.3 1.8 1.8 3.8 4 4.8l1.2-1.2 2 .8c-.3 1.2-1.3 1.9-2.4 1.8-3.3-.5-6-3.2-6.4-6.4 0-1.1.6-2.1 1.8-2.4l.8 2Z"/>',
  eitaa: '<path d="M4 6a3 3 0 0 1 3-3h10a3 3 0 0 1 3 3v8a3 3 0 0 1-3 3H9l-5 4Z"/><path d="M9 10h6"/>',
  bale: '<path d="M12 21c4.4 0 8-3.1 8-7.5S16.4 3 12 3 4 6.5 4 11c0 2 .7 3.8 2 5.2L5 21l4.2-1.5c.9.3 1.8.5 2.8.5Z"/>',
  rubika: '<path d="m12 2 8.5 5v10L12 22l-8.5-5V7Z"/><path d="m12 8 3.5 2v4L12 16l-3.5-2v-4Z"/>',
  map: '<path d="M12 22s7-6.1 7-12a7 7 0 0 0-14 0c0 5.9 7 12 7 12Z"/><circle cx="12" cy="10" r="2.5"/>',
  user: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
  tag: '<path d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3 13V3h10l7.6 7.6a2 2 0 0 1 0 2.8Z"/><circle cx="7.5" cy="7.5" r="1.2"/>',
  download: '<path d="M12 3v12m0 0-4-4m4 4 4-4"/><path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"/>',
  upload: '<path d="M12 21V9m0 0-4 4m4-4 4 4"/><path d="M4 7V5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v2"/>',
  moon: '<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8Z"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  key: '<circle cx="7.5" cy="15.5" r="4.5"/><path d="m10.7 12.3 9.3-9.3M17 6l3 3M14 9l2 2"/>',
  shield: '<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z"/><path d="m9 12 2 2 4-4"/>',
  alert: '<path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z"/><path d="M12 9v4M12 17h.01"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/>',
  refresh: '<path d="M21 12a9 9 0 0 1-15.7 6M3 12a9 9 0 0 1 15.7-6"/><path d="M21 3v5h-5M3 21v-5h5"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/>',
  move: '<path d="M5 12h14M13 6l6 6-6 6"/>',
  external: '<path d="M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  sparkle: '<path d="M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9Z"/>',
  filter: '<path d="M3 5h18l-7 8v6l-4 2v-8Z"/>',
  clock: '<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>',
  skip: '<path d="M5 4v16l10-8Z"/><path d="M19 5v14"/>',
  database: '<ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5"/>',
  megaphone: '<path d="M3 11v2a1 1 0 0 0 1 1h3l6 5V5L7 10H4a1 1 0 0 0-1 1Z"/><path d="M17 8a5 5 0 0 1 0 8"/>',
  users: '<circle cx="9" cy="8" r="4"/><path d="M2 21a7 7 0 0 1 14 0"/><path d="M16 4a4 4 0 0 1 0 8M22 21a7 7 0 0 0-4-6.3"/>',
  bell: '<path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.9 1.9 0 0 0 3.4 0"/>',
  server: '<rect x="3" y="3" width="18" height="7" rx="2"/><rect x="3" y="14" width="18" height="7" rx="2"/><path d="M7 6.5h.01M7 17.5h.01"/>',
  layers: '<path d="m12 2 10 5-10 5L2 7Z"/><path d="m2 12 10 5 10-5M2 17l10 5 10-5"/>',
  book: '<path d="M4 4a2 2 0 0 1 2-2h14v18H6a2 2 0 0 0-2 2Z"/><path d="M4 20V4M9 7h7"/>',
  cpu: '<rect x="5" y="5" width="14" height="14" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>',
  puzzle: '<path d="M10 3h4v3a2 2 0 1 0 4 0V3h3v7h-3a2 2 0 1 0 0 4h3v7h-7v-3a2 2 0 1 0-4 0v3H3v-7h3a2 2 0 1 0 0-4H3V3h7Z"/>',
  camera: '<rect x="3" y="6" width="18" height="14" rx="3"/><circle cx="12" cy="13" r="4"/><path d="M9 6l1.5-2h3L15 6"/>',
  logout: '<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/>',
  plug: '<path d="M9 2v6M15 2v6M6 8h12v4a6 6 0 0 1-12 0Z"/><path d="M12 18v4"/>',
};

export function icon(name, cls = "") {
  return h("span", { class: `icon ${cls}`.trim(), "aria-hidden": "true", html: `<svg viewBox="0 0 24 24">${P[name] || P.info}</svg>` });
}

// ---------------- formatting ----------------

/** Locale for dates: Persian (solar) calendar in both languages. */
export const LOCALE = EN ? "en-US-u-ca-persian" : "fa-IR-u-ca-persian";
export const fa = (n) => Number(n || 0).toLocaleString(EN ? "en-US" : "fa-IR");

/** Server times are UTC ("2026-10-01 08:00:00"); mark them so jdate / ago show local time. */
export const utc = (s) => (s && !String(s).includes("T") && String(s).length > 10 ? `${String(s).replace(" ", "T")}Z` : s);

/** "2026-09-30 14:05:00" (local) → Persian (Jalali) date, optionally with time. */
export function jdate(s, withTime = false) {
  if (!s) return "—";
  const d = new Date(String(s).replace(" ", "T"));
  if (Number.isNaN(d.getTime())) return s;
  const opt = { year: "numeric", month: "long", day: "numeric", calendar: "persian" };
  if (withTime) Object.assign(opt, { hour: "2-digit", minute: "2-digit" });
  return d.toLocaleString(EN ? "en-US-u-ca-persian" : "fa-IR-u-ca-persian", opt);
}

export function ago(s) {
  if (!s) return "";
  const d = new Date(String(s).replace(" ", "T"));
  const m = Math.round((Date.now() - d.getTime()) / 60000);
  if (m < 1) return "همین حالا";
  if (m < 60) return `${fa(m)} دقیقه پیش`;
  if (m < 1440) return `${fa(Math.round(m / 60))} ساعت پیش`;
  if (m < 43200) return `${fa(Math.round(m / 1440))} روز پیش`;
  return jdate(s);
}

export const KIND = {
  mobile: { label: "موبایل", icon: "mobile" },
  landline: { label: "تلفن ثابت", icon: "phone" },
  email: { label: "ایمیل", icon: "mail" },
  instagram: { label: "اینستاگرام", icon: "instagram" },
  telegram: { label: "تلگرام", icon: "telegram" },
  whatsapp: { label: "واتساپ", icon: "whatsapp" },
  eitaa: { label: "ایتا", icon: "eitaa" },
  bale: { label: "بله", icon: "bale" },
};
export const KINDS = Object.keys(KIND);

/** Messengers seen for a mobile number in the chat assistant: { whatsapp: true, telegram: false, … }. */
export function appsOf(c) {
  return Object.fromEntries((c.apps || "").split(",").filter(Boolean).map((x) => x.split(":")).map(([k, v]) => [k, v === "1"]));
}

const APP_LABEL = { whatsapp: "واتساپ", telegram: "تلگرام", eitaa: "ایتا", bale: "بله" };

/** Small coloured marks after a mobile number: has an account (coloured) / has none (struck). */
export function appBadges(c) {
  const a = appsOf(c);
  const keys = Object.keys(APP_LABEL).filter((k) => k in a);
  if (!keys.length) return null;
  return h(
    "span",
    { class: "apps" },
    keys.map((k) => h("span", { class: `ab k-${k} ${a[k] ? "" : "no"}`.trim(), title: a[k] ? `در ${APP_LABEL[k]} حساب دارد` : `در ${APP_LABEL[k]} حساب ندارد` }, icon(k))),
  );
}

export const STATUS = {
  new: { label: "جدید", tone: "" },
  contacted: { label: "پیگیری‌شده", tone: "info" },
  interested: { label: "علاقه‌مند", tone: "warn" },
  customer: { label: "مشتری", tone: "ok" },
  rejected: { label: "عدم تمایل", tone: "bad" },
};

export const COLORS = ["navy", "orange", "green", "blue", "red", "purple", "teal", "gray"];

/** Short display form of a social link: https://instagram.com/cafe → cafe */
export function handle(url) {
  try {
    const u = new URL(url);
    return decodeURIComponent(u.pathname.replace(/^\/+|\/+$/g, "")) || u.hostname;
  } catch {
    return url;
  }
}

export const initial = (name) => (String(name || "؟").trim()[0] || "؟").toUpperCase();

// ---------------- feedback ----------------

export function toast(msg, tone = "") {
  const box = $("#toasts");
  const t = h("div", { class: `toast ${tone}`.trim(), role: "status" }, String(msg));
  box.append(t);
  setTimeout(() => t.remove(), tone === "bad" ? 6500 : 3800);
}

/** Runs an IPC call; shows the error as a toast and returns undefined on failure. */
export async function safe(promise, okMsg) {
  try {
    const r = await promise;
    if (okMsg) toast(typeof okMsg === "function" ? okMsg(r) : okMsg, "ok");
    return r;
  } catch (e) {
    toast(typeof e === "string" ? e : e?.message || "خطای ناشناخته", "bad");
    return undefined;
  }
}

export async function copy(text, label = "کپی شد") {
  try {
    await navigator.clipboard.writeText(text);
    toast(label);
  } catch {
    toast("کپی ممکن نشد.", "bad");
  }
}

/** Disables a button while `fn` runs. */
export async function busy(btn, fn) {
  btn.disabled = true;
  try {
    return await fn();
  } finally {
    btn.disabled = false;
  }
}

// ---------------- modal ----------------

/**
 * modal({title, body, actions:[{label, kind, onClick → false keeps it open}], wide})
 * Returns {el, close}. Esc and the ✕ close it.
 */
export function modal({ title, sub, body, actions = [], wide = false, onClose, left }) {
  const dlg = h("dialog", { class: `modal ${wide ? "wide" : ""}`.trim() });
  const close = () => {
    dlg.close();
    dlg.remove();
    onClose?.();
  };
  const foot = h("div", { class: "modal-f" }, left ? h("div", { class: "left" }, left) : null);
  for (const a of actions) {
    const b = h("button", { class: `btn ${a.kind || ""}`.trim(), type: "button" }, a.icon ? icon(a.icon, "sm") : null, a.label);
    b.addEventListener("click", async () => {
      if (!a.onClick) return close();
      const keep = await busy(b, () => a.onClick(b));
      if (keep !== false) close();
    });
    foot.append(b);
  }
  append(
    dlg,
    [h("div", { class: "modal-h" }, h("div", { class: "grow" }, h("h2", {}, title), sub ? h("p", { class: "muted small" }, sub) : null), h("button", { class: "btn ghost icon-only sm", type: "button", "aria-label": "بستن", onclick: close }, icon("x"))),
    h("div", { class: "modal-b" }, body),
    actions.length || left ? foot : null],
  );
  dlg.addEventListener("cancel", (e) => {
    e.preventDefault();
    close();
  });
  document.body.append(dlg);
  dlg.showModal();
  return { el: dlg, close };
}

export function confirmBox(text, { ok = "تأیید", danger = false, title = "مطمئن هستید؟" } = {}) {
  return new Promise((resolve) => {
    let done = false;
    modal({
      title,
      body: h("p", {}, text),
      actions: [
        { label: "انصراف", onClick: () => { done = true; resolve(false); } },
        { label: ok, kind: danger ? "danger fill" : "primary", onClick: () => { done = true; resolve(true); } },
      ],
      onClose: () => done || resolve(false),
    });
  });
}

// ---------------- forms ----------------

export function field(label, control, hint) {
  return h("div", { class: "field" }, label ? h("label", {}, label) : null, control, hint ? h("span", { class: "hint" }, hint) : null);
}

export function input(props = {}) {
  return h("input", { class: "input", type: "text", ...props });
}

export function select(options, value, props = {}) {
  const s = h("select", { class: "input", ...props });
  for (const o of options) {
    const [v, label] = Array.isArray(o) ? o : [o, o];
    s.append(h("option", { value: v, selected: String(v) === String(value) }, label));
  }
  return s;
}

export function toggle(label, checked, props = {}) {
  const i = h("input", { type: "checkbox", checked, ...props });
  return { el: h("label", { class: "switch" }, i, h("span", {}, label)), input: i };
}

/** Chips input: Enter / comma adds; ✕ removes. */
export function tagInput(values = [], { placeholder = "", suggestions = [] } = {}) {
  const list = [...values];
  const box = h("div", { class: "tagbox" });
  const inp = h("input", { placeholder });
  let sugg = null;
  const draw = (focus = true) => {
    sugg?.querySelectorAll(".chip").forEach((c) => (c.hidden = list.includes(c.dataset.v)));
    box.replaceChildren(
      ...list.map((v, i) => h("span", { class: "chip" }, v, h("button", { type: "button", "aria-label": "حذف", onclick: () => { list.splice(i, 1); draw(); } }, icon("x", "sm")))),
      inp,
    );
    if (focus) inp.focus();
  };
  const add = (raw, focus = true) => {
    for (const v of String(raw).split(/[,،\n]/).map((s) => s.trim()).filter(Boolean)) if (!list.includes(v)) list.push(v);
    inp.value = "";
    draw(focus);
  };
  inp.addEventListener("keydown", (e) => {
    if ((e.key === "Enter" || e.key === "," || e.key === "،") && inp.value.trim()) {
      e.preventDefault();
      add(inp.value);
    } else if (e.key === "Backspace" && !inp.value && list.length) {
      list.pop();
      draw();
    }
  });
  inp.addEventListener("blur", () => inp.value.trim() && add(inp.value, false));
  box.addEventListener("click", () => inp.focus());
  // one scrollable line of suggestions; ones already added are hidden
  sugg = suggestions.length ? h("div", { class: "chips sugg" }, suggestions.map((s) => h("span", { class: "chip click", "data-v": s, onclick: () => add(s) }, "+ ", s))) : null;
  draw(false);
  return { el: h("div", { class: "stack tight" }, box, sugg), values: () => (inp.value.trim() ? (add(inp.value, false), list) : list) };
}

/** Reads a File as base64 (for import_data). */
export function fileToBase64(file) {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result).split(",")[1] || "");
    r.onerror = () => reject("فایل خوانده نشد.");
    r.readAsDataURL(file);
  });
}

export function pickFile(accept) {
  return new Promise((resolve) => {
    const i = h("input", { type: "file", accept, hidden: true });
    const done = (f) => {
      resolve(f);
      i.remove();
    };
    i.addEventListener("change", () => done(i.files[0] || null));
    i.addEventListener("cancel", () => done(null));
    document.body.append(i);
    i.click();
  });
}

/** Axis ceiling split into 4 whole steps: 7 → 8, 46 → 60, 870 → 1000. */
export function niceMax(v) {
  const step = Math.max(1, Math.ceil(v / 4));
  if (step <= 10) return step * 4;
  const unit = 10 ** (String(step).length - 1) / 2;
  return Math.ceil(step / unit) * unit * 4;
}

/** Latin digits in any text → Persian digits ("2.2.0" → "۲٫۲٫۰"). */
export const faDigits = (s) => EN ? String(s ?? "") : String(s ?? "").replace(/\d/g, (d) => "۰۱۲۳۴۵۶۷۸۹"[d]).replace(/(\d|[۰-۹])\.(?=[۰-۹])/g, "$1٫");

/**
 * Column chart with a value axis (HTML, no inline style attributes — heights are set through CSSOM).
 * data: [{label, value, tip}]; `hi` = index drawn striped with its tooltip shown.
 */
export function bars(data, { hi = data.length - 1, unit = "" } = {}) {
  const max = niceMax(Math.max(0, ...data.map((d) => d.value)));
  const axis = h("div", { class: "bc-axis" }, [max, (max * 3) / 4, max / 2, max / 4, 0].map((v) => h("span", {}, fa(Math.round(v)))));
  const cols = h(
    "div",
    { class: "bc-cols" },
    data.map((d, i) => {
      const bar = h("i", { class: d.value ? "" : "zero" });
      bar.style.height = `${Math.max(d.value ? 3 : 1.5, (100 * d.value) / max)}%`;
      return h(
        "div",
        { class: `bc-col ${i === hi ? "hi" : ""}`.trim(), tabindex: "0", "aria-label": `${d.label}: ${fa(d.value)} ${unit}` },
        h("div", { class: "bc-bar" }, h("span", { class: "bc-tip" }, h("small", {}, d.label), h("b", {}, `${fa(d.value)} ${tr(unit)}`.trim()), d.tip ? h("em", {}, d.tip) : null), bar),
        h("span", { class: "bc-x" }, d.short ?? d.label),
      );
    }),
  );
  return h("div", { class: "barchart" }, axis, cols);
}

export function setBar(el, ratio) {
  el.firstElementChild.style.width = `${Math.max(0, Math.min(1, ratio || 0)) * 100}%`;
}
export const bar = (ratio, tone = "") => {
  const b = h("div", { class: `bar ${tone}`.trim() }, h("i"));
  setBar(b, ratio);
  return b;
};

/** «رد کردن و ادامه»: ends the current captcha / sign-in wait — that source is skipped and the job goes on. */
export function skipWaitButton(label = "رد کردن و ادامه") {
  const b = h(
    "button",
    {
      class: "btn sm tonal",
      type: "button",
      title: "این منبع رد می‌شود و عملیات با منابع بعدی ادامه پیدا می‌کند",
      onclick: () => {
        b.disabled = true;
        invoke("wait_skip").catch(() => {});
      },
    },
    icon("skip", "sm"),
    label,
  );
  return b;
}
