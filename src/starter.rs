//! Maple Cove, the world that ships with the game. Made once from
//! `worlds/starter` (`pocket author`, then gzipped) and kept inside the
//! game, so it needs no network and no AI. It has two lives:
//!
//! - **behind the title** and the loading screen: a throwaway copy beside
//!   your worlds, made again whenever the game ships a newer Maple Cove,
//!   never saved to;
//! - **as your own world**: "Open Maple Cove" copies it into your worlds,
//!   and from then on it is a world like any other (yours to change, made
//!   further as you walk past its edges). A newer Maple Cove never touches it.

use anyhow::{Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};

const PACKED: &[u8] = include_bytes!("../worlds/starter/starter.pocket.gz");
/// The views shown behind the title (see `panorama`).
pub const SHOTS: &str = include_str!("../worlds/starter/shots.json");
pub const NAME: &str = "Maple Cove";

/// The world file, unpacked.
fn bytes() -> Result<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(PACKED).read_to_end(&mut out).context("unpack Maple Cove")?;
    Ok(out)
}

/// Which Maple Cove this game ships (changes with every rebuild of it).
fn version() -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in PACKED {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Write the world to `path` (through a file beside it, so a half-written
/// one is never found).
fn write_to(path: &Path) -> Result<()> {
    let tmp = path.with_extension("making");
    std::fs::write(&tmp, bytes()?).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// The copy behind the title: this game's Maple Cove, made if it isn't
/// there yet (older ones are cleared away).
pub fn backdrop() -> Result<PathBuf> {
    let dir = crate::log::dir().join("backdrop");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("maple-cove-{}.pocket", version()));
    if !path.exists() {
        for e in std::fs::read_dir(&dir)?.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
        write_to(&path)?;
    }
    Ok(path)
}

/// A new Maple Cove of your own, in your worlds.
pub fn new_copy() -> Result<PathBuf> {
    new_copy_in(&crate::log::dir().join("universes"))
}

fn new_copy_in(dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = crate::share::free_path(dir, "maple-cove");
    write_to(&path)?;
    let c = rusqlite::Connection::open(&path)?;
    crate::db::kv_set(&c, "starter.version", &version())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(path: &Path, key: &str) -> Option<String> {
        let c = rusqlite::Connection::open(path).ok()?;
        c.query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0)).ok()
    }

    #[test]
    fn the_shipped_maple_cove_is_made_from_its_folder() {
        let dir = std::env::temp_dir().join(format!("pocket-starter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = new_copy_in(&dir).unwrap();
        let folder = crate::author::source_hash(&Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/starter"));
        assert_eq!(
            kv(&path, "authored.source").as_deref(),
            Some(folder.as_str()),
            "worlds/starter changed since starter.pocket.gz was made: run `pocket author worlds/starter` and `gzip -9 -kf worlds/starter/starter.pocket`"
        );
        assert!(kv(&path, "start.hour").is_some());
        // A second copy is a world of its own, beside the first.
        let again = new_copy_in(&dir).unwrap();
        assert_ne!(path, again);
        let shots: Vec<serde_json::Value> = serde_json::from_str(SHOTS).unwrap();
        assert!(!shots.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
