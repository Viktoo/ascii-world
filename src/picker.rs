//! `pocket list`: pick a world (or start a new one) with the arrow keys.

use crate::term::{self, Cell, Screen};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub struct World {
    pub path: PathBuf,
    pub name: String,
    pub prompt: String,
    pub regions: i64,
    pub people: i64,
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
    Some(World {
        path: path.to_path_buf(),
        name: if name.is_empty() { path.file_stem().map(|s| s.to_string_lossy().replace('-', " ")).unwrap_or_default() } else { name },
        prompt,
        regions: count("SELECT COUNT(*) FROM regions WHERE status = 'done'"),
        people: count("SELECT COUNT(*) FROM characters"),
        built: count(&format!("SELECT COUNT(*) FROM versions WHERE kind = 'create' AND id NOT IN ({undone})")),
        modified: std::fs::metadata(path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH),
    })
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

fn draw(scr: &mut Screen, list: &[World], sel: usize, typing: Option<&str>) {
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
        let stats = format!("{} · {} · {}", plural(wd.regions, "place", "places"), plural(wd.people, "person", "people"), plural(wd.built, "thing you built", "things you built"));
        scr.text(4, y + 2, &clip(&stats, width), DIM, bg, false);
    }
    let fy = h.saturating_sub(2);
    match typing {
        Some(t) => {
            scr.text(2, fy.saturating_sub(1), "Describe your new world, then press Enter (Esc to cancel):", DIM, BG, false);
            let shown = clip(t, w.saturating_sub(8) as usize);
            let x = scr.text(2, fy, &format!("> {shown}"), TEXT, BG, false);
            scr.set(x, fy, Cell { ch: '▏', fg: ACCENT, bg: BG, bold: false });
        }
        None => {
            scr.text(2, fy, "↑↓ choose   Enter open   n new world   q quit", DIM, BG, false);
        }
    }
}

pub fn pick() -> Result<Choice> {
    let list = worlds();
    let _ti = term::enter()?;
    let (w, h) = crossterm::terminal::size().unwrap_or((100, 30));
    let mut scr = Screen::new(w, h, term::truecolor());
    let mut out = std::io::stdout();
    let mut sel = 0usize;
    let mut typing: Option<String> = None;
    let choice = loop {
        draw(&mut scr, &list, sel, typing.as_deref());
        scr.flush(&mut out)?;
        if !event::poll(Duration::from_millis(500))? {
            continue;
        }
        match event::read()? {
            Event::Resize(w, h) => scr.resize(w, h),
            Event::Paste(s) => {
                if let Some(t) = typing.as_mut() {
                    t.push_str(&s.replace(['\n', '\r'], " "));
                }
            }
            Event::Key(k) if k.kind != KeyEventKind::Release => {
                if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c')) {
                    break Choice::Quit;
                }
                if let Some(t) = typing.as_mut() {
                    match k.code {
                        KeyCode::Esc => typing = None,
                        KeyCode::Enter if !t.trim().is_empty() => break Choice::New(t.trim().to_string()),
                        KeyCode::Backspace => {
                            t.pop();
                        }
                        KeyCode::Char(c) => t.push(if k.modifiers.contains(KeyModifiers::SHIFT) { c.to_uppercase().next().unwrap_or(c) } else { c }),
                        _ => {}
                    }
                    continue;
                }
                match k.code {
                    KeyCode::Up => sel = sel.saturating_sub(1),
                    KeyCode::Down => sel = (sel + 1).min(list.len().saturating_sub(1)),
                    KeyCode::Enter if !list.is_empty() => break Choice::Open(list[sel].path.clone()),
                    KeyCode::Char('n') | KeyCode::Char('N') => typing = Some(String::new()),
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
