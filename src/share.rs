//! Sharing a world: a clean copy of its file to send, and taking one in.
//!
//! A copy keeps the world (what was made, its people and what they
//! remember, the cache of what words did) and leaves out what was yours:
//! the traveler's health, what they carried and how they looked, where they
//! stood, and what the world cost you. Keys never are in a world's file.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

/// The world file's format. A world from a newer one isn't taken in.
pub const FORMAT: i64 = 1;

/// Write a shareable copy of the world at `src` into `dir`, named after it.
pub fn make_copy(src: &Path, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("make {}", dir.display()))?;
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "world".into());
    let out = free_path(dir, &stem);
    // A consistent snapshot even while the world is being written.
    let c = Connection::open_with_flags(src, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).with_context(|| format!("open {}", src.display()))?;
    c.execute("VACUUM INTO ?1", [out.to_string_lossy()]).context("copy the world")?;
    drop(c);
    let cleaned = Connection::open(&out).map_err(anyhow::Error::from).and_then(|c| clean(&c));
    if let Err(e) = cleaned {
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    Ok(out)
}

/// What stays behind in a copy.
fn clean(c: &Connection) -> Result<()> {
    c.execute_batch("BEGIN")?;
    reset_travelers(c)?;
    c.execute("DELETE FROM llm_usage", [])?;
    // A new world for what is kept about worlds outside them (achievements).
    c.execute("DELETE FROM kv WHERE key = 'world_id'", [])?;
    crate::db::kv_set(c, "format", &FORMAT.to_string())?;
    crate::db::kv_set(c, "shared.at", &format!("{:.0}", now()))?;
    c.execute_batch("COMMIT")?;
    c.execute_batch("VACUUM")?;
    Ok(())
}

/// Everything about the traveler as one person, as opposed to the world
/// they walk in, put back for whoever comes next. Each place the traveler's
/// own state is kept belongs here (with more travelers it will be one row
/// each):
/// - `player`: where they stand (the world's time in it stays);
/// - kv `player.body`: the look they chose;
/// - kv `sim.night`: their wounds and whether they've fallen;
/// - `things.holder` / `co_holder` `player` or `worn:player`: what they
///   carry and wear, put down where they are;
/// - `travelers`: the guests who played here (each one's place, look and
///   wounds), and what they carry and wear (`player:N`), put down too.
fn reset_travelers(c: &Connection) -> Result<()> {
    let spawn: Option<[f32; 3]> = kv(c, "spawn")?.and_then(|s| serde_json::from_str(&s).ok());
    if let Some([x, _, z]) = spawn {
        c.execute("UPDATE player SET x = ?1, z = ?2 WHERE id = 1", params![x, z])?;
    }
    c.execute("DELETE FROM kv WHERE key = 'player.body'", [])?;
    if let Some(mut night) = kv(c, "sim.night")?.and_then(|s| serde_json::from_str::<crate::sim::night::NightState>(&s).ok()) {
        night.wounds = 0.0;
        night.fallen = None;
        night.fallen_at = 0.0;
        night.fell_tonight = false;
        night.wounded = 0;
        crate::db::kv_set(c, "sim.night", &serde_json::to_string(&night)?)?;
    }
    c.execute("UPDATE things SET holder = NULL, asleep = 0 WHERE holder IN ('player', 'worn:player') OR holder LIKE 'player:%' OR holder LIKE 'worn:player:%'", [])?;
    c.execute("UPDATE things SET co_holder = NULL WHERE co_holder = 'player' OR co_holder LIKE 'player:%'", [])?;
    c.execute("DELETE FROM travelers", [])?;
    Ok(())
}

/// Why a file can't be taken in as a world, if it can't.
pub fn check(path: &Path) -> Result<()> {
    let c = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).context("not a world file")?;
    c.query_row("SELECT 1 FROM universe WHERE id = 1", [], |_| Ok(())).map_err(|_| anyhow::anyhow!("not a world file"))?;
    let format: i64 = kv(&c, "format").ok().flatten().and_then(|s| s.parse().ok()).unwrap_or(1);
    if format > FORMAT {
        anyhow::bail!("made by a newer Infinite Pocket: update the game to open it");
    }
    Ok(())
}

