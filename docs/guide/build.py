# Builds HamistaSeller-Guide.pdf from guide.html (Chromium print, Persian shaping): python3 build.py OUT.pdf
import re, sys, pathlib, subprocess, json
from playwright.sync_api import sync_playwright
here = pathlib.Path(__file__).resolve().parent
help_js = (here / "../../ui/js/pages/help.js").read_text()
block = re.search(r"const PROBLEMS = \[(.*?)\n\];", help_js, re.S).group(1)
rows = re.findall(r'\["(.*?)", "(.*?)"\]', block)
probs = "\n".join(f"    <tr><td><b>{a}</b></td><td>{b}</td></tr>" for a, b in rows)
date = subprocess.run(["date", "+%Y-%m-%d"], capture_output=True, text=True).stdout.strip()
html = (here / "guide.html").read_text().replace("{{PROBLEMS}}", probs).replace("{{DATE}}", "مهر ۱۴۰۵")
tmp = here / "_print.html"; tmp.write_text(html)
foot = '<div style="width:100%;font:9px Tahoma;color:#888;text-align:center"><span class="pageNumber"></span> / <span class="totalPages"></span> · هامیستا سلر</div>'
with sync_playwright() as pw:
    b = pw.chromium.launch(executable_path="/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args=["--no-sandbox", "--allow-file-access-from-files"])
    p = b.new_page(); p.goto(tmp.as_uri()); p.wait_for_timeout(800)
    p.pdf(path=sys.argv[1], format="A4", print_background=True, prefer_css_page_size=True, display_header_footer=True, header_template="<span></span>", footer_template=foot)
    b.close()
tmp.unlink()
print("ok")
