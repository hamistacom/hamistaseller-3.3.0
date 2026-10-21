//! Pure helpers (std only) — unit-tested with `rustc --edition 2021 --test src/logic.rs`.

/// Persian/Arabic digits -> ASCII, everything else untouched.
pub fn ascii_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{06F0}'..='\u{06F9}' => char::from_digit(c as u32 - 0x06F0, 10).unwrap_or(c),
            '\u{0660}'..='\u{0669}' => char::from_digit(c as u32 - 0x0660, 10).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// A number as the UI shows it: Persian digits, «٬» thousands, one «٫» decimal when not whole.
pub fn fa_num(n: f64) -> String {
    let neg = n < 0.0;
    let r = (n.abs() * 10.0).round() / 10.0;
    let int = r.trunc() as u64;
    let s = int.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).rem_euclid(3) == 0 {
            out.push('٬');
        }
        out.push(c);
    }
    let frac = ((r - r.trunc()) * 10.0).round() as u32;
    if frac > 0 {
        out.push('٫');
        out.push_str(&frac.to_string());
    }
    let fa: String = out
        .chars()
        .map(|c| c.to_digit(10).map_or(c, |d| char::from_u32(0x06F0 + d).unwrap_or(c)))
        .collect();
    if neg && fa != "۰" {
        format!("-{fa}")
    } else {
        fa
    }
}

/// Any written form -> canonical Iranian number (`09121234567`, `02188776655`).
pub fn normalize_phone(raw: &str) -> String {
    let digits: String = ascii_digits(raw).chars().filter(|c| c.is_ascii_digit()).collect();
    let d = digits.strip_prefix("00").unwrap_or(&digits);
    let d = if d.starts_with("98") && d.len() >= 12 {
        format!("0{}", &d[2..])
    } else {
        d.to_string()
    };
    if d.len() == 10 && !d.starts_with('0') {
        format!("0{d}")
    } else {
        d
    }
}

/// "mobile" (09xxxxxxxxx) / "landline" (0 + area code, 11 digits) / "" (not an Iranian number).
pub fn phone_kind(p: &str) -> &'static str {
    let b = p.as_bytes();
    if b.len() != 11 || b[0] != b'0' || !b.iter().all(u8::is_ascii_digit) {
        ""
    } else if b[1] == b'9' {
        "mobile"
    } else if (b'1'..=b'8').contains(&b[1]) {
        "landline"
    } else {
        ""
    }
}

/// Every channel a business can be reached on. Lists hold at most `MAX_EACH` unique values.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Contacts {
    pub mobiles: Vec<String>,
    pub landlines: Vec<String>,
    pub emails: Vec<String>,
    pub website: String,
    pub instagram: String,
    pub telegram: String,
    pub whatsapp: String,
    pub eitaa: String,
    pub bale: String,
    pub person: String,      // named manager / owner, when the site states one
    pub person_role: String, // «مدیرعامل»، «مدیر فروش» …
}

const MAX_EACH: usize = 3;

fn push_unique(v: &mut Vec<String>, s: String) {
    if !s.is_empty() && v.len() < MAX_EACH && !v.contains(&s) {
        v.push(s);
    }
}

impl Contacts {
    pub fn add_phone(&mut self, raw: &str) {
        let p = normalize_phone(raw);
        match phone_kind(&p) {
            "mobile" => push_unique(&mut self.mobiles, p),
            "landline" => push_unique(&mut self.landlines, p),
            _ => {}
        }
    }

    pub fn add_email(&mut self, raw: &str) {
        if let Some(e) = clean_email(raw) {
            push_unique(&mut self.emails, e);
        }
    }

    /// Sorts a link into its channel. Unknown http(s) links become the website (first one wins).
    pub fn add_link(&mut self, raw: &str) {
        let url = unwrap_redirect(&decode_entities(raw.trim()));
        let lower = url.to_lowercase();
        if let Some(m) = lower.strip_prefix("mailto:") {
            return self.add_email(&pct_decode(m.split('?').next().unwrap_or("")));
        }
        if let Some(t) = lower.strip_prefix("tel:") {
            return self.add_phone(t);
        }
        let Some((host, path)) = host_path(&url) else { return };
        let first = path.split(['/', '?', '#']).find(|s| !s.is_empty()).unwrap_or("");
        let handle = |bad: &[&str]| -> Option<String> {
            let h = first.trim_start_matches('@');
            let ok = !h.is_empty() && h.len() <= 64 && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '+');
            (ok && !bad.contains(&h.to_lowercase().as_str())).then(|| h.to_string())
        };
        let set = |slot: &mut String, v: String| {
            if slot.is_empty() {
                *slot = v;
            }
        };
        match host.as_str() {
            "instagram.com" | "instagr.am" => {
                if let Some(h) = handle(&["p", "reel", "reels", "explore", "accounts", "stories", "direct", "share"]) {
                    set(&mut self.instagram, format!("https://instagram.com/{h}"));
                }
            }
            "t.me" | "telegram.me" | "telegram.dog" => {
                if let Some(h) = handle(&["share", "s", "iv", "addstickers", "proxy", "socks"]) {
                    set(&mut self.telegram, format!("https://t.me/{h}"));
                }
            }
            "wa.me" | "api.whatsapp.com" | "web.whatsapp.com" | "whatsapp.com" => {
                let num = if host == "wa.me" {
                    first.to_string()
                } else {
                    query_param(&url, "phone").unwrap_or_default()
                };
                let p = normalize_phone(&num);
                if !p.is_empty() && !phone_kind(&p).is_empty() {
                    set(&mut self.whatsapp, format!("https://wa.me/98{}", &p[1..]));
                    self.add_phone(&p);
                }
            }
            "eitaa.com" => {
                if let Some(h) = handle(&["joinchat"]) {
                    set(&mut self.eitaa, format!("https://eitaa.com/{h}"));
                }
            }
            "ble.ir" | "bale.ai" | "web.bale.ai" => {
                if let Some(h) = handle(&["join"]) {
                    set(&mut self.bale, format!("https://ble.ir/{h}"));
                }
            }
            "rubika.ir" | "web.rubika.ir" => {} // not collected
            h if (url.starts_with("http://") || url.starts_with("https://")) && !is_noise_host(h) => set(&mut self.website, url.clone()),
            _ => {}
        }
    }

    pub fn has_any(&self) -> bool {
        !(self.mobiles.is_empty() && self.landlines.is_empty() && self.emails.is_empty())
            || [&self.instagram, &self.telegram, &self.whatsapp, &self.eitaa, &self.bale]
                .iter()
                .any(|s| !s.is_empty())
    }

    /// (kind, value) rows for the `contacts` table, in display order.
    pub fn pairs(&self) -> Vec<(&'static str, String)> {
        let mut v: Vec<(&'static str, String)> = vec![];
        v.extend(self.mobiles.iter().map(|m| ("mobile", m.clone())));
        v.extend(self.landlines.iter().map(|m| ("landline", m.clone())));
        v.extend(self.emails.iter().map(|m| ("email", m.clone())));
        for (k, val) in [
            ("instagram", &self.instagram),
            ("telegram", &self.telegram),
            ("whatsapp", &self.whatsapp),
            ("eitaa", &self.eitaa),
            ("bale", &self.bale),
        ] {
            if !val.is_empty() {
                v.push((k, val.clone()));
            }
        }
        v
    }

    pub fn needs_more(&self) -> bool {
        self.emails.is_empty() || self.instagram.is_empty() || self.telegram.is_empty() || self.whatsapp.is_empty()
    }
}

