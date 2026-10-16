//! Export (Excel, CSV, vCard, mobile list) and import (Excel / CSV) of a bank.
//! Every channel has its own column; the manager's name and role are exported too.
use crate::commands::STATUSES;
use crate::db::{err, BankStats, Contact, Db, Filter, Place, Res};
use crate::logic::{csv_cell, import_field, normalize_contact, normalize_tags, parse_csv, split_values, vcard, Contacts};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const HEADERS: [&str; 29] = [
    "نام کسب‌وکار",
    "نام مسئول",
    "سمت",
    "موبایل ۱",
    "موبایل ۲",
    "موبایل ۳",
    "تلفن ثابت ۱",
    "تلفن ثابت ۲",
    "تلفن ثابت ۳",
    "ایمیل ۱",
    "ایمیل ۲",
    "وب‌سایت",
    "اینستاگرام",
    "تلگرام",
    "واتساپ",
    "ایتا",
    "بله",
    "آدرس",
    "دسته",
    "امتیاز",
    "تعداد نظر",
    "کلمه کلیدی",
    "استان",
    "شهر",
    "وضعیت",
    "برچسب‌ها",
    "یادداشت",
    "بانک",
    "لینک نقشه",
];

fn status_label(s: &str) -> &str {
    STATUSES.iter().find(|(k, _)| *k == s).map_or(s, |(_, v)| *v)
}

/// `n` columns of one contact kind; extra values go into the last column, joined with «، ».
fn spread(p: &Place, kind: &str, n: usize) -> Vec<String> {
    let vals: Vec<&str> = p.contacts.iter().filter(|c| c.kind == kind).map(|c| c.value.as_str()).collect();
    (0..n)
        .map(|i| match i {
            _ if i + 1 == n => vals.get(i..).map(|r| r.join("، ")).unwrap_or_default(),
            _ => vals.get(i).map(|s| s.to_string()).unwrap_or_default(),
        })
        .collect()
}

fn row(p: &Place, bank: &str) -> Vec<String> {
    let one = |kind: &str| p.first(kind).map(|c| c.value.clone()).unwrap_or_default();
    let mut r = vec![p.name.clone(), p.contact_person.clone(), p.contact_role.clone()];
    r.extend(spread(p, "mobile", 3));
    r.extend(spread(p, "landline", 3));
    r.extend(spread(p, "email", 2));
    r.push(p.website.clone());
    r.extend(["instagram", "telegram", "whatsapp", "eitaa", "bale"].map(one));
    r.extend([
        p.address.clone(),
        p.category.clone(),
        p.rating.map(|v| v.to_string()).unwrap_or_default(),
        p.reviews.map(|v| v.to_string()).unwrap_or_default(),
        p.keyword.clone(),
        p.province.clone(),
        p.city.clone(),
        status_label(&p.status).to_string(),
        p.tags.clone(),
        p.note.clone(),
        bank.to_string(),
        p.maps_url.clone(),
    ]);
    r
}

fn write_xlsx(rows: &[Vec<String>], stats: &BankStats, path: &Path) -> Res<()> {
    use rust_xlsxwriter::{Format, Workbook};
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("مخاطبان").map_err(err)?;
    ws.set_right_to_left(true);
    let bold = Format::new().set_bold();
    for (c, h) in HEADERS.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &bold).map_err(err)?;
    }
    for (i, r) in rows.iter().enumerate() {
        for (c, v) in r.iter().enumerate() {
            if !v.is_empty() {
                // text, not numbers: phone numbers keep their leading 0
                ws.write_string(i as u32 + 1, c as u16, v.as_str()).map_err(err)?;
            }
        }
    }
    ws.set_freeze_panes(1, 0).map_err(err)?;
    ws.autofilter(0, 0, rows.len() as u32, HEADERS.len() as u16 - 1).map_err(err)?;
    ws.autofit();

    // second sheet: the same breakdown as the «تحلیل» view
    let ws = wb.add_worksheet();
    ws.set_name("تحلیل").map_err(err)?;
    ws.set_right_to_left(true);
    let one = |rows: &[(String, i64)], f: fn(&str) -> String| rows.iter().map(|(a, n)| (vec![f(a)], *n)).collect::<Vec<_>>();
    type Block<'a> = (&'a [&'a str], Vec<(Vec<String>, i64)>); // headers, (texts, count) rows
    let blocks: [Block; 5] = [
        (&["استان", "تعداد"], one(&stats.provinces, str::to_string)),
        (
            &["شهرستان", "استان", "تعداد"],
            stats.cities.iter().map(|(p, c, n)| (vec![c.clone(), p.clone()], *n)).collect(),
        ),
        (&["حوزه کاری", "تعداد"], one(&stats.categories, str::to_string)),
        (&["کلمه کلیدی", "تعداد"], one(&stats.keywords, str::to_string)),
        (&["وضعیت", "تعداد"], one(&stats.statuses, |s| status_label(s).to_string())),
    ];
    let mut col: u16 = 0;
    for (heads, rows) in blocks {
        for (c, h) in heads.iter().enumerate() {
            ws.write_string_with_format(0, col + c as u16, *h, &bold).map_err(err)?;
        }
        for (r, (texts, n)) in rows.iter().enumerate() {
            for (c, t) in texts.iter().enumerate() {
                ws.write_string(r as u32 + 1, col + c as u16, t.as_str()).map_err(err)?;
            }
            ws.write_number(r as u32 + 1, col + texts.len() as u16, *n as f64).map_err(err)?;
        }
        col += heads.len() as u16 + 1;
    }
    ws.write_string_with_format(0, col, "کل کسب‌وکارها", &bold).map_err(err)?;
    ws.write_number(1, col, stats.total as f64).map_err(err)?;
    ws.set_freeze_panes(1, 0).map_err(err)?;
    ws.autofit();
    wb.save(path).map_err(err)
}

