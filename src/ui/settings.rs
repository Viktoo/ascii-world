//! The settings screen, in tabs: the same from the title and from a world's
//! pause menu. Changes are saved at once.

use super::{ACCENT, DIM, FAINT, Kind, PANEL, PANEL_EDGE, R, TEXT, Ui, left};
use crate::llm::{CLAUDE_MODELS, Role};
use crate::settings::{self, Settings};
use crate::term::{Deco, Face, Screen};

pub const TABS: &[&str] = &["Video", "Sound", "AI", "Together", "General", "Controls"];
const BUDGETS: &[f64] = &[0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 0.0];
const FPS: &[f64] = &[15.0, 30.0, 45.0, 60.0, 90.0, 120.0, 144.0, 240.0];
const GUEST_BUDGETS: &[f64] = &[0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0];
/// Row height, in rows.
pub const ROW: f32 = 2.2;

/// What the game around the screen knows (in a world: what is spent, what is heard).
#[derive(Default, Clone)]
pub struct Live {
    /// Spent this session, when a world is open with an LLM.
    pub spent: Option<f64>,
    pub sound_note: Option<String>,
    pub voices_note: Option<String>,
    pub ears_note: Option<String>,
    /// The detail drawn now ("240p, blended to 480p").
    pub detail_now: Option<String>,
    /// Playing together now (the name can't change mid-game).
    pub together: bool,
}

/// What changed, for the game to act on.
#[derive(Default, Debug, Clone, Copy)]
pub struct Out {
    pub changed: bool,
    pub sound: bool,
    pub ears: bool,
    pub budget: bool,
    pub ai: bool,
    pub reset_spent: bool,
    pub details: bool,
    pub done: bool,
}

#[derive(Default)]
pub struct Page {
    pub tab: usize,
    scroll: f32,
    /// The tab's rarer settings are open (under "More options").
    more: bool,
}

/// The next value along `list` from the one nearest `cur`.
pub fn step(list: &[f64], cur: f64, dir: i32) -> f64 {
    let i = list.iter().enumerate().min_by(|a, b| (a.1 - cur).abs().total_cmp(&(b.1 - cur).abs())).map(|x| x.0).unwrap_or(0);
    list[(i as i32 + dir).clamp(0, list.len() as i32 - 1) as usize]
}

fn money(v: f64) -> String {
    if v <= 0.0 { "no limit".into() } else { format!("${v:.2}") }
}

/// The detail steps by name: how sharp, in plain words.
fn detail_name(d: u32) -> String {
    match d {
        settings::AUTO => "Auto".into(),
        0 => "Sharpest".into(),
        l if l <= 180 => "Lowest".into(),
        l if l <= 240 => "Low".into(),
        l if l <= 360 => "Medium".into(),
        l if l <= 480 => "High".into(),
        l if l <= 720 => "Higher".into(),
        _ => "Very high".into(),
    }
}

impl Page {
    /// A quiet row that opens or shuts the tab's rarer settings; whether they show.
    fn more_row(&mut self, ui: &mut Ui, s: &mut Screen, rows: &mut Rows) -> bool {
        let r = rows.next(ROW);
        let b = R::new(r.x + 0.4, r.y + 0.3, 18.0, r.h - 0.6);
        if ui.button(s, b, if self.more { "Fewer options" } else { "More options" }, Kind::Quiet) {
            self.more = !self.more;
        }
        self.more
    }
}

fn view_name(r: i32) -> &'static str {
    match r {
        0 => "Just here",
        1 => "Near",
        2 => "Medium",
        3 => "Far",
        _ => "Very far",
    }
}

/// Rows down a column: each call gives the next row's rectangle.
pub struct Rows {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    /// Rows above this are scrolled off (not drawn), and below `bottom`.
    pub top: f32,
    pub bottom: f32,
}

impl Rows {
    pub fn next(&mut self, h: f32) -> R {
        let r = R::new(self.x, self.y, self.w, h);
        self.y += h;
        r
    }

    pub fn shown(&self, r: R) -> bool {
        r.y >= self.top - 0.01 && r.y + r.h <= self.bottom + 0.01
    }