/// Copy a world file into `dir` (your worlds) under a name of its own.
/// One already there is left where it is.
pub fn take_in(path: &Path, dir: &Path) -> Result<PathBuf> {
    check(path)?;
    if let (Ok(p), Ok(d)) = (path.canonicalize(), dir.canonicalize()) {
        if p.parent() == Some(d.as_path()) {
            return Ok(path.to_path_buf());
        }
    }
    std::fs::create_dir_all(dir)?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "world".into());
    let out = free_path(dir, &stem);
    std::fs::copy(path, &out).with_context(|| format!("copy to {}", out.display()))?;
    Ok(out)
}

/// Where a shared copy goes: Downloads, else beside your worlds.
pub fn copies_dir() -> PathBuf {
    dirs::download_dir().filter(|d| d.is_dir()).unwrap_or_else(|| crate::log::dir().join("shared"))
}

/// Show a file in the system's file browser.
pub fn reveal(path: &Path) -> Result<()> {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-R").arg(path);
        c
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(format!("/select,{}", path.display()));
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path.parent().unwrap_or(Path::new(".")));
        c
    };
    cmd.spawn().context("open the file browser")?;
    Ok(())
}

/// Open a web page in the browser.
pub fn open_url(url: &str) -> Result<()> {
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(url).spawn().context("open the browser")?;
    Ok(())
}

/// What the file browser is called here.
pub fn browser_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Show in Finder"
    } else if cfg!(windows) {
        "Show in Explorer"
    } else {
        "Show the folder"
    }
}

/// `dir/stem.pocket`, or `stem 2.pocket`… if that is taken.
pub(crate) fn free_path(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.pocket"));
    if !first.exists() {
        return first;
    }
    (2..).map(|n| dir.join(format!("{stem} {n}.pocket"))).find(|p| !p.exists()).unwrap()
}