/// Writes the filtered places into `dir` and returns the file path.
pub fn export(db: &Db, filter: &Filter, format: &str, dir: &Path) -> Res<String> {
    let (places, _) = db.list(filter, 0, 0)?;
    if places.is_empty() {
        return Err("داده‌ای برای خروجی وجود ندارد.".into());
    }
    std::fs::create_dir_all(dir).map_err(err)?;
    let stamp = db.stamp();
    let path = (1..)
        .map(|n| {
            dir.join(if n == 1 {
                format!("hamista-{stamp}.{format}")
            } else {
                format!("hamista-{stamp}-{n}.{format}")
            })
        })
        .find(|p| !p.exists())
        .unwrap_or_default();
    let banks: HashMap<i64, String> = db.lists()?.into_iter().map(|l| (l.id, l.name)).collect();
    let rows = || places.iter().map(|p| row(p, banks.get(&p.list_id).map_or("", String::as_str)));
    let text = match format {
        "xlsx" => {
            write_xlsx(&rows().collect::<Vec<_>>(), &db.bank_stats(filter)?, &path)?;
            return Ok(path.to_string_lossy().into_owned());
        }
        "csv" => {
            let mut out = String::from("\u{FEFF}"); // BOM so Excel reads Persian correctly
            out += &HEADERS.map(csv_cell).join(",");
            out += "\r\n";
            for r in rows() {
                out += &r.iter().map(|v| csv_cell(v)).collect::<Vec<_>>().join(",");
                out += "\r\n";
            }
            out
        }
        "vcf" => places
            .iter()
            .map(|p| {
                let vals = |k: &str| p.contacts.iter().filter(|c| c.kind == k).map(|c| c.value.as_str()).collect::<Vec<_>>();
                let who = if p.contact_person.is_empty() {
                    p.name.clone()
                } else {
                    format!("{} ({})", p.contact_person, p.name)
                };
                let email = p.first("email").map_or("", |c| c.value.as_str());
                vcard(&who, &vals("mobile"), &vals("landline"), email, &format!("{} - {}", p.keyword, p.city))
            })
            .collect(),
        // one mobile per line: the format every SMS panel's bulk upload accepts
        "txt" => {
            let mut seen = HashSet::new();
            places
                .iter()
                .flat_map(|p| p.contacts.iter().filter(|c| c.kind == "mobile"))
                .filter(|c| seen.insert(c.value.as_str()))
                .map(|c| format!("{}\r\n", c.value))
                .collect()
        }
        _ => return Err("فرمت خروجی پشتیبانی نمی‌شود.".into()),
    };
    if text.is_empty() {
        return Err("در این انتخاب شماره موبایلی نیست.".into());
    }
    std::fs::write(&path, text).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

// ---------------- import ----------------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: usize,
    pub duplicates: usize, // every contact of the row was already in the bank
    pub skipped: usize,    // rows without any usable contact
    pub truncated: usize,  // rows past the 50 000 limit, not read
    pub columns: Vec<String>,
    pub ignored: Vec<String>,
}

const MAX_ROWS: usize = 50_000;

pub(crate) fn read_rows(file_name: &str, data: Vec<u8>) -> Res<Vec<Vec<String>>> {
    let lower = file_name.to_lowercase();
    if [".xlsx", ".xlsm", ".xls", ".ods"].iter().any(|e| lower.ends_with(e)) {
        use calamine::Reader;
        let mut book = calamine::open_workbook_auto_from_rs(std::io::Cursor::new(data)).map_err(|e| format!("فایل اکسل خوانده نشد: {e}"))?;
        let first = book.sheet_names().first().cloned().ok_or("فایل اکسل برگه‌ای ندارد.")?;
        let range = book.worksheet_range(&first).map_err(|e| format!("برگه «{first}» خوانده نشد: {e}"))?;
        Ok(range.rows().map(|r| r.iter().map(|c| c.to_string()).collect()).collect())
    } else {
        Ok(parse_csv(&decode_text(&data)))
    }
}