fn is_noise_host(h: &str) -> bool {
    const NOISE: &[&str] = &[
        "google.com",
        "goo.gl",
        "g.page",
        "gstatic.com",
        "googleapis.com",
        "facebook.com",
        "fb.com",
        "twitter.com",
        "x.com",
        "youtube.com",
        "linkedin.com",
        "aparat.com",
        "w3.org",
        "schema.org",
        "wordpress.org",
        "wa.link",
        "cdn-cgi",
    ];
    NOISE.iter().any(|n| h == *n || h.ends_with(&format!(".{n}")))
}

/// `https://www.Example.com:443/a/b?c` -> ("example.com", "/a/b?c")
pub fn host_path(url: &str) -> Option<(String, String)> {
    let rest = url.split_once("://").map(|(_, r)| r).or_else(|| url.strip_prefix("//"))?;
    let (auth, path) = rest.find(['/', '?', '#']).map_or((rest, ""), |i| (&rest[..i], &rest[i..]));
    let host = auth.rsplit('@').next()?.split(':').next()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    (!host.is_empty() && host.contains('.')).then(|| (host, path.to_string()))
}

pub fn query_param(url: &str, key: &str) -> Option<String> {
    let q = url.split_once('?')?.1.split('#').next()?;
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| pct_decode(&v.replace('+', " ")))
    })
}

/// Google wraps outbound links as `/url?q=<target>`.
pub fn unwrap_redirect(url: &str) -> String {
    let is_google = host_path(url).is_some_and(|(h, p)| h.starts_with("google.") && p.starts_with("/url"));
    if is_google {
        if let Some(t) = query_param(url, "q").or_else(|| query_param(url, "url")) {
            return t;
        }
    }
    url.to_string()
}

pub fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                Ok(v) => {
                    out.push(v);
                    i += 3;
                    continue;
                }
                Err(_) => out.push(b'%'),
            },
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn pct_encode(s: &str) -> String {
    let mut q = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => q.push(b as char),
            _ => q.push_str(&format!("%{b:02X}")),
        }
    }
    q
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&#x2F;", "/")
        .replace("&#47;", "/")
        .replace("&quot;", "\"")
        .replace("&#64;", "@")
}

pub fn clean_email(raw: &str) -> Option<String> {
    let e = raw.trim().trim_matches(['.', '<', '>', '"', '\'']).to_lowercase();
    let (user, domain) = e.split_once('@')?;
    let user_ok = !user.is_empty() && user.len() <= 64 && user.chars().all(|c| c.is_ascii_alphanumeric() || "._%+-".contains(c));
    let tld = domain.rsplit('.').next().unwrap_or("");
    let domain_ok = domain.contains('.')
        && !domain.starts_with('.')
        && !domain.contains("..")
        && domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && tld.len() >= 2
        && tld.chars().all(|c| c.is_ascii_alphabetic());
    const JUNK_TLD: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "css", "js"];
    const JUNK_DOMAIN: &[&str] = &[
        "example.com",
        "domain.com",
        "email.com",
        "sentry.io",
        "wixpress.com",
        "sentry-next.wixpress.com",
    ];
    (user_ok && domain_ok && !JUNK_TLD.contains(&tld) && !JUNK_DOMAIN.iter().any(|d| domain.ends_with(d))).then_some(e)
}

/// Cloudflare "email protection": first byte is the XOR key.
pub fn cf_decode(hex: &str) -> Option<String> {
    let bytes: Vec<u8> = (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect::<Option<_>>()?;
    let (k, rest) = bytes.split_first()?;
    String::from_utf8(rest.iter().map(|b| b ^ k).collect()).ok()
}

/// All `href="…"` / `href='…'` values in document order.
pub fn hrefs(html: &str) -> Vec<String> {
    let mut out = vec![];
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    while let Some(p) = lower[i..].find("href=") {
        let s = i + p + 5;
        i = s;
        let rest = &html[s..];
        let (q, body) = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => (q, &rest[1..]),
            _ => continue,
        };
        if let Some(end) = body.find(q) {
            out.push(body[..end].to_string());
        }
    }
    out
}

/// Visible-ish text: drops <script>/<style> blocks; every tag becomes a line break.
pub fn text_of(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len() / 2);
    let mut i = 0;
    while i < html.len() {
        let rest = &lower[i..];
        if rest.starts_with("<script") || rest.starts_with("<style") {
            let close = if rest.starts_with("<script") { "</script>" } else { "</style>" };
            i += rest.find(close).map_or(rest.len(), |e| e + close.len());
            out.push('\n');
        } else if rest.starts_with('<') {
            i += rest.find('>').map_or(rest.len(), |e| e + 1);
            out.push('\n');
        } else {
            let n = rest.find('<').unwrap_or(rest.len());
            out.push_str(&html[i..i + n]);
            i += n;
        }
    }
    out
}

/// Phone numbers written in text: digit groups joined by up to 3 separator chars (space - . ( ) ZWNJ).
pub fn phones_in_text(text: &str) -> Vec<String> {
    let mut out = vec![];
    let (mut cur, mut gap, mut junk) = (String::new(), 0, false);
    let valid = |cur: &str| (11..=13).contains(&cur.len()) && !phone_kind(&normalize_phone(cur)).is_empty();
    let flush = |cur: &mut String, junk: &mut bool, out: &mut Vec<String>| {
        let p = normalize_phone(cur);
        if !*junk && valid(cur) && !out.contains(&p) {
            out.push(p);
        }
        cur.clear();
        *junk = false;
    };
    for c in ascii_digits(text).chars() {
        if c.is_ascii_digit() {
            cur.push(c);
            gap = 0;
            junk |= cur.len() > 13; // long digit runs are ids, not phones
        } else if c == '+' && cur.is_empty() {
        } else if matches!(c, ' ' | '-' | '.' | '(' | ')' | '\u{200c}') && gap < 3 && !(cur.is_empty() && c != '(') {
            if valid(&cur) {
                flush(&mut cur, &mut junk, &mut out); // a complete number followed by another one
            }
            gap += 1;
        } else {
            flush(&mut cur, &mut junk, &mut out);
            gap = 0;
        }
    }
    flush(&mut cur, &mut junk, &mut out);
    out
}

/// Emails written in text (`name@domain.tld`).
pub fn emails_in_text(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = vec![];
    for (i, _) in text.match_indices('@') {
        let l = b[..i].iter().rev().take_while(|c| c.is_ascii_alphanumeric() || b"._%+-".contains(c)).count();
        let r = b[i + 1..].iter().take_while(|c| c.is_ascii_alphanumeric() || b".-".contains(c)).count();
        if l > 0 && r > 0 {
            if let Some(e) = clean_email(&text[i - l..i + 1 + r]) {
                if !out.contains(&e) {
                    out.push(e);
                }
            }
        }
    }
    out
}

/// Pulls every contact channel out of one HTML page.
pub fn extract_contacts(html: &str, c: &mut Contacts) {
    for h in hrefs(html) {
        if let Some(i) = h.find("/cdn-cgi/l/email-protection#") {
            if let Some(e) = cf_decode(&h[i + 28..]) {
                c.add_email(&e);
            }
        } else {
            c.add_link(&h);
        }
    }
    let lower = html.to_ascii_lowercase();
    for (i, _) in lower.match_indices("data-cfemail=\"") {
        let s = &html[i + 14..];
        if let Some(e) = s.split('"').next().and_then(cf_decode) {
            c.add_email(&e);
        }
    }
    let text = text_of(html);
    if c.person.is_empty() {
        if let Some((name, role)) = find_person(html, &text) {
            (c.person, c.person_role) = (name, role);
        }
    }
    for e in emails_in_text(&text) {
        c.add_email(&e);
    }
    for p in phones_in_text(&text) {
        c.add_phone(&p);
    }
}