fn kv(c: &Connection, key: &str) -> Result<Option<String>> {
    Ok(c.query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Db, PlayerRow};

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pocket-share-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn one(c: &Connection, sql: &str) -> Option<String> {
        c.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).optional().unwrap().flatten()
    }

    #[test]
    fn a_copy_keeps_the_world_and_leaves_the_traveler() {
        let dir = scratch("copy");
        let src = dir.join("salt-flats.pocket");
        let db = Db::create(&src, 7, "salt flats", "{}").unwrap();
        db.save_player(PlayerRow { x: 40.0, z: -12.0, yaw: 1.0, t_game: 500.0 }).unwrap();
        db.kv_set("spawn", "[3.0, 1.0, 4.0]").unwrap();
        db.kv_set("player.body", r#"{"look":{"skin":0.9}}"#).unwrap();
        db.kv_set("sim.night", r#"{"wounds":0.7,"fallen":"a walker","fell_tonight":true,"wounded":2,"nights":5}"#).unwrap();
        let id = db.world_id();
        db.add_usage("builder", 10, 10, 0.5).unwrap();
        db.with(|c| {
            for (holder, co) in [(Some("player"), None), (Some("worn:player"), None), (Some("npc:3"), Some("player")), (Some("npc:4"), None)] {
                c.execute("INSERT INTO things(type_id, x, y, z, yaw, scale, params_json, state_json, props_json, holder, co_holder, asleep, anchored, origin_json, born) VALUES (1, 0, 0, 0, 0, 1, '[]', '[]', '{}', ?1, ?2, 1, 0, '{}', 0)", params![holder, co])?;
            }
            c.execute("INSERT INTO memories(character_id, t, text, importance) VALUES (3, 1, 'the traveler gave me salt', 0.5)", [])?;
            Ok(())
        })
        .unwrap();
        drop(db);

        let out = make_copy(&src, &dir.join("out")).unwrap();
        assert_eq!(out.file_name().unwrap(), "salt-flats.pocket");
        let c = Connection::open(&out).unwrap();
        let (x, z, t): (f64, f64, f64) = c.query_row("SELECT x, z, t_game FROM player", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!((x, z, t), (3.0, 4.0, 500.0), "back at the start; the world's time stays");
        assert_eq!(one(&c, "SELECT value FROM kv WHERE key = 'player.body'"), None);
        let night: serde_json::Value = serde_json::from_str(&one(&c, "SELECT value FROM kv WHERE key = 'sim.night'").unwrap()).unwrap();
        assert_eq!((night["wounds"].as_f64(), night["fallen"].is_null(), night["fell_tonight"].as_bool(), night["nights"].as_u64()), (Some(0.0), true, Some(false), Some(5)));
        assert_eq!(one(&c, "SELECT COUNT(*) || '' FROM llm_usage").as_deref(), Some("0"));
        assert_eq!(one(&c, "SELECT group_concat(COALESCE(holder, '-') || '/' || COALESCE(co_holder, '-'), ' ') FROM things").as_deref(), Some("-/- -/- npc:3/- npc:4/-"));
        assert_eq!(one(&c, "SELECT text FROM memories").as_deref(), Some("the traveler gave me salt"), "the world's people remember");
        assert_ne!(one(&c, "SELECT value FROM kv WHERE key = 'world_id'").as_deref(), Some(id.as_str()));
        assert_eq!(one(&c, "SELECT value FROM kv WHERE key = 'format'"), Some(FORMAT.to_string()));
        assert_eq!(one(&c, "SELECT value FROM kv WHERE key = 'made_with'").as_deref(), Some(crate::db::COMMIT), "which build made it stays");
        drop(c);
        // The original is untouched, and a second copy gets a name of its own.
        orig_opened(&src);
        let orig = Connection::open(&src).unwrap();
        assert_eq!(one(&orig, "SELECT value FROM kv WHERE key = 'player.body'").as_deref(), Some(r#"{"look":{"skin":0.9}}"#));
        assert_eq!(make_copy(&src, &dir.join("out")).unwrap().file_name().unwrap(), "salt-flats 2.pocket");

        // Taken in: copied into your worlds; one already there stays put.
        let mine = dir.join("universes");
        let added = take_in(&out, &mine).unwrap();
        assert_eq!(added, mine.join("salt-flats.pocket"));
        assert_eq!(take_in(&added, &mine).unwrap(), added);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Opening a world records the build that opened it.
    fn orig_opened(src: &Path) {
        Connection::open(src).unwrap().execute("UPDATE kv SET value = 'old' WHERE key = 'last_opened_with'", []).unwrap();
        let db = Db::open(src).unwrap();
        assert_eq!(db.kv_get("last_opened_with").as_deref(), Some(crate::db::COMMIT));
    }

    #[test]
    fn only_world_files_are_taken_in() {
        let dir = scratch("check");
        let junk = dir.join("notes.pocket");
        std::fs::write(&junk, "hello").unwrap();
        assert!(take_in(&junk, &dir.join("u")).is_err());
        let newer = dir.join("newer.pocket");
        Db::create(&newer, 1, "x", "{}").unwrap().kv_set("format", &(FORMAT + 1).to_string()).unwrap();
        assert!(check(&newer).unwrap_err().to_string().contains("newer"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A shareable copy of a real world, to open and look at:
    /// `POCKET_WORLD=~/.pocket/universes/x.pocket POCKET_OUT=/tmp/out cargo test real_world_share -- --ignored`
    #[test]
    #[ignore = "needs POCKET_WORLD"]
    fn real_world_share() {
        let src = PathBuf::from(std::env::var("POCKET_WORLD").unwrap());
        let out = make_copy(&src, &PathBuf::from(std::env::var("POCKET_OUT").unwrap())).unwrap();
        check(&out).unwrap();
        println!("{} ({} → {} bytes)", out.display(), std::fs::metadata(&src).unwrap().len(), std::fs::metadata(&out).unwrap().len());
    }
}
