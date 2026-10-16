//! The admin's mega-bank of PUBLIC business data, merged and de-duplicated across all customers.
//! Only public business fields are stored — never a customer's private notes, status or tags.
use crate::error::AppResult;
use serde::{Deserialize, Serialize};

const MAX_EACH: usize = 6;

/// One business as sent by a desktop client (public fields only).
#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct InPlace {
    pub place_key: String,
    pub name: String,
    pub mobiles: Vec<String>,
    pub landlines: Vec<String>,
    pub emails: Vec<String>,
    pub website: String,
    pub instagram: String,
    pub telegram: String,
    pub whatsapp: String,
    pub eitaa: String,
    pub bale: String,
    pub rubika: String,
    pub address: String,
    pub province: String,
    pub city: String,
    pub category: String,
    pub source: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: i64,
    pub name: String,
    pub mobiles: String,
    pub landlines: String,
    pub emails: String,
    pub website: String,
    pub instagram: String,
    pub telegram: String,
    pub whatsapp: String,
    pub eitaa: String,
    pub bale: String,
    pub rubika: String,
    pub address: String,
    pub province: String,
    pub city: String,
    pub category: String,
    pub source: String,
    pub report_count: i64,
    pub last_seen: String,
}

/// Merge two comma-lists, keep order, drop blanks/dupes, cap at MAX_EACH.
pub fn merge_csv(existing: &str, extra: &[String]) -> String {
    let mut out: Vec<String> = existing.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    for v in extra {
        let v = v.trim();
        if !v.is_empty() && out.len() < MAX_EACH && !out.iter().any(|x| x == v) {
            out.push(v.to_string());
        }
    }
    out.join(",")
}

fn first_or(a: &str, b: &str) -> String {
    if a.trim().is_empty() {
        b.trim().to_string()
    } else {
        a.trim().to_string()
    }
}