/// Same-site pages worth a second look (contact / about), resolved to absolute URLs.
pub fn contact_pages(html: &str, base: &str) -> Vec<String> {
    const HINTS: &[&str] = &["contact", "about", "تماس", "درباره", "ارتباط", "%d8%aa%d9%85%d8%a7%d8%b3"];
    let Some((base_host, _)) = host_path(base) else { return vec![] };
    let scheme_end = base.find("://").map_or(0, |i| i + 3);
    let origin = base[scheme_end..].find('/').map_or(base, |i| &base[..scheme_end + i]);
    let mut out: Vec<String> = vec![];
    for h in hrefs(html) {
        let h = decode_entities(&h);
        let lower = h.to_lowercase();
        if !HINTS.iter().any(|k| lower.contains(k)) || lower.starts_with("mailto:") || lower.starts_with("tel:") || lower.starts_with('#') {
            continue;
        }
        let abs = if lower.starts_with("http://") || lower.starts_with("https://") {
            h.clone()
        } else if h.starts_with("//") {
            format!("https:{h}")
        } else if h.starts_with('/') {
            format!("{origin}{h}")
        } else {
            format!("{origin}/{h}")
        };
        let abs = abs.split('#').next().unwrap_or("").to_string();
        if host_path(&abs).is_some_and(|(hh, _)| hh == base_host) && !out.contains(&abs) && abs.trim_end_matches('/') != base.trim_end_matches('/') {
            out.push(abs);
        }
        if out.len() >= 2 {
            break;
        }
    }
    out
}

/// SMS gateway success check: exact match, or ">N" for a number greater than N.
pub fn accepted(got: &str, ok_values: &[String]) -> bool {
    ok_values.iter().any(|ok| match ok.strip_prefix('>') {
        Some(n) => got.trim().parse::<f64>().ok().zip(n.trim().parse::<f64>().ok()).is_some_and(|(g, n)| g > n),
        None => got.trim() == ok.trim(),
    })
}

