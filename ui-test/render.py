"""Renders every page of the UI with a mocked backend, at 3 window sizes x 2 themes, under the app's CSP.
Reports console errors / CSP violations / exceptions and saves screenshots.  python3 ui-test/render.py OUT_DIR"""
import http.server, threading, sys, os, functools, json
from playwright.sync_api import sync_playwright

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "ui")
MOCK = os.path.join(os.path.dirname(os.path.abspath(__file__)), "mock.js")
OUT = sys.argv[1] if len(sys.argv) > 1 else "/tmp/ui-shots"
os.makedirs(OUT, exist_ok=True)
CSP = "default-src 'self' ipc: http://ipc.localhost; style-src 'self'; img-src 'self' data: https:; font-src 'self'; script-src 'self'"


class H(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Content-Security-Policy", CSP)
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, *a):
        pass


srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(H, directory=ROOT))
threading.Thread(target=srv.serve_forever, daemon=True).start()
BASE = f"http://127.0.0.1:{srv.server_address[1]}/index.html"
SRC = {}
PAGES = ["dashboard", "scan", "schedule", "contacts", "outreach", "assist", "market", "plist", "market-runs", "watch", "catalog", "ai", "brands", "history", "help", "settings"]
SIZES = [(1100, 680), (1366, 768), (1920, 1080)]
problems = []


def page_for(b, w, hgt, theme, query=""):
    ctx = b.new_context(viewport={"width": w, "height": hgt}, device_scale_factor=1)
    ctx.add_init_script(path=MOCK)
    ctx.add_init_script(f"try{{localStorage.setItem('theme','{theme}')}}catch(e){{}}")
    p = ctx.new_page()
    tag = f"{w}x{hgt}-{theme}{('-' + query) if query else ''}"
    p.on("console", lambda m: m.type in ("error", "warning") and problems.append(f"[{tag}] console.{m.type}: {m.text}"))
    p.on("pageerror", lambda e: problems.append(f"[{tag}] pageerror: {e}"))
    return ctx, p


def overflow_check(p, tag):
    # elements wider than their scroll container (horizontal page overflow)
    bad = p.evaluate("""() => {
      const v = document.querySelector('#view');
      const out = [];
      if (v && v.scrollWidth > v.clientWidth + 1) out.push('view scrolls horizontally: ' + v.scrollWidth + ' > ' + v.clientWidth);
      if (document.documentElement.scrollWidth > innerWidth + 1) out.push('document wider than window');
      for (const d of document.querySelectorAll('dialog[open]')) { const r = d.getBoundingClientRect(); if (r.height > innerHeight || r.width > innerWidth) out.push('dialog larger than window'); }
      return out;
    }""")
    for x in bad:
        problems.append(f"[{tag}] layout: {x}")


