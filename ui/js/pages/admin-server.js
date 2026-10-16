// Admin: connect the panel to the server, and the step-by-step server set-up guide (for later).
import { h, icon, invoke, clear, safe, field, input, toast, confirmBox, copy } from "../core.js";
import { go } from "../state.js";

const cmd = (text) =>
  h("div", { class: "cmd" }, h("pre", { class: "code-box" }, text), h("button", { class: "btn sm ghost icon-only", type: "button", title: "کپی", "aria-label": "کپی", onclick: () => copy(text, "کپی شد") }, icon("copy", "sm")));

const step = (n, title, ...body) => h("div", { class: "step" }, h("span", { class: "step-n" }, n), h("div", { class: "grow stack tight" }, h("h3", {}, title), ...body));
const p = (t) => h("p", { class: "small" }, t);

const ENV = (seed, pass) => `HAMISTA_BIND=127.0.0.1:8080
HAMISTA_DATA_DIR=/opt/hamista/data
HAMISTA_TOKEN_TTL_HOURS=72
HAMISTA_ADMIN_USER=admin
HAMISTA_ADMIN_PASS=${pass}
HAMISTA_SIGNING_SEED=${seed}
HAMISTA_AGENT_MIN_VERSION=2.0.0`;

const SERVICE = `[Unit]
Description=Hamista Cloud
After=network.target

[Service]
User=hamista
WorkingDirectory=/opt/hamista
EnvironmentFile=/opt/hamista/hamista.env
ExecStart=/opt/hamista/hamista-cloud
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target`;

const NGINX = (domain) => `server {
    server_name ${domain};
    client_max_body_size 20m;
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header Host $host;
    }
}`;

function randomPass() {
  const a = new Uint8Array(12);
  crypto.getRandomValues(a);
  return [...a].map((b) => "abcdefghjkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789"[b % 55]).join("");
}

