// Admin panel helpers shared by its pages.
import { h, icon, invoke, input, jdate, fa } from "../core.js";
import { go } from "../state.js";

/** Server admin API through the desktop backend: api("GET", "/users"). */
export const api = (method, path, body = null) => invoke("admin_api", { method, path, body });

/** Connection state; draws a «not connected» card into `root` and returns null when offline. */
export async function needServer(root) {
  const c = await invoke("admin_server").catch(() => null);
  if (c?.connected) return c;
  root.append(
    h(
      "div",
      { class: "card" },
      h(
        "div",
        { class: "empty" },
        icon("server", "lg"),
        h("h2", {}, "این بخش به سرور نیاز دارد"),
        h("p", { class: "muted" }, "تا وقتی سرور ندارید، مشتریان را از «مشتریان» و «صدور لایسنس» با کد آفلاین مدیریت کنید. هر وقت سرور گرفتید، از «اتصال سرور» وصل شوید؛ راهنمای قدم‌به‌قدم همان‌جاست."),
        h("button", { class: "btn primary", type: "button", onclick: () => go("server") }, icon("plug", "sm"), "اتصال سرور و راهنما"),
      ),
    ),
  );
  return null;
}

/** Local date YYYY-MM-DD `days` from today. */
export function isoIn(days) {
  const d = new Date();
  d.setDate(d.getDate() + days);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export const PERIODS = [
  ["۷ روز", 7],
  ["۱ ماه", 31],
  ["۳ ماه", 92],
  ["۶ ماه", 183],
  ["۱ سال", 365],
];

/** Gregorian date input with the Jalali reading next to it and quick-pick chips («تمدید» from `base`). */
export function dateField(value = "", { base = "" } = {}) {
  const inp = input({ type: "date", dir: "ltr", value });
  const jal = h("span", { class: "small muted" });
  const show = () => (jal.textContent = inp.value ? `= ${jdate(inp.value)}` : "");
  inp.addEventListener("change", show);
  show();
  const set = (days) => {
    // renewing an active subscription extends it from its current end, not from today
    const from = base && base > isoIn(0) ? new Date(`${base}T00:00:00`) : new Date();
    from.setDate(from.getDate() + days);
    inp.value = `${from.getFullYear()}-${String(from.getMonth() + 1).padStart(2, "0")}-${String(from.getDate()).padStart(2, "0")}`;
    show();
  };
  const chips = h("div", { class: "chips" }, PERIODS.map(([l, d]) => h("button", { type: "button", class: "chip click", onclick: () => set(d) }, `+ ${l}`)));
  return { el: h("div", { class: "stack tight" }, inp, jal, chips), input: inp, set };
}

/** Days-left pill for a YYYY-MM-DD end date. */
export function expiryPill(expires) {
  if (!expires) return h("span", { class: "pill" }, "بدون انقضا");
  const days = Math.round((new Date(`${expires}T00:00:00`) - new Date(`${isoIn(0)}T00:00:00`)) / 86400000);
  const tone = days < 0 ? "bad" : days <= 14 ? "warn" : "ok";
  const text = days < 0 ? `${fa(-days)} روز گذشته` : days === 0 ? "امروز" : `${fa(days)} روز مانده`;
  return h("span", { class: `pill ${tone}`, title: jdate(expires) }, text);
}

export const limitText = (n) => (Number(n) === 0 ? "نامحدود" : fa(n));
