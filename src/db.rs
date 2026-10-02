//! One `.pocket` file = one SQLite database. Only LLM-written source is stored;
//! WGSL and bytecode are rebuilt on load. Rollback-journal mode keeps the file
//! self-contained, so copying it copies the world.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Arc;

pub struct Db {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS universe(id INTEGER PRIMARY KEY, seed INTEGER NOT NULL, bible TEXT NOT NULL, palette TEXT NOT NULL, created_at REAL NOT NULL);
CREATE TABLE IF NOT EXISTS versions(id INTEGER PRIMARY KEY, parent_id INTEGER, kind TEXT NOT NULL, summary TEXT NOT NULL, created_at REAL NOT NULL, reverts INTEGER);
CREATE TABLE IF NOT EXISTS types(id INTEGER PRIMARY KEY, version_id INTEGER, name TEXT NOT NULL, code TEXT NOT NULL, meta_json TEXT NOT NULL, status TEXT NOT NULL, errors TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS instances(id INTEGER PRIMARY KEY, type_id INTEGER NOT NULL, version_id INTEGER NOT NULL, chunk_x INTEGER NOT NULL, chunk_z INTEGER NOT NULL, x REAL NOT NULL, y REAL NOT NULL, z REAL NOT NULL, rot_y REAL NOT NULL, scale REAL NOT NULL, params_json TEXT NOT NULL, removed_version_id INTEGER);
CREATE INDEX IF NOT EXISTS instances_chunk ON instances(chunk_x, chunk_z);
CREATE TABLE IF NOT EXISTS regions(rx INTEGER NOT NULL, rz INTEGER NOT NULL, status TEXT NOT NULL, plan_json TEXT NOT NULL DEFAULT '', version_id INTEGER, PRIMARY KEY(rx, rz));
CREATE TABLE IF NOT EXISTS characters(id INTEGER PRIMARY KEY, region TEXT NOT NULL, persona_json TEXT NOT NULL, home_x REAL NOT NULL, home_z REAL NOT NULL, state_json TEXT NOT NULL DEFAULT '{}', version_id INTEGER);
CREATE TABLE IF NOT EXISTS memories(id INTEGER PRIMARY KEY, character_id INTEGER NOT NULL, t REAL NOT NULL, text TEXT NOT NULL, importance REAL NOT NULL);
CREATE INDEX IF NOT EXISTS memories_char ON memories(character_id);
CREATE TABLE IF NOT EXISTS summaries(character_id INTEGER PRIMARY KEY, text TEXT NOT NULL, updated_at REAL NOT NULL);
CREATE TABLE IF NOT EXISTS player(id INTEGER PRIMARY KEY CHECK (id = 1), x REAL NOT NULL, z REAL NOT NULL, yaw REAL NOT NULL, t_game REAL NOT NULL);
CREATE TABLE IF NOT EXISTS llm_usage(t REAL NOT NULL, purpose TEXT NOT NULL, input_tokens INTEGER NOT NULL, output_tokens INTEGER NOT NULL, cost REAL NOT NULL);
CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS things(id INTEGER PRIMARY KEY, type_id INTEGER NOT NULL, x REAL NOT NULL, y REAL NOT NULL, z REAL NOT NULL, yaw REAL NOT NULL, scale REAL NOT NULL, params_json TEXT NOT NULL, state_json TEXT NOT NULL, props_json TEXT NOT NULL, holder TEXT, co_holder TEXT, asleep INTEGER NOT NULL, anchored INTEGER NOT NULL, origin_json TEXT NOT NULL, born REAL NOT NULL);
CREATE TABLE IF NOT EXISTS cells(gx INTEGER NOT NULL, gz INTEGER NOT NULL, props_json TEXT NOT NULL, active INTEGER NOT NULL, PRIMARY KEY(gx, gz));
CREATE TABLE IF NOT EXISTS relationships(a INTEGER NOT NULL, b INTEGER NOT NULL, json TEXT NOT NULL, PRIMARY KEY(a, b));
CREATE TABLE IF NOT EXISTS rules(id INTEGER PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS vocab(name TEXT PRIMARY KEY, meaning TEXT NOT NULL, default_value REAL NOT NULL);
CREATE TABLE IF NOT EXISTS interp_cache(key TEXT PRIMARY KEY, effect_json TEXT NOT NULL, t REAL NOT NULL);
CREATE TABLE IF NOT EXISTS events(id INTEGER PRIMARY KEY, t REAL NOT NULL, kind TEXT NOT NULL, actor TEXT, subject TEXT, json TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS events_subject ON events(subject);
CREATE TABLE IF NOT EXISTS origins(instance_id INTEGER PRIMARY KEY, made_by TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS gestures(name TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS species(name TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS thing_shapes(id INTEGER PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS spent_cells(gx INTEGER NOT NULL, gz INTEGER NOT NULL, t REAL NOT NULL, PRIMARY KEY(gx, gz));
"#;

pub fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

#[derive(Clone, Debug)]
pub struct UniverseRow {
    pub seed: u32,
    pub bible: String,
    pub look_json: String,
}

#[derive(Clone, Debug)]
pub struct VersionRow {
    pub id: i64,
    pub kind: String,
    pub summary: String,
    pub created_at: f64,
    pub reverts: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct TypeRow {
    pub id: i64,
    pub version: Option<i64>,
    pub name: String,
    pub code: String,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct InstRow {
    pub id: i64,
    pub type_id: i64,
    pub version: i64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_y: f32,
    pub scale: f32,
    pub params: [f32; 8],
    pub removed: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct RegionRow {
    pub rx: i32,
    pub rz: i32,
    pub status: String,
    pub plan_json: String,
    pub version: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct CharRow {
    pub id: i64,
    pub region: String,
    pub persona_json: String,
    pub home_x: f32,
    pub home_z: f32,
    pub state_json: String,
    pub version: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct MemoryRow {
    pub id: i64,
    pub text: String,
    pub importance: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PlayerRow {
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    pub t_game: f64,
}

impl Db {
    fn configure(conn: &Connection) -> Result<()> {
        conn.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=OFF;")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    pub fn create(path: &Path, seed: u32, bible: &str, look_json: &str) -> Result<Arc<Db>> {
        if path.exists() {
            anyhow::bail!("{} already exists", path.display());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        let conn = Connection::open(path).with_context(|| format!("creating {}", path.display()))?;
        Self::configure(&conn)?;
        conn.execute("INSERT INTO universe(id, seed, bible, palette, created_at) VALUES (1, ?1, ?2, ?3, ?4)", params![seed as i64, bible, look_json, now()])?;
        Ok(Arc::new(Db { conn: Mutex::new(conn) }))
    }

    pub fn open(path: &Path) -> Result<Arc<Db>> {
        if !path.exists() {
            anyhow::bail!("{} does not exist", path.display());
        }
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        Self::configure(&conn)?;
        Ok(Arc::new(Db { conn: Mutex::new(conn) }))
    }

    /// Run `f` inside one transaction.
    pub fn tx<T>(&self, f: impl FnOnce(&rusqlite::Transaction) -> Result<T>) -> Result<T> {
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        let r = f(&tx)?;
        tx.commit()?;
        Ok(r)
    }

    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let c = self.conn.lock();
        f(&c)
    }

    pub fn universe(&self) -> Result<UniverseRow> {
        self.with(|c| {
            Ok(c.query_row("SELECT seed, bible, palette FROM universe WHERE id = 1", [], |r| {
                Ok(UniverseRow { seed: r.get::<_, i64>(0)? as u32, bible: r.get(1)?, look_json: r.get(2)? })
            })?)
        })
    }

    pub fn kv_get(&self, key: &str) -> Option<String> {
        self.with(|c| Ok(c.query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0)).optional()?)).ok().flatten()
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        self.with(|c| kv_set(c, key, value))
    }

    pub fn versions(&self) -> Result<Vec<VersionRow>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT id, kind, summary, created_at, reverts FROM versions ORDER BY id")?;
            let rows = st.query_map([], |r| {
                Ok(VersionRow { id: r.get(0)?, kind: r.get(1)?, summary: r.get(2)?, created_at: r.get(3)?, reverts: r.get(4)? })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    pub fn types(&self) -> Result<Vec<TypeRow>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT id, version_id, name, code, status FROM types ORDER BY id")?;
            let rows = st.query_map([], |r| Ok(TypeRow { id: r.get(0)?, version: r.get(1)?, name: r.get(2)?, code: r.get(3)?, status: r.get(4)? }))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    pub fn instances(&self) -> Result<Vec<InstRow>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT id, type_id, version_id, x, y, z, rot_y, scale, params_json, removed_version_id FROM instances ORDER BY id")?;
            let rows = st.query_map([], |r| {
                let pj: String = r.get(8)?;
                let v: Vec<f32> = serde_json::from_str(&pj).unwrap_or_default();
                let mut params = [0.5f32; 8];
                for (i, x) in v.iter().take(8).enumerate() {
                    params[i] = *x;
                }
                Ok(InstRow {
                    id: r.get(0)?,
                    type_id: r.get(1)?,
                    version: r.get(2)?,
                    x: r.get::<_, f64>(3)? as f32,
                    y: r.get::<_, f64>(4)? as f32,
                    z: r.get::<_, f64>(5)? as f32,
                    rot_y: r.get::<_, f64>(6)? as f32,
                    scale: r.get::<_, f64>(7)? as f32,
                    params,
                    removed: r.get(9)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    pub fn regions(&self) -> Result<Vec<RegionRow>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT rx, rz, status, plan_json, version_id FROM regions")?;
            let rows = st.query_map([], |r| Ok(RegionRow { rx: r.get(0)?, rz: r.get(1)?, status: r.get(2)?, plan_json: r.get(3)?, version: r.get(4)? }))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    pub fn characters(&self) -> Result<Vec<CharRow>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT id, region, persona_json, home_x, home_z, state_json, version_id FROM characters ORDER BY id")?;
            let rows = st.query_map([], |r| {
                Ok(CharRow {
                    id: r.get(0)?,
                    region: r.get(1)?,
                    persona_json: r.get(2)?,
                    home_x: r.get::<_, f64>(3)? as f32,
                    home_z: r.get::<_, f64>(4)? as f32,
                    state_json: r.get(5)?,
                    version: r.get(6)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    pub fn add_memory(&self, cid: i64, t: f64, text: &str, importance: f32) -> Result<i64> {
        self.with(|c| {
            c.execute("INSERT INTO memories(character_id, t, text, importance) VALUES (?1, ?2, ?3, ?4)", params![cid, t, text, importance as f64])?;
            Ok(c.last_insert_rowid())
        })
    }

    pub fn memories(&self, cid: i64) -> Result<Vec<MemoryRow>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT id, text, importance FROM memories WHERE character_id = ?1 ORDER BY id")?;
            let rows = st.query_map([cid], |r| Ok(MemoryRow { id: r.get(0)?, text: r.get(1)?, importance: r.get::<_, f64>(2)? as f32 }))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    pub fn summary(&self, cid: i64) -> Option<(String, f64)> {
        self.with(|c| Ok(c.query_row("SELECT text, updated_at FROM summaries WHERE character_id = ?1", [cid], |r| Ok((r.get(0)?, r.get(1)?))).optional()?))
            .ok()
            .flatten()
    }

    pub fn set_summary(&self, cid: i64, text: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO summaries(character_id, text, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(character_id) DO UPDATE SET text = excluded.text, updated_at = excluded.updated_at",
                params![cid, text, now()],
            )?;
            Ok(())
        })
    }

    pub fn player(&self) -> Option<PlayerRow> {
        self.with(|c| {
            Ok(c.query_row("SELECT x, z, yaw, t_game FROM player WHERE id = 1", [], |r| {
                Ok(PlayerRow { x: r.get::<_, f64>(0)? as f32, z: r.get::<_, f64>(1)? as f32, yaw: r.get::<_, f64>(2)? as f32, t_game: r.get(3)? })
            })
            .optional()?)
        })
        .ok()
        .flatten()
    }

    pub fn save_player(&self, p: PlayerRow) -> Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO player(id, x, z, yaw, t_game) VALUES (1, ?1, ?2, ?3, ?4) ON CONFLICT(id) DO UPDATE SET x = excluded.x, z = excluded.z, yaw = excluded.yaw, t_game = excluded.t_game",
                params![p.x as f64, p.z as f64, p.yaw as f64, p.t_game],
            )?;
            Ok(())
        })
    }

    pub fn add_usage(&self, purpose: &str, input: u64, output: u64, cost: f64) -> Result<()> {
        self.with(|c| {
            c.execute("INSERT INTO llm_usage(t, purpose, input_tokens, output_tokens, cost) VALUES (?1, ?2, ?3, ?4, ?5)", params![now(), purpose, input as i64, output as i64, cost])?;
            Ok(())
        })
    }

}

/// A universe's own species (by name), replacing a built-in of the same name.
pub fn put_species(c: &Connection, name: &str, json: &str) -> Result<()> {
    c.execute("INSERT INTO species(name, json) VALUES (?1, ?2) ON CONFLICT(name) DO UPDATE SET json = excluded.json", params![name, json])?;
    Ok(())
}

pub fn species_rows(c: &Connection) -> Result<Vec<(String, String)>> {
    let mut st = c.prepare("SELECT name, json FROM species ORDER BY rowid")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn kv_set(c: &Connection, key: &str, value: &str) -> Result<()> {
    c.execute("INSERT INTO kv(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
    Ok(())
}

pub fn add_version(c: &Connection, parent: Option<i64>, kind: &str, summary: &str, reverts: Option<i64>) -> Result<i64> {
    c.execute("INSERT INTO versions(parent_id, kind, summary, created_at, reverts) VALUES (?1, ?2, ?3, ?4, ?5)", params![parent, kind, summary, now(), reverts])?;
    let id = c.last_insert_rowid();
    kv_set(c, "current_version", &id.to_string())?;
    Ok(id)
}

pub fn add_type(c: &Connection, version: Option<i64>, name: &str, code: &str, meta_json: &str, status: &str, errors: &str) -> Result<i64> {
    c.execute("INSERT INTO types(version_id, name, code, meta_json, status, errors) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![version, name, code, meta_json, status, errors])?;
    Ok(c.last_insert_rowid())
}

#[allow(clippy::too_many_arguments)]
pub fn add_instance(c: &Connection, type_id: i64, version: i64, x: f32, y: f32, z: f32, rot_y: f32, scale: f32, params: &[f32; 8]) -> Result<i64> {
    let (cx, cz) = crate::world::chunk_of(x, z);
    c.execute(
        "INSERT INTO instances(type_id, version_id, chunk_x, chunk_z, x, y, z, rot_y, scale, params_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![type_id, version, cx, cz, x as f64, y as f64, z as f64, rot_y as f64, scale as f64, serde_json::to_string(&params.to_vec())?],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn set_region(c: &Connection, rx: i32, rz: i32, status: &str, plan_json: &str, version: Option<i64>) -> Result<()> {
    c.execute(
        "INSERT INTO regions(rx, rz, status, plan_json, version_id) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(rx, rz) DO UPDATE SET status = excluded.status, plan_json = excluded.plan_json, version_id = excluded.version_id",
        params![rx, rz, status, plan_json, version],
    )?;
    Ok(())
}

pub fn add_character(c: &Connection, region: (i32, i32), persona_json: &str, home_x: f32, home_z: f32, version: i64) -> Result<i64> {
    c.execute(
        "INSERT INTO characters(region, persona_json, home_x, home_z, state_json, version_id) VALUES (?1, ?2, ?3, ?4, '{}', ?5)",
        params![format!("{},{}", region.0, region.1), persona_json, home_x as f64, home_z as f64, version],
    )?;
    Ok(c.last_insert_rowid())
}
