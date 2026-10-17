// Global operation centre: every long task (extraction, campaign, product / brand analysis, price
// watch, logins) reported by the backend job manager, shown in one floating drawer at the bottom-left
// corner — on every page, never lost on scroll or route change. Fed by the `job-update` event and a
// one-time `jobs_list` snapshot; each operation can be stopped on its own.
import { invoke, listen, fa, h, clear, icon } from "./core.js";

const ops = new Map(); // id -> JobInfo
const listeners = new Set();
let started = false;

const ACTIVE = new Set(["running", "waiting_for_user", "cancelling"]);
export const isActive = (o) => ACTIVE.has(o.state);

const LABEL = {
  running: "در حال اجرا",
  waiting_for_user: "در انتظار شما",
  cancelling: "در حال توقف",
  completed: "انجام شد",
  completed_with_warnings: "با چند هشدار انجام شد",
  partial: "متوقف شد (داده‌های تا این لحظه حفظ شد)",
  cancelled: "لغو شد",
  failed: "ناموفق",
};

function notify() {
  const list = [...ops.values()].sort((a, b) => Number(isActive(b)) - Number(isActive(a)) || b.id - a.id);
  listeners.forEach((fn) => fn(list));
}

export function onOps(fn) {
  listeners.add(fn);
  fn([...ops.values()]);
  return () => listeners.delete(fn);
}

/** Start listening (once) for backend operation updates. */
export async function startOps() {
  if (started) return;
  started = true;
  listen("job-update", (j) => {
    if (!j || !j.id) return;
    ops.set(j.id, j);
    // keep only active + the few most recent finished ones
    const finished = [...ops.values()].filter((o) => !isActive(o)).sort((a, b) => b.id - a.id);
    finished.slice(8).forEach((o) => ops.delete(o.id));
    notify();
  });
  const seed = await invoke("jobs_list").catch(() => []);
  for (const j of seed) ops.set(j.id, j);
  notify();
}

export const stopOp = (id) => invoke("job_stop", { id }).catch(() => {});
export const stopAll = () => invoke("job_stop", {}).catch(() => {});
export const clearOps = () =>
  invoke("jobs_clear")
    .then(() => {
      [...ops.values()].filter((o) => !isActive(o)).forEach((o) => ops.delete(o.id));
      notify();
    })
    .catch(() => {});

const pct = (o) => (o.target > 0 ? Math.min(100, Math.round((o.checked / o.target) * 100)) : null);

function opRow(o, expanded) {
  const p = pct(o);
  const bar = h("div", { class: "op-bar" }, h("i", { class: isActive(o) ? "live" : "" }));
  if (p !== null) requestAnimationFrame(() => (bar.firstChild.style.width = `${p}%`));
  const sub = isActive(o)
    ? o.step || LABEL[o.state]
    : o.message || LABEL[o.state];
  return h(
    "div",
    { class: `op-row st-${o.state}` },
    h(
      "div",
      { class: "op-head" },
      h("span", { class: `op-dot ${isActive(o) ? "live" : o.state === "failed" ? "bad" : "done"}` }),
      h("b", { class: "op-title", title: o.title }, o.title),
      o.state === "waiting_for_user" ? h("span", { class: "op-badge warn" }, "نیازمند شما") : null,
      isActive(o)
        ? h("button", { class: "op-stop", type: "button", title: "توقف این عملیات", onclick: () => stopOp(o.id) }, icon("stop", "sm"))
        : h("span", { class: "op-state" }, LABEL[o.state]),
    ),
    expanded || isActive(o) ? h("div", { class: "op-sub", title: sub }, sub) : null,
    (expanded || isActive(o)) && (p !== null || o.found > 0)
      ? h("div", { class: "op-meta" }, p !== null ? h("span", {}, bar) : h("span", { class: "grow" }), o.found > 0 ? h("span", { class: "op-found" }, `${fa(o.found)} مورد`) : null)
      : null,
    expanded && o.warnings?.length ? h("div", { class: "op-warn" }, `${fa(o.warnings.length)} هشدار: ${o.warnings[o.warnings.length - 1]}`) : null,
  );
}

/** Builds and wires the floating drawer; returns its root (append once to <body>). */
export function opsDrawer() {
  const root = h("div", { class: "ops-drawer", hidden: true, role: "status", "aria-label": "عملیات‌ها" });
  let open = false;
  const render = (list) => {
    const active = list.filter(isActive);
    const finished = list.filter((o) => !isActive(o));
    root.hidden = list.length === 0;
    if (!list.length) return;
    const head = h(
      "button",
      { class: "ops-toggle", type: "button", onclick: () => { open = !open; render(list); } },
      h("span", { class: `op-dot ${active.length ? "live" : "done"}` }),
      h("b", {}, active.length ? `${fa(active.length)} عملیات فعال` : `${fa(finished.length)} عملیات اخیر`),
      active.length ? h("span", { class: "muted small" }, active[0].title) : null,
      h("span", { class: "ops-caret" }, open ? "▾" : "▴"),
    );
    const body = open
      ? h(
          "div",
          { class: "ops-body" },
          list.map((o) => opRow(o, true)),
          h(
            "div",
            { class: "ops-foot" },
            active.length > 1 ? h("button", { class: "btn sm danger", type: "button", onclick: stopAll }, icon("stop", "sm"), "توقف همه") : null,
            finished.length ? h("button", { class: "btn sm ghost", type: "button", onclick: clearOps }, "پاک کردن پایان‌یافته‌ها") : null,
          ),
        )
      : active.length === 1
        ? opRow(active[0], false)
        : null;
    clear(root, head, body);
  };
  onOps(render);
  return root;
}
