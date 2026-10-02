//! `pocket list`: pick a world (or start a new one) with the arrow keys.

use crate::term::{self, Cell, Screen};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
    /// Species with someone alive.
    pub species: i64,
    /// Object types (not bodies) the world can hold.
    pub kinds: i64,
    pub built: i64,
    pub modified: SystemTime,
}

pub enum Choice {
    Open(PathBuf),
    New(String),
    Quit,
}

/// Read a world's summary without modifying the file.
fn summarize(path: &Path) -> Option<World> {
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let (prompt, look): (String, String) = c.query_row("SELECT bible, palette FROM universe WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?))).ok()?;
    let name = serde_json::from_str::<serde_json::Value>(&look).ok().and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string)).unwrap_or_default();
    let count = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0);
    let undone = "SELECT reverts FROM versions WHERE reverts IS NOT NULL";
    let (people, creatures, species) = beings(&c);
    Some(World {
        path: path.to_path_buf(),
        name: if name.is_empty() { path.file_stem().map(|s| s.to_string_lossy().replace('-', " ")).unwrap_or_default() } else { name },
        prompt,
        regions: count("SELECT COUNT(*) FROM regions WHERE status = 'done'"),
        people,
        creatures,
        species,
        kinds: count("SELECT COUNT(*) FROM types WHERE status IN ('ok', 'builtin') AND json_extract(meta_json, '$.body') IS NULL"),
        built: count(&format!("SELECT COUNT(*) FROM versions WHERE kind = 'create' AND id NOT IN ({undone})")),
        modified: std::fs::metadata(path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH),
    })
}

/// The living, by mind: (people, creatures, species among them).
fn beings(c: &Connection) -> (i64, i64, i64) {
    use crate::world::species::{Mind, SpeciesBook};
    let rows: Vec<(String, String)> = c.prepare("SELECT name, json FROM species").and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()).unwrap_or_default();
    let book = SpeciesBook::load(&rows, None);
    let alive: Vec<String> = c
        .prepare("SELECT COALESCE(json_extract(persona_json, '$.species'), '') FROM characters WHERE NOT COALESCE(json_extract(state_json, '$.dead'), 0)")
        .and_then(|mut st| st.query_map([], |r| r.get(0))?.collect())
        .unwrap_or_default();
    let (mut people, mut creatures) = (0, 0);
    let mut kinds = std::collections::HashSet::new();
    for s in &alive {
        let sp = book.of(s);
        if sp.mind == Mind::Sapient {
            people += 1;
        } else {
            creatures += 1;
        }
        kinds.insert(sp.name.clone());
    }
    (people, creatures, kinds.len() as i64)
}

/// All worlds in ~/.pocket/universes plus the last one opened, newest first.
pub fn worlds() -> Vec<World> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(crate::log::dir().join("universes"))
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "pocket")).collect())
        .unwrap_or_default();
    if let Ok(last) = std::fs::read_to_string(crate::log::dir().join("last")) {
        let p = PathBuf::from(last.trim());
        if p.exists() && !paths.iter().any(|q| q.canonicalize().ok() == p.canonicalize().ok()) {
            paths.push(p);
        }
    }
    let mut out: Vec<World> = paths.iter().filter_map(|p| summarize(p)).collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

fn ago(t: SystemTime) -> String {
    let s = SystemTime::now().duration_since(t).unwrap_or(Duration::ZERO).as_secs();
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86_399 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86_400),
    }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

const BG: [u8; 3] = [12, 13, 18];
const TEXT: [u8; 3] = [222, 224, 230];
const DIM: [u8; 3] = [125, 130, 148];
const ACCENT: [u8; 3] = [255, 200, 110];
const SEL_BG: [u8; 3] = [34, 37, 52];

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n.saturating_sub(1)).collect::<String>()) }
}

