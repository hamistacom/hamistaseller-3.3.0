// Shell: navigation, routing, sidebar job card, licence banner, theme, Ctrl+K.
import { EN, setLang } from "./i18n.js";
import { $, h, clear, icon, invoke, fa, faDigits, toast, modal, safe, bar, jdate, ago, utc } from "./core.js";
import { routesFor, SPACES } from "./modules.js";
import { app, refreshLists, refreshLicense, on, go } from "./state.js";
import { adoptRunning, followScheduled } from "./jobs.js";
import { startOps, opsDrawer } from "./ops.js";
import { openCard } from "./pages/card.js";
import { licenseModal } from "./pages/settings.js";

let routes = {};
let current = null; // {name, mod}
// the workspace shown in the sidebar: اطلاعات مشتریان | تحلیل محصول | تحلیل برند
let space = (() => {
  try {
    return localStorage.getItem("space") || "crm";
  } catch {
    return "crm";
  }
})();
function setSpace(s) {
  space = s;
  try {
    localStorage.setItem("space", s);
  } catch {}
}
const view = $("#view");

function parseHash() {
  const [path, query] = location.hash.replace(/^#\/?/, "").split("?");
  return { name: routes[path] ? path : Object.keys(routes)[0], params: Object.fromEntries(new URLSearchParams(query || "")) };
}

let seq = 0;
async function route() {
  const my = ++seq;
  const { name, params } = parseHash();
  const r = routes[name];
  if (r.space && r.space !== space) {
    setSpace(r.space);
    drawNav();
  }
  current?.mod?.leave?.();
  current = null;
  $("#page-title").textContent = r.title;
  $("#page-sub").textContent = r.sub;
  clear($("#page-actions"));
  for (const a of $("#nav").children) a.classList.toggle("on", a.dataset.route === name);
  // the old page stays (dimmed) while the next one loads; a skeleton only if loading is slow
  view.classList.remove("enter");
  view.classList.add("busy");
  const skel = setTimeout(() => my === seq && clear(view, h("div", { class: "skel", "aria-label": "در حال بارگذاری…" }, h("i"), h("i"), h("i"), h("i"))), 260);
  // drawn off-screen, then swapped in: a page that finishes after the user moved on is dropped
  const page = h("div");
  const acts = h("div");
  let mod = {};
  try {
    mod = await r.load();
    if (my !== seq) return;
    await mod.render(page, params, acts);
  } catch (e) {
    if (my !== seq) return;
    clear(page, h("div", { class: "empty" }, icon("alert"), String(e?.message || e)));
  }
  if (my !== seq) {
    mod.leave?.();
    return;
  }
  current = { name, mod };
  const hb = $("#help-btn");
  hb.hidden = !routes.help || name === "help";
  hb.onclick = () => go("help", { topic: name });
  if (!hb.firstChild) hb.append(icon("info"));
  clearTimeout(skel);
  view.classList.remove("busy");
  view.replaceChildren(...page.childNodes);
  $("#page-actions").replaceChildren(...acts.childNodes);
  view.scrollTop = 0;
  void view.offsetWidth; // restart the entrance animation
  view.classList.add("enter");
}

function drawNav() {
  const now = current?.name || parseHash().name;
  const spaces = SPACES.filter((s) => Object.values(routes).some((r) => r.space === s.id));
  if (spaces.length && !spaces.some((s) => s.id === space)) setSpace(spaces[0].id);
  const box = $("#spaces");
  box.hidden = spaces.length < 2;
  // Ensure all 3 workspace tabs are always fully visible without clipping or overflow
  box.style.maxHeight = "none";
  box.style.overflow = "visible";
  box.style.display = spaces.length < 2 ? "none" : "flex";
  box.style.flexDirection = "column";
  box.style.gap = "4px";
  box.style.padding = "6px";
  box.style.flexShrink = "0";
  clear(
    box,
    spaces.map((s) => {
      const btn = h(
        "button",
        {
          type: "button",
          role: "tab",
          class: s.id === space ? "on" : "",
          "aria-selected": s.id === space ? "true" : "false",
          title: `${s.title}: ${s.sub}`,
          onclick: () => {
            setSpace(s.id);
            drawNav();
            go(s.home);
          },
        },
        icon(s.icon),
        h("span", { class: "sp-t" }, s.title),
        h("span", { class: "sp-s" }, s.sub),
      );
      btn.style.padding = "7px 10px";
      btn.style.minHeight = "0";
      return btn;
    }),
  );
  const out = [];
  let group = null;
  for (const [k, r] of Object.entries(routes)) {
    if (r.space && r.space !== space) continue;
    if (r.group && r.group !== group) {
      group = r.group;
      out.push(h("div", { class: "nav-group" }, group));
    }
    out.push(
      h(
        "a",
        { href: `#/${k}`, "data-route": k, class: k === now ? "on" : "" },
        icon(r.icon),
        h("span", {}, r.title),
        r.badge ? h("span", { class: "nav-badge" }, r.badge) : null,
      ),
    );
  }
  clear($("#nav"), out);
}

// ---------------- user pill (who is licensed) and the sidebar insight ----------------

function drawUser() {
  const l = app.info.license || {};
  const ed = app.info.edition;
  const name = ed === "issuer" ? "مدیر هامیستا" : ed === "master" ? "نسخه کامل مدیر" : l.customer || (l.valid ? "مشتری هامیستا" : "فعال نشده");
  const sub =
    ed === "issuer"
      ? `پنل مدیر · نسخه ${faDigits(app.info.version)}`
      : ed === "master"
        ? `بدون محدودیت · نسخه ${faDigits(app.info.version)}`
        : l.valid
          ? [l.plan, l.mode === "online" ? "حساب آنلاین" : "کد آفلاین", l.expires ? `${fa(l.daysLeft)} روز اعتبار` : ""].filter(Boolean).join(" · ")
          : "برای فعال‌سازی کلیک کنید";
  $("#user-av").textContent = (name.trim()[0] || "ه").toUpperCase();
  $("#user-name").textContent = name;
  $("#user-sub").textContent = sub;
  const pill = $("#user-pill");
  pill.classList.toggle("click", ed === "customer");
  pill.onclick = ed === "customer" ? (e) => !e.target.closest("#bell") && licenseModal() : null;
}

let insightClosed = false;
/** One useful, data-driven suggestion in the sidebar (what to do next). */
async function drawInsight() {
  const box = $("#insight");
  if (app.info.edition === "issuer" || insightClosed || parseHash().name === "dashboard") return (box.hidden = true);
  const d = await invoke("dashboard").catch(() => null);
  if (!d) return (box.hidden = true);
  let title;
  let text;
  let go1;
  if (!d.total) [title, text, go1] = ["اولین بانک مشتری", "با یک استخراج چندمنبعی، بانک اولیه را در چند دقیقه بسازید.", () => go("scan")];
  else if (d.interestedWaiting)
    [title, text, go1] = [
      `${fa(d.interestedWaiting)} مشتری منتظر پیگیری`,
      "علاقه‌مندانی که بیش از ۳ روز پیگیری نشده‌اند؛ یک تماس به‌موقع شانس خرید را بالا می‌برد.",
      () => go("contacts", { status: "interested" }),
    ];
  else if (d.today) [title, text, go1] = [`${fa(d.today)} مخاطب تازه امروز`, "پیش از سرد شدن، برایشان پیام معرفی بفرستید.", () => go("outreach")];
  else [title, text, go1] = ["بانک را گسترش دهید", `${fa(d.total)} مخاطب دارید؛ با منابع دیگر (دیوار، شیپور، نشان، بلد، تلگرام) مخاطب تازه پیدا کنید.`, () => go("scan")];
  box.hidden = false;
  clear(
    box,
    h("b", {}, title),
    h("p", {}, text),
    h(
      "div",
      { class: "row" },
      h("button", { class: "btn sm insight-later", type: "button", onclick: () => { insightClosed = true; box.hidden = true; } }, "بعداً"),
      h("button", { class: "btn sm insight-go", type: "button", onclick: go1 }, "انجام بده"),
    ),
  );
}

// ---------------- licence ----------------

function drawLicense(l) {
  const chip = $("#lic-chip");
  const banner = $("#banner");
  clear(banner);
  chip.hidden = true;
  if (!l) return;
  drawUser();
  if (l.edition === "master") {
    chip.hidden = false;
    chip.textContent = "نسخه کامل";
    chip.className = "lic-chip";
    return;
  }
  if (l.edition === "issuer") return;
  chip.hidden = false;
  chip.onclick = licenseModal;
  const next = routesFor(app.info);
  if (Object.keys(next).join() !== Object.keys(routes).join()) {
    routes = next;
    drawNav();
    if (!routes[location.hash.replace(/^#\/?/, "").split("?")[0]]) route();
  }
  drawBell(l);
  if (l.mode === "online" && l.valid) checkVersion();
  if (!l.valid) {
    chip.textContent = "فعال نشده";
    chip.className = "lic-chip bad";
    banner.append(
      h(
        "div",
        { class: "banner bad" },
        icon("shield"),
        h("span", {}, l.message || "برنامه فعال نشده است.", " استخراج، ارسال و خروجی تا فعال‌سازی غیرفعال است؛ داده‌ها قابل مشاهده‌اند."),
        h("button", { class: "btn sm danger fill", type: "button", onclick: licenseModal }, "فعال‌سازی"),
      ),
    );
  } else if (l.daysLeft <= 7) {
    chip.textContent = `${fa(l.daysLeft)} روز مانده`;
    chip.className = "lic-chip warn";
    banner.append(
      h(
        "div",
        { class: "banner warn" },
        icon("clock"),
        h("span", {}, `${fa(l.daysLeft)} روز تا پایان اعتبار لایسنس. برای تمدید با پشتیبانی هامیستا در ارتباط باشید.`),
        h("button", { class: "btn sm", type: "button", onclick: licenseModal }, "جزئیات"),
      ),
    );
  } else {
    chip.textContent = l.mode === "online" ? "فعال · آنلاین" : "فعال";
    chip.className = "lic-chip";
  }
}

// ---------------- notifications (online accounts) ----------------

let bellSeq = 0;
async function drawBell(l) {
  const bell = $("#bell");
  bell.hidden = !(l?.mode === "online" && l.valid);
  if (bell.hidden) return;
  const my = ++bellSeq;
  const r = await invoke("cloud_notifications").catch(() => null);
  if (my !== bellSeq || !r) return;
  const unread = (r.items || []).filter((n) => !n.read).length;
  clear(bell, icon("bell"), unread ? h("span", { class: "bell-n" }, fa(unread)) : null);
  bell.onclick = () => notificationsModal(r.items || []);
}

function notificationsModal(items) {
  const list = items.length
    ? h(
        "div",
        { class: "feed" },
        items.map((n) =>
          h(
            "div",
            { class: `it notif ${n.read ? "" : "unread"}`.trim() },
            h("span", { class: `dot ${n.read ? "" : "live"}` }),
            h("div", { class: "grow" }, h("b", {}, n.title), n.body ? h("p", { class: "small notif-body" }, n.body) : null, h("span", {}, `${jdate(utc(n.createdAt), true)} · ${ago(utc(n.createdAt))}`)),
          ),
        ),
      )
    : h("div", { class: "empty small" }, icon("bell"), "اعلانی نیست");
  modal({ title: "اعلان‌ها", body: list });
  Promise.all(items.filter((n) => !n.read).map((n) => invoke("cloud_notification_read", { id: n.id }).catch(() => null))).then(() => drawBell(app.info.license));
}

let versionChecked = false;
async function checkVersion() {
  if (versionChecked || app.info.license?.mode !== "online") return;
  versionChecked = true;
  const v = await invoke("cloud_version").catch(() => null);
  if (!v?.version || !newer(v.version, app.info.version)) return;
  const open = () => v.url && safe(invoke("open_url", { url: v.url }));
  clear($("#banner-ver")).append(
    h(
      "div",
      { class: `banner ${v.mandatory ? "bad" : "info"}` },
      icon("download"),
      h("span", {}, `نسخه ${v.version} هامیستا سلر منتشر شده است${v.mandatory ? " و به‌روزرسانی آن لازم است" : ""}.`, v.notes ? ` ${v.notes}` : ""),
      v.url ? h("button", { class: "btn sm", type: "button", onclick: open }, "دریافت نسخه جدید") : null,
    ),
  );
}
const newer = (a, b) => {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) > (pb[i] || 0);
  return false;
};

// ---------------- theme ----------------

function drawTheme() {
  const dark = document.documentElement.dataset.theme === "dark";
  clear($("#theme"), icon(dark ? "sun" : "moon"));
}
$("#lang").textContent = EN ? "فا" : "EN";
$("#lang").title = EN ? "فارسی" : "English";
$("#lang").addEventListener("click", () => setLang(EN ? "fa" : "en"));
$("#theme").addEventListener("click", () => {
  const next = document.documentElement.dataset.theme === "dark" ? "light" : "dark";
  document.documentElement.dataset.theme = next;
  try {
    localStorage.setItem("theme", next);
  } catch {}
  drawTheme();
});

// ---------------- Ctrl+K / search ----------------

function palette(initial = "") {
  const inp = h("input", { class: "input", placeholder: "نام کسب‌وکار، شماره، ایمیل یا نام مسئول…", value: initial, autocomplete: "off" });
  const res = h("div", { class: "res" });
  const m = modal({ title: "جستجو و دسترسی سریع", body: [inp, res] });
  m.el.classList.add("palette");
  let seq = 0;
  const pages = Object.entries(routes).map(([k, r]) => ({ label: r.title, icon: r.icon, run: () => go(k) }));
  const draw = async () => {
    const q = inp.value.trim();
    const mine = ++seq;
    const items = pages.filter((p) => !q || p.label.includes(q));
    if (q && routes.contacts) items.unshift({ label: `همه نتایج «${q}» در مخاطبان`, icon: "search", run: () => go("contacts", { q }) });
    let places = [];
    if (q.length >= 2 && routes.contacts) {
      const r = await invoke("places", { filter: { listId: 0, q }, offset: 0, limit: 8 }).catch(() => null);
      if (mine !== seq) return;
      places = (r?.rows || []).map((p) => ({ label: p.name, sub: p.contactPerson || p.city, icon: "user", run: () => openCard(p.id) }));
    }
    clear(
      res,
      [...places, ...items].map((it, i) =>
        h(
          "button",
          { type: "button", class: i === 0 ? "on" : "", onclick: () => { m.close(); it.run(); } },
          icon(it.icon, "sm"),
          h("span", { class: "grow" }, it.label),
          it.sub ? h("span", { class: "muted small" }, it.sub) : null,
        ),
      ),
    );
  };
  inp.addEventListener("input", draw);
  inp.addEventListener("keydown", (e) => {
    if (e.key !== "Enter") return;
    if (inp.value.trim() && routes.contacts) {
      m.close();
      go("contacts", { q: inp.value.trim() });
    } else res.querySelector("button")?.click();
  });
  draw();
  inp.focus();
}

document.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && e.code === "KeyK") {
    e.preventDefault();
    if (!document.querySelector("dialog[open]")) palette($("#search").value);
  }
});
$("#search").addEventListener("keydown", (e) => {
  if (e.key === "Enter" && e.currentTarget.value.trim()) go("contacts", { q: e.currentTarget.value.trim() });
});
$("#search").addEventListener("focus", (e) => {
  e.currentTarget.blur();
  palette();
});

