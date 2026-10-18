//! The world's own tables, kept the same on every guest's copy: what kinds
//! of things there are and their code, what stands where, the regions, the
//! people, the species, the shaped ground. A thread on the host looks over
//! the world's file when it changes and sends each guest the rows that
//! differ from what they have; the guest writes them into its copy and,
//! when the world itself changed, makes it anew from them.

use super::wire::{Down, Rows, Val};
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;

/// A table kept the same: its name, its key columns, and the columns that
/// say it changed (all, when none are named).
struct Table {
    name: &'static str,
    key: &'static [&'static str],
    watch: &'static [&'static str],
    /// A change to it means making the world anew.
    rebuild: bool,
}

const TABLES: &[Table] = &[
    Table { name: "universe", key: &["id"], watch: &[], rebuild: true },
    Table { name: "versions", key: &["id"], watch: &[], rebuild: true },
    Table { name: "types", key: &["id"], watch: &[], rebuild: true },
    Table { name: "instances", key: &["id"], watch: &[], rebuild: true },
    Table { name: "regions", key: &["rx", "rz"], watch: &[], rebuild: true },
    // A person's state is theirs to move (sent live); who they are is the world's.
    Table { name: "characters", key: &["id"], watch: &["id", "region", "persona_json", "home_x", "home_z", "version_id"], rebuild: true },
    Table { name: "species", key: &["name"], watch: &[], rebuild: true },
    Table { name: "gestures", key: &["name"], watch: &[], rebuild: false },
    // Who made what (the creations page, and what inspecting a thing says).
    Table { name: "creations", key: &["key"], watch: &[], rebuild: false },
    Table { name: "origins", key: &["instance_id"], watch: &[], rebuild: false },
    Table { name: "kv", key: &["key"], watch: &[], rebuild: false },
];

/// The settings kept the same (the rest of `kv` is the host's own or live).
const KV: &[&str] = &["spawn", "species.world", "genesis", "sim.dug", "sim.ground", "sim.caves"];

/// Of these, the ones that make the world anew.
const KV_REBUILD: &[&str] = &["spawn", "species.world"];

/// Every row of the kept tables: (table, key) → (what it is, the row).
pub type Snapshot = HashMap<(String, Vec<u8>), (u64, Vec<Val>)>;

fn val(v: rusqlite::types::ValueRef) -> Val {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => Val::Null,
        ValueRef::Integer(i) => Val::Int(i),
        ValueRef::Real(r) => Val::Real(r),
        ValueRef::Text(t) => Val::Text(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Val::Blob(b.to_vec()),
    }
}

impl rusqlite::ToSql for Val {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        use rusqlite::types::{ToSqlOutput, Value};
        Ok(ToSqlOutput::Owned(match self {
            Val::Null => Value::Null,
            Val::Int(i) => Value::Integer(*i),
            Val::Real(r) => Value::Real(*r),
            Val::Text(t) => Value::Text(t.clone()),
            Val::Blob(b) => Value::Blob(b.clone()),
        }))
    }
}

