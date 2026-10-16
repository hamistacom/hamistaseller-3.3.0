// Admin dashboard: offline registry figures, renewals due, and the server's state when connected.
import { h, icon, invoke, fa, clear, jdate } from "../core.js";
import { go } from "../state.js";
import { expiryPill } from "./admin-common.js";
import { issueModal } from "./admin-customers.js";

const kpi = (label, value, sub, ic, hero = false) =>
  h("div", { class: `card kpi ${hero ? "hero" : ""}`.trim() }, h("div", { class: "label" }, icon(ic, "sm"), label), h("span", { class: "value" }, fa(value)), h("div", { class: "sub" }, sub));

export async function render(root, _params, actions) {
  actions.append(
    h("button", { class: "btn", type: "button", onclick: () => go("customers") }, icon("users", "sm"), "مشتریان"),
    h("button", { class: "btn accent", type: "button", onclick: () => issueModal({}, () => go("home")) }, icon("key", "sm"), "صدور لایسنس"),
  );
  const [st, due, srv] = await Promise.all([
    invoke("admin_stats"),
    invoke("admin_customers", { q: "", view: "expiring" }),
    invoke("admin_server").catch(() => null),
  ]);
  const expired = st.expired ? await invoke("admin_customers", { q: "", view: "expired" }) : [];

  const renewList = (list, empty) =>
    list.length
      ? h(
          "div",
          { class: "feed" },
          list.slice(0, 8).map((c) =>
            h(
              "div",
              { class: "it" },
              h("div", { class: "grow" }, h("b", {}, c.name), h("span", {}, `${c.plan || "—"} · تا ${jdate(c.expires)}${c.phone ? ` · ${c.phone}` : ""}`)),
              expiryPill(c.expires),
              h("button", { class: "btn sm", type: "button", onclick: () => issueModal(c, () => go("home")) }, icon("key", "sm"), "تمدید"),
            ),
          ),
        )
      : h("div", { class: "empty small" }, empty);

  let server;
  if (srv?.connected && srv.overview) {
    const o = srv.overview;
    server = h(
      "div",
      { class: "card stack" },
      h("div", { class: "card-h" }, h("h2", {}, "سرور"), h("span", { class: "pill ok" }, "متصل"), h("span", { class: "grow" }), h("span", { class: "small muted ltr" }, srv.url)),
      srv.keyMatches === false
        ? h("div", { class: "callout" }, icon("alert"), h("div", { class: "small grow" }, "سرور با کلید دیگری امضا می‌کند؛ حساب‌های آنلاین در برنامه مشتری تأیید نمی‌شوند. کلید خصوصی پنل را در HAMISTA_SIGNING_SEED سرور بگذارید."), h("button", { class: "btn sm", type: "button", onclick: () => go("server") }, "راهنما"))
        : null,
      h(
        "div",
        { class: "grid-3" },
        [
          ["کاربران آنلاین", o.users, `${fa(o.activeUsers)} فعال`],
          ["دستگاه‌ها", o.devices, `${fa(o.loginsToday)} ورود امروز`],
          ["ابربانک", o.megabank, `${fa(o.newMegaToday)} کسب‌وکار جدید امروز`],
        ].map(([l, v, s]) => h("div", { class: "stat" }, h("span", {}, l), h("b", {}, fa(v)), h("span", {}, s))),
      ),
      h("div", { class: "row wrap" }, h("button", { class: "btn sm", type: "button", onclick: () => go("accounts") }, "کاربران"), h("button", { class: "btn sm", type: "button", onclick: () => go("mega") }, "ابربانک"), h("button", { class: "btn sm", type: "button", onclick: () => go("logs") }, "گزارش‌ها")),
    );
  } else {
    server = h(
      "div",
      { class: "card stack" },
      h("div", { class: "card-h" }, h("h2", {}, "سرور"), h("span", { class: "pill" }, srv?.url ? "قطع" : "تنظیم نشده")),
      h("p", { class: "small muted" }, srv?.url ? "اتصال به سرور برقرار نیست (اینترنت یا سرور را بررسی کنید). بخش آفلاین پنل کامل کار می‌کند." : "پنل الان در حالت آفلاین است: مشتریان و لایسنس‌ها را همین‌جا مدیریت کنید. هر وقت سرور گرفتید، با یک اتصال، کاربران آنلاین، سهمیه‌ها، تعلیق از راه دور و ابربانک فعال می‌شوند."),
      h("button", { class: "btn sm", type: "button", onclick: () => go("server") }, icon("plug", "sm"), srv?.url ? "اتصال سرور" : "راهنمای راه‌اندازی سرور"),
    );
  }

  root.append(
    h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "grid-4" },
        kpi("مشتریان", st.customers, `${fa(st.active)} فعال`, "users", true),
        kpi("رو به اتمام", st.expiring, "تا ۱۴ روز آینده", "clock"),
        kpi("منقضی", st.expired, "نیازمند تمدید", "alert"),
        kpi("لایسنس‌ها", st.licences, `${fa(st.licencesMonth)} صدور در این ماه`, "key"),
      ),
      h(
        "div",
        { class: "grid-2" },
        h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "تمدیدهای نزدیک"), h("button", { class: "link small", type: "button", onclick: () => go("customers", { view: "expiring" }) }, "همه")), renewList(due, "تمدیدی در ۱۴ روز آینده نیست")),
        server,
      ),
      expired.length ? h("div", { class: "card stack" }, h("div", { class: "card-h" }, h("h2", {}, "منقضی‌شده‌ها"), h("button", { class: "link small", type: "button", onclick: () => go("customers", { view: "expired" }) }, "همه")), renewList(expired, "")) : null,
    ),
  );
}
