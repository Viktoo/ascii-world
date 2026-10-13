//! Your worlds (the first screen): pick one, or start a new one, with the arrow keys.

use crate::term::{Cell, Deco, Face, Label, Screen};
use crate::window::{Ev, Win};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
const CARD: [u8; 3] = [24, 26, 38];
const CARD_SEL: [u8; 3] = [34, 37, 54];
const CARD_EDGE: [u8; 3] = [60, 64, 88];
const FRESH: [u8; 3] = [240, 214, 170];

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
}

/// The prompt box: its first column and width (in cells).
fn draft_box(w: u16) -> (u16, usize) {
    let width = (w as usize).saturating_sub(10).clamp(10, 96).min(w as usize);
    ((w as usize - width) as u16 / 2, width)
}

/// Columns the draft wraps to (one spare for the cursor at a line's end).
fn draft_width(w: u16) -> usize {
    draft_box(w).1.saturating_sub(4).max(4)
}

fn blank(scr: &mut Screen) {
    scr.clear_decor();
    for y in 0..scr.h {
        scr.fill_row(y, Cell { bg: BG, ..Cell::BLANK });
    }
}

/// The new-world screen: one place to see and change the prompt before
/// the world begins (from `pocket new` and from the list alike).
fn draw_new(scr: &mut Screen, d: &Draft, back: &str) {
    let (w, h) = (scr.w, scr.h);
    blank(scr);
    let cx = w as f32 / 2.0;
    // The whole block sits a little above the middle.
    let top = (h as f32 * 0.32 - 6.0).max(0.0) as u16;
    scr.title(cx, top as f32 + 2.6, "A new world", 2.6, ACCENT);
    let intro = if d.fresh { "A place picked at random. Keep it, roll another, or write your own." } else { "The world you want, as you'll begin it:" };
    scr.center_text(cx, top + 5, &clip(intro, w.saturating_sub(4) as usize), DIM);
    let (bx, bw) = draft_box(w);
    let lines = d.lines(draft_width(w));
    let at = lines.iter().rposition(|(s, _)| *s <= d.cursor).unwrap_or(0);
    let first = at.saturating_sub(DRAFT_ROWS - 1);
    let y0 = top + 8;
    let shown = lines.len().min(DRAFT_ROWS).max(1) as f32;
    scr.deco(Deco::panel(bx as f32, y0 as f32 - 1.0, bw as f32, shown + 2.0, CARD).border(if d.fresh { CARD_EDGE } else { ACCENT }));
    let tx = bx + 2;
    for (row, (s, e)) in lines.iter().enumerate().skip(first).take(DRAFT_ROWS) {
        let y = y0 + (row - first) as u16;
        // A fresh roll reads as a suggestion (warm), what you write as your own.
        let fg = if d.fresh { FRESH } else { TEXT };
        for (i, ch) in d.text[*s..*e].iter().enumerate() {
            scr.set(tx + i as u16, y, Cell { ch: *ch, fg, bg: BG, bold: false });
        }
        let last = row == lines.len() - 1;
        if !d.fresh && d.cursor >= *s && (d.cursor < *e || (last && d.cursor == *e)) {
            scr.caret = Some((tx + (d.cursor - s) as u16, y, ACCENT));
        }
    }
    if lines.is_empty() || d.text.is_empty() {
        scr.caret = Some((tx, y0, ACCENT));
    }
    let hint = if d.fresh {
        format!("Enter begin   Tab another   Type your own   ←→ edit   Esc {back}")
    } else {
        format!("Enter begin   Tab random   Ctrl+U clear   Esc {back}")
    };
    let y = (y0 + shown as u16 + 2).min(h.saturating_sub(2));
    let n = hint_width(&hint);
    scr.hints(w.saturating_sub(n) / 2, y, &hint, DIM, BG);
}

/// Columns a line of hints takes (each key drawn one wider, as a cap).
fn hint_width(s: &str) -> u16 {
    let parts: Vec<&str> = s.split("  ").map(str::trim).filter(|p| !p.is_empty()).collect();
    let mut n = parts.len().saturating_sub(1) * 3;
    for p in parts {
        let keyed = p.split_once(' ').is_some_and(|(k, _)| crate::term::is_key(k.trim_end_matches(':')));
        n += p.chars().count() + keyed as usize;
    }
    n as u16
}

/// The screen as big as the window.
fn fit(win: &Win, scr: &mut Screen) {
    let (w, h, _) = win.grid();
    if (scr.w, scr.h) != (w, h) {
        scr.resize(w, h);
    }
}

/// What the new-world screen ended with.
pub enum Composed {
    Prompt(String),
    Back,
    Quit,
}

/// Run the new-world screen until the prompt is taken or left.
fn compose_on(win: &mut Win, scr: &mut Screen, mut d: Draft, back: &str) -> Composed {
    loop {
        fit(win, scr);
        draw_new(scr, &d, back);
        win.present(scr, 0, None);
        for ev in win.pump(Duration::from_millis(500)) {
            match ev {
                Ev::Close => return Composed::Quit,
                Ev::Paste(t) => d.insert(&t.replace(['\n', '\r'], " ")),
                Ev::Key(k) if k.kind != KeyEventKind::Release => match d.key(k) {
                    Edit::Stay => {}
                    Edit::Cancel => return Composed::Back,
                    Edit::Done(p) => return Composed::Prompt(p),
                },
                _ => {}
            }
        }
    }
}

