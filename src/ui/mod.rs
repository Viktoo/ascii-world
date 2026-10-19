//! Menus as buttons: a small kit of controls drawn into a `Screen` (soft
//! shapes and free text), worked by the mouse or by the keys (↑↓ or Tab to
//! move, Enter to press, ←→ to change, Esc to go back).
//!
//! Immediate mode: every frame a screen asks for its controls in order
//! (`ui.button(..)` and so on); each draws itself and says whether it was
//! used, from the input fed in since the last frame.

mod cosmos;
pub mod screens;
pub mod settings;
pub mod title;

use crate::term::{Deco, Face, Label, Screen};
use crate::window::Ev;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

pub const TEXT: [u8; 3] = [226, 228, 236];
pub const DIM: [u8; 3] = [128, 133, 152];
pub const FAINT: [u8; 3] = [78, 82, 100];
pub const ACCENT: [u8; 3] = [255, 200, 110];
/// Words on an amber button.
pub const ON_ACCENT: [u8; 3] = [40, 26, 8];
pub const PANEL: [u8; 3] = [22, 24, 34];
pub const PANEL_EDGE: [u8; 3] = [48, 52, 72];
pub const BUTTON: [u8; 3] = [38, 41, 58];
pub const BUTTON_HOT: [u8; 3] = [54, 58, 82];
pub const BUTTON_EDGE: [u8; 3] = [72, 77, 104];
pub const DANGER: [u8; 3] = [232, 110, 96];
pub const ROW_HOT: [u8; 3] = [32, 35, 50];

/// A rectangle in cells (fractions allowed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct R {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl R {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> R {
        R { x, y, w, h }
    }

    pub fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }

    /// Split across into `n` equal parts with `gap` between them.
    pub fn cols(&self, n: usize, gap: f32) -> Vec<R> {
        let w = (self.w - gap * (n as f32 - 1.0)) / n as f32;
        (0..n).map(|i| R::new(self.x + i as f32 * (w + gap), self.y, w, self.h)).collect()
    }

    pub fn mid_y(&self) -> f32 {
        self.y + self.h * 0.5
    }
}

/// What happened to a list item this frame.
#[derive(Default, Clone, Copy, Debug)]
pub struct Item {
    pub click: bool,
    /// The second click of a double click.
    pub double: bool,
}

/// How a button looks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// The main thing to do here: amber.
    Primary,
    Normal,
    /// Deletes or ends something: red words.
    Danger,
    /// Small and quiet (Back, links).
    Quiet,
}

#[derive(Default)]
pub struct Ui {
    /// Where the pointer is (cells), once it has moved.
    pub mouse: Option<(f32, f32)>,
    moved: bool,
    click: Option<(f32, f32)>,
    /// This click is the second in quick succession at the same place.
    pub double: bool,
    last_click: Option<(Instant, (f32, f32))>,
    keys: Vec<KeyEvent>,
    /// Text pasted this frame.
    pub pasted: Vec<String>,
    /// Wheel rows this frame (up positive).
    pub wheel: f32,
    /// The control the keys work (also the one under the pointer). None
    /// until a key moves it or the pointer is over one.
    pub focus: Option<usize>,
    n: usize,
    last_n: usize,
    /// The open dropdown, and which of its items the keys are on.
    open: Option<(usize, usize)>,
    /// The pointer is over something to click (the window shows a hand).
    pub hot: bool,
    hot_now: bool,
    /// Something was pressed or changed this frame (draw again).
    pub acted: bool,
    /// When the frame began (carets blink, things move).
    pub t: f32,
    born: Option<Instant>,
    /// Controls outside these rows (top, bottom) are not drawn or worked (a scrolled page).
    pub clip: Option<(f32, f32)>,
    /// Where the focused control was last drawn (or would have been).
    pub focus_rect: Option<R>,
    /// The same, last frame.
    pub last_focus_rect: Option<R>,
    /// The controls that take typing, this frame and the last.
    fields: Vec<usize>,
    last_fields: Vec<usize>,
    /// Lists (↑↓ move within them first), this frame and the last.
    lists: Vec<usize>,
    last_lists: Vec<usize>,
}

impl Ui {
    pub fn new() -> Ui {
        Ui::default()
    }

