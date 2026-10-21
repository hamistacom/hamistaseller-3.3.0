// Settings: general / browser, sending accounts, templates, do-not-contact list, backups, licence, about.
import { h, icon, invoke, fa, clear, safe, select, field, input, toggle, toast, modal, confirmBox, copy, jdate, ago } from "../core.js";
import { app, refreshLicense, refreshLists } from "../state.js";
import { ACCOUNT_KINDS, SMS_PRESETS, PLACEHOLDERS } from "../data.js";
import { job, runJob } from "../jobs.js";

const TABS = [
  ["general", "عمومی"],
  ["accounts", "پنل پیامک"],
  ["templates", "قالب‌های پیام"],
  ["optouts", "لیست عدم ارسال"],
  ["modules", "ماژول‌ها"],
  ["backups", "پشتیبان‌گیری"],
  ["license", "لایسنس"],
  ["about", "درباره"],
];

export async function render(root, params) {
  let tab = params.tab || "general";
  const bar = h("div", { class: "tabs" });
  const body = h("div", { class: "stack" });
  let seq = 0;
  const draw = async () => {
    const my = ++seq;
    clear(bar, TABS.map(([k, l]) => h("button", { class: k === tab ? "on" : "", type: "button", onclick: () => { tab = k; draw(); } }, l)));
    const box = h("div", { class: "stack" }); // a fresh container per tab: a slow tab can't land in another
    await { general, accounts, templates, optouts, modules, backups, license, about }[tab](box);
    if (my === seq) clear(body, box);
  };
  root.append(h("div", { class: "stack" }, bar, body));
  await draw();
}

// ---------------- general ----------------

async function general(root) {
  const s = { browserPath: "", concurrency: 3, headless: true, delayMs: 400, expandAreas: true, ...(await invoke("settings_get")) };
  const save = (patch, msg = "ذخیره شد") => safe(invoke("settings_save", { patch }), msg);
  const path = input({ value: s.browserPath, dir: "ltr", placeholder: "خالی = شناسایی خودکار Edge / Chrome" });
  const detected = h("div", { class: "stack tight" });
  const detect = async () => {
    const p = await invoke("browser_detect", { custom: path.value }).catch(() => null);
    clear(
      detected,
      p
        ? [h("span", { class: "small muted" }, "مرورگر مورد استفاده:"), h("div", { class: "code-box" }, p)]
        : h("span", { class: "small bad-text" }, "Edge یا Chrome پیدا نشد؛ مسیر msedge.exe یا chrome.exe را وارد کنید."),
    );
  };
  path.addEventListener("change", () => save({ browserPath: path.value.trim() }).then(detect));
  detect();
  const tabs = select([1, 2, 3, 4, 5, 6, 8].map((n) => [n, `${fa(n)} تب`]), s.concurrency, { onchange: (e) => save({ concurrency: Number(e.target.value) }) });
  const delay = select([[200, "سریع"], [400, "معمولی (پیشنهادی)"], [900, "آرام — درخواست تأیید کمتر"]], s.delayMs, { onchange: (e) => save({ delayMs: Number(e.target.value) }) });
  const visible = toggle("نمایش پنجره مرورگر هنگام استخراج (برای عیب‌یابی)", !s.headless, { onchange: (e) => save({ headless: !e.target.checked }) });
  const expand = toggle("جستجو در محله‌ها و مناطق اطراف وقتی نتیجه کم است", s.expandAreas, { onchange: (e) => save({ expandAreas: e.target.checked }) });
  root.append(
    h(
      "div",
      { class: "grid-2" },
      h("div", { class: "card stack" }, h("h2", {}, "موتور استخراج"), field("مسیر مرورگر", path), detected, h("div", { class: "grid-2" }, field("تب‌های هم‌زمان", tabs), field("سرعت", delay)), visible.el, expand.el),
      h(
        "div",
        { class: "card stack" },
        h("h2", {}, "داده‌ها"),
        h("p", { class: "small muted" }, "همه داده‌ها در یک پوشه کنار برنامه ذخیره می‌شوند. برای انتقال به کامپیوتر دیگر، exe و این پوشه را با هم کپی کنید."),
        h("div", { class: "code-box" }, app.info.dataDir),
        h(
          "div",
          { class: "row wrap" },
          h("button", { class: "btn sm", type: "button", onclick: () => safe(invoke("reveal", { path: `${app.info.dataDir}\\hamista.db` })) }, icon("folder", "sm"), "باز کردن پوشه"),
          h(
            "button",
            {
              class: "btn sm",
              type: "button",
              onclick: async () => {
                const p = await safe(invoke("logs_folder"));
                if (p) safe(invoke("reveal", { path: `${p}\\hamista.log` }));
              },
            },
            icon("info", "sm"),
            "گزارش خطاها",
          ),
        ),
        h("p", { class: "small faint" }, "رمز حساب‌ها و کلیدها با رمزنگاری ویندوز (DPAPI) ذخیره می‌شوند و روی کامپیوتر دیگری باز نمی‌شوند؛ بعد از انتقال، رمز پنل پیامک را دوباره وارد کنید."),
      ),
    ),
  );
}