/// The new-world screen on its own, on the prompt given (to check before it
/// begins) or a random one.
pub fn compose(win: &mut Win, given: Option<&str>) -> Composed {
    let given = given.map(str::trim).filter(|p| !p.is_empty());
    let (w, h, _) = win.grid();
    let mut scr = Screen::new(w, h, true);
    compose_on(win, &mut scr, given.map_or_else(Draft::rolled, Draft::given), "back")
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

/// Your worlds, as cards.
fn draw(scr: &mut Screen, list: &[World], sel: usize, deleting: bool) {
    let (w, h) = (scr.w, scr.h);
    blank(scr);
    let cx = w as f32 / 2.0;
    scr.title(cx, 2.6, "Pocket Universe", 2.8, ACCENT);
    scr.center_text(cx, 5, "your worlds", DIM);
    let cw = (w as usize).saturating_sub(10).clamp(20, 96).min(w as usize);
    let x0 = ((w as usize - cw) / 2) as u16;
    let width = cw.saturating_sub(4);
    let per = 5u16;
    let top = 8u16;
    let visible = ((h.saturating_sub(top + 3)) / per).max(1) as usize;
    let first = sel.saturating_sub(visible - 1).min(list.len().saturating_sub(visible));
    if list.is_empty() {
        scr.center_text(cx, top + 1, "No worlds yet. Press N to start one.", DIM);
    }
    for (row, (i, wd)) in list.iter().enumerate().skip(first).take(visible).enumerate() {
        let y = top + row as u16 * per;
        let selected = i == sel;
        let card = Deco::panel(x0 as f32, y as f32 - 0.6, cw as f32, 4.2, if selected { CARD_SEL } else { CARD });
        scr.deco(if selected { card.border(ACCENT) } else { card });
        let when = ago(wd.modified);
        let name = clip(&wd.name, width.saturating_sub(when.len() + 2));
        scr.labels.push(Label { x: x0 as f32 + 2.0, y: y as f32 - 0.25, text: name, size: 1.45, color: if selected { ACCENT } else { TEXT }, face: Face::Title, center: false });
        scr.text(x0 + cw as u16 - when.len() as u16 - 2, y, &when, DIM, BG, false);
        scr.text(x0 + 2, y + 1, &clip(&format!("“{}”", wd.prompt), width), TEXT, BG, false);
        let stats = [
            plural(wd.regions, "place", "places"),
            plural(wd.people, "person", "people"),
            plural(wd.creatures, "creature", "creatures"),
            plural(wd.species, "species", "species"),
            plural(wd.kinds, "kind of thing", "kinds of things"),
            plural(wd.built, "thing you made", "things you made"),
        ]
        .join(" · ");
        scr.text(x0 + 2, y + 2, &clip(&stats, width), DIM, BG, false);
    }
    let fy = h.saturating_sub(2);
    if deleting && sel < list.len() {
        let q = clip(&format!("Delete “{}” for good?", list[sel].name), w.saturating_sub(40) as usize);
        let n = q.chars().count() as u16 + 3 + hint_width("Y delete   Any other key keep");
        let x = scr.text(w.saturating_sub(n) / 2, fy, &q, ACCENT, BG, true);
        scr.hints(x + 3, fy, "Y delete   Any other key keep", DIM, BG);
    } else {
        let keys = if list.is_empty() { "N new world   Q quit" } else { "↑↓ choose   Enter open   N new world   D delete   Q quit" };
        scr.hints(w.saturating_sub(hint_width(keys)) / 2, fy, keys, DIM, BG);
    }
}

/// Your worlds: open one, start a new one, or delete one.
pub fn pick(win: &mut Win) -> Choice {
    let mut list = worlds();
    let (w, h, _) = win.grid();
    let mut scr = Screen::new(w, h, true);
    let mut sel = 0usize;
    let mut deleting = false;
    loop {
        fit(win, &mut scr);
        draw(&mut scr, &list, sel, deleting);
        win.present(&scr, 0, None);
        for ev in win.pump(Duration::from_millis(500)) {
            let k = match ev {
                Ev::Close => return Choice::Quit,
                Ev::Key(k) if k.kind != KeyEventKind::Release => k,
                _ => continue,
            };
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
                KeyCode::Enter if !list.is_empty() => return Choice::Open(list[sel].path.clone()),
                KeyCode::Char('n') | KeyCode::Char('N') => match compose_on(win, &mut scr, Draft::rolled(), "back") {
                    Composed::Prompt(p) => return Choice::New(p),
                    Composed::Quit => return Choice::Quit,
                    Composed::Back => {}
                },
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Choice::Quit,
                _ => {}
            }
        }
    }
}

/// A title and a line in the middle (while a world opens).
pub fn notice(win: &mut Win, title: &str, line: &str) {
    let (w, h, _) = win.grid();
    let mut scr = Screen::new(w, h, true);
    blank(&mut scr);
    scr.title(w as f32 / 2.0, h as f32 / 2.0 - 1.5, title, 2.4, ACCENT);
    scr.center_text(w as f32 / 2.0, h / 2 + 1, &clip(line, w.saturating_sub(4) as usize), DIM);
    win.present(&scr, 0, None);
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
            let shown = screen_text(&scr);
            assert!((shown.contains("Enter") && shown.contains("begin")) || w < 20, "{shown}");
            d.key(key(KeyCode::Home));
            draw_new(&mut scr, &d, "back");
            draw(&mut scr, &[], 0, false);
        }
        let mut scr = Screen::new(100, 30, true);
        draw_new(&mut scr, &Draft::rolled(), "quit");
        let shown = screen_text(&scr);
        assert!(shown.contains("A new world") && shown.contains("Esc") && shown.contains("quit"), "{shown}");
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