/// The new-world text. It opens on a random prompt, selected: Enter takes it,
/// Tab rolls another, typing replaces it, and the arrows step in to edit it.
struct Draft {
    text: Vec<char>,
    cursor: usize,
    /// The whole text is a fresh roll: the next thing typed replaces it.
    fresh: bool,
}

enum Edit {
    Stay,
    Cancel,
    Done(String),
}

/// Most lines of the draft shown at once.
const DRAFT_ROWS: usize = 8;

impl Draft {
    fn rolled() -> Draft {
        let text: Vec<char> = crate::random_world::prompt().chars().collect();
        Draft { cursor: text.len(), text, fresh: true }
    }

    /// Text the person already wrote, ready to fix or take as it is.
    fn given(text: &str) -> Draft {
        let text: Vec<char> = text.chars().collect();
        Draft { cursor: text.len(), text, fresh: false }
    }

    fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.fresh = false;
    }

    fn insert(&mut self, s: &str) {
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

    fn key(&mut self, k: KeyEvent) -> Edit {
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
    fn lines(&self, width: usize) -> Vec<(usize, usize)> {
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

    /// How many rows the box takes.
    fn rows(&self, width: usize) -> usize {
        self.lines(width).len().min(DRAFT_ROWS)
    }
}

/// Columns the draft wraps to (one spare for the cursor at a line's end).
fn draft_width(w: u16) -> usize {
    w.saturating_sub(7) as usize
}

/// The new-world screen: one place to see and change the prompt before
/// the world begins (from `pocket new` and from the list alike).
fn draw_new(scr: &mut Screen, d: &Draft, back: &str) {
    let (w, h) = (scr.w, scr.h);
    for y in 0..h {
        scr.fill_row(y, Cell { bg: BG, ..Cell::BLANK });
    }
    scr.text(2, 1, "Pocket Universe", ACCENT, BG, true);
    scr.text(18, 1, " · a new world", DIM, BG, false);
    let intro = if d.fresh { "A world picked at random. Keep it, roll another, or write your own:" } else { "The world you want, as you'll begin it:" };
    scr.text(2, 3, &clip(intro, w.saturating_sub(4) as usize), DIM, BG, false);
    let lines = d.lines(draft_width(w));
    let at = lines.iter().rposition(|(s, _)| *s <= d.cursor).unwrap_or(0);
    let first = at.saturating_sub(DRAFT_ROWS - 1);
    let y0 = 5u16;
    for (row, (s, e)) in lines.iter().enumerate().skip(first).take(DRAFT_ROWS) {
        let y = y0 + (row - first) as u16;
        scr.text(2, y, if row == 0 { ">" } else { " " }, ACCENT, BG, true);
        for (i, ch) in d.text[*s..*e].iter().enumerate() {
            let here = !d.fresh && s + i == d.cursor;
            let (fg, bg) = if here { (BG, ACCENT) } else if d.fresh { (TEXT, SEL_BG) } else { (TEXT, BG) };
            scr.set(4 + i as u16, y, Cell { ch: *ch, fg, bg, bold: false });
        }
        if d.cursor == d.text.len() && row == lines.len() - 1 {
            scr.set(4 + (e - s) as u16, y, Cell { ch: '▏', fg: ACCENT, bg: BG, bold: false });
        }
    }
    let hint = if d.fresh {
        format!("Enter begin   Tab another   type your own   ←→ edit   Esc {back}")
    } else {
        format!("Enter begin   Tab random   Ctrl+U clear   Esc {back}")
    };
    let y = (y0 + d.rows(draft_width(w)) as u16 + 1).min(h.saturating_sub(2));
    scr.text(2, y, &clip(&hint, w.saturating_sub(4) as usize), DIM, BG, false);
}

/// Run the new-world screen until the prompt is taken (Some) or left (None).
fn compose_on(scr: &mut Screen, mut d: Draft, back: &str) -> Result<Option<String>> {
    let mut out = std::io::stdout();
    loop {
        draw_new(scr, &d, back);
        scr.flush(&mut out)?;
        if !event::poll(Duration::from_millis(500))? {
            continue;
        }
        match event::read()? {
            Event::Resize(w, h) => scr.resize(w, h),
            Event::Paste(s) => d.insert(&s.replace(['\n', '\r'], " ")),
            Event::Key(k) if k.kind != KeyEventKind::Release => {
                if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c')) {
                    return Ok(None);
                }
                match d.key(k) {
                    Edit::Stay => {}
                    Edit::Cancel => return Ok(None),
                    Edit::Done(p) => return Ok(Some(p)),
                }
            }
            _ => {}
        }
    }
}

/// `pocket new`: the new-world screen on its own, on the prompt given (to
/// check before it begins) or a random one. Without a terminal (scripts)
/// a given prompt is taken as it is.
pub fn compose(given: Option<&str>) -> Result<Option<String>> {
    use std::io::IsTerminal;
    let given = given.map(str::trim).filter(|p| !p.is_empty());
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        return Ok(Some(given.map(str::to_string).unwrap_or_else(crate::random_world::prompt)));
    }
    let _ti = term::enter()?;
    let (w, h) = crossterm::terminal::size().unwrap_or((100, 30));
    let mut scr = Screen::new(w, h, term::truecolor());
    let r = compose_on(&mut scr, given.map_or_else(Draft::rolled, Draft::given), "quit");
    term::restore();
    r
}