// ---------------- accounts ----------------

async function accounts(root) {
  const list = await invoke("accounts");
  const add = h("button", { class: "btn primary", type: "button", onclick: () => accountModal(null, () => accounts(clear(root))) }, icon("plus", "sm"), "افزودن پنل پیامک");
  root.append(
    h("div", { class: "row between" }, h("p", { class: "muted small" }, "پنل پیامک برای کمپین‌های خودکار. واتساپ، تلگرام، ایتا و بله با حساب شخصی خودتان در «دستیار گفتگو» وصل می‌شوند."), add),
    list.length
      ? h(
          "div",
          { class: "grid-3" },
          list.map((a) => {
            const k = ACCOUNT_KINDS[a.kind] || { label: "در این نسخه پشتیبانی نمی‌شود", icon: "info", old: true };
            return h(
              "div",
              { class: "card stack tight" },
              h("div", { class: "row" }, h("div", { class: "avatar sm-av" }, icon(k.icon)), h("div", { class: "grow" }, h("b", {}, a.name), h("div", { class: "small muted" }, k.label))),
              h("div", { class: "small muted" }, k.old ? "می‌توانید آن را حذف کنید." : `${fa(a.perMinute)} پیامک در دقیقه · ${a.hasSecret ? "رمز ذخیره شده" : "بدون رمز"}`),
              h(
                "div",
                { class: "row end" },
                h(
                  "button",
                  {
                    class: "btn sm danger",
                    type: "button",
                    onclick: async () => {
                      if (await confirmBox(`حساب «${a.name}» حذف شود؟`, { ok: "حذف", danger: true })) {
                        await safe(invoke("account_delete", { id: a.id }), "حذف شد");
                        accounts(clear(root));
                      }
                    },
                  },
                  icon("trash", "sm"),
                ),
                k.old ? null : h("button", { class: "btn sm", type: "button", onclick: () => accountModal(a, () => accounts(clear(root))) }, icon("edit", "sm"), "ویرایش"),
              ),
            );
          }),
        )
      : h("div", { class: "card" }, h("div", { class: "empty" }, icon("send", "lg"), "هنوز حسابی اضافه نشده است.")),
  );
}

const SECRET_KEYS = { sms: ["apiKey", "username", "password"] };