export async function render(root) {
  const conn = h("div", { class: "card stack license-card" });
  const domain = input({ dir: "ltr", class: "input mono", placeholder: "api.example.ir", value: "api.example.ir" });
  const envBox = h("div");
  const nginxBox = h("div");
  const certBox = h("div");
  const drawDomain = () => {
    const d = domain.value.trim() || "api.example.ir";
    clear(nginxBox, cmd(NGINX(d)));
    clear(certBox, cmd(`sudo apt install -y nginx certbot python3-certbot-nginx\nsudo nano /etc/nginx/sites-available/hamista   # متن بالا را بچسبانید\nsudo ln -s /etc/nginx/sites-available/hamista /etc/nginx/sites-enabled/\nsudo nginx -t && sudo systemctl reload nginx\nsudo certbot --nginx -d ${d}`));
  };
  domain.addEventListener("input", drawDomain);
  drawDomain();
  clear(envBox, cmd(ENV("‹کلید خصوصی پنل — دکمه پایین›", "‹یک رمز قوی›")));
  const fillEnv = async () => {
    if (!(await confirmBox("کلید خصوصی پنل در متن تنظیمات قرار می‌گیرد. این متن را فقط روی سرور خودتان بگذارید و جای دیگری نفرستید.", { ok: "نمایش تنظیمات", title: "تنظیمات سرور با کلید" }))) return;
    const seed = await safe(invoke("license_export_key"));
    if (seed) clear(envBox, cmd(ENV(seed, randomPass())));
  };

  async function drawConn() {
    const c = await invoke("admin_server").catch(() => ({ connected: false, url: "", user: "" }));
    if (c.connected) {
      clear(
        conn,
        h("div", { class: "card-h" }, h("h2", {}, "اتصال سرور"), h("span", { class: "pill ok" }, "متصل")),
        h(
          "dl",
          { class: "kv" },
          h("dt", {}, "آدرس"),
          h("dd", {}, h("span", { class: "ltr" }, c.url)),
          h("dt", {}, "حساب مدیر"),
          h("dd", {}, h("span", { class: "ltr" }, c.user)),
          h("dt", {}, "نسخه سرور"),
          h("dd", {}, c.serverVersion || "—"),
          h("dt", {}, "کلید امضا"),
          h("dd", {}, c.keyMatches === false ? h("span", { class: "pill bad" }, "با کلید پنل یکی نیست") : c.keyMatches ? h("span", { class: "pill ok" }, "همان کلید پنل") : "—"),
        ),
        c.keyMatches === false
          ? h("div", { class: "callout" }, icon("alert"), h("div", { class: "small" }, "سرور با کلید دیگری امضا می‌کند، پس برنامه مشتری حساب‌های آنلاین را نمی‌پذیرد. در فایل hamista.env سرور، HAMISTA_SIGNING_SEED را برابر کلید خصوصی همین پنل بگذارید (گام ۴ راهنما) و سرویس را restart کنید."))
          : null,
        h(
          "div",
          { class: "row wrap" },
          h("button", { class: "btn sm", type: "button", onclick: drawConn }, icon("refresh", "sm"), "بررسی دوباره"),
          h("button", { class: "btn sm", type: "button", onclick: () => go("accounts") }, icon("cpu", "sm"), "کاربران آنلاین"),
          h(
            "button",
            {
              class: "btn sm ghost",
              type: "button",
              onclick: async () => {
                if (!(await confirmBox("اتصال پنل به سرور قطع شود؟ سرور و مشتریان آنلاین بدون تغییر کار می‌کنند.", { ok: "قطع اتصال" }))) return;
                await safe(invoke("admin_disconnect"));
                drawConn();
              },
            },
            icon("logout", "sm"),
            "قطع اتصال",
          ),
        ),
      );
      return;
    }
    const url = input({ dir: "ltr", class: "input mono", placeholder: "https://api.example.ir", value: c.url || "" });
    const user = input({ dir: "ltr", value: c.user || "admin", autocomplete: "username" });
    const pass = input({ type: "password", dir: "ltr", autocomplete: "current-password" });
    const btn = h("button", { class: "btn primary", type: "button" }, icon("plug", "sm"), "اتصال");
    btn.addEventListener("click", async () => {
      if (!url.value.trim() || !pass.value) return toast("آدرس سرور و رمز مدیر را وارد کنید.", "bad");
      btn.disabled = true;
      const r = await safe(invoke("admin_connect", { url: url.value.trim(), username: user.value.trim(), password: pass.value }), "به سرور وصل شد");
      btn.disabled = false;
      if (r) drawConn();
    });
    pass.addEventListener("keydown", (e) => e.key === "Enter" && btn.click());
    clear(
      conn,
      h("div", { class: "card-h" }, h("h2", {}, "اتصال سرور"), h("span", { class: "pill" }, c.url ? "قطع" : "آفلاین")),
      p("تا وقتی سرور ندارید، پنل در حالت آفلاین کامل کار می‌کند (مشتریان و لایسنس آفلاین). بعد از راه‌اندازی سرور طبق راهنمای پایین، آدرس و حساب مدیر را اینجا وارد کنید."),
      field("آدرس سرور", url),
      h("div", { class: "grid-2" }, field("نام کاربری مدیر", user), field("رمز مدیر", pass)),
      h("div", { class: "row end" }, btn),
    );
  }

  const guide = h(
    "div",
    { class: "card stack guide" },
    h("div", { class: "card-h" }, h("h2", {}, "راهنمای راه‌اندازی سرور (برای وقتی سرور گرفتید)"), icon("book")),
    p("سرور فقط موتور پشت صحنه است و رابط کاربری ندارد؛ همه مدیریت از همین پنل انجام می‌شود. کل کار حدود یک ساعت است."),
    step(
      "۱",
      "تهیه سرور و دامنه",
      p("یک سرور مجازی لینوکس (Ubuntu 22.04 یا 24.04) با ۱ هسته و ۱ گیگابایت رم کافی است؛ سرور ایران سرعت بهتری برای مشتریان دارد. یک زیردامنه مثل api.yourdomain.ir بسازید و رکورد A آن را به IP سرور بدهید."),
      field("زیردامنه سرور شما", domain),
    ),
    step(
      "۲",
      "ساخت فایل اجرایی سرور",
      p("پوشه hamista-cloud همراه همین بسته است. روی خود سرور (یا هر لینوکسی) Rust را نصب و build کنید؛ خروجی یک فایل تنهاست."),
      cmd("sudo apt update && sudo apt install -y build-essential\ncurl https://sh.rustup.rs -sSf | sh -s -- -y\nsource \"$HOME/.cargo/env\"\ncd hamista-cloud && cargo build --release"),
    ),
    step(
      "۳",
      "نصب در مسیر سرویس",
      cmd("sudo useradd --system --home /opt/hamista hamista\nsudo mkdir -p /opt/hamista/data\nsudo cp target/release/hamista-cloud /opt/hamista/\nsudo chown -R hamista:hamista /opt/hamista\nsudo chmod 700 /opt/hamista/data"),
    ),
    step(
      "۴",
      "تنظیمات و کلید امضا",
      p("سرور باید با همان کلید این پنل لایسنس امضا کند تا یک نسخه مشتری هم کد آفلاین و هم حساب آنلاین را بپذیرد. با دکمه زیر متن کامل تنظیمات (همراه کلید و یک رمز تصادفی برای مدیر) ساخته می‌شود؛ آن را در فایل /opt/hamista/hamista.env بگذارید."),
      envBox,
      h("div", { class: "row wrap" }, h("button", { class: "btn sm", type: "button", onclick: fillEnv }, icon("key", "sm"), "ساخت تنظیمات با کلید این پنل")),
      cmd("sudo nano /opt/hamista/hamista.env     # متن بالا را بچسبانید\nsudo chmod 600 /opt/hamista/hamista.env\nsudo chown hamista:hamista /opt/hamista/hamista.env"),
    ),
    step(
      "۵",
      "سرویس دائمی (systemd)",
      p("فایل /etc/systemd/system/hamista-cloud.service را با این متن بسازید:"),
      cmd(SERVICE),
      cmd("sudo systemctl daemon-reload\nsudo systemctl enable --now hamista-cloud\ncurl http://127.0.0.1:8080/health    # باید ok برگرداند"),
    ),
    step("۶", "دامنه و HTTPS (Nginx + گواهی رایگان)", p("پیکربندی Nginx:"), nginxBox, certBox),
    step(
      "۷",
      "اتصال این پنل",
      p("در کادر «اتصال سرور» بالای همین صفحه، آدرس https://زیردامنه، نام کاربری admin و رمزی که در گام ۴ ساخته شد را وارد کنید. باید «همان کلید پنل» نمایش داده شود."),
    ),
    step(
      "۸",
      "پلن‌ها، کاربران و نسخه مشتری",
      p("از «پلن‌ها» اشتراک‌ها را تعریف کنید (تعداد دستگاه، سهمیه ماهانه، منابع، بانک جامع) و از «کاربران آنلاین» برای هر مشتری نام کاربری و رمز بسازید. مشتری در برنامه: تنظیمات ← لایسنس ← حساب آنلاین. اگر بخواهید آدرس سرور از قبل در برنامه مشتری باشد، آن را در فایل src-tauri/server.url بنویسید و نسخه مشتری را دوباره build کنید."),
      p("مشتریانی که کد آفلاین دارند بدون هیچ تغییری کار می‌کنند؛ هر وقت خواستید برایشان حساب آنلاین بسازید."),
    ),
    step(
      "۹",
      "پشتیبان‌گیری",
      p("همه اطلاعات سرور در یک فایل است. پشتیبان روزانه بگیرید و فایل را خارج از سرور نگه دارید:"),
      cmd("sudo apt install -y sqlite3\nsudo sqlite3 /opt/hamista/data/hamista-cloud.db \".backup '/opt/hamista/backup-$(date +%F).db'\""),
    ),
  );

  root.append(h("div", { class: "stack" }, conn, guide));
  await drawConn();
}
