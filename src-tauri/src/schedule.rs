//! Scheduled extractions («زمان‌بندی»): once, daily or weekly at a set time. The app (open or in the
//! Windows tray) checks every half minute and starts a due one when no other job is running.
//! Times are the PC's local time; next runs are computed by SQLite's date functions.
use crate::db::{err, Db, Res};
use crate::scanner::ScanRequest;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Schedule {
    pub id: i64,
    pub name: String,
    pub req: ScanRequest,
    pub every: String,   // once | daily | weekly
    pub at: String,      // HH:MM
    pub weekday: i64,    // weekly: 0 = Sunday … 6 = Saturday (SQLite %w)
    pub on_date: String, // once: YYYY-MM-DD
    pub enabled: bool,
    pub next_run: String, // YYYY-MM-DD HH:MM ('' = not scheduled)
    pub last_run: String,
    pub last_result: String,
    pub created_at: String,
}

const COLS: &str = "id,name,req,every,at,weekday,on_date,enabled,next_run,last_run,last_result,created_at";

fn to_schedule(r: &Row) -> rusqlite::Result<Schedule> {
    Ok(Schedule {
        id: r.get(0)?,
        name: r.get(1)?,
        req: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default(),
        every: r.get(3)?,
        at: r.get(4)?,
        weekday: r.get(5)?,
        on_date: r.get(6)?,
        enabled: r.get::<_, i64>(7)? != 0,
        next_run: r.get(8)?,
        last_run: r.get(9)?,
        last_result: r.get(10)?,
        created_at: r.get(11)?,
    })
}

fn valid_time(t: &str) -> bool {
    let b = t.as_bytes();
    b.len() == 5 && b[2] == b':' && t[..2].parse::<u32>().is_ok_and(|h| h < 24) && t[3..].parse::<u32>().is_ok_and(|m| m < 60)
}

/// Next run after now (local time), or "" when a one-off date has passed.
pub fn next_run(db: &Db, every: &str, at: &str, weekday: i64, on_date: &str) -> String {
    let c = db.c();
    let q = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Option<String> { c.query_row(sql, p, |r| r.get::<_, Option<String>>(0)).ok().flatten() };
    match every {
        "once" => q(
            "SELECT CASE WHEN datetime(?1||' '||?2) > datetime('now','localtime') THEN strftime('%Y-%m-%d %H:%M', ?1||' '||?2) END",
            &[&on_date, &at],
        )
        .unwrap_or_default(),
        "daily" => q(
            "SELECT strftime('%Y-%m-%d %H:%M', CASE WHEN datetime(date('now','localtime')||' '||?1) > datetime('now','localtime')
               THEN date('now','localtime')||' '||?1 ELSE date('now','localtime','+1 day')||' '||?1 END)",
            &[&at],
        )
        .unwrap_or_default(),
        _ => (0..8)
            .find_map(|k| {
                q(
                    "SELECT strftime('%Y-%m-%d %H:%M', d) FROM (SELECT date('now','localtime', ?1)||' '||?2 AS d)
                     WHERE CAST(strftime('%w', d) AS INTEGER)=?3 AND datetime(d) > datetime('now','localtime')",
                    &[&format!("+{k} days"), &at, &weekday],
                )
            })
            .unwrap_or_default(),
    }
}

pub fn list(db: &Db) -> Res<Vec<Schedule>> {
    let c = db.c();
    let mut st = c
        .prepare(&format!(
            "SELECT {COLS} FROM schedules ORDER BY enabled DESC, CASE WHEN next_run='' THEN 1 ELSE 0 END, next_run, id DESC"
        ))
        .map_err(err)?;
    let rows = st.query_map([], to_schedule).map_err(err)?.collect::<Result<Vec<_>, _>>().map_err(err)?;
    Ok(rows)
}

pub fn get(db: &Db, id: i64) -> Res<Schedule> {
    db.c()
        .query_row(&format!("SELECT {COLS} FROM schedules WHERE id=?1"), [id], to_schedule)
        .optional()
        .map_err(err)?
        .ok_or_else(|| "زمان‌بندی پیدا نشد.".into())
}