function accountModal(a, done) {
  const editing = !!a;
  const kind = "sms"; // the only sending account kind; messengers use the chat assistant
  const cfg = { ...(a?.config || {}) };
  const name = input({ value: a?.name || "" });
  const perMinute = input({ type: "number", min: 1, max: 1200, value: a?.perMinute || 30, dir: "ltr" });
  const formBox = h("div", { class: "stack" });
  const inputs = {}; // key → element; secret keys are read separately
  const secretHint = editing ? "ذخیره شده — برای تغییر، مقدار جدید را وارد کنید" : "";
  const sec = (key, label, ltr = true) => {
    inputs[key] = input({
      type: key.toLowerCase().includes("password") || key === "apiKey" || key === "token" ? "password" : "text",
      dir: ltr ? "ltr" : "rtl",
      placeholder: secretHint,
      autocomplete: "off",
    });
    return field(label, inputs[key]);
  };
  const pub = (key, label, value, props = {}) => {
    inputs[key] = input({ value: value ?? cfg[key] ?? "", dir: "ltr", ...props });
    return field(label, inputs[key]);
  };

  const drawForm = () => {
    for (const k of Object.keys(inputs)) delete inputs[k];
    if (kind === "sms") {
      const preset = select(SMS_PRESETS.map((p) => [p.preset, p.name]), cfg.preset || "kavenegar");
      const fieldsBox = h("div", { class: "grid-2" });
      const adv = h("div", { class: "stack" });
      const apply = (fill) => {
        const p = SMS_PRESETS.find((x) => x.preset === preset.value);
        if (fill) Object.assign(cfg, { preset: p.preset, method: p.method, url: p.url, bodyType: p.bodyType, body: p.body, headers: p.headers, okPath: p.okPath, okValues: p.okValues });
        if (!name.value || SMS_PRESETS.some((x) => x.name === name.value)) name.value = p.name;
        clear(
          fieldsBox,
          p.fields.includes("apikey") ? sec("apiKey", "کلید API (API Key)") : null,
          p.fields.includes("username") ? sec("username", "نام کاربری") : null,
          p.fields.includes("password") ? sec("password", "رمز عبور") : null,
          pub("sender", "شماره خط ارسال", cfg.sender),
        );
        clear(
          adv,
          p.hint ? h("div", { class: "callout info small" }, icon("info"), p.hint) : null,
          h(
            "div",
            { class: "grid-2" },
            field("روش", (inputs.method = select(["POST", "GET"], cfg.method || "POST"))),
            field("نوع بدنه", (inputs.bodyType = select([["json", "JSON"], ["form", "فرم"], ["none", "بدون بدنه"]], cfg.bodyType || "json"))),
          ),
          pub("url", "آدرس API", cfg.url),
          field("بدنه (JSON)", (inputs.body = h("textarea", { class: "input mono", rows: 4, dir: "ltr" }, JSON.stringify(cfg.body || {}, null, 1)))),
          field("هدرها (JSON)", (inputs.headers = h("textarea", { class: "input mono", rows: 2, dir: "ltr" }, JSON.stringify(cfg.headers || [])))),
          h("div", { class: "grid-2" }, pub("okPath", "مسیر نتیجه در پاسخ", cfg.okPath), pub("okValues", "مقادیر موفق (با ویرگول)", (cfg.okValues || []).join(","))),
          h("p", { class: "small faint" }, "جای‌نگهدارها: {apikey} {username} {password} {from} {to} {to98} {text}"),
        );
      };
      preset.addEventListener("change", () => apply(true));
      apply(!editing);
      clear(formBox, field("پنل", preset), fieldsBox, h("details", {}, h("summary", { class: "muted small" }, "تنظیمات پیشرفته API"), adv));
    }
  };
  drawForm();
  const rate = field("سرعت ارسال (پیامک در دقیقه)", perMinute, "طبق محدودیت پنل پیامک شما؛ ۳۰ تا ۶۰ معمول است.");
  modal({
    title: editing ? "ویرایش پنل پیامک" : "افزودن پنل پیامک",
    wide: true,
    body: [h("div", { class: "grid-2" }, field("نام حساب", name), rate), formBox],
    actions: [
      { label: "انصراف" },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () => {
          const config = { ...cfg };
          const secret = {};
          for (const [k, el] of Object.entries(inputs)) {
            const v = el.value.trim();
            if (SECRET_KEYS[kind].includes(k)) {
              if (v) secret[k] = v;
            } else config[k] = v;
          }
          if (kind === "sms") {
            try {
              config.body = JSON.parse(config.body || "{}");
              config.headers = JSON.parse(config.headers || "[]");
            } catch {
              toast("بدنه یا هدرها JSON معتبر نیستند.", "bad");
              return false;
            }
            config.okValues = String(config.okValues || "").split(",").map((s) => s.trim()).filter(Boolean);
          }
          const account = { id: a?.id || "", kind, name: name.value.trim() || ACCOUNT_KINDS[kind].label, config, perMinute: Number(perMinute.value) || 30 };
          // editing with every secret field empty keeps the stored secret; a secret is saved whole
          const keep = editing && !Object.keys(secret).length;
          const shown = Object.keys(inputs).filter((k) => SECRET_KEYS[kind].includes(k));
          if (editing && !keep && shown.some((k) => !secret[k])) {
            toast("برای تغییر رمزها، همه فیلدهای رمز این حساب را دوباره وارد کنید.", "bad");
            return false;
          }
          const r = await safe(invoke("account_save", { account, secret: keep ? null : secret }), "حساب ذخیره شد");
          if (r === undefined) return false;
          done?.();
        },
      },
    ],
  });
}

// ---------------- templates ----------------

async function templates(root) {
  const list = await invoke("templates");
  root.append(
    h(
      "div",
      { class: "row between" },
      h("p", { class: "muted" }, "متن‌های آماده برای کمپین‌ها و دستیار گفتگو. جای‌نگهدارها هنگام ارسال با اطلاعات هر مخاطب پر می‌شوند."),
      h("button", { class: "btn primary", type: "button", onclick: () => templateModal(null, () => templates(clear(root))) }, icon("plus", "sm"), "قالب جدید"),
    ),
    list.length
      ? h(
          "div",
          { class: "grid-3" },
          list.map((t) =>
            h(
              "div",
              { class: "card stack tight" },
              h("div", { class: "row" }, h("b", { class: "grow" }, t.name), h("span", { class: "pill" }, ACCOUNT_KINDS[t.channel]?.label || "همه")),
              h("p", { class: "small muted clamp-3" }, t.body),
              h(
                "div",
                { class: "row end" },
                h(
                  "button",
                  {
                    class: "btn sm danger",
                    type: "button",
                    onclick: async () => {
                      if (await confirmBox(`قالب «${t.name}» حذف شود؟`, { ok: "حذف", danger: true })) {
                        await safe(invoke("template_delete", { id: t.id }), "حذف شد");
                        templates(clear(root));
                      }
                    },
                  },
                  icon("trash", "sm"),
                ),
                h("button", { class: "btn sm", type: "button", onclick: () => templateModal(t, () => templates(clear(root))) }, icon("edit", "sm"), "ویرایش"),
              ),
            ),
          ),
        )
      : h("div", { class: "card" }, h("div", { class: "empty" }, icon("edit", "lg"), "هنوز قالبی ندارید.")),
  );
}