    /// Take a window event; false if it is not the kit's (a dropped file, Close).
    pub fn feed(&mut self, ev: &Ev) -> bool {
        match ev {
            Ev::Mouse(x, y) => {
                self.mouse = Some((*x, *y));
                self.moved = true;
            }
            Ev::Click(x, y) => self.press_at(*x, *y),
            Ev::Scroll(d) => self.wheel += d,
            Ev::Key(k) if k.kind != KeyEventKind::Release => self.keys.push(*k),
            Ev::Key(_) => {}
            Ev::Paste(t) => self.pasted.push(t.clone()),
            _ => return false,
        }
        true
    }

    pub fn press_at(&mut self, x: f32, y: f32) {
        self.mouse = Some((x, y));
        self.moved = true;
        let now = Instant::now();
        self.double = self.last_click.is_some_and(|(t, p)| now.duration_since(t) < Duration::from_millis(400) && (p.0 - x).abs() < 1.0 && (p.1 - y).abs() < 0.6);
        self.last_click = Some((now, (x, y)));
        self.click = Some((x, y));
    }

    pub fn key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Release {
            self.keys.push(k);
        }
    }

    /// Start a frame's controls.
    pub fn begin(&mut self) {
        // Moves pressed before anything else happen first ("down, down, Enter"
        // presses the third control, even all in one frame).
        let in_list = self.focus.is_some_and(|f| self.lists.contains(&f));
        if self.open.is_none() && self.last_n > 0 && !in_list {
            while let Some(k) = self.keys.first().copied() {
                let back = k.code == KeyCode::BackTab || (k.code == KeyCode::Tab && k.modifiers.contains(KeyModifiers::SHIFT));
                match k.code {
                    KeyCode::Up | KeyCode::BackTab => self.step(-1, self.last_n),
                    KeyCode::Tab if back => self.step(-1, self.last_n),
                    KeyCode::Down | KeyCode::Tab if !self.typing() || k.code == KeyCode::Down => self.step(1, self.last_n),
                    _ => break,
                }
                self.keys.remove(0);
            }
        }
        self.n = 0;
        self.last_focus_rect = self.focus_rect.take();
        self.last_fields = std::mem::take(&mut self.fields);
        self.last_lists = std::mem::take(&mut self.lists);
        self.hot_now = false;
        self.acted = false;
        self.t = self.born.get_or_insert_with(Instant::now).elapsed().as_secs_f32();
    }

    /// End a frame: ↑↓ and Tab move between controls; the input is used up.
    pub fn end(&mut self) {
        let n = self.n;
        for k in std::mem::take(&mut self.keys) {
            let back = k.code == KeyCode::BackTab || (k.code == KeyCode::Tab && k.modifiers.contains(KeyModifiers::SHIFT));
            match k.code {
                _ if n == 0 => {}
                KeyCode::Up | KeyCode::BackTab => self.step(-1, n),
                KeyCode::Tab if back => self.step(-1, n),
                KeyCode::Down | KeyCode::Tab => self.step(1, n),
                _ => {}
            }
        }
        if let Some(f) = self.focus {
            if n > 0 && f >= n {
                self.focus = Some(n - 1);
            }
        }
        // A click on nothing closes an open list.
        if self.click.is_some() && self.open.is_some() {
            self.open = None;
            self.acted = true;
        }
        self.last_n = n;
        self.click = None;
        self.moved = false;
        self.wheel = 0.0;
        self.pasted.clear();
        self.hot = self.hot_now;
    }

    fn step(&mut self, d: i32, n: usize) {
        self.focus = Some(match self.focus {
            None => if d > 0 { 0 } else { n - 1 },
            Some(f) => (f as i32 + d).rem_euclid(n as i32) as usize,
        });
        self.acted = true;
    }

    /// Forget where the keys were (a new screen).
    pub fn reset(&mut self) {
        self.focus = None;
        self.open = None;
        self.keys.clear();
        self.click = None;
    }

    /// A key pressed this frame and not yet used by a control; it is used now.
    pub fn take(&mut self, code: KeyCode) -> bool {
        match self.keys.iter().position(|k| k.code == code) {
            Some(i) => {
                self.keys.remove(i);
                true
            }
            None => false,
        }
    }

    /// Esc was pressed (and did not just close a list).
    pub fn back(&mut self) -> bool {
        if self.open.is_some() && self.keys.iter().any(|k| k.code == KeyCode::Esc) {
            return false;
        }
        self.take(KeyCode::Esc)
    }

    /// A letter pressed (either case), for quiet shortcuts.
    pub fn letter(&mut self, c: char) -> bool {
        if self.typing() {
            return false;
        }
        self.take(KeyCode::Char(c.to_ascii_lowercase())) || self.take(KeyCode::Char(c.to_ascii_uppercase()))
    }

    /// The focused control takes typed letters (a field).
    fn typing(&self) -> bool {
        self.focus.is_some_and(|f| self.fields.contains(&f) || self.last_fields.contains(&f))
    }

    /// The next control's number (and whether the keys are on it).
    fn id(&mut self) -> (usize, bool) {
        let id = self.n;
        self.n += 1;
        (id, self.focus == Some(id))
    }

    /// Note where a control is; true if it is clipped away (skip it).
    fn place(&mut self, id: usize, r: R) -> bool {
        if self.focus == Some(id) {
            self.focus_rect = Some(r);
        }
        self.clip.is_some_and(|(t, b)| r.y < t - 0.01 || r.y + r.h > b + 0.01)
    }

    /// The pointer over `r` (not while a list is open elsewhere).
    fn over(&mut self, id: usize, r: R) -> bool {
        if self.open.is_some_and(|(o, _)| o != id) {
            return false;
        }
        let over = self.mouse.is_some_and(|m| r.contains(m));
        if over {
            self.hot_now = true;
            // The pointer takes the keys along, but not out of a field being typed in.
            if self.moved && !self.typing() {
                self.focus = Some(id);
            }
        }
        over
    }

    /// A free area that takes typing when focused (a box of words drawn by
    /// the caller): (clicked, focused, the keys for it, text pasted).
    pub fn area(&mut self, r: R) -> (bool, bool, Vec<KeyEvent>, Vec<String>) {
        let (id, _) = self.id();
        self.fields.push(id);
        self.over(id, r);
        let hit = self.clicked(id, r);
        let focused = self.focus == Some(id);
        if !focused {
            return (hit, false, Vec::new(), Vec::new());
        }
        let (mine, rest): (Vec<KeyEvent>, Vec<KeyEvent>) = std::mem::take(&mut self.keys).into_iter().partition(|k| !matches!(k.code, KeyCode::Up | KeyCode::Down | KeyCode::Tab | KeyCode::BackTab | KeyCode::Esc));
        self.keys = rest;
        if !mine.is_empty() {
            self.acted = true;
        }
        (hit, true, mine, std::mem::take(&mut self.pasted))
    }

    fn clicked(&mut self, id: usize, r: R) -> bool {
        if self.open.is_some_and(|(o, _)| o != id) {
            return false;
        }
        if self.click.is_some_and(|c| r.contains(c)) {
            self.click = None;
            self.focus = Some(id);
            return true;
        }
        false
    }

    /// Enter (or Space) on the focused control.
    fn pressed(&mut self, focused: bool) -> bool {
        focused && (self.take(KeyCode::Enter) || (!self.typing() && self.take(KeyCode::Char(' '))))
    }

    /// ← or → on the focused control: -1, 1 or 0.
    fn arrows(&mut self, focused: bool) -> i32 {
        if !focused {
            return 0;
        }
        if self.take(KeyCode::Left) {
            -1
        } else if self.take(KeyCode::Right) {
            1
        } else {
            0
        }
    }

    // ------------------------------------------------------------- controls

    /// A button; true when pressed.
    pub fn button(&mut self, s: &mut Screen, r: R, text: &str, kind: Kind) -> bool {
        self.button_on(s, r, text, kind, true)
    }

    /// A button that may be greyed out (`on` false: drawn, not pressable).
    pub fn button_on(&mut self, s: &mut Screen, r: R, text: &str, kind: Kind, on: bool) -> bool {
        let (id, focused) = self.id();
        if self.place(id, r) {
            return false;
        }
        let over = on && self.over(id, r);
        let hot = on && (over || focused);
        let hit = on && (self.clicked(id, r) || self.pressed(focused));
        let (fill, edge, fg) = match kind {
            _ if !on => (PANEL, PANEL_EDGE, FAINT),
            Kind::Primary if hot => ([255, 214, 140], [255, 236, 200], ON_ACCENT),
            Kind::Primary => (ACCENT, [255, 220, 160], ON_ACCENT),
            Kind::Quiet if hot => (BUTTON, BUTTON_EDGE, TEXT),
            Kind::Quiet => (PANEL, PANEL_EDGE, DIM),
            Kind::Danger if hot => (BUTTON_HOT, DANGER, DANGER),
            Kind::Danger => (BUTTON, BUTTON_EDGE, DANGER),
            Kind::Normal if hot => (BUTTON_HOT, ACCENT, TEXT),
            Kind::Normal => (BUTTON, BUTTON_EDGE, TEXT),
        };
        let lift = if hot { -0.06 } else { 0.0 };
        s.deco(Deco { x: r.x, y: r.y + lift, w: r.w, h: r.h, fill, alpha: 1.0, radius: (r.h * 0.32).min(0.7), shadow: kind == Kind::Primary || hot, border: Some(edge) });
        let size = if r.h >= 1.8 { 1.0 } else { 0.85 };
        s.labels.push(Label { x: r.x + r.w * 0.5, y: r.mid_y() + lift, text: fit(text, r.w - 1.0, size), size, color: fg, face: Face::Bold, center: true });
        if hit {
            self.acted = true;
        }
        hit
    }

    /// A list worked as one control: ↑↓ move `sel` within it (and out of
    /// it past either end), Enter is returned. Its items are `card`s.
    pub fn list(&mut self, r: R, len: usize, sel: &mut usize) -> (bool, bool) {
        let (id, focused) = self.id();
        self.lists.push(id);
        if self.place(id, r) || !focused || len == 0 {
            return (false, focused);
        }
        let mut keep = Vec::new();
        for k in std::mem::take(&mut self.keys) {
            match k.code {
                KeyCode::Up if *sel > 0 => *sel -= 1,
                KeyCode::Down if *sel + 1 < len => *sel += 1,
                _ => keep.push(k),
            }
        }
        if keep.len() != self.keys.len() {
            self.acted = true;
        }
        self.keys = keep;
        let enter = self.take(KeyCode::Enter);
        (enter, true)
    }

    /// An item of a `list`, worked by the pointer: drawn as a card.
    /// `glow`: the list has the keys (the selected one shows it).
    pub fn card(&mut self, s: &mut Screen, r: R, selected: bool, glow: bool) -> Item {
        let over = self.open.is_none() && self.mouse.is_some_and(|m| r.contains(m));
        if over {
            self.hot_now = true;
        }
        let click = self.open.is_none() && self.click.is_some_and(|c| r.contains(c));
        if click {
            self.click = None;
            self.acted = true;
        }
        let fill = if selected { [34, 37, 54] } else if over { [28, 30, 44] } else { PANEL };
        let d = Deco::panel(r.x, r.y, r.w, r.h, fill);
        let edge = if selected && glow { Some([255, 226, 170]) } else if selected { Some(ACCENT) } else if over { Some(BUTTON_EDGE) } else { None };
        s.deco(match edge {
            Some(e) => d.border(e),
            None => d,
        });
        Item { click, double: click && self.double }
    }

    /// A settings row's ground: a soft highlight when it is the one in hand.
    fn row(&mut self, s: &mut Screen, r: R, hot: bool, label: &str, locked: bool) {
        if hot {
            s.deco(Deco::pill(r.x, r.y + 0.1, r.w, r.h - 0.2, ROW_HOT));
        }
        let color = if locked { DIM } else { TEXT };
        s.labels.push(left(r.x + 1.0, r.mid_y(), label, 1.0, color, Face::Body));
        if locked {
            let note = "set by env";
            s.labels.push(Label { x: r.x + r.w * 0.5, y: r.mid_y(), text: note.into(), size: 0.8, color: FAINT, face: Face::Body, center: true });
        }
    }

    /// An on/off switch on a row; true when flipped.
    pub fn toggle(&mut self, s: &mut Screen, r: R, label: &str, v: &mut bool, locked: bool) -> bool {
        let (id, focused) = self.id();
        if self.place(id, r) {
            return false;
        }
        let over = self.over(id, r);
        let flip = !locked && (self.clicked(id, r) || self.pressed(focused) || self.arrows(focused) != 0);
        self.row(s, r, over || focused, label, locked);
        if flip {
            *v = !*v;
            self.acted = true;
        }
        let (tw, th) = (3.4, 1.0);
        let tx = r.x + r.w - tw - 1.0;
        let ty = r.mid_y() - th * 0.5;
        let on = *v;
        s.deco(Deco::pill(tx, ty, tw, th, if on { ACCENT } else { [50, 54, 74] }).border(if on { [255, 220, 160] } else { BUTTON_EDGE }));
        let kx = if on { tx + tw - 1.05 } else { tx + 0.15 };
        s.deco(Deco::pill(kx, ty + 0.12, 0.9, th - 0.24, if on { [255, 248, 236] } else { [150, 155, 175] }));
        s.labels.push(Label { x: tx - 2.2, y: r.mid_y(), text: if on { "On" } else { "Off" }.into(), size: 0.9, color: if locked { FAINT } else { DIM }, face: Face::Body, center: true });
        flip
    }

    /// One of a few values, stepped with ‹ › (or ←→); `values` its names.
    /// Returns the step taken (-1, 1) or 0.
    pub fn choice(&mut self, s: &mut Screen, r: R, label: &str, value: &str, locked: bool) -> i32 {
        let (id, focused) = self.id();
        if self.place(id, r) {
            return 0;
        }
        let over = self.over(id, r);
        let cw = 16.0f32.min(r.w * 0.45);
        let cx = r.x + r.w - cw - 1.0;
        let left = R::new(cx, r.y, 3.0, r.h);
        let mut d = 0;
        if !locked {
            if let Some(c) = self.click.filter(|c| r.contains(*c)) {
                d = if left.contains(c) { -1 } else { 1 };
                self.clicked(id, r);
            }
            if d == 0 {
                d = self.arrows(focused);
            }
            if d == 0 && self.pressed(focused) {
                d = 1;
            }
        }
        self.row(s, r, over || focused, label, locked);
        let fg = if locked { FAINT } else { TEXT };
        let arrow = if locked { FAINT } else if over || focused { ACCENT } else { DIM };
        s.deco(Deco::pill(cx, r.mid_y() - 0.55, cw, 1.1, [30, 33, 46]).border(if over || focused { BUTTON_EDGE } else { PANEL_EDGE }));
        s.labels.push(Label { x: cx + 1.4, y: r.mid_y() - 0.08, text: "‹".into(), size: 2.0, color: arrow, face: Face::Bold, center: true });
        s.labels.push(Label { x: cx + cw - 1.4, y: r.mid_y() - 0.08, text: "›".into(), size: 2.0, color: arrow, face: Face::Bold, center: true });
        s.labels.push(Label { x: cx + cw * 0.5, y: r.mid_y(), text: fit(value, cw - 4.0, 0.92), size: 0.92, color: fg, face: Face::Bold, center: true });
        if d != 0 {
            self.acted = true;
        }
        d
    }

    /// A 0..1 amount (volume): drag or click on the bar, ←→ in steps of `step`.
    pub fn slider(&mut self, s: &mut Screen, r: R, label: &str, v: &mut f32, step: f32, locked: bool) -> bool {
        let (id, focused) = self.id();
        if self.place(id, r) {
            return false;
        }
        let over = self.over(id, r);
        let bw = 16.0f32.min(r.w * 0.45);
        let bx = r.x + r.w - bw - 1.0;
        let bar = R::new(bx, r.y, bw, r.h);
        let mut changed = false;
        if !locked {
            if let Some(c) = self.click.filter(|c| bar.contains(*c)) {
                self.clicked(id, bar);
                *v = (((c.0 - bx) / bw).clamp(0.0, 1.0) / step).round() * step;
                changed = true;
            }
            let d = self.arrows(focused);
            if d != 0 {
                *v = ((*v + d as f32 * step) / step).round() * step;
                changed = true;
            }
            *v = v.clamp(0.0, 1.0);
        }
        self.row(s, r, over || focused, label, locked);
        let y = r.mid_y() - 0.2;
        s.deco(Deco::pill(bx, y, bw, 0.4, [44, 48, 66]));
        s.deco(Deco::pill(bx, y, (bw * *v).max(0.4), 0.4, if locked { FAINT } else { ACCENT }));
        let kx = bx + bw * *v - 0.45;
        s.deco(Deco::pill(kx, r.mid_y() - 0.45, 0.9, 0.9, [255, 248, 236]).border(if over || focused { ACCENT } else { BUTTON_EDGE }));
        s.labels.push(Label { x: bx - 2.6, y: r.mid_y(), text: format!("{:.0}%", *v * 100.0), size: 0.9, color: DIM, face: Face::Body, center: true });
        if changed {
            self.acted = true;
        }
        changed
    }

    /// A list to pick from: shows the chosen one; opens below when pressed.
    /// Returns the newly picked index.
    pub fn dropdown(&mut self, s: &mut Screen, r: R, label: &str, items: &[String], sel: usize, locked: bool) -> Option<usize> {
        let (id, focused) = self.id();
        if self.place(id, r) {
            return None;
        }
        let over = self.over(id, r);
        let bw = 22.0f32.min(r.w * 0.55);
        let bx = r.x + r.w - bw - 1.0;
        let field = R::new(bx, r.mid_y() - 0.6, bw, 1.2);
        let open = self.open.is_some_and(|(o, _)| o == id);
        let mut picked = None;
        if !locked && !open && (self.clicked(id, r) || self.pressed(focused)) {
            self.open = Some((id, sel.min(items.len().saturating_sub(1))));
            self.click = None;
            self.acted = true;
        }
        self.row(s, r, over || focused || open, label, locked);
        s.deco(Deco::pill(bx, field.y, bw, field.h, [30, 33, 46]).border(if open || over || focused { ACCENT } else { PANEL_EDGE }));
        let shown = items.get(sel).map(String::as_str).unwrap_or("");
        s.labels.push(left(bx + 1.0, field.mid_y(), &fit(shown, bw - 3.5, 0.92), 0.92, if locked { FAINT } else { TEXT }, Face::Bold));
        s.labels.push(Label { x: bx + bw - 1.3, y: field.mid_y(), text: "▾".into(), size: 1.7, color: if locked { FAINT } else { DIM }, face: Face::Body, center: true });
        if let Some((_, mut at)) = self.open.filter(|(o, _)| *o == id) {
            // The list, over everything, under the field (or above, near the bottom).
            let ih = 1.6;
            let lh = items.len() as f32 * ih + 0.4;
            let below = field.y + field.h + 0.2;
            let ly = if below + lh > s.h as f32 - 0.5 { (field.y - lh - 0.2).max(0.0) } else { below };
            let list = R::new(bx, ly, bw, lh);
            s.over_decor.push(Deco::panel(list.x, list.y, list.w, list.h, [28, 31, 44]).border(BUTTON_EDGE));
            while self.take(KeyCode::Up) {
                at = at.saturating_sub(1);
            }
            while self.take(KeyCode::Down) || self.take(KeyCode::Tab) {
                at = (at + 1).min(items.len().saturating_sub(1));
            }
            let enter = self.take(KeyCode::Enter) || self.take(KeyCode::Char(' '));
            if self.take(KeyCode::Esc) {
                self.open = None;
                self.acted = true;
                return None;
            }
            for (i, it) in items.iter().enumerate() {
                let ir = R::new(list.x + 0.2, list.y + 0.2 + i as f32 * ih, list.w - 0.4, ih);
                let hover = self.mouse.is_some_and(|m| ir.contains(m));
                if hover {
                    self.hot_now = true;
                    if self.moved {
                        at = i;
                    }
                }
                if i == at {
                    s.over_decor.push(Deco::pill(ir.x, ir.y + 0.1, ir.w, ir.h - 0.2, BUTTON_HOT));
                }
                let color = if i == sel { ACCENT } else { TEXT };
                s.over_labels.push(left(ir.x + 0.8, ir.mid_y(), &fit(it, ir.w - 1.6, 0.92), 0.92, color, Face::Body));
                if self.click.is_some_and(|c| ir.contains(c)) {
                    self.click = None;
                    picked = Some(i);
                }
            }
            if enter {
                picked = Some(at);
            }
            if picked.is_some() {
                self.open = None;
                self.acted = true;
            } else if self.open.is_some() {
                self.open = Some((id, at));
            }
        }
        picked
    }

    /// A line to type in (`secret`: shown as dots). True when it changed.
    pub fn field(&mut self, s: &mut Screen, r: R, label: &str, text: &mut String, placeholder: &str, secret: bool, locked: bool) -> bool {
        let (id, focused) = self.id();
        self.fields.push(id);
        if self.place(id, r) {
            return false;
        }
        let over = self.over(id, r);
        let bw = if label.is_empty() { r.w - 2.0 } else { 26.0f32.min(r.w * 0.6) };
        let bx = r.x + r.w - bw - 1.0;
        let field = R::new(bx, r.mid_y() - 0.65, bw, 1.3);
        if self.clicked(id, r) {
            self.acted = true;
        }
        let focused = focused || self.focus == Some(id);
        let mut changed = false;
        if focused && !locked {
            let mut keep = Vec::new();
            for k in std::mem::take(&mut self.keys) {
                let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
                match k.code {
                    KeyCode::Char('u') if ctrl => {
                        text.clear();
                        changed = true;
                    }
                    KeyCode::Char(c) if !ctrl => {
                        text.push(if k.modifiers.contains(KeyModifiers::SHIFT) { c.to_uppercase().next().unwrap_or(c) } else { c });
                        changed = true;
                    }
                    KeyCode::Backspace => {
                        text.pop();
                        changed = true;
                    }
                    _ => keep.push(k),
                }
            }
            self.keys = keep;
            for p in std::mem::take(&mut self.pasted) {
                text.push_str(p.trim());
                changed = true;
            }
        }
        if !label.is_empty() {
            self.row(s, r, over || focused, label, locked);
        }
        s.deco(Deco::panel(field.x, field.y, field.w, field.h, [18, 20, 28]).border(if focused { ACCENT } else if over { BUTTON_EDGE } else { PANEL_EDGE }));
        if text.is_empty() {
            // The hint stays until something is typed; the caret before it.
            let blink = (self.t * 2.0) as i64 % 2 == 0;
            s.labels.push(left(field.x + 0.8, field.mid_y(), &fit(placeholder, bw - 1.6, 0.92), 0.92, FAINT, Face::Body));
            if focused && !locked && blink {
                s.labels.push(left(field.x + 0.55, field.mid_y(), "▏", 0.92, ACCENT, Face::Body));
            }
        } else {
            let shown: String = if secret { secret_view(text) } else { text.clone() };
            // The end of it, as much as fits, then the caret.
            let room = (bw - 2.0).max(2.0) as usize;
            let n = shown.chars().count();
            let tail: String = shown.chars().skip(n.saturating_sub(room)).collect();
            let blink = (self.t * 2.0) as i64 % 2 == 0;
            let caret = if focused && blink { "▏" } else { "" };
            s.labels.push(left(field.x + 0.8, field.mid_y(), &format!("{tail}{caret}"), 0.92, if locked { FAINT } else { TEXT }, Face::Body));
        }
        if changed {
            self.acted = true;
        }
        changed
    }

    /// Tabs across the top: returns the newly chosen one.
    pub fn tabs(&mut self, s: &mut Screen, x: f32, y: f32, names: &[&str], cur: usize) -> Option<usize> {
        let mut px = x;
        let mut out = None;
        for (i, name) in names.iter().enumerate() {
            let w = name.chars().count() as f32 * 0.95 + 3.0;
            let r = R::new(px, y, w, 1.6);
            let (id, focused) = self.id();
            let over = self.over(id, r);
            if self.clicked(id, r) || self.pressed(focused) {
                out = Some(i);
                self.acted = true;
            }
            let on = i == cur;
            let fill = if on { ACCENT } else if over || focused { BUTTON_HOT } else { BUTTON };
            let d = Deco::pill(r.x, r.y, r.w, r.h, fill);
            s.deco(if on { d } else { d.border(if over || focused { ACCENT } else { BUTTON_EDGE }) });
            s.labels.push(Label { x: r.x + r.w * 0.5, y: r.mid_y(), text: name.to_string(), size: 0.92, color: if on { ON_ACCENT } else { TEXT }, face: Face::Bold, center: true });
            px += w + 0.8;
        }
        // ←→ anywhere on the page step through the tabs, unless a control takes them.
        out
    }
}