    /// A small heading over a group of rows.
    pub fn head(&mut self, s: &mut Screen, text: &str) {
        let r = self.next(1.8);
        if self.shown(r) {
            s.labels.push(left(r.x + 1.0, r.y + 1.05, text, 1.05, [150, 170, 220], Face::Title));
        }
    }

    /// A quiet line under a row.
    pub fn note(&mut self, s: &mut Screen, text: &str) {
        // Small words: more of them fit a line than cells.
        for l in crate::term::wrap(text, ((self.w - 2.0) / 0.75).max(10.0) as usize) {
            let r = self.next(1.1);
            if self.shown(r) {
                s.labels.push(left(r.x + 1.0, r.mid_y(), &l, 0.85, DIM, Face::Body));
            }
        }
    }
}

/// The models rows (also on the new-world screen): a list for Claude, a
/// field for another provider. True when one changed.
pub fn models(ui: &mut Ui, s: &mut Screen, rows: &mut Rows, st: &mut Settings) -> bool {
    let claude = st.provider != "other";
    let mut changed = false;
    for role in Role::ALL {
        let i = role as usize;
        let r = rows.next(ROW);
        let locked = settings::env_model(i);
        if claude {
            let mut items: Vec<String> = CLAUDE_MODELS.iter().map(|(_, n)| n.to_string()).collect();
            let cur = if st.models[i].trim().is_empty() { role.default_model(true) } else { st.models[i].trim() };
            let sel = match CLAUDE_MODELS.iter().position(|(id, _)| *id == cur) {
                Some(p) => p,
                None => {
                    items.push(cur.to_string());
                    items.len() - 1
                }
            };
            if let Some(p) = ui.dropdown(s, r, role.job(), &items, sel, locked) {
                if let Some((id, _)) = CLAUDE_MODELS.get(p) {
                    st.models[i] = if *id == role.default_model(true) { String::new() } else { id.to_string() };
                    changed = true;
                }
            }
        } else {
            changed |= ui.field(s, r, role.job(), &mut st.models[i], role.default_model(false), false, locked);
        }
    }
    changed
}

/// The AI rows: who answers, the key, the models.
pub fn ai(ui: &mut Ui, s: &mut Screen, rows: &mut Rows, st: &mut Settings) -> bool {
    let mut changed = ai_who(ui, s, rows, st);
    rows.head(s, "Models");
    changed |= models(ui, s, rows, st);
    changed
}

/// Who answers, and the key.
fn ai_who(ui: &mut Ui, s: &mut Screen, rows: &mut Rows, st: &mut Settings) -> bool {
    let mut changed = false;
    let providers = vec!["Claude (Anthropic)".to_string(), "Other (OpenAI-compatible)".to_string()];
    let r = rows.next(ROW);
    let sel = (st.provider == "other") as usize;
    if let Some(p) = ui.dropdown(s, r, "Who answers", &providers, sel, settings::env_provider()) {
        st.provider = if p == 1 { "other".into() } else { "anthropic".into() };
        changed = true;
    }
    if st.provider == "other" {
        let r = rows.next(ROW);
        changed |= ui.field(s, r, "Address", &mut st.base_url, "http://localhost:11434/v1", false, settings::env_provider());
    }
    let r = rows.next(ROW);
    let hint = if st.provider == "other" { "if it needs one" } else { "sk-ant-… from platform.claude.com" };
    changed |= ui.field(s, r, "API key", &mut st.api_key, hint, true, settings::env_key(&st.provider));
    changed
}

