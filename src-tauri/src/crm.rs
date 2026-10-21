//! B2B follow-up: a timeline of calls / messages / meetings / deals per business (`place_logs`) and a
//! reminder time (`places.follow_up_at`). The dashboard lists today's and overdue follow-ups, and the
//! background loop tells the user once when one falls due.
use crate::db::{err, Db, Res};
use rusqlite::params;
use serde::{Deserialize, Serialize};

pub const KINDS: &[&str] = &["call", "message", "meeting", "deal", "note"];

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PlaceLog {
    pub id: i64,
    pub place_id: i64,
    pub kind: String,
    pub note: String,
    pub created_at: String,
}

/// A business whose follow-up time has come.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Due {
    pub id: i64,
    pub name: String,
    pub person: String,
    pub phone: String,
    pub status: String,
    pub follow_up_at: String,
    /// Whole days past the reminder day (0 = today).
    pub overdue_days: i64,
    /// The last thing written on the timeline.
    pub last: String,
}

/// "YYYY-MM-DD HH:MM" (local time) or "" — anything else is refused.
pub fn valid_time(at: &str) -> bool {
    let b = at.as_bytes();
    if b.len() != 16 || b[4] != b'-' || b[7] != b'-' || b[10] != b' ' || b[13] != b':' {
        return false;
    }
    let n = |r: std::ops::Range<usize>| at[r].parse::<u32>().ok();
    matches!((n(0..4), n(5..7), n(8..10), n(11..13), n(14..16)), (Some(y), Some(m), Some(d), Some(h), Some(mi)) if (2000..=2100).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d) && h < 24 && mi < 60)
}

impl Db {
    /// Writes one interaction on the business's timeline. `next`: None keeps the reminder, Some("") clears
    /// it, Some(time) sets it — logging a call usually means «call again on …».
    pub fn place_log_add(&self, place_id: i64, kind: &str, note: &str, next: Option<&str>) -> Res<i64> {
        if !KINDS.contains(&kind) {
            return Err("نوع تعامل نامعتبر است.".into());
        }
        let note = note.trim();
        if note.is_empty() {
            return Err("متن تماس یا مذاکره را بنویسید.".into());
        }
        if note.chars().count() > 2000 {
            return Err("متن حداکثر ۲۰۰۰ حرف است.".into());
        }
        if let Some(at) = next {
            if !at.is_empty() && !valid_time(at) {
                return Err("زمان پیگیری معتبر نیست.".into());
            }
        }
        let c = self.c();
        let exists: i64 = c.query_row("SELECT count(*) FROM places WHERE id=?1", [place_id], |r| r.get(0)).map_err(err)?;
        if exists == 0 {
            return Err("مخاطب پیدا نشد.".into());
        }
        c.execute("INSERT INTO place_logs(place_id,kind,note) VALUES(?1,?2,?3)", params![place_id, kind, note]).map_err(err)?;
        let id = c.last_insert_rowid();
        c.execute("UPDATE places SET updated_at=datetime('now','localtime') WHERE id=?1", [place_id]).map_err(err)?;
        if let Some(at) = next {
            c.execute("UPDATE places SET follow_up_at=?1 WHERE id=?2", params![at, place_id]).map_err(err)?;
        }
        Ok(id)
    }

    /// The timeline of a business, newest first.
    pub fn place_logs(&self, place_id: i64) -> Res<Vec<PlaceLog>> {
        let c = self.c();
        let mut st = c.prepare("SELECT id,place_id,kind,note,created_at FROM place_logs WHERE place_id=?1 ORDER BY id DESC LIMIT 500").map_err(err)?;
        let rows = st
            .query_map([place_id], |r| Ok(PlaceLog { id: r.get(0)?, place_id: r.get(1)?, kind: r.get(2)?, note: r.get(3)?, created_at: r.get(4)? }))
            .map_err(err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(err)
    }

    pub fn place_log_delete(&self, id: i64) -> Res<()> {
        self.c().execute("DELETE FROM place_logs WHERE id=?1", [id]).map(|_| ()).map_err(err)
    }

    /// Sets or clears (empty) the next follow-up time.
    pub fn follow_up_set(&self, place_id: i64, at: &str) -> Res<()> {
        if !at.is_empty() && !valid_time(at) {
            return Err("زمان پیگیری معتبر نیست.".into());
        }
        let n = self.c().execute("UPDATE places SET follow_up_at=?1 WHERE id=?2", params![at, place_id]).map_err(err)?;
        if n == 0 {
            return Err("مخاطب پیدا نشد.".into());
        }
        Ok(())
    }

    /// Follow-ups that fall today or earlier, the most overdue first.
    pub fn follow_ups_due(&self, limit: i64) -> Res<Vec<Due>> {
        let c = self.c();
        let mut st = c
            .prepare(
                "SELECT p.id, p.name, p.contact_person, coalesce((SELECT value FROM contacts WHERE place_id=p.id AND kind='mobile' ORDER BY id LIMIT 1),''),
                        p.status, p.follow_up_at,
                        CAST(julianday(date('now','localtime')) - julianday(date(p.follow_up_at)) AS INTEGER),
                        coalesce((SELECT note FROM place_logs WHERE place_id=p.id ORDER BY id DESC LIMIT 1),'')
                 FROM places p
                 WHERE p.follow_up_at<>'' AND date(p.follow_up_at) <= date('now','localtime')
                 ORDER BY p.follow_up_at LIMIT ?1",
            )
            .map_err(err)?;
        let rows = st
            .query_map([limit], |r| {
                Ok(Due { id: r.get(0)?, name: r.get(1)?, person: r.get(2)?, phone: r.get(3)?, status: r.get(4)?, follow_up_at: r.get(5)?, overdue_days: r.get(6)?, last: r.get(7)? })
            })
            .map_err(err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(err)
    }

    /// Follow-ups whose time (not just day) has passed and that the user was not told about yet; each is
    /// marked as told, so a reminder shows once.
    pub fn follow_ups_to_notify(&self) -> Res<Vec<Due>> {
        let all: Vec<Due> = {
            let c = self.c();
            let mut st = c
                .prepare(
                    "SELECT p.id, p.name, p.contact_person, '', p.status, p.follow_up_at, 0, '' FROM places p
                     WHERE p.follow_up_at<>'' AND p.follow_up_at <= datetime('now','localtime') ORDER BY p.follow_up_at LIMIT 100",
                )
                .map_err(err)?;
            let rows = st
                .query_map([], |r| Ok(Due { id: r.get(0)?, name: r.get(1)?, person: r.get(2)?, phone: r.get(3)?, status: r.get(4)?, follow_up_at: r.get(5)?, overdue_days: r.get(6)?, last: r.get(7)? }))
                .map_err(err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(err)?
        };
        let mut fresh = vec![];
        for d in all {
            let key = format!("fu_told:{}", d.id);
            if self.kv_get(&key).as_deref() != Some(d.follow_up_at.as_str()) {
                self.kv_set(&key, &d.follow_up_at)?;
                fresh.push(d);
            }
        }
        Ok(fresh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format_is_strict() {
        assert!(valid_time("2026-10-21 09:30"));
        assert!(!valid_time(""));
        assert!(!valid_time("2026-10-21"));
        assert!(!valid_time("2026-13-21 09:30"));
        assert!(!valid_time("2026-10-21 24:00"));
        assert!(!valid_time("2026/10/21 09:30"));
        assert!(!valid_time("1999-10-21 09:30"));
    }
}
