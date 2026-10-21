// Seller tools shared by the market analysis, the catalogue and the watchlist:
// «سود روی هر کانال فروش» (commission, VAT, packaging → net profit and the floor price per platform),
// the buy-box line (am I the cheapest listing?) and the alert settings (Telegram bot / SMS).
import { h, icon, invoke, fa, clear, safe, input, field, toast, modal, select } from "../core.js";

const DIGITS = "۰۱۲۳۴۵۶۷۸۹";
/** "۱٬۲۰۰٬۰۰۰" / "1,200,000" → 1200000 */
export const num = (s) => Number(String(s ?? "").replace(/[۰-۹]/g, (d) => DIGITS.indexOf(d)).replace(/[^\d.]/g, "")) || 0;
const toman = (n) => (n > 0 ? fa(Math.round(n)) : "—");
const moneyInput = (value, props = {}) => {
  const i = input({ inputmode: "numeric", dir: "ltr", value: value ? fa(value) : "", ...props });
  i.addEventListener("blur", () => (i.value = num(i.value) ? fa(num(i.value)) : ""));
  return i;
};

/**
 * Net profit of one price on every platform. `price` / `cost` are Toman per unit; the commission of each
 * platform can be edited (the seller's own contract) and is remembered.
 */
export function profitCard({ price = 0, cost = 0, category = "", title = "سود روی هر کانال فروش", compact = false } = {}) {
  const priceIn = moneyInput(price);
  const costIn = moneyInput(cost);
  const packIn = moneyInput(0);
  const adIn = moneyInput(0);
  const vatIn = select([[10, "مالیات ارزش‌افزوده ۱۰٪"], [0, "بدون مالیات ارزش‌افزوده"]], 10);
  const out = h("div", { class: "table-wrap" });
  const note = h("p", { class: "small muted" });
  let overrides = {};
  let seq = 0;
  const draw = async (setRates = null) => {
    const mine = ++seq;
    const input_ = { cost: num(costIn.value), price: num(priceIn.value), category, packCost: num(packIn.value), adCost: num(adIn.value), vatPct: Number(vatIn.value), overrides };
    const r = await safe(invoke("profit_compare", { input: input_, setRates }));
    if (!r || mine !== seq) return;
    overrides = Object.fromEntries(r.rows.filter((x) => !x.estimated).map((x) => [x.platform, x.commissionPct]));
    const best = r.rows[0];
    note.textContent = input_.price && input_.cost
      ? `بهترین کانال: ${best.name} با سود خالص ${toman(best.net)} تومان (${fa(best.marginPct)}٪). نرخ‌های «تخمینی» میانگین بازار است؛ کارمزد قرارداد خودتان را در ستون کارمزد بنویسید تا ذخیره شود.`
      : "بهای خرید و قیمت فروش را بنویسید تا سود هر کانال حساب شود. نرخ‌های «تخمینی» میانگین بازار است و با کارمزد قرارداد شما فرق دارد.";
    const rate = (x) => {
      const i = h("input", { class: "input sm rate-in", type: "number", min: 0, max: 60, step: 0.5, dir: "ltr", value: x.commissionPct, "aria-label": `کارمزد ${x.name}` });
      i.addEventListener("change", () => {
        const v = Number(i.value);
        if (Number.isFinite(v) && v >= 0 && v <= 60) {
          overrides = { ...overrides, [x.platform]: v };
          draw(overrides);
        }
      });
      return h("div", { class: "row tight-row" }, i, x.estimated ? h("span", { class: "pill", title: "نرخ میانگین بازار برای این گروه کالا" }, "تخمینی") : h("span", { class: "pill ok" }, "شما"));
    };
    clear(
      out,
      h(
        "table",
        { class: "t profit-t" },
        h("thead", {}, h("tr", {}, ["کانال", "کارمزد ٪", "کارمزد", "ارزش افزوده", "سود خالص", "حاشیه واقعی", "کف قیمت (سربه‌سر)"].map((t) => h("th", {}, t)))),
        h(
          "tbody",
          {},
          r.rows.map((x, i) =>
            h(
              "tr",
              { class: i === 0 && input_.price && input_.cost ? "best-row" : "" },
              h("td", {}, h("b", {}, x.name)),
              h("td", {}, rate(x)),
              h("td", { class: "num" }, toman(x.commission)),
              h("td", { class: "num" }, toman(x.vat)),
              h("td", { class: `num ${x.net < 0 ? "bad-t" : "ok-t"}`.trim() }, input_.cost ? toman(x.net) : "—"),
              h("td", { class: `num ${x.marginPct < 0 ? "bad-t" : ""}`.trim() }, input_.cost ? `${fa(x.marginPct)}٪` : "—"),
              h("td", { class: "num" }, input_.cost ? toman(x.floorPrice) : "—"),
            ),
          ),
        ),
      ),
    );
  };
  for (const el of [priceIn, costIn, packIn, adIn]) el.addEventListener("change", () => draw());
  vatIn.addEventListener("change", () => draw());
  draw();
  return h(
    "div",
    { class: `card stack profit-card ${compact ? "compact" : ""}`.trim() },
    h("div", { class: "card-h" }, h("h2", {}, icon("chart", "sm"), title), h("span", { class: "small muted" }, "به تومان، برای هر عدد")),
    h("div", { class: "mkt-row2" }, field("بهای خرید / تمام‌شده", costIn), field("قیمت فروش", priceIn), field("بسته‌بندی + ارسال", packIn)),
    h("div", { class: "mkt-row2" }, field("هزینه تبلیغ برای هر فروش", adIn, "مثلاً کلیک ترب"), field("مالیات", vatIn)),
    out,
    note,
  );
}

