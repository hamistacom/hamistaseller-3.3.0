//! Admin panel (issuer edition), offline half: the admin's own customer registry and every licence
//! issued. Works with no server at all; the online half (server users, plans, devices…) is `cloud::admin_*`.
use crate::db::{err, Db, Res};
use crate::license;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub phone: String,
    pub email: String,
    pub machine: String,
    pub plan: String,
    pub expires: String,
    pub status: String, // active | paused
    pub note: String,
    pub cloud_user: String, // the same customer's account on the server, if any
    pub created_at: String,
    pub updated_at: String,
    pub licenses: i64,
    pub days_left: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Licence {
    pub id: i64,
    pub customer_id: Option<i64>,
    pub customer: String,
    pub machine: String,
    pub plan: String,
    pub expires: String,
    pub code: String,
    pub issued_at: String,
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub customers: i64,
    pub active: i64,
    pub expiring: i64, // within 14 days
    pub expired: i64,
    pub licences: i64,
    pub licences_month: i64,
}

const CCOLS: &str = "c.id,c.name,c.phone,c.email,c.machine,c.plan,c.expires,c.status,c.note,c.cloud_user,c.created_at,c.updated_at,
  (SELECT count(*) FROM adm_licenses l WHERE l.customer_id=c.id),
  CASE WHEN c.expires='' THEN NULL ELSE CAST(julianday(c.expires)-julianday(date('now','localtime')) AS INTEGER) END";

fn to_customer(r: &Row) -> rusqlite::Result<Customer> {
    Ok(Customer {
        id: r.get(0)?,
        name: r.get(1)?,
        phone: r.get(2)?,
        email: r.get(3)?,
        machine: r.get(4)?,
        plan: r.get(5)?,
        expires: r.get(6)?,
        status: r.get(7)?,
        note: r.get(8)?,
        cloud_user: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        licenses: r.get(12)?,
        days_left: r.get(13)?,
    })
}

/// `view`: all | active | expiring (≤14 days) | expired | paused
pub fn customers(db: &Db, q: &str, view: &str) -> Res<Vec<Customer>> {
    let cond = match view {
        "active" => "c.status='active' AND (c.expires='' OR c.expires>=date('now','localtime'))",
        "expiring" => "c.status='active' AND c.expires<>'' AND c.expires BETWEEN date('now','localtime') AND date('now','localtime','+14 days')",
        "expired" => "c.expires<>'' AND c.expires<date('now','localtime')",
        "paused" => "c.status='paused'",
        _ => "1=1",
    };
    let like = format!("%{}%", q.trim());
    let c = db.c();
    let mut st = c
        .prepare(&format!(
            "SELECT {CCOLS} FROM adm_customers c WHERE {cond}
               AND (?1='%%' OR c.name LIKE ?1 OR c.phone LIKE ?1 OR c.email LIKE ?1 OR c.machine LIKE ?1 OR c.cloud_user LIKE ?1)
             ORDER BY CASE WHEN c.expires='' THEN 1 ELSE 0 END, c.expires, c.id DESC LIMIT 2000"
        ))
        .map_err(err)?;
    let rows = st.query_map([like], to_customer).map_err(err)?.collect::<Result<Vec<_>, _>>().map_err(err)?;
    Ok(rows)
}

pub fn customer(db: &Db, id: i64) -> Res<Customer> {
    db.c()
        .query_row(&format!("SELECT {CCOLS} FROM adm_customers c WHERE c.id=?1"), [id], to_customer)
        .optional()
        .map_err(err)?
        .ok_or_else(|| "مشتری پیدا نشد.".into())
}

fn clean_machine(m: &str) -> Res<String> {
    let m = m.trim().to_uppercase();
    if m.is_empty() || (m.len() == 19 && crate::logic::machine_code(&m.replace('-', "")) == m) {
        Ok(m)
    } else {
        Err("کد سیستم باید به شکل XXXX-XXXX-XXXX-XXXX باشد.".into())
    }
}

pub fn customer_save(db: &Db, c: &Customer) -> Res<i64> {
    let name = c.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("نام مشتری را وارد کنید (حداکثر ۸۰ حرف).".into());
    }
    if !c.expires.is_empty() && !crate::logic::valid_date(&c.expires) {
        return Err("تاریخ اعتبار نامعتبر است.".into());
    }
    let status = if c.status == "paused" { "paused" } else { "active" };
    let machine = clean_machine(&c.machine)?;
    let conn = db.c();
    let args = params![name, c.phone.trim(), c.email.trim(), machine, c.plan.trim(), c.expires.trim(), status, c.note.trim(), c.cloud_user.trim()];
    if c.id == 0 {
        conn.execute(
            "INSERT INTO adm_customers(name,phone,email,machine,plan,expires,status,note,cloud_user) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            args,
        )
        .map_err(err)?;
        Ok(conn.last_insert_rowid())
    } else {
        let n = conn
            .execute(
                "UPDATE adm_customers SET name=?1,phone=?2,email=?3,machine=?4,plan=?5,expires=?6,status=?7,note=?8,cloud_user=?9,
                   updated_at=datetime('now','localtime') WHERE id=?10",
                params![name, c.phone.trim(), c.email.trim(), machine, c.plan.trim(), c.expires.trim(), status, c.note.trim(), c.cloud_user.trim(), c.id],
            )
            .map_err(err)?;
        if n == 0 {
            return Err("مشتری پیدا نشد.".into());
        }
        Ok(c.id)
    }
}