/// `{name}` style placeholders, filled in one pass (a value containing `{city}` stays literal).
pub fn render(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(tpl.len());
    let mut rest = tpl;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('}').and_then(|j| vars.iter().find(|(k, _)| *k == &after[..j]).map(|(_, v)| (j, v))) {
            Some((j, v)) => {
                out.push_str(v);
                rest = &after[j + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out + rest
}

// ---------------- manager / owner name ----------------

const ROLE_LABELS: &[(&str, &str)] = &[
    ("مدیرعامل", "مدیرعامل"),
    ("مدیر عامل", "مدیرعامل"),
    ("مدیر فروش", "مدیر فروش"),
    ("مدیر بازرگانی", "مدیر بازرگانی"),
    ("مدیر مسئول", "مدیر مسئول"),
    ("مدیر مجموعه", "مدیر مجموعه"),
    ("مؤسس", "مؤسس"),
    ("موسس", "مؤسس"),
    ("بنیان‌گذار", "مؤسس"),
    ("بنیانگذار", "مؤسس"),
    ("CEO", "مدیرعامل"),
    ("Founder", "مؤسس"),
    ("Sales Manager", "مدیر فروش"),
    ("Managing Director", "مدیرعامل"),
];
const TITLES: &[&str] = &[
    "دکتر",
    "مهندس",
    "استاد",
    "خانم",
    "آقای",
    "آقا",
    "سرکار",
    "Dr.",
    "Dr",
    "Eng.",
    "Mr.",
    "Mrs.",
    "Ms.",
];
const NOT_A_NAME: &[&str] = &[
    "شرکت",
    "فروشگاه",
    "مجموعه",
    "تماس",
    "ما",
    "واحد",
    "بخش",
    "محترم",
    "گروه",
    "سایت",
    "خدمات",
    "the",
    "our",
    "and",
    "of",
    "team",
    "تلفن",
    "تماس",
    "شماره",
    "ایمیل",
    "موبایل",
    "همراه",
    "آدرس",
    "داخلی",
    "phone",
    "tel",
    "email",
    "mobile",
    "company",
];

/// A plausible personal name: 2–5 words of letters (titles allowed), no company words.
pub fn clean_person(raw: &str) -> Option<String> {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let letters = |w: &str| w.chars().all(|c| c.is_alphabetic() || c == '\u{200c}' || c == '.' || c == '\'' || c == '-');
    let core = words.iter().filter(|w| !TITLES.contains(w)).count();
    let bad = words.iter().any(|w| NOT_A_NAME.contains(&w.to_lowercase().as_str()) || !letters(w));
    let name = words.join(" ");
    (!bad && (2..=4).contains(&core) && words.len() <= 5 && (4..=48).contains(&name.chars().count())).then_some(name)
}

/// The words right after `label`, skipping `:` `-` and spaces, up to 5 words or the next delimiter.
fn name_after(text: &str, label: &str) -> Option<String> {
    let mut out = None;
    for (i, _) in text.match_indices(label) {
        let rest = text[i + label.len()..].trim_start_matches([' ', ':', '：', '-', '–', '—', '\u{200c}', '\t', '\n', '\r']);
        let end = rest.find([
            '\n', '\r', '|', '،', ',', '؛', ';', '(', ')', '<', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '۰', '۱', '۲', '۳', '۴', '۵', '۶', '۷', '۸',
            '۹',
        ]);
        let chunk: String = rest[..end.unwrap_or(rest.len())].split_whitespace().take(5).collect::<Vec<_>>().join(" ");
        // try the longest prefix that still looks like a name (drops trailing words like «تلفن»)
        let words: Vec<&str> = chunk.split(' ').collect();
        out = (2..=words.len()).rev().find_map(|n| clean_person(&words[..n].join(" ")));
        if out.is_some() {
            break;
        }
    }
    out
}

/// JSON-LD `"founder"` (object or string) → name.
fn jsonld_founder(html: &str) -> Option<String> {
    for key in ["\"founder\"", "\"ceo\""] {
        for (i, _) in html.match_indices(key) {
            let rest = html[i + key.len()..].trim_start_matches([' ', ':', '[', '\n', '\t']);
            let window = &rest[..rest.len().min(300)];
            let value = if let Some(quoted) = window.strip_prefix('"') {
                quoted.split('"').next()
            } else {
                window
                    .find("\"name\"")
                    .and_then(|j| window[j + 6..].trim_start_matches([' ', ':']).strip_prefix('"'))
                    .and_then(|v| v.split('"').next())
            };
            if let Some(n) = value.and_then(clean_person) {
                return Some(n);
            }
        }
    }
    None
}

/// Best-effort owner / manager for "name under the brand": structured data first, then labelled text.
pub fn find_person(html: &str, text: &str) -> Option<(String, String)> {
    if let Some(n) = jsonld_founder(html) {
        return Some((n, "مؤسس".into()));
    }
    ROLE_LABELS
        .iter()
        .find_map(|(label, role)| name_after(text, label).map(|n| (n, role.to_string())))
}

// ---------------- licence helpers ----------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// URL-safe base64 without padding (decoding also takes standard base64 with padding).
pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (*b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let vals: Vec<u32> = s
        .trim()
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .map(|b| {
            match b {
                b'A'..=b'Z' => Some(b - b'A'),
                b'a'..=b'z' => Some(b - b'a' + 26),
                b'0'..=b'9' => Some(b - b'0' + 52),
                b'-' | b'+' => Some(62), // url-safe and standard alphabets both accepted
                b'_' | b'/' => Some(63),
                _ => None,
            }
            .map(u32::from)
        })
        .collect::<Option<_>>()?;
    if vals.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(vals.len() * 3 / 4);
    for chunk in vals.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, v)| acc | v << (18 - 6 * i));
        for i in 0..chunk.len() - 1 {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

/// `YYYY-MM-DD` with a real month/day.
pub fn valid_date(d: &str) -> bool {
    let p: Vec<&str> = d.split('-').collect();
    let num = |s: &str, len: usize| (s.len() == len && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u32>().unwrap_or(0));
    match (p.as_slice(), p.len()) {
        ([y, m, dd], 3) => matches!((num(y, 4), num(m, 2), num(dd, 2)), (Some(y), Some(1..=12), Some(1..=31)) if y >= 2020),
        _ => false,
    }
}

/// `abcdef0123456789…` → `ABCD-EF01-2345-6789` (first 16 hex chars).
pub fn machine_code(hex: &str) -> String {
    let h: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).take(16).collect::<String>().to_uppercase();
    h.as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

/// Cleans one manually entered contact. Bare handles (`@cafe`, `cafe`) become full profile links.
pub fn normalize_contact(kind: &str, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    match kind {
        "mobile" | "landline" => {
            let p = normalize_phone(raw);
            (phone_kind(&p) == kind).then_some(p)
        }
        "email" => clean_email(raw),
        "instagram" | "telegram" | "whatsapp" | "eitaa" | "bale" => {
            let host = match kind {
                "instagram" => "instagram.com",
                "telegram" => "t.me",
                "whatsapp" => "wa.me",
                "eitaa" => "eitaa.com",
                _ => "ble.ir",
            };
            let url = if raw.contains("://") || raw.contains('.') && raw.contains('/') {
                if raw.contains("://") {
                    raw.to_string()
                } else {
                    format!("https://{raw}")
                }
            } else {
                format!("https://{host}/{}", raw.trim_start_matches('@'))
            };
            let mut c = Contacts::default();
            c.add_link(&url);
            c.pairs().into_iter().find(|(k, _)| *k == kind).map(|(_, v)| v)
        }
        _ => None,
    }
}

/// Unix seconds → `YYYY-MM-DD HH:MM:SS` (UTC), for log lines without a date crate.
pub fn utc_datetime(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // civil-from-days (H. Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// What a licence says. `plan` is free text («سالانه»، «آزمایشی» …).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LicenceInfo {
    pub machine: String,
    pub expires: String,
    pub issued: String,
    pub customer: String,
    pub plan: String,
}

/// Bytes that get signed: `HS2|machine|expires|issued|plan|customer` (`|` removed from free text).
pub fn licence_payload(l: &LicenceInfo) -> String {
    let clean = |s: &str| s.replace(['|', '\n', '\r'], " ").trim().to_string();
    format!("HS2|{}|{}|{}|{}|{}", clean(&l.machine), l.expires, l.issued, clean(&l.plan), clean(&l.customer))
}

/// `HS2.<b64 payload>.<b64 signature>` → (payload bytes, 64-byte signature, parsed fields).
pub fn parse_licence(code: &str) -> Option<(Vec<u8>, [u8; 64], LicenceInfo)> {
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let mut parts = code.strip_prefix("HS2.")?.split('.');
    let (p, s) = (b64_decode(parts.next()?)?, b64_decode(parts.next()?)?);
    if parts.next().is_some() {
        return None;
    }
    let sig: [u8; 64] = s.try_into().ok()?;
    let text = String::from_utf8(p.clone()).ok()?;
    let f: Vec<&str> = text.splitn(6, '|').collect();
    if f.len() != 6 || f[0] != "HS2" || !valid_date(f[2]) || !valid_date(f[3]) {
        return None;
    }
    let info = LicenceInfo {
        machine: f[1].into(),
        expires: f[2].into(),
        issued: f[3].into(),
        plan: f[4].into(),
        customer: f[5].into(),
    };
    Some((p, sig, info))
}

pub fn licence_code(payload: &str, sig: &[u8; 64]) -> String {
    format!("HS2.{}.{}", b64_encode(payload.as_bytes()), b64_encode(sig))
}

/// Value of `name` in `reg query` output (`    MachineGuid    REG_SZ    …`).
pub fn reg_value(output: &str, name: &str) -> Option<String> {
    output.lines().find_map(|l| {
        let mut w = l.split_whitespace();
        (w.next()? == name).then(|| w.nth(1).map(str::to_string)).flatten()
    })
}

/// Comma / Persian comma / newline separated tags → trimmed, unique, max 10 × 24 chars.
pub fn normalize_tags(raw: &str) -> String {
    let mut out: Vec<String> = vec![];
    for t in raw.split([',', '،', '\n', '#']).map(str::trim).filter(|t| !t.is_empty()) {
        let t: String = t.chars().take(24).collect();
        if !out.contains(&t) && out.len() < 10 {
            out.push(t);
        }
    }
    out.join(",")
}

/// Stable id of a Google Maps place: the `0x..:0x..` feature id, else the URL without query.
pub fn place_key(url: &str) -> String {
    let b = url.as_bytes();
    let hex = |i: usize| b[i..].iter().take_while(|c| c.is_ascii_hexdigit()).count();
    let mut i = 0;
    while let Some(p) = url[i..].find("0x") {
        let s = i + p;
        let a = hex(s + 2);
        let m = s + 2 + a;
        if a > 0 && url[m..].starts_with(":0x") {
            let c = hex(m + 3);
            if c > 0 {
                return url[s..m + 3 + c].to_lowercase();
            }
        }
        i = s + 2;
    }
    url.split('?').next().unwrap_or(url).to_string()
}

/// `!3d35.7!4d51.4` -> (35.7, 51.4)
pub fn coords(url: &str) -> Option<(f64, f64)> {
    let num = |s: &str| s.split('!').next().and_then(|v| v.parse::<f64>().ok());
    let lat = num(url.split("!3d").nth(1)?)?;
    let lng = num(url.split("!4d").nth(1)?)?;
    Some((lat, lng))
}

pub fn search_url(query: &str) -> String {
    format!("https://www.google.com/maps/search/{}?hl=fa", pct_encode(query.trim()).replace("%20", "+"))
}

const TEHRAN_AREAS: &[&str] = &[
    "تجریش",
    "ونک",
    "جردن",
    "نیاوران",
    "سعادت آباد",
    "پونک",
    "صادقیه",
    "ستارخان",
    "مرزداران",
    "تهرانپارس",
    "نارمک",
    "پیروزی",
    "شهرری",
    "بازار",
    "انقلاب",
    "اکباتان",
    "چیتگر",
    "فرمانیه",
    "پاسداران",
    "شهرک غرب",
    "یوسف آباد",
    "میرداماد",
    "الهیه",
    "زعفرانیه",
    "شهران",
    "جنت آباد",
    "تهرانسر",
    "حکیمیه",
    "نازی آباد",
    "یافت آباد",
    "امیرآباد",
    "گیشا",
    "هفت تیر",
    "سیدخندان",
    "قیطریه",
    "شریعتی",
    "ولیعصر",
    "آزادی",
    "افسریه",
    "مجیدیه",
];

/// Base query first, then sub-areas used only when the base query runs out of results.
pub fn plan_queries(keyword: &str, province: &str, city: &str) -> Vec<String> {
    let place = if city.trim().is_empty() { province.trim() } else { city.trim() };
    let kw = keyword.trim();
    let mut out = vec![format!("{kw} {place}").trim().to_string()];
    if place.is_empty() {
        return out;
    }
    if place == "تهران" {
        out.extend(TEHRAN_AREAS.iter().map(|a| format!("{kw} {a} تهران")));
    } else {
        out.extend(["شمال", "جنوب", "شرق", "غرب", "مرکز"].iter().map(|s| format!("{kw} {s} {place}")));
    }
    out
}

/// Address as shown by the map page, without icon-font glyphs, stray marks and doubled commas.
pub fn clean_address(s: &str) -> String {
    let s: String = s
        .chars()
        .filter(|c| !('\u{E000}'..='\u{F8FF}').contains(c) && !matches!(c, '\u{200E}' | '\u{200F}' | '\u{FFFD}'))
        .collect();
    let mut out = String::new();
    for part in s.split([',', '،']).map(str::trim).filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            out.push_str("، ");
        }
        out.push_str(part);
    }
    out
}

/// County of a place: the one asked for, or — when every county of the province was searched —
/// the first of them named in the address.
pub fn city_in(address: &str, city: &str, areas: &[String]) -> String {
    if !city.trim().is_empty() {
        return city.trim().to_string();
    }
    areas
        .iter()
        .map(|a| a.trim())
        .find(|a| !a.is_empty() && address.contains(a))
        .unwrap_or("")
        .to_string()
}

/// Only an https picture from the map provider's image host is kept (the card shows it).
pub fn photo_url(s: &str) -> String {
    let ok = host_path(s).is_some_and(|(h, _)| h.ends_with("googleusercontent.com") || h.ends_with("ggpht.com"));
    if s.starts_with("https://") && ok && s.len() < 1000 {
        s.to_string()
    } else {
        String::new()
    }
}

/// Cheap jitter without a rand crate.
pub fn jitter(min_ms: u64, max_ms: u64) -> u64 {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    min_ms + n % (max_ms.saturating_sub(min_ms) + 1)
}

pub fn csv_cell(s: &str) -> String {
    // leading = + - @ would be run as a formula by Excel
    let s = if s.starts_with(['=', '+', '-', '@']) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub fn vcard(name: &str, mobiles: &[&str], landlines: &[&str], email: &str, note: &str) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace(',', "\\,").replace(';', "\\;").replace('\n', " ");
    let mut v = format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:{}\r\n", esc(name));
    for m in mobiles.iter().filter(|s| !s.is_empty()) {
        v += &format!("TEL;TYPE=CELL:{m}\r\n");
    }
    for l in landlines.iter().filter(|s| !s.is_empty()) {
        v += &format!("TEL;TYPE=WORK:{l}\r\n");
    }
    if !email.is_empty() {
        v += &format!("EMAIL;TYPE=WORK:{email}\r\n");
    }
    v + &format!("NOTE:{}\r\nEND:VCARD\r\n", esc(note))
}

// ---------------- sending, import, chat assistant ----------------

/// `0912 345 6789` → `989123456789` (Bale Safir, WhatsApp). Empty for anything that is not a mobile.
pub fn intl_mobile(raw: &str) -> String {
    let p = normalize_phone(raw);
    if phone_kind(&p) == "mobile" {
        format!("98{}", &p[1..])
    } else {
        String::new()
    }
}

/// CSV / TSV text → rows (RFC 4180 quotes; delimiter guessed from the first line; blank rows dropped).
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("");
    let delim = [',', ';', '\t'].into_iter().max_by_key(|d| first.matches(*d).count()).unwrap_or(',');
    let (mut rows, mut row, mut cell, mut quoted) = (vec![], vec![], String::new(), false);
    let push_row = |row: &mut Vec<String>, rows: &mut Vec<Vec<String>>| {
        if row.iter().any(|c| !c.trim().is_empty()) {
            rows.push(std::mem::take(row));
        }
        row.clear();
    };
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c != '"' {
                cell.push(c);
            } else if chars.peek() == Some(&'"') {
                cell.push('"');
                chars.next();
            } else {
                quoted = false;
            }
        } else if c == '"' && cell.trim().is_empty() {
            cell.clear();
            quoted = true;
        } else if c == delim {
            row.push(std::mem::take(&mut cell));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            push_row(&mut row, &mut rows);
        } else {
            cell.push(c);
        }
    }
    row.push(cell);
    push_row(&mut row, &mut rows);
    rows
}