pub fn save(db: &Db, s: &Schedule) -> Res<i64> {
    let name = s.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err("یک نام کوتاه برای زمان‌بندی بنویسید.".into());
    }
    if !["once", "daily", "weekly"].contains(&s.every.as_str()) {
        return Err("نوع تکرار نامعتبر است.".into());
    }
    if !valid_time(&s.at) {
        return Err("ساعت را به شکل ۲۴ساعته (مثلاً 09:30) وارد کنید.".into());
    }
    if s.every == "weekly" && !(0..=6).contains(&s.weekday) {
        return Err("روز هفته را انتخاب کنید.".into());
    }
    if s.every == "once" && !crate::logic::valid_date(&s.on_date) {
        return Err("تاریخ اجرا را انتخاب کنید.".into());
    }
    if s.req.keywords.iter().all(|k| k.trim().is_empty()) {
        return Err("حداقل یک کلمه کلیدی وارد کنید.".into());
    }
    if !db.list_exists(s.req.list_id) {
        return Err("بانک مقصد را انتخاب کنید.".into());
    }
    let next = next_run(db, &s.every, &s.at, s.weekday, &s.on_date);
    if s.every == "once" && next.is_empty() {
        return Err("زمان انتخاب‌شده گذشته است.".into());
    }
    let req = serde_json::to_string(&ScanRequest {
        first_limit: 0,
        lookups: vec![],
        rest: vec![],
        ..s.req.clone()
    })
    .map_err(err)?;
    let c = db.c();
    if s.id == 0 {
        c.execute(
            "INSERT INTO schedules(name,req,every,at,weekday,on_date,enabled,next_run) VALUES(?1,?2,?3,?4,?5,?6,1,?7)",
            params![name, req, s.every, s.at, s.weekday, s.on_date, next],
        )
        .map_err(err)?;
        Ok(c.last_insert_rowid())
    } else {
        c.execute(
            "UPDATE schedules SET name=?1,req=?2,every=?3,at=?4,weekday=?5,on_date=?6,enabled=CASE WHEN ?3='once' THEN 1 ELSE enabled END,next_run=CASE WHEN ?3='once' OR enabled=1 THEN ?7 ELSE '' END WHERE id=?8",
            params![name, req, s.every, s.at, s.weekday, s.on_date, next, s.id],
        )
        .map_err(err)?;
        Ok(s.id)
    }
}

pub fn set_enabled(db: &Db, id: i64, on: bool) -> Res<()> {
    let s = get(db, id)?;
    let next = if on {
        next_run(db, &s.every, &s.at, s.weekday, &s.on_date)
    } else {
        String::new()
    };
    if on && next.is_empty() {
        return Err("زمان این اجرای یک‌باره گذشته است؛ تاریخ تازه‌ای بگذارید.".into());
    }
    db.c()
        .execute("UPDATE schedules SET enabled=?1, next_run=?2 WHERE id=?3", params![on as i64, next, id])
        .map(|_| ())
        .map_err(err)
}

pub fn delete(db: &Db, id: i64) -> Res<()> {
    db.c().execute("DELETE FROM schedules WHERE id=?1", [id]).map(|_| ()).map_err(err)
}

/// The earliest enabled schedule whose time has come.
pub fn due(db: &Db) -> Option<Schedule> {
    db.c()
        .query_row(
            &format!(
                "SELECT {COLS} FROM schedules WHERE enabled=1 AND next_run<>'' AND next_run <= strftime('%Y-%m-%d %H:%M','now','localtime')
                 ORDER BY next_run LIMIT 1"
            ),
            [],
            to_schedule,
        )
        .optional()
        .ok()
        .flatten()
}

/// Records the run and moves the schedule on (a one-off is switched off).
pub fn finished(db: &Db, id: i64, result: &str) {
    let Ok(s) = get(db, id) else { return };
    let next = if s.every == "once" {
        String::new()
    } else {
        next_run(db, &s.every, &s.at, s.weekday, &s.on_date)
    };
    let _ = db.c().execute(
        "UPDATE schedules SET last_run=strftime('%Y-%m-%d %H:%M','now','localtime'), last_result=?1, next_run=?2,
           enabled=CASE WHEN every='once' THEN 0 ELSE enabled END WHERE id=?3",
        params![result, next, id],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert!(valid_time("09:30") && valid_time("23:59") && valid_time("00:00"));
        assert!(!valid_time("24:00") && !valid_time("9:30") && !valid_time("09-30"));
    }
}
