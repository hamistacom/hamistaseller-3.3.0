// «دریافت لیست محصول»: every product (photo, name, price, link, seller, brand, rating, date …) of
// a shop link — a Basalam booth (that booth only), a whole site (its home page), one category page or
// a public Telegram channel — with a short analysis and an Excel file.
import { h, icon, invoke, fa, clear, safe, input, field, toast, newChannel, skipWaitButton, select, toggle } from "../core.js";

const toman = (n) => (n > 0 ? `${fa(n)} تومان` : "—");
const remember = (k, v) => {
  try {
    if (v === undefined) return localStorage.getItem(k);
    localStorage.setItem(k, v);
  } catch {}
  return null;
};
const EXAMPLES = [
  ["غرفه باسلام", "https://basalam.com/نام-غرفه", "فقط محصولات همان غرفه"],
  ["کل سایت", "https://shop.ir", "صفحه اصلی و دسته‌بندی‌ها"],
  ["یک دسته", "https://shop.ir/category/honey", "همان صفحه و صفحه‌های بعدی"],
  ["کانال تلگرام", "https://t.me/name", "پست‌ها با قیمت، عکس، لینک و تاریخ"],
];

/** Scope the backend will apply, said before the run (same rules as products.rs `scope_of`). */
function scopeHint(raw) {
  const u = raw.trim();
  if (!u) return "";
  let url;
  try {
    url = new URL(/^https?:\/\//.test(u) ? u : `https://${u}`);
  } catch {
    return "لینک معتبر نیست.";
  }
  const host = url.hostname.replace(/^www\./, "");
  const first = decodeURIComponent(url.pathname).split("/").filter(Boolean)[0] || "";
  if (host === "t.me" || host === "telegram.me") return "پست‌های کانال عمومی تلگرام: متن، قیمت نوشته‌شده، عکس، لینک پست و لینک محصول، تاریخ و بازدید.";
  const reserved = ["search", "accounts", "cart", "profile", "category", "categories", "blog", "about", "help", "landing", "p", "product"];
  if (host === "basalam.com" && first && !reserved.includes(first)) return `فقط محصولات غرفه «${first}» — محصولات غرفه‌های دیگر و پیشنهادهای باسلام کنار گذاشته می‌شوند.`;
  if (!first) return `کل سایت ${host}: صفحه اصلی و صفحه‌های دسته‌بندی (تا سقف صفحه‌ها).`;
  return `همین صفحه از ${host} و صفحه‌های «بعدی» آن؛ فقط لینک‌های خود ${host}.`;
}

export async function render(root, params = {}) {
  const url = input({ class: "input ltr", placeholder: "https://basalam.com/… یا آدرس هر فروشگاه / کانال تلگرام", value: params.url || remember("plistUrl") || "" });
  const query = input({ placeholder: "فقط این محصولات (اختیاری)، مثلاً عسل -موم", value: remember("plistQuery") || "" });
  const limit = select([100, 300, 600, 1000, 2000, 3000].map((n) => [n, `تا ${fa(n)} محصول`]), Number(remember("plistLimit")) || 300, { class: "input" });
  const pages = select([[0, "صفحه‌ها: خودکار"], ...[3, 5, 10, 20, 40, 60].map((n) => [n, `تا ${fa(n)} صفحه`])], Number(remember("plistPages")) || 0, { class: "input" });
  const details = toggle("خواندن جزئیات هر محصول (موجودی، برند، توضیح) — کندتر", remember("plistDetails") === "1");
  const hint = h("div", { class: "small muted" });
  const drawHint = () => (hint.textContent = scopeHint(url.value));
  url.addEventListener("input", drawHint);
  drawHint();

  const go_ = h("button", { class: "btn accent", type: "button" }, icon("download", "sm"), "دریافت لیست");
  const progress = h("div", { class: "mkt-progress", hidden: true });
  const out = h("div", { class: "stack" });
  let current = null;

  const run = async () => {
    const u = url.value.trim();
    if (u.length < 4) return toast("لینک سایت، غرفه یا کانال را وارد کنید.", "bad");
    remember("plistUrl", u);
    remember("plistQuery", query.value.trim());
    remember("plistLimit", limit.value);
    remember("plistPages", pages.value);
    remember("plistDetails", details.input.checked ? "1" : "0");
    go_.disabled = true;
    clear(go_, h("span", { class: "spin-dot" }), "در حال دریافت…");
    progress.hidden = false;
    const line = h("span", { class: "small" }, "در حال آماده‌سازی…");
    const waitBox = h("div", { class: "callout compact row", hidden: true });
    const log = h("div", { class: "small muted mkt-log" });
    const stop = h("button", { class: "btn sm danger", type: "button", onclick: (e) => { e.currentTarget.disabled = true; safe(invoke("job_stop", {})); } }, icon("stop", "sm"), "توقف");
    clear(progress, h("div", { class: "row between" }, line, stop), waitBox, log);
    const ch = newChannel();
    ch.onmessage = (e) => {
      if (e.type === "status") {
        line.textContent = e.message;
        waitBox.hidden = true;
      }
      if (e.type === "log") log.append(h("div", { class: e.level }, e.message));
      if (e.type === "captcha") {
        clear(waitBox, icon("alert", "sm"), h("span", { class: "small grow" }, e.message), skipWaitButton());
        waitBox.hidden = false;
      }
    };
    const req = { url: u, limit: Number(limit.value) || 300, pages: Number(pages.value) || 0, details: details.input.checked, query: query.value.trim() };
    const r = await safe(invoke("product_list", { req, onEvent: ch }));
    go_.disabled = false;
    clear(go_, icon("download", "sm"), "دریافت لیست");
    progress.hidden = true;
    if (r) show(r);
  };
  go_.addEventListener("click", run);
  url.addEventListener("keydown", (e) => e.key === "Enter" && run());

  const exportXlsx = async () => {
    if (!current) return;
    const path = await safe(invoke("product_list_export", { report: current }));
    if (!path) return;
    toast("فایل اکسل ذخیره شد.", "ok");
    invoke("reveal", { path }).catch(() => {});
  };

  function show(r) {
    current = r;
    const st = r.stats || {};
    let sort = "default";
    const table = h("div", { class: "table-wrap" });
    const sorted = () => {
      const list = [...r.items];
      if (sort === "cheap") list.sort((a, b) => (a.price || Infinity) - (b.price || Infinity));
      if (sort === "expensive") list.sort((a, b) => (b.price || 0) - (a.price || 0));
      if (sort === "newest") list.sort((a, b) => (b.date || "").localeCompare(a.date || ""));
      return list;
    };
    const drawTable = () =>
      clear(
        table,
        h(
          "table",
          { class: "t" },
          h("thead", {}, h("tr", {}, ["", "محصول", "قیمت", "فروشنده / غرفه", "جزئیات", "تاریخ"].map((t) => h("th", {}, t)))),
          h(
            "tbody",
            {},
            sorted().map((it) => {
              const pic = it.image ? h("img", { src: it.image, alt: "", loading: "lazy", class: "pl-pic" }) : h("span", { class: "pl-pic" });
              if (it.image) pic.addEventListener("error", () => (pic.hidden = true));
              const open = (u) => () => safe(invoke("open_url", { url: u }));
              return h(
                "tr",
                {},
                h("td", {}, pic),
                h(
                  "td",
                  {},
                  h(
                    "div",
                    { class: "name-cell" },
                    h("button", { class: "link n", type: "button", title: it.url, onclick: open(it.url), "data-no-tr": "" }, it.title),
                    it.link ? h("button", { class: "link small c", type: "button", onclick: open(it.link) }, icon("external", "sm"), "لینک محصول در سایت") : null,
                  ),
                ),
                h("td", { class: "nowrap" }, toman(it.price)),
                h("td", {}, it.sellerUrl ? h("button", { class: "link small", type: "button", onclick: open(it.sellerUrl), "data-no-tr": "" }, it.seller || "مشاهده") : h("span", { "data-no-tr": "" }, it.seller || "—")),
                h(
                  "td",
                  { class: "small muted" },
                  [it.brand && `برند: ${it.brand}`, it.rating && `امتیاز ${fa(it.rating)}`, it.reviews && `${fa(it.reviews)} نظر`, it.sales && `${fa(it.sales)} فروش`, it.views && `${fa(it.views)} بازدید`, it.availability, it.location]
                    .filter(Boolean)
                    .join(" · ") || "—",
                ),
                h("td", { class: "small nowrap" }, it.date ? it.date.slice(0, 10) : "—"),
              );
            }),
          ),
        ),
      );
    drawTable();
    const sortSel = select(
      [
        ["default", "ترتیب سایت"],
        ["cheap", "ارزان‌ترین"],
        ["expensive", "گران‌ترین"],
        ["newest", "جدیدترین"],
      ],
      sort,
      { class: "input", onchange: (e) => { sort = e.target.value; drawTable(); } },
    );
    const stat = (v, l) => h("div", { class: "stat" }, h("b", {}, v), h("span", {}, l));
    clear(
      out,
      h(
        "div",
        { class: "card" },
        h("div", { class: "card-h" }, h("h2", {}, `${fa(st.count)} محصول`), h("span", { class: "pill info" }, r.scope)),
        h(
          "div",
          { class: "grid-4" },
          stat(fa(st.priced), "دارای قیمت"),
          stat(toman(st.min), "کمترین"),
          stat(toman(st.median), "میانه"),
          stat(toman(st.max), "بیشترین"),
        ),
        st.newest
          ? h("p", { class: "small muted" }, `بازه تاریخ: ${st.oldest.slice(0, 10)} تا ${st.newest.slice(0, 10)}${st.perWeek ? ` · حدود ${fa(st.perWeek)} مورد در هفته` : ""}`)
          : null,
        st.sellers?.length > 1 ? h("p", { class: "small" }, h("b", {}, "فروشندگان پرتکرار: "), st.sellers.slice(0, 6).map(([n, c]) => `${n} (${fa(c)})`).join("، ")) : null,
        st.brands?.length ? h("p", { class: "small" }, h("b", {}, "برندها: "), st.brands.slice(0, 6).map(([n, c]) => `${n} (${fa(c)})`).join("، ")) : null,
        r.notes?.length ? h("div", { class: "small muted" }, r.notes.map((n) => h("div", {}, n))) : null,
        h("div", { class: "row between wrap" }, h("span", { class: "small muted" }, `${fa(r.pages)} صفحه خوانده شد`), h("div", { class: "row" }, sortSel, h("button", { class: "btn primary", type: "button", onclick: exportXlsx }, icon("download", "sm"), "خروجی اکسل"))),
      ),
      h("div", { class: "card flush" }, table),
    );
  }

  root.append(
    h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "card" },
        h("div", { class: "row wrap" }, h("div", { class: "grow" }, url), go_),
        hint,
        h("div", { class: "mkt-row2" }, field("فیلتر نام محصول", query), field("حداکثر محصول", limit), field("حداکثر صفحه", pages)),
        details.el,
        h(
          "div",
          { class: "chips" },
          EXAMPLES.map(([t, u, s]) => h("button", { class: "chip", type: "button", title: s, onclick: () => { url.value = u; drawHint(); url.focus(); } }, t)),
        ),
      ),
      progress,
      out,
    ),
  );
}