/// UTF-8 / UTF-16 (with BOM) or, failing that, Windows-1256 — what Excel on Persian Windows saves as "CSV".
fn decode_text(data: &[u8]) -> String {
    if let Some((enc, bom)) = encoding_rs::Encoding::for_bom(data) {
        return enc.decode_without_bom_handling(&data[bom..]).0.into_owned();
    }
    match std::str::from_utf8(data) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1256.decode_without_bom_handling(data).0.into_owned(),
    }
}

/// FNV-1a: a stable key for an imported row, so the same file imported twice adds nothing.
fn row_key(name: &str, first: &str) -> String {
    let h = format!("{name}|{first}")
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    format!("import:{h:016x}")
}

pub fn import(db: &Db, list_id: i64, file_name: &str, data: Vec<u8>) -> Res<ImportReport> {
    let rows = read_rows(file_name, data)?;
    let head_at = rows
        .iter()
        .take(10)
        .position(|r| r.iter().any(|h| !import_field(h).is_empty()))
        .ok_or("ستون‌ها شناخته نشد؛ ردیف اول فایل باید عنوان ستون‌ها باشد (مثل «نام»، «موبایل»، «ایمیل»).")?;
    let fields: Vec<&str> = rows[head_at].iter().map(|h| import_field(h)).collect();
    let mut rep = ImportReport::default();
    for (h, f) in rows[head_at].iter().zip(&fields) {
        let h = h.trim().to_string();
        if !h.is_empty() {
            (if f.is_empty() { &mut rep.ignored } else { &mut rep.columns }).push(h);
        }
    }
    rep.truncated = rows.len().saturating_sub(head_at + 1 + MAX_ROWS);
    for r in rows.iter().skip(head_at + 1).take(MAX_ROWS) {
        let mut p = Place {
            list_id,
            status: "new".into(),
            source: "import".into(),
            ..Default::default()
        };
        let mut c = Contacts::default();
        for (cell, field) in r.iter().zip(&fields) {
            let v = cell.trim();
            if v.is_empty() {
                continue;
            }
            // Arabic ي / ك (Windows-1256, Arabic keyboards) → Persian ی / ک, so search finds them
            let text = |max: usize| {
                v.chars()
                    .take(max)
                    .map(|ch| match ch {
                        'ي' => 'ی',
                        'ك' => 'ک',
                        c => c,
                    })
                    .collect::<String>()
            };
            match *field {
                "name" if p.name.is_empty() => p.name = text(160),
                "person" => p.contact_person = text(80),
                "role" => p.contact_role = text(40),
                "address" => p.address = text(400),
                "category" => p.category = text(80),
                "province" => p.province = text(40),
                "city" => p.city = text(40),
                "keyword" => p.keyword = text(80),
                "note" => p.note = text(2000),
                "tags" => p.tags = normalize_tags(v),
                "maps" if v.starts_with("http") => p.maps_url = text(500),
                // `9123456789.0`: a number column written by pandas & co.
                "mobile" | "landline" | "phone" => split_values(v).iter().for_each(|x| c.add_phone(x.strip_suffix(".0").unwrap_or(x))),
                "email" => split_values(v).iter().for_each(|x| c.add_email(x)),
                "website" => c.add_link(&if v.contains("://") { v.to_string() } else { format!("https://{v}") }),
                kind @ ("instagram" | "telegram" | "whatsapp" | "eitaa" | "bale") => {
                    if let Some(url) = normalize_contact(kind, v) {
                        c.add_link(&url);
                    }
                }
                _ => {}
            }
        }
        let pairs = c.pairs();
        let Some((_, first)) = pairs.first() else {
            rep.skipped += 1;
            continue;
        };
        if p.name.is_empty() {
            p.name = first.clone();
        }
        p.place_key = row_key(&p.name, first);
        p.website = c.website.clone();
        p.contacts = pairs
            .iter()
            .map(|(kind, value)| Contact {
                kind: kind.to_string(),
                value: value.clone(),
                origin: "import".into(),
                ..Default::default()
            })
            .collect();
        match db.insert_place(&p)? {
            (true, n) if n > 0 => rep.added += 1,
            _ => rep.duplicates += 1,
        }
    }
    db.log(
        "import",
        &format!("{} مخاطب از «{file_name}» وارد بانک «{}» شد", rep.added, db.list_name(list_id)),
    );
    Ok(rep)
}
