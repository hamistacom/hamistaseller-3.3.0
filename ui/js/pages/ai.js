// «دستیار هوش مصنوعی»: Google's AI chat, opened outside the app in the user's own browser. The app only
// prepares the question (optionally with a summary of the latest product analysis) — nothing is sent
// anywhere by the app itself.
import { h, icon, invoke, safe, field } from "../core.js";
import { askGoogle, reportPrompt } from "./market.js";

const READY = [
  ["قیمت‌گذاری کالای من", "برای کالای زیر در بازار ایران قیمت فروش مناسب، حاشیه سود منطقی و استراتژی تخفیف پیشنهاد بده:\n"],
  ["تحلیل رقبا و برندها", "برندهای اصلی و رقبای این بازار در ایران را با هم مقایسه کن (قیمت، کیفیت، کانال فروش، نقطه قوت و ضعف):\n"],
  ["توضیح و عنوان محصول", "برای این کالا یک عنوان جذاب و سئوشده برای دیجی‌کالا و باسلام، و یک توضیح فروش کوتاه بنویس:\n"],
  ["ایده کمپین فروش", "پنج ایده کمپین فروش و تبلیغ کم‌هزینه برای این کالا در اینستاگرام، تلگرام و پیامک بده:\n"],
  ["پاسخ به مشتری", "یک پاسخ مؤدبانه و متقاعدکننده به مشتری‌ای بنویس که می‌گوید قیمت ما گران است:\n"],
];

export async function render(root) {
  const runs = await invoke("market_runs").catch(() => []);
  const last = runs[0] ? await invoke("market_get", { id: runs[0][0] }).catch(() => null) : null;
  const q = h("textarea", { class: "input ai-q", rows: 6, placeholder: "سؤال خود را بنویسید؛ مثلاً «برای فروش عسل گون در دیجی‌کالا چه قیمتی بگذارم؟»" });
  const withData = h("input", { type: "checkbox", checked: !!last, disabled: !last });
  const ask = () => {
    const text = q.value.trim();
    const data = withData.checked && last ? `${reportPrompt(last).split("\n")[0]}\n` : "";
    if (!text && !data) return q.focus();
    askGoogle(`${data}${text || "این بازار را تحلیل کن و پیشنهاد کاربردی بده."}`);
  };
  root.append(
    h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "card ai-hero" },
        h("div", { class: "ic-box lg" }, icon("sparkle")),
        h(
          "div",
          { class: "stack tight grow" },
          h("h2", {}, "دستیار هوش مصنوعی گوگل"),
          h("span", { class: "muted" }, "گفتگو در مرورگر شما و با حساب گوگل خودتان انجام می‌شود، بیرون از برنامه. برنامه فقط متن سؤال را آماده و کپی می‌کند."),
        ),
        h(
          "div",
          { class: "row wrap" },
          h("button", { class: "btn primary", type: "button", onclick: () => safe(invoke("open_url", { url: "https://gemini.google.com/app" })) }, icon("external", "sm"), "باز کردن Gemini"),
          h("button", { class: "btn", type: "button", onclick: () => safe(invoke("open_url", { url: "https://www.google.com/search?udm=50" })) }, icon("search", "sm"), "حالت هوش مصنوعی جستجوی گوگل"),
        ),
      ),
      h(
        "div",
        { class: "card stack" },
        h("div", { class: "card-h" }, h("h2", {}, "سؤال آماده کنید")),
        h("div", { class: "chips" }, READY.map(([t, p]) => h("button", { type: "button", class: "chip click", onclick: () => { q.value = p + (last ? `کالا: ${last.query}` : ""); q.focus(); } }, t))),
        field("سؤال", q),
        h("label", { class: "check" }, withData, h("span", {}, last ? `خلاصه آخرین تحلیل محصول («${last.query}») همراه سؤال فرستاده شود` : "هنوز تحلیل محصولی ندارید؛ بعد از اولین تحلیل، خلاصه‌اش را می‌توانید همراه سؤال بفرستید")),
        h("div", { class: "row wrap" }, h("button", { class: "btn primary", type: "button", onclick: ask }, icon("sparkle", "sm"), "پرسیدن از گوگل"), h("span", { class: "small muted" }, "سؤال در حالت هوش مصنوعی جستجوی گوگل باز و هم‌زمان کپی می‌شود تا در Gemini هم بچسبانید.")),
      ),
    ),
  );
}