function templateModal(t, done) {
  const name = input({ value: t?.name || "" });
  const channel = select([["sms", "پیامک"], ["assist", "پیام‌رسان (دستیار گفتگو)"]], t?.channel || "sms");
  const subject = input({ value: t?.subject || "" });
  const body = h("textarea", { class: "input", rows: 7 }, t?.body || "");
  modal({
    title: t ? "ویرایش قالب" : "قالب جدید",
    wide: true,
    body: [
      h("div", { class: "grid-2" }, field("نام قالب", name), field("کانال", channel)),
      field("متن", body),
      h(
        "div",
        { class: "chips ph-chips" },
        PLACEHOLDERS.map(([ph, label]) =>
          h("span", { class: "chip", onclick: () => { body.setRangeText(ph, body.selectionStart, body.selectionEnd, "end"); body.focus(); } }, `+ ${label}`),
        ),
      ),
    ],
    actions: [
      { label: "انصراف" },
      {
        label: "ذخیره",
        kind: "primary",
        onClick: async () =>
          (await safe(
            invoke("template_save", { template: { id: t?.id || 0, channel: channel.value, name: name.value, subject: subject.value, body: body.value } }),
            "قالب ذخیره شد",
          )) === undefined
            ? false
            : done?.(),
      },
    ],
  });
}

// ---------------- do-not-contact ----------------

async function optouts(root) {
  const list = await invoke("optouts");
  const text = h("textarea", { class: "input mono", rows: 14, dir: "ltr", placeholder: "09121234567\ninfo@example.com" }, list.join("\n"));
  const count = h("span", { class: "pill" }, `${fa(list.length)} مورد`);
  root.append(
    h(
      "div",
      { class: "side-grid" },
      h(
        "div",
        { class: "card stack" },
        h("div", { class: "card-h" }, h("h2", {}, "لیست عدم ارسال"), count),
        text,
        h(
          "div",
          { class: "row end" },
          h(
            "button",
            {
              class: "btn primary",
              type: "button",
              onclick: async () => {
                const n = await safe(invoke("optouts_save", { text: text.value }), (k) => `${fa(k)} مورد ذخیره شد`);
                if (n !== undefined) count.textContent = `${fa(n)} مورد`;
              },
            },
            "ذخیره",
          ),
        ),
      ),
      h(
        "div",
        { class: "card stack" },
        h("h2", {}, "چرا مهم است؟"),
        h(
          "p",
          { class: "small muted" },
          "هر شماره‌ای که درخواست لغو داد را اینجا وارد کنید (هر کدام در یک خط). کمپین‌ها و دستیار گفتگو هیچ‌وقت به این‌ها پیام نمی‌فرستند. رعایت این لیست از مسدود شدن خط پیامک و حساب پیام‌رسان شما جلوگیری می‌کند.",
        ),
      ),
    ),
  );
}

// ---------------- backups ----------------

export async function backups(root) {
  const list = await invoke("backups");
  const when = (name) => {
    const m = name.match(/(\d{4})(\d{2})(\d{2})-?(\d{2})?(\d{2})?/);
    return m ? jdate(`${m[1]}-${m[2]}-${m[3]} ${m[4] || "00"}:${m[5] || "00"}:00`, true) : "";
  };
  const kind = (name) => (name.startsWith("auto") ? "خودکار روزانه" : name.startsWith("manual") ? "دستی" : "قبل از بازگردانی");
  root.append(
    h(
      "div",
      { class: "card stack" },
      h(
        "div",
        { class: "card-h" },
        h("h2", {}, "پشتیبان‌ها"),
        h(
          "button",
          {
            class: "btn primary",
            type: "button",
            onclick: async () => {
              if ((await safe(invoke("backup_now"), "پشتیبان گرفته شد")) !== undefined) backups(clear(root));
            },
          },
          icon("database", "sm"),
          "پشتیبان‌گیری الان",
        ),
      ),
      h("p", { class: "small muted" }, "هر روز یک پشتیبان خودکار گرفته می‌شود و ۱۰ نسخه آخر هر نوع نگه داشته می‌شود. برای امنیت بیشتر، گاهی پوشه backups را روی فلش یا فضای ابری کپی کنید."),
      list.length
        ? h(
            "div",
            { class: "feed" },
            list.map((b) =>
              h(
                "div",
                { class: "it" },
                h("div", { class: "grow" }, h("b", {}, when(b.name) || b.name), h("span", {}, `${kind(b.name)} · ${fa((b.size / 1024 / 1024).toFixed(1))} مگابایت`)),
                h(
                  "button",
                  {
                    class: "btn sm",
                    type: "button",
                    onclick: async () => {
                      if (
                        !(await confirmBox("همه داده‌های فعلی با این پشتیبان جایگزین می‌شوند. قبل از آن از وضعیت فعلی هم پشتیبان گرفته می‌شود.", {
                          ok: "بازگردانی",
                          danger: true,
                          title: "بازگردانی پشتیبان",
                        }))
                      )
                        return;
                      if ((await safe(invoke("backup_restore", { name: b.name }), "داده‌ها بازگردانی شد")) !== undefined) {
                        if (app.info.edition !== "issuer") await refreshLists();
                        backups(clear(root));
                      }
                    },
                  },
                  icon("refresh", "sm"),
                  "بازگردانی",
                ),
              ),
            ),
          )
        : h("div", { class: "empty small" }, "هنوز پشتیبانی وجود ندارد."),
    ),
  );
}