/// Delete a world's file and SQLite's files beside it.
fn delete_world(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path)?;
    for ext in ["-wal", "-shm", "-journal"] {
        let mut side = path.as_os_str().to_owned();
        side.push(ext);
        let _ = std::fs::remove_file(PathBuf::from(side));
    }
    Ok(())
}

fn draw(scr: &mut Screen, list: &[World], sel: usize, deleting: bool) {
    let (w, h) = (scr.w, scr.h);
    for y in 0..h {
        scr.fill_row(y, Cell { bg: BG, ..Cell::BLANK });
    }
    scr.text(2, 1, "Pocket Universe", ACCENT, BG, true);
    scr.text(18, 1, " · your worlds", DIM, BG, false);
    let width = w.saturating_sub(6) as usize;
    let per = 4u16;
    let visible = ((h.saturating_sub(7)) / per).max(1) as usize;
    let first = sel.saturating_sub(visible - 1).min(list.len().saturating_sub(visible));
    if list.is_empty() {
        scr.text(4, 4, "No worlds yet. Press n to start one.", DIM, BG, false);
    }
    for (row, (i, wd)) in list.iter().enumerate().skip(first).take(visible).enumerate() {
        let y = 3 + row as u16 * per;
        let selected = i == sel;
        let bg = if selected { SEL_BG } else { BG };
        for dy in 0..3 {
            for x in 2..w.saturating_sub(2) {
                scr.set(x, y + dy, Cell { bg, ..Cell::BLANK });
            }
        }
        scr.text(2, y, if selected { "▸" } else { " " }, ACCENT, bg, true);
        let when = ago(wd.modified);
        scr.text(4, y, &clip(&wd.name, width.saturating_sub(when.len() + 2)), if selected { ACCENT } else { TEXT }, bg, true);
        scr.text(w.saturating_sub(when.len() as u16 + 3), y, &when, DIM, bg, false);
        scr.text(4, y + 1, &clip(&format!("“{}”", wd.prompt), width), TEXT, bg, false);
        let stats = [
            plural(wd.regions, "place", "places"),
            plural(wd.people, "person", "people"),
            plural(wd.creatures, "creature", "creatures"),
            plural(wd.species, "species", "species"),
            plural(wd.kinds, "kind of thing", "kinds of things"),
            plural(wd.built, "thing you made", "things you made"),
        ]
        .join(" · ");
        scr.text(4, y + 2, &clip(&stats, width), DIM, bg, false);
    }
    if deleting && sel < list.len() {
        let x = scr.text(2, h.saturating_sub(2), &clip(&format!("Delete “{}” for good? ", list[sel].name), w.saturating_sub(40) as usize), ACCENT, BG, true);
        scr.text(x, h.saturating_sub(2), "y delete   any other key keep", DIM, BG, false);
    } else {
        let keys = if list.is_empty() { "n new world   q quit" } else { "↑↓ choose   Enter open   n new world   d delete   q quit" };
        scr.text(2, h.saturating_sub(2), keys, DIM, BG, false);
    }
}