/** A modal with the profit card for one product. */
export function profitModal({ name, price, cost, category }) {
  modal({ title: "سود روی هر کانال فروش", sub: name, wide: true, body: profitCard({ price, cost, category: category || name, title: "مقایسه کانال‌ها" }), actions: [{ label: "بستن" }] });
}

const BOX = {
  won: ["بای‌باکس با شماست", "ok"],
  lost: ["بای‌باکس را از دست داده‌اید", "bad"],
  not_listed: ["کالای شما در نتایج پیدا نشد", "warn"],
};

/** The buy-box state of a watched / catalogue item: status pill, the winner and the repricer's advice. */
export function buyboxLine(bb) {
  if (!bb?.status) return h("span", { class: "x small" }, "—");
  const [label, tone] = BOX[bb.status] || ["", ""];
  return h(
    "div",
    { class: "stack tight bb-line" },
    h("span", { class: `pill ${tone}`.trim() }, label),
    bb.status === "lost" && bb.winnerPrice ? h("span", { class: "small" }, `برنده: ${bb.winnerSeller || "—"} با ${toman(bb.winnerPrice)} تومان (قیمت شما ${toman(bb.myPrice)})`) : null,
    bb.advice ? h("span", { class: `small ${bb.status === "lost" && !bb.suggested ? "warn-t" : "muted"}`.trim() }, bb.advice) : null,
  );
}

/** «هشدار تلگرام / پیامک»: bot token + chat id, or an SMS panel and a number. */
export async function alertModal() {
  const v = await safe(invoke("alert_config", { config: null, token: null }));
  if (!v) return;
  const token = input({ type: "password", dir: "ltr", autocomplete: "off", placeholder: v.hasToken ? "ذخیره شده — برای تغییر، توکن جدید را بنویسید" : "123456:ABC-DEF…" });
  const chat = input({ dir: "ltr", value: v.telegramChat, placeholder: "شناسه عددی چت یا @نام_کانال" });
  const acc = select([["", "— بدون پیامک —"], ...v.smsAccounts], v.smsAccount);
  const to = input({ dir: "ltr", inputmode: "numeric", value: v.smsTo, placeholder: "09123456789" });
  const save = async () => {
    const r = await safe(invoke("alert_config", { config: { telegramChat: chat.value.trim(), smsAccount: acc.value, smsTo: to.value.trim() }, token: token.value.trim() || null }));
    if (r) token.value = "";
    return r;
  };
  const test = (channel) => async (e) => {
    const b = e.currentTarget;
    b.disabled = true;
    if (await save()) await safe(invoke("alert_test", { channel }), "پیام آزمایشی ارسال شد");
    b.disabled = false;
  };
  modal({
    title: "هشدار بای‌باکس و قیمت — تلگرام / پیامک",
    sub: "وقتی بای‌باکس یک کالا را از دست بدهید (یا پس بگیرید)، همین‌جا خبر می‌گیرید. در هر کالا کانال هشدار را انتخاب می‌کنید.",
    body: [
      h("p", { class: "small muted" }, "تلگرام: از @BotFather یک ربات بسازید، توکنش را اینجا بگذارید و به ربات پیام بدهید؛ شناسه چت خودتان را بنویسید. توکن رمز می‌شود و فقط روی همین ویندوز باز می‌شود."),
      field("توکن ربات تلگرام", token),
      field("شناسه چت (Chat ID)", chat),
      h("hr", { class: "sep" }),
      field("پنل پیامک", acc, v.smsAccounts.length ? "از «حساب‌های ارسال» در بخش ارسال؛ پیامک با همان پنل می‌رود." : "هنوز پنل پیامکی تعریف نشده است."),
      field("شماره گیرنده هشدار", to),
      h("div", { class: "row wrap" }, h("button", { class: "btn sm", type: "button", onclick: test("telegram") }, icon("send", "sm"), "آزمایش تلگرام"), h("button", { class: "btn sm", type: "button", onclick: test("sms") }, icon("send", "sm"), "آزمایش پیامک")),
    ],
    actions: [{ label: "انصراف" }, { label: "ذخیره", kind: "primary", onClick: async () => ((await save()) ? toast("تنظیمات هشدار ذخیره شد.", "ok") : false) }],
  });
}

/** Alert-channel options of an item. */
export const CHANNELS = [["", "فقط داخل برنامه"], ["telegram", "تلگرام"], ["sms", "پیامک"], ["both", "تلگرام و پیامک"]];

/** The fields that switch buy-box tracking on for an item (shared by the watch dialog and the catalogue). */
export function trackFields(v = {}) {
  const seller = input({ value: v.mySellerName || "", placeholder: "نام فروشگاه شما در دیجی‌کالا / باسلام / ترب" });
  const url = input({ dir: "ltr", value: v.myProductUrl || "", placeholder: "https://… (لینک کالای شما، اختیاری)" });
  const margin = input({ type: "number", min: 0, max: 90, dir: "ltr", value: v.minMarginPct ?? 10 });
  const channel = select(CHANNELS, v.alertChannel || "");
  return {
    el: h(
      "div",
      { class: "stack" },
      h("div", { class: "mkt-row2" }, field("نام فروشگاه شما", seller, "برای پیدا کردن کالای شما بین پیشنهادها"), field("لینک کالای شما", url)),
      h("div", { class: "mkt-row2" }, field("حداقل حاشیه سود (٪)", margin, "ریپرایسر هیچ‌وقت قیمت را زیر کف سود پیشنهاد نمی‌دهد"), field("هشدار از دست رفتن بای‌باکس", channel, "کانال‌ها را در «تنظیمات هشدار» وصل کنید")),
    ),
    value: () => ({ mySellerName: seller.value.trim(), myProductUrl: url.value.trim(), minMarginPct: Number(margin.value) || 0, alertChannel: channel.value }),
  };
}