with sync_playwright() as pw:
    b = pw.chromium.launch(executable_path="/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args=["--no-sandbox"])
    for (w, hgt) in SIZES:
        for theme in ("light", "dark"):
            ctx, p = page_for(b, w, hgt, theme)
            for name in PAGES:
                p.goto(f"{BASE}#/{name}")
                p.wait_for_timeout(450)
                overflow_check(p, f"{w}x{hgt}-{theme}-{name}")
                p.screenshot(path=f"{OUT}/{name}-{w}-{theme}.png")
            ctx.close()

    # interactions (1366, light + dark)
    for theme in ("light", "dark"):
        ctx, p = page_for(b, 1366, 768, theme)
        p.goto(f"{BASE}#/contacts")
        p.wait_for_timeout(400)
        p.click(".name-cell .n >> nth=1")
        p.wait_for_timeout(300)
        overflow_check(p, f"card-{theme}")
        p.screenshot(path=f"{OUT}/x-card-{theme}.png")
        p.keyboard.press("Escape")
        p.click("tbody input[type=checkbox] >> nth=0")
        p.click("tbody input[type=checkbox] >> nth=2")
        p.wait_for_timeout(200)
        p.screenshot(path=f"{OUT}/x-bulk-{theme}.png")
        p.click("text=ویرایش >> nth=0") if False else None
        p.goto(f"{BASE}#/settings?tab=accounts")
        p.wait_for_timeout(300)
        p.click("text=افزودن پنل پیامک")
        p.wait_for_timeout(300)
        overflow_check(p, f"account-modal-{theme}")
        p.screenshot(path=f"{OUT}/x-account-{theme}.png")
        p.keyboard.press("Escape")
        p.goto(f"{BASE}#/settings?tab=license")
        p.wait_for_timeout(300)
        p.screenshot(path=f"{OUT}/x-license-master-{theme}.png")
        ctx.close()

    # bank analysis by province / county / line of business, then drill into one province
    for theme in ("light", "dark"):
        ctx, p = page_for(b, 1366, 768, theme)
        p.goto(f"{BASE}#/contacts?view=stats")
        p.wait_for_timeout(500)
        overflow_check(p, f"stats-{theme}")
        p.screenshot(path=f"{OUT}/x-stats-{theme}.png", full_page=False)
        p.click("text=به تفکیک استان >> xpath=../.. >> button.link >> nth=0")
        p.wait_for_timeout(400)
        p.screenshot(path=f"{OUT}/x-stats-drill-{theme}.png")
        ctx.close()
    ctx, p = page_for(b, 1100, 680, "light")
    p.goto(f"{BASE}#/contacts?view=stats")
    p.wait_for_timeout(500)
    overflow_check(p, "stats-1100")
    p.screenshot(path=f"{OUT}/x-stats-1100.png")
    ctx.close()

    # live scan with captcha, held running
    ctx, p = page_for(b, 1366, 768, "light", "captcha=1&hold=1")
    p.goto(f"{BASE}?captcha=1&hold=1#/scan")
    p.wait_for_timeout(400)
    p.fill(".tagbox input", "کافه")
    p.keyboard.press("Enter")
    p.select_option("select >> nth=1", "تهران")
    p.click("text=شروع استخراج")
    p.wait_for_timeout(2200)
    overflow_check(p, "scan-live")
    p.screenshot(path=f"{OUT}/x-scan-live.png")
    p.goto(f"{BASE}?captcha=1&hold=1#/dashboard")
    p.wait_for_timeout(400)
    ctx.close()

    # campaign running
    ctx, p = page_for(b, 1366, 768, "dark", "hold=1")
    p.goto(f"{BASE}?hold=1#/outreach")
    p.wait_for_timeout(500)
    p.fill("textarea", "سلام {person}، پیشنهاد ویژه برای {name}")
    p.wait_for_timeout(500)
    p.screenshot(path=f"{OUT}/x-campaign-form.png", full_page=True)
    p.click("text=شروع ارسال")
    p.wait_for_timeout(300)
    p.click("dialog >> text=شروع ارسال")
    p.wait_for_timeout(1200)
    p.screenshot(path=f"{OUT}/x-campaign-live.png")
    ctx.close()

    # market report: exercise the new tabs (exact-link badges, suppliers, web sources)
    for theme in ("light", "dark"):
        ctx, p = page_for(b, 1440, 900, theme, "market-report")
        p.goto(f"{BASE}#/market?run=7")
        p.wait_for_timeout(700)
        overflow_check(p, f"market-report-{theme}")
        for tab in ("محصولات", "عمده‌فروشان", "منابع وب", "برندها"):
            try:
                p.click(f".rep-tabs >> text={tab}", timeout=1500)
                p.wait_for_timeout(300)
                overflow_check(p, f"market-{tab}-{theme}")
            except Exception:
                pass
        p.screenshot(path=f"{OUT}/x-market-report-{theme}.png", full_page=True)
        ctx.close()

    # B2B / RFQ modal
    ctx, p = page_for(b, 1366, 768, "light", "rfq")
    p.goto(f"{BASE}#/market?run=7")
    p.wait_for_timeout(600)
    try:
        p.click("text=عمده‌فروشان", timeout=1500)
        p.wait_for_timeout(200)
        p.click("text=ساخت درخواست استعلام", timeout=1500)
        p.wait_for_timeout(300)
        overflow_check(p, "rfq-modal")
        p.screenshot(path=f"{OUT}/x-rfq.png")
    except Exception:
        pass
    ctx.close()

    # customer: expiring soon, and not activated (modal opens by itself)
    for q in ("edition=customer", "edition=customer&lic=invalid"):
        ctx, p = page_for(b, 1366, 768, "light", q)
        p.goto(f"{BASE}?{q}#/dashboard")
        p.wait_for_timeout(600)
        overflow_check(p, q)
        p.screenshot(path=f"{OUT}/x-{q.replace('&', '_').replace('=', '-')}.png")
        ctx.close()

    # issuer edition
    for theme in ("light", "dark"):
        ctx, p = page_for(b, 1366, 768, theme, "edition=issuer")
        p.goto(f"{BASE}?edition=issuer#/issuer")
        p.wait_for_timeout(500)
        p.fill("input.mono", "3f2504e04f8911d3")
        p.fill("input[placeholder='نام مشتری یا شرکت']", "فروشگاه نمونه")
        p.click("text=صدور لایسنس >> nth=-1")
        p.wait_for_timeout(400)
        overflow_check(p, f"issuer-{theme}")
        p.screenshot(path=f"{OUT}/x-issuer-{theme}.png")
        ctx.close()

    # ---------- phase 5: admin panel (offline + connected), online customer ----------
    ADMIN_PAGES = ["home", "customers", "issuer", "accounts", "plans", "mega", "notify", "logs", "server", "keys"]
    for q, theme in (("edition=issuer", "light"), ("edition=issuer&srv=1", "light"), ("edition=issuer&srv=1", "dark")):
        for (w, hgt) in ((1100, 680), (1366, 768)):
            ctx, p = page_for(b, w, hgt, theme, q)
            for name in ADMIN_PAGES:
                p.goto(f"{BASE}?{q}#/{name}")
                p.wait_for_timeout(450)
                tag = f"{q}-{name}-{w}-{theme}"
                overflow_check(p, tag)
                p.screenshot(path=f"{OUT}/a-{tag.replace('&', '_').replace('=', '-')}.png")
            ctx.close()
    # admin modals
    ctx, p = page_for(b, 1366, 768, "light", "edition=issuer&srv=1")
    for page, button, shot in (("customers", "صدور لایسنس", "issue"), ("customers", "مشتری جدید", "customer"), ("accounts", "کاربر جدید", "user"), ("plans", "پلن جدید", "plan")):
        p.goto(f"{BASE}?edition=issuer&srv=1#/{page}")
        p.wait_for_timeout(450)
        p.click(f"#page-actions >> text={button}")
        p.wait_for_timeout(350)
        overflow_check(p, f"modal-{shot}")
        p.screenshot(path=f"{OUT}/m-{shot}.png")
        p.keyboard.press("Escape")
    p.goto(f"{BASE}?edition=issuer&srv=1#/customers")
    p.wait_for_timeout(400)
    p.click("#page-actions >> text=صدور لایسنس")
    p.wait_for_timeout(300)
    p.fill("dialog input.mono", "3f2504e04f8911d3")
    p.click("dialog >> text=صدور لایسنس >> nth=-1")
    p.wait_for_timeout(400)
    p.screenshot(path=f"{OUT}/m-issued.png")
    ctx.close()

    # customer: online account, mega-bank, notifications, licence tabs, modules
    for theme in ("light", "dark"):
        q = "edition=customer&online=1"
        ctx, p = page_for(b, 1366, 768, theme, q)
        for name in ("dashboard", "megabank", "settings?tab=license", "settings?tab=modules", "settings?tab=about"):
            p.goto(f"{BASE}?{q}#/{name}")
            p.wait_for_timeout(500)
            tag = f"online-{name.replace('?tab=', '-')}-{theme}"
            overflow_check(p, tag)
            p.screenshot(path=f"{OUT}/c-{tag}.png")
        p.goto(f"{BASE}?{q}#/megabank")
        p.wait_for_timeout(500)
        p.click("tbody input[type=checkbox] >> nth=0")
        p.click("tbody input[type=checkbox] >> nth=3")
        p.click("text=افزودن ۲ مورد به بانک")
        p.wait_for_timeout(300)
        p.click("#bell")
        p.wait_for_timeout(300)
        overflow_check(p, f"bell-{theme}")
        p.screenshot(path=f"{OUT}/c-bell-{theme}.png")
        ctx.close()
    ctx, p = page_for(b, 1100, 680, "light", "edition=customer&lic=invalid")
    p.goto(f"{BASE}?edition=customer&lic=invalid#/dashboard")
    p.wait_for_timeout(600)
    p.click("dialog .tabs >> text=کد آفلاین")
    p.wait_for_timeout(200)
    p.screenshot(path=f"{OUT}/c-license-offline-tab.png")
    p.click("dialog .tabs >> text=حساب آنلاین")
    p.wait_for_timeout(200)
    overflow_check(p, "license-online-tab-1100")
    p.screenshot(path=f"{OUT}/c-license-online-tab.png")
    ctx.close()

    # sources: scan page per source, module settings
    for theme in ("light", "dark"):
        ctx, p = page_for(b, 1366, 768, theme, "nkey=0")
        p.goto(f"{BASE}#/scan")
        p.wait_for_timeout(500)
        # the whole extraction form fits a 1366×768 Windows screen without scrolling
        p.set_viewport_size({"width": 1366, "height": 690})
        p.select_option(".area-pick select >> nth=0", "تهران")
        p.click(".mini-tabs button:has-text('شهر / منطقه')")
        p.wait_for_timeout(200)
        fits = p.evaluate("() => { const v = document.querySelector('#view'); return [v.scrollHeight, v.clientHeight]; }")
        if fits[0] > fits[1] + 1:
            problems.append(f"[scan-{theme}] form needs scrolling at 1366x690: {fits}")
        p.screenshot(path=f"{OUT}/s-scan-fit-{theme}.png")
        p.goto(f"{BASE}#/settings?tab=modules")
        p.wait_for_timeout(500)
        overflow_check(p, f"modules-{theme}")
        p.screenshot(path=f"{OUT}/s-modules-{theme}.png", full_page=True)
        ctx.close()

    # empty install
    ctx, p = page_for(b, 1366, 768, "light", "empty=1")
    p.goto(f"{BASE}?empty=1#/dashboard")
    p.wait_for_timeout(500)
    p.screenshot(path=f"{OUT}/x-empty.png")
    p.goto(f"{BASE}?empty=1#/outreach")
    p.wait_for_timeout(400)
    p.screenshot(path=f"{OUT}/x-empty-outreach.png")
    ctx.close()

    # Ctrl+K
    ctx, p = page_for(b, 1366, 768, "light")
    p.goto(f"{BASE}#/dashboard")
    p.wait_for_timeout(400)
    p.keyboard.press("Control+k")
    p.keyboard.type("کافه")
    p.wait_for_timeout(500)
    p.screenshot(path=f"{OUT}/x-palette.png")
    ctx.close()
    b.close()

print(json.dumps(problems, ensure_ascii=False, indent=1) if problems else "NO PROBLEMS")
srv.shutdown()