impl Page {
    /// Draw the settings in `area`; `live` from the world when one is open.
    pub fn draw(&mut self, ui: &mut Ui, s: &mut Screen, area: R, st: &mut Settings, live: &Live) -> Out {
        let mut out = Out::default();
        s.deco(Deco::panel(area.x, area.y, area.w, area.h, PANEL).border(PANEL_EDGE));
        s.labels.push(left(area.x + 2.0, area.y + 2.0, "Settings", 1.8, ACCENT, Face::Title));
        if let Some(t) = ui.tabs(s, area.x + 2.0, area.y + 3.6, TABS, self.tab) {
            self.tab = t;
            self.scroll = 0.0;
            self.more = false;
        }
        let top = area.y + 6.2;
        let bottom = area.y + area.h - 3.2;
        // The page scrolls with the wheel when it is longer than the panel.
        self.scroll = (self.scroll - ui.wheel * 2.0).max(0.0);
        // Keep what the keys are on in view.
        if let Some(f) = ui.last_focus_rect.filter(|f| f.h > 1.7) {
            if f.y + f.h > bottom {
                self.scroll += f.y + f.h - bottom;
            } else if f.y < top {
                self.scroll = (self.scroll - (top - f.y)).max(0.0);
            }
        }
        let mut rows = Rows { x: area.x + 1.0, y: top - self.scroll, w: area.w - 2.0, top, bottom };
        ui.clip = Some((top, bottom));
        let before = st.clone();
        match TABS[self.tab] {
            "Video" => self.video(ui, s, &mut rows, st, live),
            "Sound" => {
                let r = rows.next(ROW);
                out.sound |= ui.toggle(s, r, "Sound", &mut st.sound, settings::env_sound());
                if let Some(n) = &live.sound_note {
                    rows.note(s, n);
                }
                let r = rows.next(ROW);
                out.sound |= ui.slider(s, r, "Volume", &mut st.volume, 0.1, false);
                let r = rows.next(ROW);
                out.sound |= ui.toggle(s, r, "Voices", &mut st.voices, settings::env_voices());
                if let Some(n) = &live.voices_note {
                    rows.note(s, n);
                }
                let r = rows.next(ROW);
                out.sound |= ui.slider(s, r, "Voice volume", &mut st.voice_volume, 0.1, false);
                let r = rows.next(ROW);
                out.ears |= ui.toggle(s, r, "Speak to talk (microphone)", &mut st.speak_to_talk, settings::env_mic());
                rows.note(s, live.ears_note.as_deref().unwrap_or("Say what you want to say instead of typing it."));
            }
            "AI" => {
                out.ai |= ai_who(ui, s, &mut rows, st);
                rows.head(s, "Spending");
                let r = rows.next(ROW);
                let d = ui.choice(s, r, "Budget per session", &money(st.budget_usd), settings::env_budget());
                if d != 0 {
                    let cur = if st.budget_usd <= 0.0 { 0.0 } else { st.budget_usd };
                    let i = if cur == 0.0 { BUDGETS.len() as i32 - 1 } else { BUDGETS.iter().position(|b| (*b - cur).abs() < 1e-9).unwrap_or(2) as i32 };
                    st.budget_usd = BUDGETS[(i + d).clamp(0, BUDGETS.len() as i32 - 1) as usize];
                    out.budget = true;
                }
                if let Some(spent) = live.spent {
                    let r = rows.next(ROW);
                    if rows.shown(r) {
                        s.labels.push(left(r.x + 1.0, r.mid_y(), &format!("Spent this session: ${spent:.2}"), 1.0, TEXT, Face::Body));
                    }
                    let b = R::new(r.x + r.w - 25.0, r.y + 0.3, 11.0, r.h - 0.6);
                    out.reset_spent |= ui.button(s, b, "Reset to $0", Kind::Normal);
                    let b = R::new(r.x + r.w - 13.0, r.y + 0.3, 12.0, r.h - 0.6);
                    out.details |= ui.button(s, b, "Details", Kind::Normal);
                }
                if self.more_row(ui, s, &mut rows) {
                    rows.head(s, "Models");
                    out.ai |= models(ui, s, &mut rows, st);
                }
                rows.note(s, "Changes to who answers and the models take effect when a world opens.");
            }
            "Together" => {
                let r = rows.next(ROW);
                out.changed |= ui.field(s, r, "Your name", &mut st.name, &settings::default_name(), false, live.together);
                let r = rows.next(ROW);
                let v = if st.guest_budget_usd <= 0.0 { "Nothing".into() } else { format!("${:.2}", st.guest_budget_usd) };
                let d = ui.choice(s, r, "Each guest may spend", &v, false);
                if d != 0 {
                    st.guest_budget_usd = step(GUEST_BUDGETS, st.guest_budget_usd, d);
                }
                rows.note(s, "Guests use your AI while they play in your world; this is the most each may spend a session.");
            }
            "General" => {
                let r = rows.next(ROW);
                ui.toggle(s, r, "Key prompts and tips", &mut st.show_hints, false);
                let r = rows.next(ROW);
                let d = ui.choice(s, r, "Show what's being made", &cap(st.show_work.name()), false);
                if d != 0 {
                    st.show_work = st.show_work.next(d);
                }
                let r = rows.next(ROW);
                ui.toggle(s, r, "Tell me about new versions", &mut st.check_updates, false);
                let r = rows.next(ROW);
                s.labels.push(left(r.x + 1.0, r.mid_y(), &format!("Infinite Pocket {}", crate::db::build_name()), 0.9, FAINT, Face::Body));
            }
            _ => controls(s, &mut rows),
        }
        ui.clip = None;
        // Long pages: keep the scroll inside what there is.
        let len = rows.y + self.scroll - top;
        self.scroll = self.scroll.min((len - (bottom - top)).max(0.0));
        if self.scroll > 0.0 || len > bottom - top {
            let note = if self.scroll > 0.0 { "scroll for more ↑" } else { "scroll for more ↓" };
            s.labels.push(left(area.x + 2.0, bottom + 1.4, note, 0.8, FAINT, Face::Body));
        }
        let done = R::new(area.x + area.w - 14.0, area.y + area.h - 2.8, 12.0, 2.0);
        out.done = ui.button(s, done, "Done", Kind::Primary) || ui.back();
        out.changed |= out.sound || out.ears || out.budget || out.ai || *st != before;
        if out.changed {
            st.save();
        }
        out
    }