// ---------------- boot ----------------

async function boot() {
  drawTheme();
  try {
    app.info = await invoke("app_info");
  } catch (e) {
    clear(view, h("div", { class: "empty" }, icon("alert"), String(e)));
    return;
  }
  routes = routesFor(app.info);
  drawUser();
  if (app.info.edition === "issuer") {
    $("#brand-sub").textContent = "پنل مدیر";
    $("#search-box").hidden = true;
  } else {
    drawInsight();
    window.addEventListener("hashchange", () => drawInsight());
    await refreshLists();
    document.body.append(opsDrawer());
    startOps();
    followScheduled();
    on("license", drawLicense);
    drawLicense(app.info.license);
    const running = await invoke("job_current").catch(() => null);
    if (running) adoptRunning(running);
  }
  drawNav();
  window.addEventListener("hashchange", route);
  window.addEventListener("reroute", route);
  await route();
  const warm = () => Object.values(routes).forEach((r) => r.load().catch(() => {}));
  "requestIdleCallback" in window ? requestIdleCallback(warm, { timeout: 2000 }) : setTimeout(warm, 800);
  if (app.info.migrated) {
    const m = app.info.migrated;
    toast(`داده‌های SellerMap منتقل شد: ${fa(m.places)} کسب‌وکار در ${fa(m.lists)} بانک.`, "ok");
  }
  if (app.info.edition === "customer" && !app.info.license.valid) licenseModal();
  if (app.info.edition === "customer") {
    setInterval(() => refreshLicense().catch(() => null), 10 * 60 * 1000);
  }
}

boot();