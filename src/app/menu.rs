//! The settings screen (Esc or F10 in walk mode): budget, display, this
//! world's life, and the keys. Changes take effect at once and are saved.
//! Its second page (2, or F3 in walk mode) lists this world's achievements.

use super::{ACCENT, App, DIM, TEXT};
use crate::achievements::{self, Tier};
use crate::settings;
use crossterm::event::{KeyCode, KeyEvent};

const BG: [u8; 3] = [16, 18, 26];
const SEL_BG: [u8; 3] = [48, 52, 72];
const HEAD: [u8; 3] = [150, 170, 220];

const BUDGETS: &[f64] = &[0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 0.0];
const FPS: &[f64] = &[15.0, 30.0, 45.0, 60.0, 90.0, 120.0, 144.0, 240.0];
const RADII: &[f64] = &[0.0, 1.0, 2.0, 3.0, 4.0];
const CREATURES: &[f64] = &[6.0, 12.0, 24.0, 48.0, 96.0, 200.0];
const LIFE: &[f64] = &[0.25, 0.5, 1.0, 2.0, 4.0, 8.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Row {
    Budget,
    Spent,
    Details,
    Fps,
    Shadows,
    Radius,
    ShowWork,
    Creatures,
    LifeSpeed,
    Hunting,
    Beings,
}

const ROWS: &[Row] = &[Row::Budget, Row::Spent, Row::Details, Row::Fps, Row::Shadows, Row::Radius, Row::ShowWork, Row::Creatures, Row::LifeSpeed, Row::Hunting, Row::Beings];

pub struct Menu {
    sel: usize,
    /// The spend details page: this world's total and the latest calls.
    details: Option<(f64, Vec<(f64, String, f64)>)>,
    /// The achievements page instead of the settings.
    achievements: bool,
    /// The chosen achievement on that page.
    ach_sel: usize,
}

/// A medal: more ornate as the challenge grows.
pub fn icon(t: Tier) -> &'static str {
    match t {
        Tier::Bronze => "(•)",
        Tier::Silver => "(✦)",
        Tier::Gold => "«★»",
        Tier::Diamond => "✧◆✧",
    }
}

pub fn tier_color(t: Tier) -> [u8; 3] {
    match t {
        Tier::Bronze => [205, 127, 50],
        Tier::Silver => [200, 208, 222],
        Tier::Gold => [255, 200, 70],
        Tier::Diamond => [140, 225, 255],
    }
}

pub fn tier_name(t: Tier) -> &'static str {
    match t {
        Tier::Bronze => "bronze",
        Tier::Silver => "silver",
        Tier::Gold => "gold",
        Tier::Diamond => "diamond",
    }
}

/// The next value along `list` from the one nearest `cur`.
fn step(list: &[f64], cur: f64, dir: i32) -> f64 {
    let i = list.iter().enumerate().min_by(|a, b| (a.1 - cur).abs().total_cmp(&(b.1 - cur).abs())).map(|x| x.0).unwrap_or(0);
    list[(i as i32 + dir).clamp(0, list.len() as i32 - 1) as usize]
}

/// What a paid call was for, in plain words.
fn purpose(p: &str) -> &str {
    match p {
        "dialogue" | "chat" => "conversation",
        "decide" => "someone deciding what to do",
        "create" => "building something",
        "type" => "a new kind of thing",
        "interpret" => "an action",
        "region" => "a new region",
        "genesis" => "the world",
        "summary" => "a memory",
        "gesture" => "a new gesture",
        other => other,
    }
}

fn ago(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    match s {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", s / 60),
        3600..86400 => format!("{} h ago", s / 3600),
        _ => format!("{} d ago", s / 86400),
    }
}

impl App {
    pub(super) fn toggle_menu(&mut self) {
        self.open_menu(false);
    }

    /// Open the settings (or its achievements page), or close it.
    pub(super) fn open_menu(&mut self, achievements: bool) {
        self.menu = match self.menu {
            Some(_) => None,
            None => Some(Menu { sel: 0, details: None, achievements, ach_sel: 0 }),
        };
        if self.log_view == 2 {
            self.set_log_view(0);
        }
        self.keys.down.clear();
        self.keys.until.clear();
        self.dirty = true;
    }