/// Insert or merge one place. Returns (was_new, contact_values_added).
pub fn upsert(c: &rusqlite::Connection, p: &InPlace) -> AppResult<bool> {
    let key = if p.place_key.trim().is_empty() {
        // no maps id: derive a stable key from the best phone or the name+city
        let seed = p.mobiles.first().or(p.landlines.first()).cloned().unwrap_or_else(|| format!("{}|{}", p.name, p.city));
        format!("k:{}", short_hash(seed.trim()))
    } else {
        p.place_key.trim().to_string()
    };
    let existing: Option<(i64, String, String, String, String, String, String, String, String, String)> = c
        .query_row(
            "SELECT id,mobiles,landlines,emails,instagram,telegram,whatsapp,eitaa,bale,rubika FROM megabank WHERE place_key=?1",
            [&key],
            |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?,
                ))
            },
        )
        .ok();
    match existing {
        Some((id, mob, land, mail, ig, tg, wa, ei, ba, ru)) => {
            c.execute(
                "UPDATE megabank SET mobiles=?1,landlines=?2,emails=?3,instagram=?4,telegram=?5,whatsapp=?6,eitaa=?7,bale=?8,rubika=?9,
                   website=CASE WHEN website='' THEN ?10 ELSE website END,
                   address=CASE WHEN address='' THEN ?11 ELSE address END,
                   province=CASE WHEN province='' THEN ?12 ELSE province END,
                   city=CASE WHEN city='' THEN ?13 ELSE city END,
                   category=CASE WHEN category='' THEN ?14 ELSE category END,
                   report_count=report_count+1, last_seen=datetime('now')
                 WHERE id=?15",
                rusqlite::params![
                    merge_csv(&mob, &p.mobiles), merge_csv(&land, &p.landlines), merge_csv(&mail, &p.emails),
                    first_or(&ig, &p.instagram), first_or(&tg, &p.telegram), first_or(&wa, &p.whatsapp),
                    first_or(&ei, &p.eitaa), first_or(&ba, &p.bale), first_or(&ru, &p.rubika),
                    p.website.trim(), p.address.trim(), p.province.trim(), p.city.trim(), p.category.trim(), id
                ],
            )?;
            Ok(false)
        }
        None => {
            c.execute(
                "INSERT INTO megabank(place_key,name,mobiles,landlines,emails,website,instagram,telegram,whatsapp,eitaa,bale,rubika,
                   address,province,city,category,source,lat,lng)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
                rusqlite::params![
                    key, p.name.trim(), merge_csv("", &p.mobiles), merge_csv("", &p.landlines), merge_csv("", &p.emails),
                    p.website.trim(), p.instagram.trim(), p.telegram.trim(), p.whatsapp.trim(), p.eitaa.trim(), p.bale.trim(), p.rubika.trim(),
                    p.address.trim(), p.province.trim(), p.city.trim(), p.category.trim(), p.source.trim(), p.lat, p.lng
                ],
            )?;
            Ok(true)
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Search {
    pub q: String,
    pub province: String,
    pub city: String,
    pub category: String,
    pub source: String,
    pub offset: i64,
    pub limit: i64,
}

fn where_clause(s: &Search) -> (String, Vec<String>) {
    let mut sql = String::from(" WHERE 1=1");
    let mut args: Vec<String> = vec![];
    let eq = |sql: &mut String, col: &str, v: &str, args: &mut Vec<String>| {
        if !v.trim().is_empty() {
            args.push(v.trim().to_string());
            sql.push_str(&format!(" AND {col}=?{}", args.len()));
        }
    };
    eq(&mut sql, "province", &s.province, &mut args);
    eq(&mut sql, "city", &s.city, &mut args);
    eq(&mut sql, "category", &s.category, &mut args);
    eq(&mut sql, "source", &s.source, &mut args);
    if !s.q.trim().is_empty() {
        args.push(format!("%{}%", s.q.trim()));
        let i = args.len();
        sql.push_str(&format!(" AND (name LIKE ?{i} OR mobiles LIKE ?{i} OR address LIKE ?{i} OR category LIKE ?{i})"));
    }
    (sql, args)
}

pub fn search(c: &rusqlite::Connection, s: &Search) -> AppResult<(Vec<Row>, i64)> {
    let (w, args) = where_clause(s);
    let p = rusqlite::params_from_iter(args.iter());
    let total: i64 = c.query_row(&format!("SELECT count(*) FROM megabank{w}"), p, |r| r.get(0))?;
    let limit = s.limit.clamp(1, 1000);
    let offset = s.offset.max(0);
    let mut st = c.prepare(&format!(
        "SELECT id,name,mobiles,landlines,emails,website,instagram,telegram,whatsapp,eitaa,bale,rubika,address,province,city,category,source,report_count,last_seen
         FROM megabank{w} ORDER BY last_seen DESC LIMIT {limit} OFFSET {offset}"
    ))?;
    let rows = st
        .query_map(rusqlite::params_from_iter(args.iter()), |r| {
            Ok(Row {
                id: r.get(0)?, name: r.get(1)?, mobiles: r.get(2)?, landlines: r.get(3)?, emails: r.get(4)?, website: r.get(5)?,
                instagram: r.get(6)?, telegram: r.get(7)?, whatsapp: r.get(8)?, eitaa: r.get(9)?, bale: r.get(10)?, rubika: r.get(11)?,
                address: r.get(12)?, province: r.get(13)?, city: r.get(14)?, category: r.get(15)?, source: r.get(16)?,
                report_count: r.get(17)?, last_seen: r.get(18)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok((rows, total))
}

/// Facet counts for the admin panel (province / city / category / source).
pub fn facets(c: &rusqlite::Connection, col: &str, s: &Search) -> AppResult<Vec<(String, i64)>> {
    if !["province", "city", "category", "source"].contains(&col) {
        return Ok(vec![]);
    }
    let (w, args) = where_clause(s);
    let mut st = c.prepare(&format!(
        "SELECT {col}, count(*) FROM megabank{w} AND {col}<>'' GROUP BY {col} ORDER BY 2 DESC LIMIT 300"
    ))?;
    let rows = st
        .query_map(rusqlite::params_from_iter(args.iter()), |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn short_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes()).iter().take(10).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_merge_dedup_cap() {
        assert_eq!(merge_csv("", &["0912".into(), "0912".into(), "0935".into()]), "0912,0935");
        assert_eq!(merge_csv("0912", &["0935".into()]), "0912,0935");
        let many: Vec<String> = (0..10).map(|i| format!("09{i:09}")).collect();
        assert_eq!(merge_csv("", &many).split(',').count(), MAX_EACH);
    }
}
