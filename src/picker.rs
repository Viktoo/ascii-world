//! Your worlds (the first screen): pick one, or start a new one, with the arrow keys.

use crate::term::Screen;
use crate::window::Win;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub struct World {
    pub path: PathBuf,
    pub name: String,
    pub prompt: String,
    pub regions: i64,
    /// Living beings who talk and plan, and the rest (animals, beasts).
    pub people: i64,
    pub creatures: i64,
    pub modified: SystemTime,
}

/// Read a world's summary without modifying the file.
fn summarize(path: &Path) -> Option<World> {
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let (prompt, look): (String, String) = c.query_row("SELECT bible, palette FROM universe WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?))).ok()?;
    let name = serde_json::from_str::<serde_json::Value>(&look).ok().and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string)).unwrap_or_default();
    let count = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0);
    let (people, creatures) = beings(&c);
    Some(World {
        path: path.to_path_buf(),
        name: if name.is_empty() { path.file_stem().map(|s| s.to_string_lossy().replace('-', " ")).unwrap_or_default() } else { name },
        prompt,
        regions: count("SELECT COUNT(*) FROM regions WHERE status = 'done'"),
        people,
        creatures,
        modified: std::fs::metadata(path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH),
    })
}

/// The living, by mind: (people, creatures).
fn beings(c: &Connection) -> (i64, i64) {
    use crate::world::species::{Mind, SpeciesBook};
    let rows: Vec<(String, String)> = c.prepare("SELECT name, json FROM species").and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()).unwrap_or_default();
    let book = SpeciesBook::load(&rows, None);
    let alive: Vec<String> = c
        .prepare("SELECT COALESCE(json_extract(persona_json, '$.species'), '') FROM characters WHERE NOT COALESCE(json_extract(state_json, '$.dead'), 0)")
        .and_then(|mut st| st.query_map([], |r| r.get(0))?.collect())
        .unwrap_or_default();
    let (mut people, mut creatures) = (0, 0);
    for s in &alive {
        let sp = book.of(s);
        if sp.mind == Mind::Sapient {
            people += 1;
        } else {
            creatures += 1;
        }
    }
    (people, creatures)
}

/// The world last played, if it is still there. One in a temporary folder
/// (a test's or a tool's scratch copy) is never offered.
pub fn last() -> Option<PathBuf> {
    let p = PathBuf::from(std::fs::read_to_string(crate::log::dir().join("last")).ok()?.trim());
    let tmp = std::env::temp_dir().canonicalize().unwrap_or_else(|_| std::env::temp_dir());
    let full = p.canonicalize().ok()?;
    let scratch = full.starts_with(&tmp) || ["/tmp", "/private/tmp", "/var/folders", "/private/var/folders"].iter().any(|d| full.starts_with(d));
    (!scratch).then_some(p)
}

/// All worlds in ~/.pocket/universes plus the last one opened, newest first.
pub fn worlds() -> Vec<World> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(crate::log::dir().join("universes"))
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "pocket")).collect())
        .unwrap_or_default();
    if let Some(p) = last() {
        if !paths.iter().any(|q| q.canonicalize().ok() == p.canonicalize().ok()) {
            paths.push(p);
        }
    }
    let mut out: Vec<World> = paths.iter().filter_map(|p| summarize(p)).collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

pub fn ago(t: SystemTime) -> String {
    let s = SystemTime::now().duration_since(t).unwrap_or(Duration::ZERO).as_secs();
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86_399 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86_400),
    }
}

/// The new-world text. It opens on a random prompt, selected: Enter takes it,
/// Tab rolls another, typing replaces it, and the arrows step in to edit it.
pub(crate) struct Draft {
    pub text: Vec<char>,
    pub cursor: usize,
    /// The whole text is a fresh roll: the next thing typed replaces it.
    pub fresh: bool,
}

pub(crate) enum Edit {
    Stay,
    Cancel,
    Done(String),
}

impl Draft {
    pub fn rolled() -> Draft {
        let text: Vec<char> = crate::random_world::prompt().chars().collect();
        Draft { cursor: text.len(), text, fresh: true }
    }

    /// Text the person already wrote, ready to fix or take as it is.
    pub fn given(text: &str) -> Draft {
        let text: Vec<char> = text.chars().collect();
        Draft { cursor: text.len(), text, fresh: false }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.fresh = false;
    }