// ---------------- modules (data sources) ----------------

// settings each source needs (stored sealed on this PC; values are never shown again)
const MODULE_FIELDS = {
  neshan: {
    help: "کلید رایگان را از platform.neshan.org بسازید و سرویس‌های «جستجو» و «ژئوکدینگ» را برایش فعال کنید.",
    fields: [["apiKey", "کلید API نشان", "service.xxxxxxxx"]],
  },
  instagram: {
    help: "اختیاری. با حساب Business اینستاگرام خودتان (اپ متا، Graph API) بیو، سایت و تعداد دنبال‌کننده هر پیج کسب‌وکاری از API رسمی خوانده می‌شود و پیج‌های شخصی خودکار کنار می‌روند. بدون آن هم می‌توانید با دکمه «ورود در مرورگر» یا جستجوی وب استخراج کنید.",
    fields: [
      ["igUserId", "شناسه حساب Business (IG User ID)", "17841400000000000"],
      ["accessToken", "توکن دسترسی (Access Token)", "EAAB…"],
    ],
  },
};
const MODULE_NOTES = {
  maps: "کسب‌وکارها از منابع عمومی وب، با تلفن، آدرس، سایت و ساعت کاری.",
  divar: "استخراج خودکار شماره‌ها و فروشگاه‌ها از دیوار بر اساس شهر و منطقه با نشست مرورگر.",
  sheypoor: "استخراج خودکار شماره‌ها و فروشگاه‌ها از شیپور بر اساس شهر و منطقه با نشست مرورگر.",
  neshan: "فهرست کسب‌وکارهای نشان + یافتن شماره هر کدام در گوگل‌مپ.",
  balad: "صفحه‌های عمومی کسب‌وکارها در بلد، از طریق جستجوی وب.",
  instagram: "پیج‌های کسب‌وکاری، از طریق جستجوی وب و نشست مرورگر.",
  telegram: "کانال‌ها و گروه‌های عمومی با آیدی ادمین و راه تماس؛ حساب‌های شخصی کنار گذاشته می‌شوند.",
};
const LOGIN_SOURCES = new Set(["divar", "sheypoor", "instagram", "telegram"]);

function moduleConfig(m, redraw) {
  const spec = MODULE_FIELDS[m.id];
  if (!spec) return null;
  const inputs = spec.fields.map(([k, label, ph]) => [
    k,
    label,
    input({ type: "password", dir: "ltr", class: "input mono", placeholder: m.config?.[k] ? "•••••• (ذخیره شده)" : ph, autocomplete: "off" }),
  ]);
  const save = h("button", { class: "btn sm", type: "button" }, icon("check", "sm"), "ذخیره");
  save.addEventListener("click", async () => {
    const values = Object.fromEntries(inputs.filter(([, , i]) => i.value.trim()).map(([k, , i]) => [k, i.value.trim()]));
    if (!Object.keys(values).length) return toast("مقداری وارد نشده است.", "bad");
    if ((await safe(invoke("source_config_save", { id: m.id, values }), "ذخیره شد")) !== undefined) redraw();
  });
  const clearBtn = Object.values(m.config || {}).some(Boolean)
    ? h(
        "button",
        {
          class: "btn sm ghost",
          type: "button",
          onclick: async () => {
            if (!(await confirmBox(`تنظیمات «${m.name}» پاک شود؟`, { ok: "پاک کن", danger: true }))) return;
            const values = Object.fromEntries(spec.fields.map(([k]) => [k, null]));
            if ((await safe(invoke("source_config_save", { id: m.id, values }), "پاک شد")) !== undefined) redraw();
          },
        },
        "پاک کردن",
      )
    : null;
  return h(
    "details",
    { class: "module-cfg", open: !Object.values(m.config || {}).some(Boolean) && m.id === "neshan" },
    h("summary", { class: "small" }, Object.values(m.config || {}).every(Boolean) ? "✓ تنظیم شده — تغییر" : "تنظیمات API"),
    h("div", { class: "stack tight adv-body" }, h("p", { class: "small muted" }, spec.help), inputs.map(([, label, i]) => field(label, i)), h("div", { class: "row" }, save, clearBtn)),
  );
}