/// Which field an import column holds, from its header (Persian or English; our own export headers included).
/// `phone` = a number that may be mobile or landline; `""` = ignored column.
pub fn import_field(header: &str) -> &'static str {
    let h: String = header
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '\u{200c}' | '_' | '-' | '.'))
        .collect();
    const RULES: &[(&[&str], &str)] = &[
        (&["نقشه", "map"], "maps"),
        (&["اینستا", "instagram"], "instagram"),
        (&["تلگرام", "telegram"], "telegram"),
        (&["واتس", "whatsapp"], "whatsapp"),
        (&["ایتا", "eitaa"], "eitaa"),
        (&["بله", "bale"], "bale"),
        (&["مسئول", "مدیر", "person", "owner", "manager"], "person"),
        (&["سمت", "role", "position"], "role"),
        (&["موبایل", "همراه", "mobile", "cell"], "mobile"),
        (&["ثابت", "landline"], "landline"),
        (&["تلفن", "شماره", "phone", "tel", "number"], "phone"),
        (&["ایمیل", "email", "mail"], "email"),
        (&["سایت", "website", "web", "url"], "website"),
        (&["آدرس", "نشانی", "address"], "address"),
        (&["دسته", "صنف", "زمینه", "category"], "category"),
        (&["استان", "province", "state"], "province"),
        (&["شهر", "city"], "city"),
        (&["کلیدی", "keyword"], "keyword"),
        (&["برچسب", "tag"], "tags"),
        (&["یادداشت", "توضیح", "note"], "note"),
        (&["نام", "name", "عنوان", "برند", "company", "business", "title"], "name"),
    ];
    RULES.iter().find(|(keys, _)| keys.iter().any(|k| h.contains(k))).map_or("", |(_, f)| f)
}