fn columns(c: &Connection, table: &str) -> Result<Vec<String>> {
    let mut st = c.prepare(&format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid"))?;
    let rows = st.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Read the kept tables as they are.
pub fn read(c: &Connection) -> Result<(Snapshot, BTreeMap<&'static str, Vec<String>>)> {
    let mut out = Snapshot::new();
    let mut cols = BTreeMap::new();
    for t in TABLES {
        let names = columns(c, t.name)?;
        let key_at: Vec<usize> = t.key.iter().filter_map(|k| names.iter().position(|n| n == k)).collect();
        let watch_at: Vec<usize> = if t.watch.is_empty() { (0..names.len()).collect() } else { t.watch.iter().filter_map(|k| names.iter().position(|n| n == k)).collect() };
        let mut st = c.prepare(&format!("SELECT * FROM {}", t.name))?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let row: Vec<Val> = (0..names.len()).map(|i| r.get_ref(i).map(val).unwrap_or(Val::Null)).collect();
            if t.name == "kv" && !matches!(&row[key_at[0]], Val::Text(k) if KV.contains(&k.as_str())) {
                continue;
            }
            let key: Vec<Val> = key_at.iter().map(|i| row[*i].clone()).collect();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for i in &watch_at {
                super::encode(&row[*i]).hash(&mut h);
            }
            out.insert((t.name.to_string(), super::encode(&key)), (h.finish(), row));
        }
        cols.insert(t.name, names);
    }
    Ok((out, cols))
}

/// What a guest with `had` lacks of `now`: changed rows by table, and
/// whether the world must be made anew.
pub fn diff(had: &Snapshot, now: &Snapshot, cols: &BTreeMap<&'static str, Vec<String>>) -> (Vec<Rows>, bool) {
    let mut by: BTreeMap<&str, Rows> = BTreeMap::new();
    let mut rebuild = false;
    let table = |name: &str| TABLES.iter().find(|t| t.name == name);
    let touch = |name: &str, row: Option<&Vec<Val>>, rebuild: &mut bool| {
        let Some(t) = table(name) else { return None };
        let kv_key = row.and_then(|r| r.first()).and_then(|v| match v {
            Val::Text(k) => Some(k.clone()),
            _ => None,
        });
        if t.rebuild || (t.name == "kv" && kv_key.is_some_and(|k| KV_REBUILD.contains(&k.as_str()))) {
            *rebuild = true;
        }
        Some(t.name)
    };
    for ((name, k), (h, row)) in now {
        if had.get(&(name.clone(), k.clone())).is_some_and(|(x, _)| x == h) {
            continue;
        }
        let Some(n) = touch(name, Some(row), &mut rebuild) else { continue };
        let r = by.entry(n).or_insert_with(|| Rows { table: n.to_string(), columns: cols.get(n).cloned().unwrap_or_default(), ..Default::default() });
        r.rows.push(row.clone());
    }
    for ((name, key), (_, row)) in had {
        if now.contains_key(&(name.clone(), key.clone())) {
            continue;
        }
        let Some(n) = touch(name, Some(row), &mut rebuild) else { continue };
        let r = by.entry(n).or_insert_with(|| Rows { table: n.to_string(), columns: cols.get(n).cloned().unwrap_or_default(), ..Default::default() });
        r.gone.push(key_of(name, row, cols));
    }
    (by.into_values().collect(), rebuild)
}

fn key_of(table: &str, row: &[Val], cols: &BTreeMap<&'static str, Vec<String>>) -> Vec<Val> {
    let Some(t) = TABLES.iter().find(|t| t.name == table) else { return Vec::new() };
    let names = cols.get(t.name).map(|v| v.as_slice()).unwrap_or(&[]);
    t.key.iter().filter_map(|k| names.iter().position(|n| n == k)).map(|i| row[i].clone()).collect()
}

/// Write rows the host sent into a guest's copy. True when the made
/// ground changed (pits, ways: put back on the land).
pub fn apply(db: &crate::db::Db, tables: &[Rows]) -> Result<bool> {
    let mut ground = false;
    db.tx(|c| {
        for t in tables {
            let Some(spec) = TABLES.iter().find(|x| x.name == t.table) else { continue };
            if t.columns.is_empty() {
                continue;
            }
            let marks = vec!["?"; t.columns.len()].join(", ");
            let sql = format!("INSERT OR REPLACE INTO {} ({}) VALUES ({marks})", spec.name, t.columns.join(", "));
            for r in &t.rows {
                if r.len() != t.columns.len() {
                    continue;
                }
                c.execute(&sql, rusqlite::params_from_iter(r.iter()))?;
                if spec.name == "kv" {
                    ground |= matches!(r.first(), Some(Val::Text(k)) if k.starts_with("sim."));
                }
            }
            let del = format!("DELETE FROM {} WHERE {}", spec.name, spec.key.iter().map(|k| format!("{k} = ?")).collect::<Vec<_>>().join(" AND "));
            for k in &t.gone {
                c.execute(&del, rusqlite::params_from_iter(k.iter()))?;
            }
        }
        Ok(())
    })?;
    Ok(ground)
}

/// Joining or leaving the watch.
pub enum Watch {
    /// A guest who has the world as `had`.
    Add(u8, Sender<Down>, Snapshot),
    Remove(u8),
}

/// Watch the world's file and keep every guest's copy of its tables the
/// same. Stops when `rx` closes.
pub fn watch(path: PathBuf, rx: Receiver<Watch>) {
    let open = || Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok();
    let mut c = open();
    let mut guests: BTreeMap<u8, (Sender<Down>, Snapshot)> = BTreeMap::new();
    let mut seen_version: Option<i64> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(700)) {
            Ok(Watch::Add(no, tx, had)) => {
                guests.insert(no, (tx, had));
                seen_version = None;
            }
            Ok(Watch::Remove(no)) => {
                guests.remove(&no);
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
        }
        if guests.is_empty() {
            continue;
        }
        if c.is_none() {
            c = open();
        }
        let Some(conn) = c.as_ref() else { continue };
        // Only when someone wrote the file since.
        let v: Option<i64> = conn.query_row("PRAGMA data_version", [], |r| r.get(0)).ok();
        if v.is_some() && v == seen_version {
            continue;
        }
        let (now, cols) = match read(conn) {
            Ok(x) => x,
            Err(e) => {
                crate::log::error(format!("watching the world for guests: {e:#}"));
                c = None;
                continue;
            }
        };
        seen_version = v;
        for (tx, had) in guests.values_mut() {
            let (tables, rebuild) = diff(had, &now, &cols);
            if !tables.is_empty() {
                let _ = tx.send(Down::Rows { tables, rebuild });
            }
            *had = now.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_is_kept_the_same_row_by_row() {
        let host = crate::db::Db::from_bytes(&std::fs::read(world()).unwrap()).unwrap();
        let guest = crate::db::Db::from_bytes(&std::fs::read(world()).unwrap()).unwrap();
        let had = host.with(|c| read(c)).unwrap().0;
        host.with(|c| {
            c.execute("INSERT INTO versions(parent_id, kind, summary, created_at) VALUES (NULL, 'make', 'a well', 1)", [])?;
            c.execute("INSERT INTO kv(key, value) VALUES ('sim.dug', '[]')", [])?;
            c.execute("INSERT INTO kv(key, value) VALUES ('sim.night', 'the host''s own')", [])?;
            c.execute("UPDATE characters SET state_json = '{\"x\": 4}'", [])?;
            Ok(())
        })
        .unwrap();
        let (now, cols) = host.with(|c| read(c)).unwrap();
        let (tables, rebuild) = diff(&had, &now, &cols);
        assert!(rebuild, "a new version makes the world anew");
        let names: Vec<&str> = tables.iter().map(|t| t.table.as_str()).collect();
        assert_eq!(names, vec!["kv", "versions"], "a person's moving isn't the world changing; the host's own settings stay");
        assert!(apply(&guest, &tables).unwrap(), "the made ground changed");
        let n: i64 = guest.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM versions WHERE summary = 'a well'", [], |r| r.get(0))?)).unwrap();
        assert_eq!(n, 1);
        assert_eq!(guest.kv_get("sim.night"), None);
        // Gone on the host, gone on the guest.
        host.with(|c| Ok(c.execute("DELETE FROM kv WHERE key = 'sim.dug'", [])?)).unwrap();
        let (later, cols) = host.with(|c| read(c)).unwrap();
        let (tables, _) = diff(&now, &later, &cols);
        apply(&guest, &tables).unwrap();
        assert_eq!(guest.kv_get("sim.dug"), None);
    }

    fn world() -> PathBuf {
        let p = std::env::temp_dir().join(format!("pocket-rows-{}.pocket", std::process::id()));
        if !p.exists() {
            crate::db::Db::create(&p, 3, "rows", "{}").unwrap();
        }
        p
    }
}