pub fn pick() -> Result<Choice> {
    let mut list = worlds();
    let _ti = term::enter()?;
    let (w, h) = crossterm::terminal::size().unwrap_or((100, 30));
    let mut scr = Screen::new(w, h, term::truecolor());
    let mut out = std::io::stdout();
    let mut sel = 0usize;
    let mut deleting = false;
    let choice = loop {
        draw(&mut scr, &list, sel, deleting);
        scr.flush(&mut out)?;
        if !event::poll(Duration::from_millis(500))? {
            continue;
        }
        match event::read()? {
            Event::Resize(w, h) => scr.resize(w, h),
            Event::Key(k) if k.kind != KeyEventKind::Release => {
                if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c')) {
                    break Choice::Quit;
                }
                if deleting {
                    deleting = false;
                    if matches!(k.code, KeyCode::Char('y') | KeyCode::Char('Y')) && sel < list.len() {
                        if let Err(e) = delete_world(&list[sel].path) {
                            crate::log::error(format!("delete {}: {e}", list[sel].path.display()));
                        }
                        list = worlds();
                        sel = sel.min(list.len().saturating_sub(1));
                    }
                    continue;
                }
                match k.code {
                    KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete if !list.is_empty() => deleting = true,
                    KeyCode::Up => sel = sel.saturating_sub(1),
                    KeyCode::Down => sel = (sel + 1).min(list.len().saturating_sub(1)),
                    KeyCode::Enter if !list.is_empty() => break Choice::Open(list[sel].path.clone()),
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        if let Some(p) = compose_on(&mut scr, Draft::rolled(), "back")? {
                            break Choice::New(p);
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => break Choice::Quit,
                    _ => {}
                }
            }
            _ => {}
        }
    };
    term::restore();
    Ok(choice)
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
        (0..scr.h).map(|y| (0..scr.w).map(|x| scr.get(x, y).map_or(' ', |c| c.ch)).collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn draft_opens_random_and_enter_takes_it() {
        let mut d = Draft::rolled();
        let rolled = text(&d);
        assert!(d.fresh && rolled.contains("You arrive where"));
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
    fn wraps_on_words_and_draws_at_any_size() {
        let d = Draft { text: "salt flats where caravans trade salt for everything".chars().collect(), cursor: 0, fresh: false };
        let lines = d.lines(20);
        assert!(lines.len() > 1 && lines.iter().all(|(s, e)| e - s <= 20));
        assert_eq!(lines.first().unwrap().0, 0);
        assert_eq!(lines.last().unwrap().1, d.text.len());
        for (w, h) in [(30, 10), (80, 24), (12, 6), (200, 60)] {
            let mut scr = Screen::new(w, h, true);
            let mut d = Draft::rolled();
            draw_new(&mut scr, &d, "back");
            assert!(screen_text(&scr).contains("Enter begin") || w < 20);
            d.key(key(KeyCode::Home));
            draw_new(&mut scr, &d, "back");
            draw(&mut scr, &[], 0, false);
        }
        let mut scr = Screen::new(100, 30, true);
        draw_new(&mut scr, &Draft::rolled(), "quit");
        let shown = screen_text(&scr);
        assert!(shown.contains("a new world") && shown.contains("arrive") && shown.contains("Esc quit"), "{shown}");
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