/// Removes the customer; their issued licences stay in the history (unlinked).
pub fn customer_delete(db: &Db, id: i64) -> Res<()> {
    let c = db.c();
    c.execute("UPDATE adm_licenses SET customer_id=NULL WHERE customer_id=?1", [id]).map_err(err)?;
    c.execute("DELETE FROM adm_customers WHERE id=?1", [id]).map_err(err)?;
    Ok(())
}

/// Signs an offline licence, records it, and keeps the customer's card current
/// (creates the customer when `customer_id` is 0).
pub fn issue(db: &Db, customer_id: i64, machine: &str, name: &str, plan: &str, expires: &str) -> Res<(String, i64)> {
    let code = license::issue(db, machine, name, plan, expires)?;
    let machine = machine.trim().to_uppercase();
    let id = if customer_id > 0 {
        let mut c = customer(db, customer_id)?;
        (c.machine, c.plan, c.expires) = (machine.clone(), plan.trim().into(), expires.trim().into());
        if !name.trim().is_empty() {
            c.name = name.trim().into();
        }
        customer_save(db, &c)?
    } else {
        // same PC issued before → same customer card
        let known: Option<i64> = db
            .c()
            .query_row("SELECT id FROM adm_customers WHERE machine=?1 ORDER BY id DESC LIMIT 1", [&machine], |r| r.get(0))
            .optional()
            .map_err(err)?;
        let mut c = match known {
            Some(id) => customer(db, id)?,
            None => Customer { name: if name.trim().is_empty() { "بدون نام".into() } else { name.trim().into() }, ..Default::default() },
        };
        (c.machine, c.plan, c.expires) = (machine.clone(), plan.trim().into(), expires.trim().into());
        customer_save(db, &c)?
    };
    db.c()
        .execute(
            "INSERT INTO adm_licenses(customer_id,customer,machine,plan,expires,code) VALUES(?1,?2,?3,?4,?5,?6)",
            params![id, name.trim(), machine, plan.trim(), expires.trim(), code],
        )
        .map_err(err)?;
    Ok((code, id))
}

pub fn licences(db: &Db, customer_id: i64, limit: i64) -> Res<Vec<Licence>> {
    let c = db.c();
    let mut st = c
        .prepare(
            "SELECT id,customer_id,customer,machine,plan,expires,code,issued_at FROM adm_licenses
             WHERE (?1=0 OR customer_id=?1) ORDER BY id DESC LIMIT ?2",
        )
        .map_err(err)?;
    let rows = st
        .query_map(params![customer_id, limit.clamp(1, 5000)], |r| {
            Ok(Licence {
                id: r.get(0)?,
                customer_id: r.get(1)?,
                customer: r.get(2)?,
                machine: r.get(3)?,
                plan: r.get(4)?,
                expires: r.get(5)?,
                code: r.get(6)?,
                issued_at: r.get(7)?,
            })
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    Ok(rows)
}

pub fn stats(db: &Db) -> Res<Stats> {
    db.c()
        .query_row(
            "SELECT (SELECT count(*) FROM adm_customers),
               (SELECT count(*) FROM adm_customers WHERE status='active' AND (expires='' OR expires>=date('now','localtime'))),
               (SELECT count(*) FROM adm_customers WHERE status='active' AND expires<>'' AND expires BETWEEN date('now','localtime') AND date('now','localtime','+14 days')),
               (SELECT count(*) FROM adm_customers WHERE expires<>'' AND expires<date('now','localtime')),
               (SELECT count(*) FROM adm_licenses),
               (SELECT count(*) FROM adm_licenses WHERE issued_at>=date('now','localtime','start of month'))",
            [],
            |r| {
                Ok(Stats {
                    customers: r.get(0)?,
                    active: r.get(1)?,
                    expiring: r.get(2)?,
                    expired: r.get(3)?,
                    licences: r.get(4)?,
                    licences_month: r.get(5)?,
                })
            },
        )
        .map_err(err)
}

/// Customer registry as CSV text (UTF-8 BOM for Excel; formula-looking cells neutralised).
pub fn customers_csv(db: &Db) -> Res<String> {
    let cell = |s: &str| {
        let s = if s.starts_with(['=', '+', '-', '@']) { format!("'{s}") } else { s.to_string() };
        format!("\"{}\"", s.replace('"', "\"\""))
    };
    let mut out = String::from("\u{feff}");
    out += &["نام", "تلفن", "ایمیل", "کد سیستم", "نوع", "اعتبار تا", "وضعیت", "حساب آنلاین", "تعداد لایسنس", "یادداشت"]
        .map(cell)
        .join(",");
    out += "\r\n";
    for c in customers(db, "", "all")? {
        let row = [&c.name, &c.phone, &c.email, &c.machine, &c.plan, &c.expires, &c.status, &c.cloud_user, &c.licenses.to_string(), &c.note];
        out += &row.map(|v| cell(v)).join(",");
        out += "\r\n";
    }
    Ok(out)
}