async function modules(root) {
  const list = await invoke("sources_list");
  const KIND = { scan: "یافتن کسب‌وکار", enrich: "تکمیل اطلاعات", import: "داده‌های خودتان" };
  const redraw = () => modules(clear(root));
  const openLogin = async (m) => {
    if (job.running) return toast("ابتدا عملیات در حال اجرا را متوقف کنید.", "bad");
    const listId = app.lists[0]?.id || 1;
    await runJob("scan", `ورود به حساب ${m.name}`, "scan_start", {
      req: { listId, source: m.id, keywords: ["__login__"], province: "تهران", city: "تهران", limit: 1, enrich: false },
    });
  };
  root.append(
    h(
      "div",
      { class: "card stack" },
      h("div", { class: "card-h" }, h("h2", {}, "ماژول‌های منبع داده"), h("span", { class: "muted small" }, "ماژول خاموش در صفحه «استخراج» نمایش داده نمی‌شود و اجرا نمی‌شود")),
      h(
        "div",
        { class: "grid-3 modules" },
        list.map((m) => {
          const on = m.enabled !== false;
          const sw = m.switchable
            ? toggle(on ? "روشن" : "خاموش", on, {
                "aria-label": `روشن یا خاموش کردن ${m.name}`,
                onchange: async (e) => {
                  const want = e.target.checked;
                  if ((await safe(invoke("source_toggle", { id: m.id, on: want }), want ? `«${m.name}» روشن شد` : `«${m.name}» خاموش شد`)) === undefined) e.target.checked = !want;
                  redraw();
                },
              })
            : null;
          const status = !m.allowed ? h("span", { class: "pill warn" }, "خارج از اشتراک") : !on ? h("span", { class: "pill" }, "خاموش") : h("span", { class: "pill ok" }, "فعال");
          const loginBtn =
            on && LOGIN_SOURCES.has(m.id)
              ? h("button", { class: "btn sm tonal", type: "button", onclick: () => openLogin(m) }, icon("user", "sm"), "ورود در مرورگر")
              : null;
          return h(
            "div",
            { class: `module ${on ? "" : "off"}`.trim() },
            h("div", { class: "row" }, h("span", { class: `src-ic c-${m.id}` }, icon(m.icon)), h("b", { class: "grow" }, m.name), status),
            h("p", { class: "small muted" }, m.desc),
            h(
              "div",
              { class: "row wrap small" },
              h("span", { class: "faint grow" }, [KIND[m.kind], MODULE_NOTES[m.id]].filter(Boolean).join(" · ")),
              m.kind === "scan" ? h("span", { class: "pill info" }, m.places ? `${fa(m.places)} مخاطب · ${fa(m.recent)} در ۳۰ روز` : "هنوز مخاطبی نیاورده") : null,
            ),
            sw || loginBtn ? h("div", { class: "row between wrap" }, sw ? sw.el : h("span"), loginBtn) : null,
            on ? moduleConfig(m, redraw) : null,
          );
        }),
      ),
      h("p", { class: "small faint" }, "همه منابع فقط اطلاعات کسب‌وکارها و آگهی‌های عمومی را جمع می‌کنند."),
    ),
  );
}

// ---------------- licence (hybrid: offline code or online account) ----------------

const DATA_NOTE =
  "در حالت آنلاین، اطلاعات عمومی کسب‌وکارهایی که برنامه از منابع عمومی پیدا می‌کند — نام، شماره‌ها، ایمیل، سایت، شبکه‌ها، آدرس، استان، شهر و حوزه کاری — به بانک جامع هامیستا ارسال می‌شود. یادداشت‌ها، وضعیت پیگیری، برچسب‌ها، شماره‌هایی که خودتان وارد کرده‌اید، فایل‌های واردشده و سابقه کمپین‌های شما هرگز ارسال نمی‌شوند.";
const left = (n) => (n < 0 ? "نامحدود" : fa(n));

function statusRows(l) {
  const rows = [
    ["وضعیت", l.valid ? h("span", { class: "pill ok" }, l.mode === "online" ? "فعال — حساب آنلاین" : "فعال — کد آفلاین") : h("span", { class: "pill bad" }, l.message || "غیرفعال")],
    ["مشتری", l.customer || "—"],
    ["نوع", l.plan || "—"],
    ["اعتبار تا", l.expires ? `${jdate(l.expires)} (${fa(l.daysLeft)} روز مانده)` : "—"],
  ];
  if (l.mode === "online" && l.user) {
    rows.push(
      ["حساب", h("span", { class: "ltr" }, l.user)],
      ["سهمیه این ماه", `استخراج: ${left(l.scanLeft)} · ارسال: ${left(l.sendLeft)}`],
      ["بانک جامع", l.megabank ? "فعال" : "در اشتراک شما نیست"],
      ["آخرین همگام‌سازی", l.syncedAt ? ago(new Date(l.syncedAt * 1000).toISOString()) : "—"],
    );
  }
  return h("dl", { class: "kv" }, rows.map(([k, v]) => [h("dt", {}, k), h("dd", {}, v)]));
}