/// Splits a cell that may hold several values (`0912…، 0935…`).
pub fn split_values(cell: &str) -> Vec<String> {
    // our CSV export guards = + - @ with a leading quote; undo that
    let cell = cell.strip_prefix('\'').filter(|r| r.starts_with(['=', '+', '-', '@'])).unwrap_or(cell);
    cell.split([',', '،', ';', '؛', '\n', '|'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Web client the in-app assistant opens for one recipient (`to` = a phone number or a profile link).
/// Returns (url, text_is_prefilled). Only WhatsApp takes the message in the link; for the
/// others the app copies the text and the user pastes it.
pub fn chat_link(app: &str, to: &str, text: &str) -> Option<(String, bool)> {
    let intl = intl_mobile(to);
    let handle = |host: &str| {
        host_path(to)
            .filter(|(h, _)| h == host)
            .and_then(|(_, p)| p.trim_start_matches('/').split(['/', '?', '#']).next().map(String::from))
            .filter(|h| !h.is_empty())
    };
    match app {
        "whatsapp" if !intl.is_empty() => Some((format!("https://web.whatsapp.com/send?phone={intl}&text={}", pct_encode(text)), true)),
        "whatsapp" => None,
        "telegram" => {
            let target = handle("t.me")
                .map(|h| format!("tg://resolve?domain={h}"))
                .or_else(|| (!intl.is_empty()).then(|| format!("tg://resolve?phone={intl}")));
            Some((
                target.map_or("https://web.telegram.org/k/".into(), |t| {
                    format!("https://web.telegram.org/k/#?tgaddr={}", pct_encode(&t))
                }),
                false,
            ))
        }
        "eitaa" => Some(("https://web.eitaa.com/".into(), false)),
        "bale" => Some(("https://web.bale.ai/".into(), false)),
        _ => None,
    }
}

/// Test-only overrides (local mock servers). Compiled in only with the `test-seams` feature,
/// which only tests-harness enables — a release build never reads these variables.
pub fn test_seam(name: &str) -> Option<String> {
    if cfg!(feature = "test-seams") {
        std::env::var(name).ok()
    } else {
        None
    }
}

/// Words of a business name for matching: Persian/Arabic letters unified, punctuation dropped.
fn name_words(s: &str) -> Vec<String> {
    let s: String = s
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'ي' | 'ى' => 'ی',
            'ك' => 'ک',
            'ة' => 'ه',
            'أ' | 'إ' | 'آ' => 'ا',
            '\u{200c}' | '-' | '_' | '.' | '،' | ',' | '(' | ')' | '|' | '/' | '«' | '»' | '"' | '\'' => ' ',
            c => c,
        })
        .collect();
    s.split_whitespace().filter(|w| w.chars().count() > 1).map(String::from).collect()
}

/// Is the place Maps returned (`found`) the business we looked up by name (`wanted`, no city)?
/// All of the wanted name's words must be in the found name (one may be missing when it has 4+),
/// or every word of a multi-word found name must be in the wanted one.
pub fn same_business(found: &str, wanted: &str) -> bool {
    let f = name_words(found);
    let w = name_words(wanted);
    if f.is_empty() || w.is_empty() {
        return false;
    }
    let hits_w = w.iter().filter(|x| f.contains(x)).count();
    let need = if w.len() >= 4 { w.len() - 1 } else { w.len() };
    hits_w >= need || (f.len() >= 2 && f.iter().all(|x| w.contains(x)))
}

/// Normalised form of a name, for de-duplication ("كافه  روبرتو" = "کافه روبرتو").
pub fn name_key(s: &str) -> String {
    name_words(s).join(" ")
}

/// Value of `<meta property|name="key" content="…">` in an HTML page (entities decoded).
pub fn meta_content(html: &str, key: &str) -> Option<String> {
    for tag in html.split("<meta").skip(1) {
        let tag = tag.split('>').next().unwrap_or_default();
        if tag.contains(&format!("property=\"{key}\"")) || tag.contains(&format!("name=\"{key}\"")) {
            let v = tag.split("content=\"").nth(1)?.split('"').next()?;
            return Some(html_unescape(v).trim().to_string());
        }
    }
    None
}

/// Named and numeric HTML entities (`&amp;` `&#1601;` `&#x6A9;` …).
pub fn html_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|&e| e <= 12) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// http(s) links written as plain text (bios, channel descriptions).
/// `@handle` mentions in a bio / channel description («سفارش: @lamiz_admin») — Telegram and
/// Instagram ids (5–32 chars, latin letters, digits, `_`, `.`); e-mail addresses are not mentions.
pub fn mentions_in_text(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for (i, _) in text.match_indices('@') {
        if text[..i].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
            continue; // name@mail.com
        }
        let h: String = text[i + 1..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.').collect();
        let h = h.trim_end_matches('.').to_string();
        if (5..=32).contains(&h.len()) && h.starts_with(|c: char| c.is_ascii_alphabetic()) && !out.contains(&h) {
            out.push(h);
        }
    }
    out
}