/// Words from `x`, their line centred on `mid` (labels are otherwise set from their top).
pub fn left(x: f32, mid: f32, text: &str, size: f32, color: [u8; 3], face: Face) -> Label {
    Label { x, y: mid - 0.52 * size, text: text.to_string(), size, color, face, center: false }
}

/// Shorten `text` to about `w` cells at `size` (words are about half a row wide each letter).
pub fn fit(text: &str, w: f32, size: f32) -> String {
    let n = ((w / (size * 0.85)).max(1.0)) as usize;
    if text.chars().count() <= n { text.to_string() } else { format!("{}…", text.chars().take(n.saturating_sub(1)).collect::<String>()) }
}

/// A key shown as dots, with its last four letters (to know which it is).
pub fn secret_view(t: &str) -> String {
    let n = t.chars().count();
    if n <= 8 {
        return "•".repeat(n);
    }
    let tail: String = t.chars().skip(n - 4).collect();
    format!("{}{tail}", "•".repeat(8))
}

/// A line of quiet words, centred.
pub fn note(s: &mut Screen, cx: f32, y: f32, text: &str, color: [u8; 3]) {
    s.labels.push(Label { x: cx, y, text: text.to_string(), size: 0.92, color, face: Face::Body, center: true });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> Ev {
        Ev::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn keys_move_between_buttons_and_enter_presses() {
        let mut ui = Ui::new();
        let mut s = Screen::new(80, 30, true);
        let frame = |ui: &mut Ui, s: &mut Screen| {
            ui.begin();
            let a = ui.button(s, R::new(10.0, 5.0, 20.0, 2.0), "One", Kind::Primary);
            let b = ui.button(s, R::new(10.0, 8.0, 20.0, 2.0), "Two", Kind::Normal);
            ui.end();
            (a, b)
        };
        assert_eq!(frame(&mut ui, &mut s), (false, false));
        ui.feed(&key(KeyCode::Down));
        frame(&mut ui, &mut s);
        ui.feed(&key(KeyCode::Down));
        frame(&mut ui, &mut s);
        ui.feed(&key(KeyCode::Enter));
        assert_eq!(frame(&mut ui, &mut s), (false, true), "the second, after two downs");
        ui.feed(&Ev::Click(12.0, 5.5));
        assert_eq!(frame(&mut ui, &mut s), (true, false), "clicked");
    }

    #[test]
    fn a_dropdown_opens_and_picks() {
        let mut ui = Ui::new();
        let mut s = Screen::new(80, 30, true);
        let items: Vec<String> = ["a", "b", "c"].iter().map(|x| x.to_string()).collect();
        let mut frame = |ui: &mut Ui, s: &mut Screen| {
            ui.begin();
            let p = ui.dropdown(s, R::new(0.0, 2.0, 60.0, 2.0), "Pick", &items, 0, false);
            ui.end();
            p
        };
        ui.focus = Some(0);
        ui.feed(&key(KeyCode::Enter));
        assert_eq!(frame(&mut ui, &mut s), None);
        assert!(!s.over_decor.is_empty(), "open over the rest");
        ui.feed(&key(KeyCode::Down));
        ui.feed(&key(KeyCode::Down));
        ui.feed(&key(KeyCode::Enter));
        s.clear_decor();
        assert_eq!(frame(&mut ui, &mut s), Some(2));
    }

    #[test]
    fn a_field_takes_typing_only_when_focused() {
        let mut ui = Ui::new();
        let mut s = Screen::new(80, 30, true);
        let mut t = String::new();
        ui.feed(&key(KeyCode::Char('x')));
        ui.begin();
        ui.field(&mut s, R::new(0.0, 2.0, 60.0, 2.0), "Name", &mut t, "", false, false);
        ui.end();
        assert!(t.is_empty());
        ui.focus = Some(0);
        ui.feed(&key(KeyCode::Char('h')));
        ui.feed(&key(KeyCode::Char('i')));
        ui.begin();
        ui.field(&mut s, R::new(0.0, 2.0, 60.0, 2.0), "Name", &mut t, "", false, false);
        ui.end();
        assert_eq!(t, "hi");
    }
}