function machineRow(l) {
  return h(
    "div",
    { class: "stack tight" },
    h("span", { class: "label-t" }, "کد این سیستم"),
    h("div", { class: "row" }, h("span", { class: "machine grow" }, l.machine), h("button", { class: "btn sm", type: "button", onclick: () => copy(l.machine, "کد سیستم کپی شد") }, icon("copy", "sm"), "کپی")),
  );
}

function onlineForm(l, done) {
  const url = input({ dir: "ltr", class: "input mono", placeholder: "https://api.example.ir", value: l.server || app.info.defaultServer || "" });
  const user = input({ dir: "ltr", autocomplete: "username", value: l.user || "" });
  const pass = input({ type: "password", dir: "ltr", autocomplete: "current-password" });
  const agree = toggle("شرایط استفاده و ارسال اطلاعات عمومی کسب‌وکارها به بانک جامع را می‌پذیرم", false);
  const go = h("button", { class: "btn primary", type: "button" }, icon("plug", "sm"), "ورود و فعال‌سازی");
  go.addEventListener("click", async () => {
    if (!url.value.trim() || !user.value.trim() || !pass.value) return toast("آدرس سرور، نام کاربری و رمز را وارد کنید.", "bad");
    if (!agree.input.checked) return toast("برای حساب آنلاین، پذیرش شرایط لازم است.", "bad");
    go.disabled = true;
    const st = await safe(invoke("cloud_login", { url: url.value.trim(), username: user.value.trim(), password: pass.value }), "با حساب آنلاین فعال شد");
    go.disabled = false;
    if (st) {
      await refreshLicense();
      done?.();
    }
  });
  pass.addEventListener("keydown", (e) => e.key === "Enter" && go.click());
  const trialBtn = h("button", { class: "btn ghost", type: "button" }, icon("sparkle", "sm"), "دوره آزمایشی رایگان");
  trialBtn.addEventListener("click", async () => {
    const server = url.value.trim() || app.info.defaultServer;
    if (!server) return toast("ابتدا آدرس سرور را وارد کنید.", "bad");
    trialBtn.disabled = true;
    const info = await safe(invoke("cloud_trial_info", { url: server }));
    trialBtn.disabled = false;
    if (!info) return;
    if (!info.enabled) return toast("دوره آزمایشی در حال حاضر ارائه نمی‌شود؛ با پشتیبانی هامیستا تماس بگیرید.", "bad");
    const name = input({ placeholder: "نام شما یا کسب‌وکار" });
    const phone = input({ dir: "ltr", placeholder: "09xxxxxxxxx", inputmode: "numeric" });
    const ok = toggle("شرایط استفاده و ارسال اطلاعات عمومی کسب‌وکارها به بانک جامع را می‌پذیرم", false);
    modal({
      title: `دوره آزمایشی ${fa(info.days)} روزه`,
      sub: "هر سیستم فقط یک بار می‌تواند دوره آزمایشی بگیرد. بعد از پایان دوره، داده‌های شما روی همین کامپیوتر می‌ماند.",
      body: [h("div", { class: "grid-2" }, field("نام", name), field("شماره موبایل", phone)), h("div", { class: "callout info" }, icon("info"), h("div", { class: "small" }, DATA_NOTE)), ok.el],
      actions: [
        { label: "انصراف" },
        {
          label: "شروع دوره آزمایشی",
          kind: "primary",
          onClick: async () => {
            if (!ok.input.checked) {
              toast("پذیرش شرایط لازم است.", "bad");
              return false;
            }
            const st = await safe(invoke("cloud_trial", { url: server, name: name.value, phone: phone.value }), "دوره آزمایشی فعال شد");
            if (!st) return false;
            await refreshLicense();
            done?.();
          },
        },
      ],
    });
  });
  return h(
    "div",
    { class: "stack" },
    h("p", { class: "small muted" }, "نام کاربری و رمز را از پشتیبانی هامیستا دریافت کنید. بعد از ورود، برنامه هر چند ساعت یک بار خودکار با سرور همگام می‌شود و تا ۷۲ ساعت بدون اینترنت هم کار می‌کند."),
    app.info.defaultServer ? null : field("آدرس سرور", url),
    h("div", { class: "grid-2" }, field("نام کاربری", user), field("رمز عبور", pass)),
    h("div", { class: "callout info" }, icon("info"), h("div", { class: "small" }, DATA_NOTE)),
    agree.el,
    h("div", { class: "row between" }, trialBtn, go),
  );
}