    pub(super) fn menu_key(&mut self, k: KeyEvent) {
        let Some(m) = self.menu.as_mut() else { return };
        self.dirty = true;
        if m.details.is_some() {
            if matches!(k.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Left | KeyCode::Backspace) {
                m.details = None;
            }
            return;
        }
        match k.code {
            KeyCode::Char('1') => m.achievements = false,
            KeyCode::Char('2') => m.achievements = true,
            _ => {}
        }
        if m.achievements {
            let n = achievements::ALL.len();
            match k.code {
                KeyCode::Esc | KeyCode::F(10) | KeyCode::F(3) => self.menu = None,
                KeyCode::Up => m.ach_sel = (m.ach_sel + n - 1) % n,
                KeyCode::Down | KeyCode::Tab => m.ach_sel = (m.ach_sel + 1) % n,
                KeyCode::PageUp => m.ach_sel = m.ach_sel.saturating_sub(8),
                KeyCode::PageDown => m.ach_sel = (m.ach_sel + 8).min(n - 1),
                _ => {}
            }
            return;
        }
        let row = ROWS[m.sel];
        match k.code {
            KeyCode::Esc | KeyCode::F(10) => self.menu = None,
            KeyCode::Up => m.sel = (m.sel + ROWS.len() - 1) % ROWS.len(),
            KeyCode::Down | KeyCode::Tab => m.sel = (m.sel + 1) % ROWS.len(),
            KeyCode::Left => self.change(row, -1),
            KeyCode::Right => self.change(row, 1),
            KeyCode::Enter | KeyCode::Char(' ') => {
                match row {
                    Row::Spent => self.change(row, 0),
                    Row::Details => {
                        let d = (self.db.usage_total(), self.db.usage_recent(10));
                        if let Some(m) = self.menu.as_mut() {
                            m.details = Some(d);
                        }
                    }
                    Row::Shadows | Row::ShowWork | Row::Hunting | Row::Beings => self.change(row, 1),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Whether an environment variable fixes this row for the run.
    fn locked(row: Row) -> bool {
        match row {
            Row::Budget => settings::env_budget(),
            Row::Fps => settings::env_fps(),
            Row::Shadows => settings::env_shadows(),
            Row::Radius => settings::env_region_radius(),
            _ => false,
        }
    }

    /// Change a row by one step (`dir` ±1; 0 for an action like reset).
    fn change(&mut self, row: Row, dir: i32) {
        if Self::locked(row) {
            return;
        }
        let s = &mut self.settings;
        match row {
            Row::Budget => {
                // "No limit" sits at the top of the list.
                let cur = if s.budget_usd <= 0.0 { 0.0 } else { s.budget_usd };
                let i = if cur == 0.0 { BUDGETS.len() as f64 - 1.0 } else { BUDGETS.iter().position(|b| (*b - cur).abs() < 1e-9).map(|i| i as f64).unwrap_or(2.0) };
                let n = (i as i32 + dir).clamp(0, BUDGETS.len() as i32 - 1) as usize;
                s.budget_usd = BUDGETS[n];
            }
            Row::Spent => {
                if let Some(l) = &self.llm {
                    *l.spent.lock() = 0.0;
                }
            }
            Row::Fps => s.fps = step(FPS, s.fps as f64, dir) as f32,
            Row::Shadows => s.shadows = !s.shadows,
            Row::Radius => s.region_radius = step(RADII, s.region_radius as f64, dir) as i32,
            Row::ShowWork => s.show_work = s.show_work.next(dir),
            Row::Creatures => self.set_world("max_creatures", step(CREATURES, self.sim.cfg.max_creatures as f64, dir)),
            Row::LifeSpeed => self.set_world("life_speed", step(LIFE, self.sim.cfg.life_speed as f64, dir)),
            Row::Hunting => self.set_world("hunting", if self.sim.cfg.hunting { 0.0 } else { 1.0 }),
            Row::Beings => self.set_world("create_beings", if self.sim.cfg.create_beings { 0.0 } else { 1.0 }),
            Row::Details => {}
        }
        if matches!(row, Row::Budget | Row::Fps | Row::Shadows | Row::Radius | Row::ShowWork) {
            self.settings.save();
        }
        if let Some(l) = &self.llm {
            *l.budget.lock() = self.settings.budget();
            if self.budget_paused && !l.over_budget() {
                self.budget_paused = false;
                self.say(None, "Budget raised: generation and dialogue are back on.", DIM);
            }
        }
    }

    /// A setting of this world only (kept in its kv table, like `sim.<key>`).
    fn set_world(&mut self, key: &str, v: f64) {
        self.sim.cfg.set(key, v as f32);
        let _ = self.db.kv_set(&format!("sim.{key}"), &v.to_string());
    }

    fn row_text(&self, row: Row) -> (String, String) {
        let s = &self.settings;
        let on = |b: bool| if b { "on" } else { "off" }.to_string();
        let (label, value) = match row {
            Row::Budget => ("Budget per session", s.budget().map(|b| format!("${b:.2}")).unwrap_or_else(|| "no limit".into())),
            Row::Spent => ("Spent this session", format!("${:.2}   Enter: reset to $0", self.spent())),
            Row::Details => ("Spend details", "Enter →".into()),
            Row::Fps => ("Frame rate cap", format!("{:.0} fps", s.fps)),
            Row::Shadows => ("Shadows", on(s.shadows)),
            Row::Radius => ("World loads around you", format!("{} region{}", s.region_radius, if s.region_radius == 1 { "" } else { "s" })),
            Row::ShowWork => ("Show work in progress", s.show_work.name().to_string()),
            Row::Creatures => ("Most of one kind of creature", format!("{} per region", self.sim.cfg.max_creatures)),
            Row::LifeSpeed => ("How fast lives go", format!("×{}", self.sim.cfg.life_speed)),
            Row::Hunting => ("Hunters kill their prey", on(self.sim.cfg.hunting)),
            Row::Beings => ("Actions can bring new beings", on(self.sim.cfg.create_beings)),
        };
        let value = if Self::locked(row) { format!("{value}   (set by env)") } else { value };
        (label.to_string(), value)
    }

    pub(super) fn draw_menu(&mut self, w: u16, vh: u16) {
        let Some(m) = self.menu.as_ref() else { return };
        let pw = (w as usize).saturating_sub(4).clamp(20, 76);
        let x0 = (w as usize).saturating_sub(pw) as u16 / 2;
        let mut lines: Vec<(String, [u8; 3], bool)> = Vec::new();
        let blank = (String::new(), TEXT, false);
        let tabs = format!("[1] Settings    [2] Achievements {}/{}", self.achievements.count(), achievements::ALL.len());
        if m.achievements {
            return self.draw_achievements(x0, pw, vh, &tabs);
        }
        if let Some((total, recent)) = &m.details {
            lines.push(("Spend details".into(), ACCENT, false));
            lines.push(blank.clone());
            lines.push((format!("This session            ${:.2}{}", self.spent(), self.settings.budget().map(|b| format!(" of ${b:.2}")).unwrap_or_default()), TEXT, false));
            lines.push((format!("This world, all time    ${total:.2}"), TEXT, false));
            lines.push(blank.clone());
            lines.push(("Latest calls".into(), HEAD, false));
            if recent.is_empty() {
                lines.push(("  none yet".into(), DIM, false));
            }
            let now = crate::db::now();
            for (t, p, c) in recent {
                lines.push((format!("  {:<12} {:<30} ${c:.3}", ago(now - t), purpose(p)), TEXT, false));
            }
            lines.push(blank.clone());
            lines.push(("Esc back".into(), DIM, false));
        } else {
            lines.push((tabs, ACCENT, false));
            for (i, row) in ROWS.iter().enumerate() {
                match row {
                    Row::Budget => lines.push(("Spending".into(), HEAD, false)),
                    Row::Fps => lines.push(("Display".into(), HEAD, false)),
                    Row::Creatures => lines.push(("This world".into(), HEAD, false)),
                    _ => {}
                }
                let (label, value) = self.row_text(*row);
                let sel = i == m.sel;
                lines.push((format!("{} {label:<30} {value}", if sel { "›" } else { " " }), if Self::locked(*row) { DIM } else { TEXT }, sel));
            }
            lines.push(blank.clone());
            lines.push(("↑↓ choose · ←→ change · Enter select · 2 achievements · Esc close".into(), DIM, false));
            lines.push(blank.clone());
            lines.push(("Keys".into(), HEAD, false));
            for l in [
                "Walk  W/S move · A/D strafe · ←→ turn · ↑↓ look · Tab blocks/ASCII",
                "      Enter talk · / do or make anything · e use · g grab/drop",
                "      f throw · y/n answer · F1 stats · F2 inspect · F3 achievements · q quit",
                "Log   1 bigger (half → full → small) · PgUp/PgDn scroll back",
                "Talk  type and Enter · Esc back to walking",
                "/help lists every / shortcut (/wave, /give, /ride, /undo…)",
            ] {
                lines.push((l.into(), DIM, false));
            }
        }
        let y0 = (vh as usize).saturating_sub(lines.len() + 2) as u16 / 2;
        let rows = std::iter::once(blank.clone()).chain(lines).chain(std::iter::once(blank));
        for (i, (text, fg, sel)) in rows.enumerate() {
            let y = y0 + i as u16;
            if y >= vh {
                break;
            }
            let t: String = text.chars().take(pw - 3).collect();
            let s = format!("  {t:<width$} ", width = pw - 3);
            self.screen.text(x0, y, &s, fg, if sel { SEL_BG } else { BG }, sel);
        }
    }

    /// The achievements page: every one, by group; the chosen one in full below.
    fn draw_achievements(&mut self, x0: u16, pw: usize, vh: u16, tabs: &str) {
        let Some(m) = self.menu.as_ref() else { return };
        let sel = m.ach_sel.min(achievements::ALL.len() - 1);
        let now = crate::db::now();
        // (text, colour, selected, medal colour for the first 3 columns)
        let mut list: Vec<(String, [u8; 3], bool, Option<[u8; 3]>)> = Vec::new();
        let mut sel_line = 0;
        let mut group = "";
        for (i, d) in achievements::ALL.iter().enumerate() {
            if d.group != group {
                group = d.group;
                list.push((group.to_string(), HEAD, false, None));
            }
            let got = self.achievements.earned(d.id);
            let when = got.map(|t| ago(now - t)).unwrap_or_default();
            if i == sel {
                sel_line = list.len();
            }
            let icon = if got.is_some() { icon(d.tier).to_string() } else { "( )".into() };
            list.push((format!("{icon} {:<22} {when}", d.title), if got.is_some() { TEXT } else { DIM }, i == sel, got.map(|_| tier_color(d.tier))));
        }
        // The chosen one, in full.
        let d = &achievements::ALL[sel];
        let got = self.achievements.earned(d.id);
        let mut detail = vec![(String::new(), TEXT, false, None)];
        let state = match got {
            Some(t) => format!("{} · earned {}", tier_name(d.tier), ago(now - t)),
            None => format!("{} · not yet", tier_name(d.tier)),
        };
        detail.push((format!("{}   {state}", d.title), if got.is_some() { tier_color(d.tier) } else { TEXT }, false, None));
        for l in crate::term::wrap(d.text, pw.saturating_sub(6)) {
            detail.push((l, DIM, false, None));
        }
        detail.push((String::new(), TEXT, false, None));
        detail.push(("↑↓ choose · PgUp/PgDn page · 1 settings · Esc close".into(), DIM, false, None));
        // As much of the list as fits, scrolled to keep the chosen one in view.
        let fit = (vh as usize).saturating_sub(detail.len() + 6).max(3);
        let top = sel_line.saturating_sub(fit / 2).min(list.len().saturating_sub(fit));
        let mut lines = vec![(tabs.to_string(), ACCENT, false, None), (String::new(), TEXT, false, None)];
        lines.extend(list.into_iter().skip(top).take(fit));
        lines.extend(detail);
        let blank = (String::new(), TEXT, false, None);
        let y0 = (vh as usize).saturating_sub(lines.len() + 2) as u16 / 2;
        let rows = std::iter::once(blank.clone()).chain(lines).chain(std::iter::once(blank));
        for (i, (text, fg, sel, medal)) in rows.enumerate() {
            let y = y0 + i as u16;
            if y >= vh {
                break;
            }
            let bg = if sel { SEL_BG } else { BG };
            let t: String = text.chars().take(pw - 3).collect();
            let s = format!("{}{t:<width$} ", if sel { "› " } else { "  " }, width = pw - 3);
            self.screen.text(x0, y, &s, fg, bg, sel);
            if let Some(c) = medal {
                let icon: String = t.chars().take(3).collect();
                self.screen.text(x0 + 2, y, &icon, c, bg, true);
            }
        }
    }
}