pub fn links_in_text(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || "()<>«»\"'،".contains(c))
        .filter(|w| {
            w.starts_with("http://")
                || w.starts_with("https://")
                || w.starts_with("www.")
                || w.starts_with("t.me/")
                || w.starts_with("wa.me/")
                || w.starts_with("instagram.com/")
        })
        .map(|w| {
            let w = w.trim_end_matches(['.', ',', '!', '؟']);
            if w.starts_with("http") {
                w.to_string()
            } else {
                format!("https://{w}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn mentions() {
        assert_eq!(super::mentions_in_text("سفارش: @lamiz_admin و پشتیبانی @Lamiz.Support. ایمیل info@lamiz.ir"), vec!["lamiz_admin", "Lamiz.Support"]);
        assert!(super::mentions_in_text("@ab @12345").is_empty());
    }

    #[test]
    fn fa_numbers() {
        assert_eq!(super::fa_num(1_234_567.0), "۱٬۲۳۴٬۵۶۷");
        assert_eq!(super::fa_num(16.3), "۱۶٫۳");
        assert_eq!(super::fa_num(5.0), "۵");
    }

    #[test]
    fn business_name_matching() {
        assert!(same_business("کافه روبرتو", "كافه روبرتو"));
        assert!(same_business("Cafe Lamiz | کافه لمیز", "کافه لمیز"));
        assert!(same_business("کافه روبرتو شعبه پاسداران", "کافه روبرتو"));
        assert!(!same_business("رستوران شب", "کافه روبرتو"));
        assert!(!same_business("", "کافه روبرتو"));
        assert!(!same_business("کافه لمیز", "کافه روبرتو"), "a shared category word is not enough");
        assert!(!same_business("هتل تهران", "هتل اسپیناس"), "the city in a name is not a match");
        assert!(!same_business("نانوایی سنگکی تهران", "نانوایی سنگکی بربری"));
        assert_eq!(name_key("كافه  روبرتو"), name_key("کافه روبرتو"));
        assert_eq!(
            meta_content(
                r#"<meta property="og:title" content="Caf&amp;e &#1705;&#x627;"><meta name="x" content="y">"#,
                "og:title"
            )
            .as_deref(),
            Some("Caf&e کا")
        );
        assert_eq!(html_unescape("a &unknown; & b&lt;"), "a &unknown; & b<");
        assert_eq!(html_unescape("&کافهٔ طولانی;"), "&کافهٔ طولانی;"); // no panic on multibyte text after &
        assert_eq!(
            links_in_text("سفارش: https://lamiz.ir/menu، کانال t.me/lamiz."),
            vec!["https://lamiz.ir/menu", "https://t.me/lamiz"]
        );
    }

    use super::*;

    #[test]
    fn address_city_photo() {
        assert_eq!(
            clean_address("\u{E0C8} استان چهارمحال و بختیاری، شهرکرد،، بهارستان، ایران"),
            "استان چهارمحال و بختیاری، شهرکرد، بهارستان، ایران"
        );
        let areas = vec!["بروجن".to_string(), "شهرکرد".to_string()];
        assert_eq!(city_in("XV4R+XFV، بروجن، استان چهارمحال و بختیاری", "", &areas), "بروجن");
        assert_eq!(city_in("تهران", " کرج ", &areas), "کرج");
        assert_eq!(city_in("جایی دیگر", "", &areas), "");
        assert!(photo_url("https://lh5.googleusercontent.com/p/AF1Q=w408").starts_with("https://"));
        assert!(photo_url("https://evil.com/x.jpg").is_empty() && photo_url("http://lh5.googleusercontent.com/x").is_empty());
    }

    #[test]
    fn phones() {
        assert_eq!(normalize_phone("۰۹۱۲ ۳۴۵ ۶۷۸۹"), "09123456789");
        assert_eq!(normalize_phone("+98 912 345 6789"), "09123456789");
        assert_eq!(normalize_phone("0098-21-88776655"), "02188776655");
        assert_eq!(normalize_phone("021 8877 6655"), "02188776655");
        assert_eq!(normalize_phone("9123456789"), "09123456789");
        assert_eq!(normalize_phone("٠٢١٨٨٧٧٦٦٥٥"), "02188776655");
        assert_eq!(normalize_phone(""), "");
        assert_eq!(phone_kind("09123456789"), "mobile");
        assert_eq!(phone_kind("02188776655"), "landline");
        assert_eq!(phone_kind("00123456789"), "");
        assert_eq!(phone_kind("1575"), "");
        let mut c = Contacts::default();
        for p in [
            "0912 345 6789",
            "+989123456789",
            "021-88776655",
            "۰۳۱۳۲۲۲۱۱۱۱",
            "0912 000 1111",
            "0912 000 2222",
            "0912 000 3333",
        ] {
            c.add_phone(p);
        }
        assert_eq!(c.mobiles, vec!["09123456789", "09120001111", "09120002222"]); // capped at 3, deduped
        assert_eq!(c.landlines, vec!["02188776655", "03132221111"]);
    }

    #[test]
    fn links() {
        let mut c = Contacts::default();
        for l in [
            "https://www.instagram.com/cafe.lamiz/?hl=en",
            "https://instagram.com/p/xyz",
            "https://t.me/cafelamiz",
            "https://api.whatsapp.com/send?phone=989121112233&text=hi",
            "https://eitaa.com/lamiz",
            "https://ble.ir/lamiz_cafe",
            "https://rubika.ir/lamiz",
            "mailto:Info@Lamiz.IR?subject=x",
            "tel:+98-21-66554433",
            "https://www.google.com/url?q=https://lamiz.ir/&sa=U",
            "https://facebook.com/lamiz",
        ] {
            c.add_link(l);
        }
        assert_eq!(c.instagram, "https://instagram.com/cafe.lamiz");
        assert_eq!(c.telegram, "https://t.me/cafelamiz");
        assert_eq!(c.whatsapp, "https://wa.me/989121112233");
        assert_eq!(c.mobiles, vec!["09121112233"]);
        assert_eq!(c.landlines, vec!["02166554433"]);
        assert_eq!(c.eitaa, "https://eitaa.com/lamiz");
        assert_eq!(c.bale, "https://ble.ir/lamiz_cafe");
        assert!(!c.pairs().iter().any(|(_, v)| v.contains("rubika")), "Rubika links are not collected");
        assert_eq!(c.emails, vec!["info@lamiz.ir"]);
        assert_eq!(c.website, "https://lamiz.ir/");
        let mut w = Contacts::default();
        w.add_link("https://wa.me/09121112233");
        assert_eq!(w.whatsapp, "https://wa.me/989121112233");
        assert!(w.has_any());
        assert!(!Contacts::default().has_any());
    }

    #[test]
    fn html_extraction() {
        let html = r#"<html><head><style>.a{color:red}</style><script>var id=09123334444;</script></head>
        <body><a href="/fa/contact-us">تماس با ما</a><a href='https://t.me/shop_ir'>tg</a>
        <p>تلفن: ۰۲۱-۸۸۷۷ ۶۶۵۵ | موبایل: 0935 111 2222 | قیمت 1,200,000 | کد پستی 1234567890</p>
        <p>ایمیل: sales@shop.ir و logo@2x.png</p>
        <a href="/cdn-cgi/l/email-protection#c3aaada5ac83b0abacb3edaab1">[email&#160;protected]</a>
        <a href="https://other.com/contact">x</a><a href="about.html">درباره</a></body></html>"#;
        let mut c = Contacts::default();
        extract_contacts(html, &mut c);
        assert_eq!(c.telegram, "https://t.me/shop_ir");
        assert_eq!(c.landlines, vec!["02188776655"]);
        assert_eq!(c.mobiles, vec!["09351112222"]); // not the one inside <script>
        assert_eq!(c.emails, vec!["info@shop.ir", "sales@shop.ir"]);
        assert_eq!(
            contact_pages(html, "https://shop.ir/"),
            vec!["https://shop.ir/fa/contact-us", "https://shop.ir/about.html"]
        );
        assert_eq!(
            contact_pages(html, "https://shop.ir"),
            vec!["https://shop.ir/fa/contact-us", "https://shop.ir/about.html"]
        );
    }

    #[test]
    fn small_parsers() {
        assert_eq!(cf_decode("c3aaada5ac83b0abacb3edaab1").as_deref(), Some("info@shop.ir"));
        assert_eq!(cf_decode("zz"), None);
        assert_eq!(host_path("https://WWW.Shop.ir:8080/a?b"), Some(("shop.ir".into(), "/a?b".into())));
        assert_eq!(host_path("nope"), None);
        assert_eq!(pct_decode("a%20b+c%D8%AA%zz"), "a b+cت%zz");
        assert_eq!(unwrap_redirect("https://www.google.com/url?q=https%3A%2F%2Fx.ir%2F&opi=1"), "https://x.ir/");
        assert_eq!(emails_in_text("a@b.co, x.y+z@mail.shop.ir. bad@x"), vec!["a@b.co", "x.y+z@mail.shop.ir"]);
        assert_eq!(
            phones_in_text("شماره ۰۹۱۲۱۲۳۴۵۶۷ و +98 912 765 4321 و 12345678909121234567890"),
            vec!["09121234567", "09127654321"]
        );
        assert_eq!(phones_in_text("Tel: 021 8877 6655 - 0935 111 2222"), vec!["02188776655", "09351112222"]);
        assert_eq!(phones_in_text("(021) 8877-6655"), vec!["02188776655"]);
        assert_eq!(render("سلام {name} از {city}", &[("name", "کافه"), ("city", "تهران")]), "سلام کافه از تهران");
        assert_eq!(render("{name}{x} {", &[("name", "a{city}"), ("city", "no")]), "a{city}{x} {");
        let ok = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(accepted("200", &ok(&["200"])) && accepted(" true", &ok(&["true"])) && accepted("12345678901", &ok(&[">1000"])));
        assert!(!accepted("0", &ok(&["1"])) && !accepted("abc", &ok(&[">5"])));
    }

    #[test]
    fn people() {
        let html = r#"<script type="application/ld+json">{"@type":"Organization","founder":{"@type":"Person","name":"Ali Rezaei"}}</script>"#;
        assert_eq!(find_person(html, ""), Some(("Ali Rezaei".into(), "مؤسس".into())));
        let t = "درباره ما\nمدیرعامل: مهندس رضا احمدی تلفن 021\nمدیر فروش - سارا کریمی";
        assert_eq!(find_person("", t), Some(("مهندس رضا احمدی".into(), "مدیرعامل".into())));
        assert_eq!(
            find_person("", "مدیر فروش: سارا کریمی، داخلی ۲۱۲"),
            Some(("سارا کریمی".into(), "مدیر فروش".into()))
        );
        assert_eq!(find_person("", "مدیرعامل شرکت ما همیشه پاسخگوست"), None);
        assert_eq!(find_person("", "CEO: John Smith | contact"), Some(("John Smith".into(), "مدیرعامل".into())));
        assert_eq!(clean_person("علی"), None);
        assert_eq!(clean_person("خدمات فنی رضا"), None);
        let mut c = Contacts::default();
        extract_contacts("<p>مدیر مسئول : دکتر نرگس امینی</p><a href='mailto:a@b.ir'>x</a>", &mut c);
        assert_eq!((c.person.as_str(), c.person_role.as_str()), ("دکتر نرگس امینی", "مدیر مسئول"));
        assert_eq!(c.pairs(), vec![("email", "a@b.ir".to_string())]);
    }

    #[test]
    fn manual_contacts() {
        assert_eq!(normalize_contact("mobile", "۰۹۱۲ ۱۲۳ ۴۵۶۷").as_deref(), Some("09121234567"));
        assert_eq!(normalize_contact("mobile", "02188776655"), None);
        assert_eq!(normalize_contact("landline", "021-88776655").as_deref(), Some("02188776655"));
        assert_eq!(normalize_contact("email", " Info@Shop.IR ").as_deref(), Some("info@shop.ir"));
        assert_eq!(
            normalize_contact("instagram", "@cafe.lamiz").as_deref(),
            Some("https://instagram.com/cafe.lamiz")
        );
        assert_eq!(
            normalize_contact("instagram", "instagram.com/cafe/").as_deref(),
            Some("https://instagram.com/cafe")
        );
        assert_eq!(normalize_contact("telegram", "https://t.me/shop_ir").as_deref(), Some("https://t.me/shop_ir"));
        assert_eq!(normalize_contact("whatsapp", "09121112233").as_deref(), Some("https://wa.me/989121112233"));
        assert_eq!(normalize_contact("eitaa", "shop").as_deref(), Some("https://eitaa.com/shop"));
        assert_eq!(normalize_contact("fax", "x"), None);
        assert_eq!(normalize_contact("email", ""), None);
    }

    #[test]
    fn licence_helpers() {
        for data in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar", &[0, 255, 254, 1, 2]] {
            assert_eq!(b64_decode(&b64_encode(data)).as_deref(), Some(data));
        }
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64_encode(&[251, 255]), "-_8");
        assert_eq!(b64_decode("a"), None);
        assert_eq!(b64_decode("ab!c"), None);
        assert!(valid_date("2027-02-28") && !valid_date("2027-13-01") && !valid_date("27-01-01") && !valid_date("2027-1-01"));
        assert_eq!(machine_code("abcdef0123456789ffff"), "ABCD-EF01-2345-6789");
        assert_eq!(normalize_tags(" vip، کافه,vip,#تهران "), "vip,کافه,تهران");
        let l = LicenceInfo {
            machine: "ABCD-EF01-2345-6789".into(),
            expires: "2027-10-01".into(),
            issued: "2026-10-01".into(),
            customer: "کافه|لمیز".into(),
            plan: "سالانه".into(),
        };
        let payload = licence_payload(&l);
        assert_eq!(payload, "HS2|ABCD-EF01-2345-6789|2027-10-01|2026-10-01|سالانه|کافه لمیز");
        let code = licence_code(&payload, &[7u8; 64]);
        let (p, sig, info) = parse_licence(&format!(" {}\n", code)).unwrap();
        assert_eq!((p, sig), (payload.into_bytes(), [7u8; 64]));
        assert_eq!(info.customer, "کافه لمیز");
        assert_eq!(info.expires, "2027-10-01");
        assert!(parse_licence("HS2.abc").is_none() && parse_licence(&code.replace("HS2.", "HS1.")).is_none());
        let reg = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Cryptography\r\n    MachineGuid    REG_SZ    3f2504e0-4f89-11d3-9a0c-0305e82c3301\r\n";
        assert_eq!(reg_value(reg, "MachineGuid").as_deref(), Some("3f2504e0-4f89-11d3-9a0c-0305e82c3301"));
        assert_eq!(reg_value("nothing", "MachineGuid"), None);
        assert_eq!(utc_datetime(0), "1970-01-01 00:00:00");
        assert_eq!(utc_datetime(1_790_000_000), "2026-09-21 14:13:20");
        assert_eq!(utc_datetime(951_782_400), "2000-02-29 00:00:00");
    }

    #[test]
    fn keys_and_coords() {
        let u = "https://www.google.com/maps/place/X/data=!4m7!3m6!1s0x3f8e01:0xAB12cd!8m2!3d35.7219!4d51.3347!16s%2Fg%2F1?authuser=0";
        assert_eq!(place_key(u), "0x3f8e01:0xab12cd");
        assert_eq!(coords(u), Some((35.7219, 51.3347)));
        assert_eq!(place_key("https://x/maps/place/a?b=1"), "https://x/maps/place/a");
        assert_eq!(coords("https://x"), None);
    }

    #[test]
    fn queries() {
        assert_eq!(
            search_url("کافه تهران"),
            "https://www.google.com/maps/search/%DA%A9%D8%A7%D9%81%D9%87+%D8%AA%D9%87%D8%B1%D8%A7%D9%86?hl=fa"
        );
        let t = plan_queries("کافه", "تهران", "");
        assert_eq!(t[0], "کافه تهران");
        assert!(t.len() > 30 && t[1].ends_with("تهران"));
        let k = plan_queries(" پزشک ", "البرز", "کرج");
        assert_eq!(
            k,
            vec!["پزشک کرج", "پزشک شمال کرج", "پزشک جنوب کرج", "پزشک شرق کرج", "پزشک غرب کرج", "پزشک مرکز کرج"]
        );
    }

    #[test]
    fn exports() {
        assert_eq!(csv_cell("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_cell("=SUM(1)"), "\"'=SUM(1)\"");
        let v = vcard("a;b", &["09123456789", ""], &["02188776655"], "x@y.ir", "n");
        assert!(v.contains("FN:a\\;b\r\nTEL;TYPE=CELL:09123456789\r\nTEL;TYPE=WORK:02188776655\r\nEMAIL;TYPE=WORK:x@y.ir"));
        assert!((10..=20).contains(&jitter(10, 20)));
    }

    #[test]
    fn phase3_helpers() {
        assert_eq!(intl_mobile("0912 345 6789"), "989123456789");
        assert_eq!(intl_mobile("+98 912 345 6789"), "989123456789");
        assert_eq!(intl_mobile("021-88776655"), "");

        let rows = parse_csv("\u{feff}نام,موبایل\r\n\"کافه, لمیز\",\"0912 \"\"1\"\"\"\r\n\r\nب,0935\n");
        assert_eq!(rows, vec![vec!["نام", "موبایل"], vec!["کافه, لمیز", "0912 \"1\""], vec!["ب", "0935"]]);
        let rows = parse_csv("name;phone\nA;021\n");
        assert_eq!(rows[1], vec!["A", "021"]);
        assert_eq!(parse_csv("only\nx"), vec![vec!["only"], vec!["x"]]);

        for (h, f) in [
            ("نام کسب‌وکار", "name"),
            ("نام مسئول", "person"),
            ("سمت", "role"),
            ("موبایل ۱", "mobile"),
            ("تلفن ثابت ۲", "landline"),
            ("شماره تماس", "phone"),
            ("Phone Number", "phone"),
            ("E-mail", "email"),
            ("وب‌سایت", "website"),
            ("لینک نقشه", "maps"),
            ("تلگرام", "telegram"),
            ("کلمه کلیدی", "keyword"),
            ("Company Name", "name"),
            ("تاریخ ثبت", ""),
        ] {
            assert_eq!(import_field(h), f, "{h}");
        }
        assert_eq!(split_values("0912، 0935 ; 021"), vec!["0912", "0935", "021"]);
        assert_eq!(split_values("'+98912"), vec!["+98912"]);
        assert_eq!(b64_decode("aGk/Pz8+").unwrap(), b"hi???>"); // standard alphabet, as btoa() gives
        assert_eq!(b64_decode("aGk=").unwrap(), b"hi");

        let (u, pre) = chat_link("whatsapp", "09123456789", "سلام علی").unwrap();
        assert!(pre && u.starts_with("https://web.whatsapp.com/send?phone=989123456789&text=%D8%B3"), "{u}");
        assert!(!u.contains(' '));
        assert!(chat_link("whatsapp", "02188776655", "x").is_none());
        let (u, _) = chat_link("telegram", "https://t.me/cafe_lamiz", "").unwrap();
        assert_eq!(u, "https://web.telegram.org/k/#?tgaddr=tg%3A%2F%2Fresolve%3Fdomain%3Dcafe_lamiz");
        assert_eq!(chat_link("bale", "0912", "").unwrap().0, "https://web.bale.ai/");
    }
}