function offlineForm(l, done) {
  const code = h("textarea", { class: "input mono", rows: 3, dir: "ltr", placeholder: "HS2.…" });
  const activate = h("button", { class: "btn primary", type: "button" }, icon("key", "sm"), l.valid && l.mode === "offline" ? "جایگزینی لایسنس (تمدید)" : "فعال‌سازی");
  activate.addEventListener("click", async () => {
    if (!code.value.trim()) return toast("کد لایسنس را بچسبانید.", "bad");
    activate.disabled = true;
    const st = await safe(invoke("license_activate", { code: code.value.trim() }), "برنامه با موفقیت فعال شد");
    activate.disabled = false;
    if (st) {
      await refreshLicense();
      done?.();
    }
  });
  return h(
    "div",
    { class: "stack" },
    machineRow(l),
    h("p", { class: "small muted" }, "این کد را برای پشتیبانی هامیستا (تلگرام @hamistaa) بفرستید. کد لایسنسی که دریافت می‌کنید را اینجا بچسبانید؛ فعال‌سازی بدون اینترنت انجام می‌شود."),
    field("کد لایسنس", code),
    h("div", { class: "row end" }, activate),
  );
}

function onlineActive(l, done) {
  const sync = h("button", { class: "btn", type: "button" }, icon("refresh", "sm"), "همگام‌سازی");
  sync.addEventListener("click", async () => {
    sync.disabled = true;
    const st = await safe(invoke("cloud_sync"), "همگام‌سازی انجام شد");
    sync.disabled = false;
    if (st) {
      await refreshLicense();
      done?.();
    }
  });
  const out = h("button", { class: "btn ghost", type: "button" }, icon("logout", "sm"), "خروج از حساب");
  out.addEventListener("click", async () => {
    if (!(await confirmBox("با خروج، استخراج و ارسال تا ورود دوباره (یا وارد کردن کد آفلاین) غیرفعال می‌شود. داده‌های شما روی همین کامپیوتر می‌ماند.", { ok: "خروج", danger: true }))) return;
    await safe(invoke("cloud_logout"));
    await refreshLicense();
    done?.();
  });
  return h(
    "div",
    { class: "stack" },
    h("div", { class: "row wrap" }, h("span", { class: "small muted grow" }, h("span", {}, "سرور: "), h("span", { class: "ltr" }, l.server)), sync, out),
    h("p", { class: "small faint" }, DATA_NOTE),
  );
}

function licenseBody(l, done) {
  const box = h("div", { class: "stack" });
  const onlineOn = l.mode === "online" && (l.valid || l.user);
  let tab = onlineOn ? "online" : app.info.defaultServer ? "online" : "offline";
  const tabs = h("div", { class: "tabs" });
  const draw = () => {
    clear(tabs, [["online", "حساب آنلاین"], ["offline", "کد آفلاین"]].map(([k, t]) => h("button", { class: k === tab ? "on" : "", type: "button", onclick: () => { tab = k; draw(); } }, t)));
    clear(box, tab === "online" ? (onlineOn ? onlineActive(l, done) : onlineForm(l, done)) : offlineForm(l, done));
  };
  draw();
  return [statusRows(l), tabs, box];
}

export async function licenseModal() {
  const l = await refreshLicense().catch(() => null);
  if (l?.edition !== "customer") return;
  let m;
  m = modal({ title: "لایسنس هامیستا سلر", body: licenseBody(l, () => m.close()) });
}

async function license(root) {
  const l = await refreshLicense();
  if (l.edition !== "customer") {
    root.append(h("div", { class: "card stack" }, h("h2", {}, "لایسنس"), h("div", { class: "callout ok" }, icon("shield"), l.message)));
    return;
  }
  root.append(h("div", { class: "card stack license-card" }, h("h2", {}, "لایسنس"), licenseBody(l, () => license(clear(root)))));
}

// ---------------- about ----------------

async function about(root) {
  const link = (url, label) => h("button", { class: "link", type: "button", onclick: () => safe(invoke("open_url", { url })) }, label);
  root.append(
    h(
      "div",
      { class: "card stack about" },
      h("img", { src: "brand/logo-wide.png", alt: "هامیستا سلر", class: "about-logo" }),
      h("h2", {}, `هامیستا سلر — نسخه ${app.info.version}`),
      h("p", { class: "muted" }, "سامانه استخراج اطلاعات مشتریان و کسب و کار ها"),
      h("dl", { class: "kv" }, h("dt", {}, "سازنده"), h("dd", {}, "تیم هامیستا (Hamista Creative Agency)"), h("dt", {}, "وب‌سایت"), h("dd", {}, link("https://hamista.ir", "hamista.ir")), h("dt", {}, "پشتیبانی"), h("dd", {}, link("https://t.me/hamistaa", "تلگرام ‎@hamistaa")), h("dt", {}, "نسخه"), h("dd", {}, { master: "نسخه کامل", customer: "نسخه مشتری", issuer: "پنل مدیر" }[app.info.edition])),
      h("p", { class: "small faint" }, "استفاده از داده‌ها باید مطابق قوانین و با احترام به حریم خصوصی کسب‌وکارها باشد؛ به درخواست لغو مخاطبان احترام بگذارید."),
      app.info.edition === "customer" ? h("div", { class: "callout info" }, icon("info"), h("div", { class: "small" }, h("b", {}, "درباره داده‌ها: "), DATA_NOTE)) : null,
    ),
  );
}