    pub fn insert(&mut self, s: &str) {
        if self.fresh {
            self.clear();
        }
        for c in s.chars() {
            self.text.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// The start of the word before the cursor.
    fn word_start(&self) -> usize {
        let mut i = self.cursor;
        while i > 0 && self.text[i - 1] == ' ' {
            i -= 1;
        }
        while i > 0 && self.text[i - 1] != ' ' {
            i -= 1;
        }
        i
    }

    pub fn key(&mut self, k: KeyEvent) -> Edit {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        match k.code {
            KeyCode::Esc => return Edit::Cancel,
            KeyCode::Enter => {
                let t: String = self.text.iter().collect();
                if !t.trim().is_empty() {
                    return Edit::Done(t.trim().to_string());
                }
            }
            KeyCode::Tab => *self = Draft::rolled(),
            KeyCode::Char('u') if ctrl => self.clear(),
            KeyCode::Backspace | KeyCode::Delete if self.fresh => self.clear(),
            KeyCode::Char('w') if ctrl => {
                let from = self.word_start();
                self.text.drain(from..self.cursor);
                self.cursor = from;
            }
            KeyCode::Backspace if alt || ctrl => {
                let from = self.word_start();
                self.text.drain(from..self.cursor);
                self.cursor = from;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.text.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.text.len() => {
                self.text.remove(self.cursor);
            }
            KeyCode::Left => {
                self.fresh = false;
                self.cursor = if alt || ctrl { self.word_start() } else { self.cursor.saturating_sub(1) };
            }
            KeyCode::Right => {
                self.fresh = false;
                self.cursor = (self.cursor + 1).min(self.text.len());
            }
            KeyCode::Home => {
                self.fresh = false;
                self.cursor = 0;
            }
            KeyCode::Char('a') if ctrl => {
                self.fresh = false;
                self.cursor = 0;
            }
            KeyCode::End => {
                self.fresh = false;
                self.cursor = self.text.len();
            }
            KeyCode::Char('e') if ctrl => {
                self.fresh = false;
                self.cursor = self.text.len();
            }
            KeyCode::Char(c) if !ctrl => self.insert(&if k.modifiers.contains(KeyModifiers::SHIFT) { c.to_uppercase().collect() } else { c.to_string() }),
            _ => {}
        }
        Edit::Stay
    }

    /// Word-wrapped lines as char ranges, so the cursor can be placed.
    pub fn lines(&self, width: usize) -> Vec<(usize, usize)> {
        let width = width.max(8);
        let mut out = Vec::new();
        let mut start = 0;
        while self.text.len() - start > width {
            let end = start + width;
            let brk = (start + 1..=end).rev().find(|&i| self.text[i - 1] == ' ').unwrap_or(end);
            out.push((start, brk));
            start = brk;
        }
        out.push((start, self.text.len()));
        out
    }
}

pub fn size_text(path: &Path) -> String {
    let b = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) as f64;
    if b >= 1e6 { format!("{:.1} MB", b / 1e6) } else { format!("{:.0} KB", (b / 1e3).max(1.0)) }
}

/// Delete a world's file and SQLite's files beside it.
pub fn delete_world(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path)?;
    for ext in ["-wal", "-shm", "-journal"] {
        let mut side = path.as_os_str().to_owned();
        side.push(ext);
        let _ = std::fs::remove_file(PathBuf::from(side));
    }
    Ok(())
}

/// A title and a line in the middle (while a world opens), over the land of words.
pub fn notice(win: &mut Win, title: &str, line: &str) {
    let (w, h, g) = win.grid();
    let mut scr = Screen::new(w, h, true);
    let mut back = crate::ui::title::Backdrop::new();
    back.draw(&mut scr, 0.0, 0.5);
    scr.title(w as f32 / 2.0, h as f32 / 2.0 - 1.5, title, 2.4, crate::ui::ACCENT);
    crate::ui::note(&mut scr, w as f32 / 2.0, h as f32 / 2.0 + 1.0, &crate::ui::fit(line, w as f32 - 4.0, 0.92), crate::ui::DIM);
    win.present(&scr, h, Some(back.frame((w as f32 * g.cell_w, h as f32 * g.cell_h), 0.0)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn text(d: &Draft) -> String {
        d.text.iter().collect()
    }

    fn screen_text(scr: &Screen) -> String {
        scr.plain_text()
    }

    #[test]
    fn draft_opens_random_and_enter_takes_it() {
        let mut d = Draft::rolled();
        let rolled = text(&d);
        assert!(d.fresh && rolled.ends_with('.') && !rolled.is_empty());
        assert!(matches!(d.key(key(KeyCode::Enter)), Edit::Done(p) if p == rolled));
    }

    #[test]
    fn typing_replaces_and_arrows_edit() {
        let mut d = Draft::rolled();
        d.key(key(KeyCode::Char('a')));
        assert_eq!(text(&d), "a");
        let mut d = Draft::rolled();
        let rolled = text(&d);
        d.key(key(KeyCode::Left));
        assert!(!d.fresh);
        d.key(key(KeyCode::Char('!')));
        assert_eq!(text(&d), format!("{}!.", &rolled[..rolled.len() - 1]));
        d.key(key(KeyCode::Backspace));
        assert_eq!(text(&d), rolled);
        d.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert!(text(&d).is_empty());
        assert!(matches!(d.key(key(KeyCode::Enter)), Edit::Stay), "nothing to begin");
        d.key(key(KeyCode::Tab));
        assert!(d.fresh && !d.text.is_empty());
    }

    #[test]
    fn a_given_prompt_is_kept_and_typing_adds_to_it() {
        let mut d = Draft::given("a quiet farm");
        d.key(key(KeyCode::Char('s')));
        assert_eq!(text(&d), "a quiet farms");
        assert!(matches!(d.key(key(KeyCode::Enter)), Edit::Done(p) if p == "a quiet farms"));
    }

    #[test]
    fn deleting_a_word() {
        let mut d = Draft { text: "a quiet farm".chars().collect(), cursor: 12, fresh: false };
        d.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT));
        assert_eq!(text(&d), "a quiet ");
        d.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(text(&d), "a ");
    }

    #[test]
    fn deleting_a_world_removes_its_files() {
        let dir = std::env::temp_dir().join(format!("pocket-del-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("w.pocket");
        for f in ["w.pocket", "w.pocket-wal", "w.pocket-shm", "other.pocket"] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        delete_world(&p).unwrap();
        assert!(!p.exists() && !dir.join("w.pocket-wal").exists() && !dir.join("w.pocket-shm").exists());
        assert!(dir.join("other.pocket").exists(), "only that world");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