    fn video(&mut self, ui: &mut Ui, s: &mut Screen, rows: &mut Rows, st: &mut Settings, live: &Live) {
        let r = rows.next(ROW);
        let d = ui.choice(s, r, "Quality", &detail_name(st.window_detail), std::env::var_os("POCKET_WINDOW_DETAIL").is_some());
        if d != 0 {
            let i = settings::DETAILS.iter().position(|x| *x == st.window_detail).unwrap_or(1) as i32;
            st.window_detail = settings::DETAILS[(i + d).clamp(0, settings::DETAILS.len() as i32 - 1) as usize];
        }
        rows.note(s, "Auto keeps the game smooth; higher is sharper and slower.");
        let r = rows.next(ROW);
        let d = ui.choice(s, r, "View distance", view_name(st.region_radius), settings::env_region_radius());
        if d != 0 {
            st.region_radius = (st.region_radius + d).clamp(0, 4);
        }
        if !self.more_row(ui, s, rows) {
            return;
        }
        if let Some(n) = &live.detail_now {
            rows.note(s, &format!("Drawn at {n} now."));
        }
        let r = rows.next(ROW);
        ui.toggle(s, r, "Smooth edges (blend frames)", &mut st.blend_frames, settings::env_blend());
        let r = rows.next(ROW);
        ui.toggle(s, r, "Shadows", &mut st.shadows, settings::env_shadows());
        let r = rows.next(ROW);
        let d = ui.choice(s, r, "Frame rate cap", &format!("{:.0} fps", st.fps), settings::env_fps());
        if d != 0 {
            st.fps = step(FPS, st.fps as f64, d) as f32;
        }
    }
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The keys, to read.
fn controls(s: &mut Screen, rows: &mut Rows) {
    const KEYS: &[(&str, &str)] = &[
        ("W A S D", "walk"),
        ("Shift", "run (R keeps running)"),
        ("Mouse, or arrows", "look around (click the world to look with the mouse)"),
        ("Space / C", "jump, crouch"),
        ("Enter", "talk to whoever is in front of you"),
        ("/", "do or make anything, in words"),
        ("E", "use, sit, get in, pet"),
        ("G", "pick up or put down"),
        ("F", "throw"),
        ("Y / N", "answer yes or no"),
        ("1", "journal"),
        ("Esc", "pause menu"),
        ("F2", "inspect what you look at"),
    ];
    for (k, what) in KEYS {
        let r = rows.next(1.6);
        if rows.shown(r) {
            s.labels.push(left(r.x + 1.0, r.mid_y(), k, 0.95, ACCENT, Face::Bold));
            s.labels.push(left(r.x + 18.0, r.mid_y(), what, 0.95, TEXT, Face::Body));
        }
    }
    rows.note(s, "In a world, /help lists every word command (/wave, /give, /ride, /undo…).");
}
