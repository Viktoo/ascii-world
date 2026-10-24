//! What floats over the world while you play: the whole window is the view,
//! and over it only a few small things, each where the eye already is.
//!
//! - Top left, where you are (place, time, weather); under it, work in
//!   progress. Top right, your health, and any notice.
//! - Under the crosshair, the name of what you look at and the one or two
//!   keys that act on it.
//! - Bottom middle, the line to do or make anything (`/`), which is also
//!   where you talk; above it, what you are doing now (driving, holding a
//!   lit firework…) and, at first, how to do it.
//! - Bottom left, the last few things said and done, fading away. The
//!   journal (key 1) keeps them all.
//!
//! Key tips that teach (how to throw what you hold, how to steer) show the
//! first few times only; the count is kept in the settings.

use super::{ACCENT, App, DIM, Mode, TEXT, speaker_color};
use crate::sim::{ActorId, Target};
use crate::term::{Deco, Face, KEYCAP, KEYCAP_EDGE, KEYCAP_TEXT, Label, Screen};
use crate::world::region_of;
use crate::render::sky;
use std::time::Instant;

/// The shade behind words over the world.
pub(super) const SHADE: [u8; 3] = [12, 13, 20];
pub(super) const SHADE_A: f32 = 0.62;
/// Words in the HUD, in rows.
pub(super) const SIZE: f32 = 0.95;
/// How tall a pill of words is, in rows.
const PILL_H: f32 = 1.9;
/// From the window's edges, in rows (columns are half as wide: twice as many).
const EDGE: f32 = 0.8;
/// A tip that teaches is shown this many times, then no more.
const RETIRE: u32 = 3;
/// How long a state's tips show after it begins (seconds).
const STATE_TIPS_FOR: f32 = 8.0;

/// How wide words are, in cells.
pub(super) fn width(text: &str, face: Face, size: f32) -> f32 {
    crate::window::text::measure(text, face, size)
}

/// Words from `x` with their line's middle on `mid`; the x after them.
pub(super) fn words(s: &mut Screen, x: f32, mid: f32, text: &str, size: f32, color: [u8; 3], face: Face) -> f32 {
    s.labels.push(crate::ui::left(x, mid, text, size, color, face));
    x + width(text, face, size)
}

/// Wrap words to `w` cells set at `size` (the first line `first` wide).
pub(super) fn wrap(text: &str, first: f32, w: f32, face: Face, size: f32) -> Vec<String> {
    let space = width(" ", face, size);
    let mut lines = vec![String::new()];
    let mut used = 0.0;
    for word in text.split_whitespace() {
        let ww = width(word, face, size);
        let room = if lines.len() == 1 { first } else { w };
        if !lines.last().unwrap().is_empty() && used + space + ww > room {
            lines.push(String::new());
            used = 0.0;
        }
        let line = lines.last_mut().unwrap();
        if !line.is_empty() {
            line.push(' ');
            used += space;
        }
        line.push_str(word);
        used += ww;
    }
    lines
}

/// Shorten words to `w` cells, ending in "…".
pub(super) fn fit(text: &str, w: f32, face: Face, size: f32) -> String {
    if width(text, face, size) <= w {
        return text.to_string();
    }
    let mut out: String = text.to_string();
    while !out.is_empty() && width(&format!("{out}…"), face, size) > w {
        out.pop();
    }
    format!("{}…", out.trim_end())
}

/// A soft dark shape for words over the world.
pub(super) fn shade(s: &mut Screen, x: f32, y: f32, w: f32, h: f32, radius: f32) {
    s.deco(Deco { x, y, w, h, fill: SHADE, alpha: SHADE_A, radius, shadow: false, border: None });
}

/// How wide the input line is in a window `w` cells wide.
pub(super) fn input_w(w: f32) -> f32 {
    (w - 8.0).min(64.0)
}

/// One key: a keycap with what it does after it. The width.
fn chip_w(key: &str, what: &str) -> f32 {
    cap_w(key) + 0.6 + width(what, Face::Body, SIZE)
}

fn cap_w(key: &str) -> f32 {
    (width(key, Face::Bold, 0.74) + 1.0).max(1.7)
}

/// A keycap at `x`, centred on `mid`; the x after it.
pub(super) fn keycap(s: &mut Screen, x: f32, mid: f32, key: &str) -> f32 {
    let w = cap_w(key);
    s.deco(Deco { x, y: mid - 0.55, w, h: 1.1, fill: KEYCAP, alpha: 1.0, radius: 0.22, shadow: false, border: Some(KEYCAP_EDGE) });
    s.labels.push(Label { x: x + w * 0.5, y: mid, text: key.to_string(), size: 0.74, color: KEYCAP_TEXT, face: Face::Bold, center: true });
    x + w
}

/// Keys in a row of pills, centred on `cx`, their middle on `mid`.
fn chips(s: &mut Screen, cx: f32, mid: f32, keys: &[(String, String)]) {
    const PAD: f32 = 0.9;
    const GAP: f32 = 1.0;
    let total: f32 = keys.iter().map(|(k, w)| chip_w(k, w) + 2.0 * PAD).sum::<f32>() + GAP * keys.len().saturating_sub(1) as f32;
    let mut x = cx - total * 0.5;
    for (k, what) in keys {
        let w = chip_w(k, what) + 2.0 * PAD;
        shade(s, x, mid - PILL_H * 0.5, w, PILL_H, PILL_H * 0.5);
        let after = keycap(s, x + PAD, mid, k);
        words(s, after + 0.6, mid, what, SIZE, TEXT, Face::Body);
        x += w + GAP;
    }
}

/// A key and what it does.
fn k(key: &str, what: impl Into<String>) -> (String, String) {
    (key.to_string(), what.into())
}

/// What the keys can do now, worked out from what you look at, hold, ride.
#[derive(Default)]
pub(super) struct Prompt {
    /// The name of what is in view, under the crosshair.
    pub name: Option<String>,
    /// Keys for it, under its name.
    pub keys: Vec<(String, String)>,
    /// What you are doing (over the input line): its words, its keys,
    /// which kind of state it is (for its tips), and whether its words
    /// always show (a speed, a lit fuse) or only with its tips.
    pub state: Option<State>,
}

pub(super) struct State {
    pub kind: &'static str,
    pub text: String,
    pub keys: Vec<(String, String)>,
    pub always: bool,
    /// Its keys always show too (a lit fuse: throw it now).
    pub urgent: bool,
}

impl App {
    /// May this tip show: tips are on and it hasn't been shown enough.
    pub(super) fn tip_ok(&self, name: &str) -> bool {
        self.settings.show_hints && self.settings.tips.get(name).copied().unwrap_or(0) < RETIRE
    }

    /// A tip was shown once more.
    pub(super) fn tip_shown(&mut self, name: &str) {
        if !self.settings.show_hints {
            return;
        }
        *self.settings.tips.entry(name.to_string()).or_default() += 1;
        self.settings.save();
    }

    pub(super) fn prompt(&self) -> Prompt {
        let me = ActorId::Player;
        let mut p = Prompt::default();
        if self.sim.fallen() || self.mode != Mode::Walk {
            return p;
        }
        let near = self.pointed.as_ref().filter(|p| p.dist < 3.2 && !matches!(p.target, Target::Point(_)));
        let seen = self.pointed.as_ref().filter(|p| p.dist < 12.0 && !matches!(p.target, Target::Point(_)));
        let held = self.sim.player.held;
        let thing = |h| self.sim.thing_name(h);
        // What you are doing.
        if let Some(v) = self.sim.player.aboard {
            let kmh = self.sim.vehicle_speed(me).unwrap_or(0.0).abs() * 3.6;
            let mut keys = vec![k("W/S", "speed and brake"), k("A/D", "steer")];
            if matches!(self.sim.vehicle_on(me).as_deref(), Some("under" | "air")) {
                keys.push(k("Space/C", "up and down"));
            }
            keys.push(k("E", "get out"));
            p.state = Some(State { kind: "drive", text: format!("The {} · {kmh:.0} km/h", thing(v)), keys, always: true, urgent: false });
        } else if let Some(m) = self.sim.player.riding {
            p.state = Some(State { kind: "ride", text: format!("Riding {}", self.sim.actor_name(m)), keys: vec![k("W/A/S/D", "go"), k("E", "get down")], always: false, urgent: false });
        } else if let Some(c) = self.sim.carried_of(me) {
            p.state = Some(State { kind: "carry", text: format!("Carrying {}", self.sim.actor_name(c)), keys: vec![k("E", "pet"), k("G", "put down")], always: false, urgent: false });
        } else if let Some(h) = held {
            if self.sim.burst_of(h).is_some() {
                p.state = Some(match self.sim.things.get(h).and_then(|t| t.shape.fuse_at) {
                    Some(at) => State { kind: "lit", text: format!("The {} is lit · {:.0} s", thing(h), (at - self.sim.t).max(0.0)), keys: vec![k("F", "throw it!")], always: true, urgent: true },
                    None => State { kind: "burst", text: format!("The {}", thing(h)), keys: vec![k("E", "light"), k("F", "throw"), k("G", "put down")], always: false, urgent: false },
                });
            } else if self.sim.launch_of(h).is_some() {
                let left = self.sim.shots_left(h).map(|n| format!(" · {n} left")).unwrap_or_default();
                p.state = Some(State { kind: "launch", text: format!("The {}{left}", thing(h)), keys: vec![k("E", "shoot"), k("F", "throw"), k("G", "put down")], always: !left.is_empty(), urgent: false });
            } else {
                p.state = Some(State { kind: "hold", text: format!("The {}", thing(h)), keys: vec![k("E", "use"), k("F", "throw"), k("G", "put down")], always: false, urgent: false });
            }
        }
        // What you look at, or who is in front of you.
        if p.state.as_ref().is_some_and(|s| matches!(s.kind, "drive" | "ride" | "carry")) {
            return p;
        }
        if let Some((id, n)) = &self.talk_hint {
            let other = self.being_in_front().unwrap_or(ActorId::Npc(*id));
            // Their name is over their head: the keys say who.
            p.keys.push(k("Enter", format!("{} {n}", if self.sim.speaks(ActorId::Npc(*id)) { "talk to" } else { "call" })));
            p.keys.push(k("E", self.sim.approach_words(me, other)));
            if self.sim.mind_of(ActorId::Npc(*id)) != crate::world::species::Mind::Sapient && held.is_none() {
                p.keys.push(k("G", "pick up"));
            }
            return p;
        }
        let Some(at) = seen else { return p };
        p.name = Some(at.name.clone());
        let Some(at) = near else { return p };
        let t = &at.target;
        p.keys = match held {
            Some(h) if self.sim.launch_of(h).is_some() => {
                let ammo = self.sim.liven_peek_pub(t).is_some_and(|id| self.sim.launch_of(h).as_ref().is_some_and(|l| self.sim.is_ammo_for(l, id)));
                vec![k("E", if ammo { format!("load the {}", thing(h)) } else { format!("shoot the {}", thing(h)) })]
            }
            Some(h) if self.sim.burst_of(h).is_some() => vec![],
            Some(h) => vec![k("E", format!("use the {} on it", thing(h)))],
            None if matches!(t, Target::Actor(ActorId::Traveler(_))) => vec![k("/", "say hello to everyone")],
            None if self.sim.target_drive(t).is_some() => vec![k("E", "get in")],
            None if self.sim.target_seats(t) => vec![k("E", "sit")],
            None if self.pointed_loose => vec![k("E", "pick up")],
            None => vec![k("E", "use")],
        };
        // Something with a use of its own can still be taken up with g.
        if held.is_none() && !self.pointed_loose {
            if let Some(alone) = self.pointed_grab {
                p.keys.push(k("G", if alone { "pick up" } else { "grab one end" }));
            }
        }
        p
    }

    /// The HUD, over the view `w` × `h` cells.
    pub(super) fn draw_hud(&mut self, w: u16, h: u16) {
        let (w, h) = (w as f32, h as f32);
        self.draw_place(w);
        self.draw_status(w);
        let prompt = self.prompt();
        self.draw_prompt(w, h, &prompt);
        self.draw_state(w, h, prompt.state);
        self.draw_input(w, h);
        self.draw_feed(w, h);
    }

    /// Top left: where you are.
    fn draw_place(&mut self, _w: f32) {
        let me = self.pos();
        let place = self.snap.region_name(region_of(me.x, me.z)).map(str::to_string).unwrap_or_else(|| {
            let b = self.snap.terrain.biome_at(me.x, me.z);
            super::menu::cap(&self.snap.terrain.biomes[b].name)
        });
        let mut rest = vec![sky::time_label(self.sim.t).to_string(), self.sim.wx.name.clone()];
        rest.extend(self.sim.night_status());
        let rest = format!(" · {}", rest.join(" · "));
        let pw = width(&place, Face::Bold, SIZE) + width(&rest, Face::Body, SIZE) + 2.0;
        let (x, y) = (EDGE * 2.0, EDGE);
        let s = &mut self.screen;
        shade(s, x, y, pw, PILL_H, PILL_H * 0.5);
        let mid = y + PILL_H * 0.5;
        let nx = words(s, x + 1.0, mid, &place, SIZE, TEXT, Face::Bold);
        words(s, nx, mid, &rest, SIZE, [176, 182, 200], Face::Body);
    }

    /// Top right: health, and any notice to its left.
    fn draw_status(&mut self, w: f32) {
        const BAR: f32 = 9.0;
        const TRACK: [u8; 3] = [58, 60, 76];
        let health = self.sim.health();
        let flash = self.hurt_flash.map(|t| t.elapsed().as_secs_f32()).filter(|s| *s < 0.6);
        let tint = super::health_color(health);
        let heart = match flash {
            Some(s) if (s * 10.0) as u32 % 2 == 0 => [255, 255, 255],
            _ => tint,
        };
        let pw = BAR + 4.4;
        let (x, y) = (w - EDGE * 2.0 - pw, EDGE);
        let mid = y + PILL_H * 0.5;
        // A notice: the LLM's trouble, playing together, the budget.
        let mut note = self.llm_note.clone();
        if let Some(t) = self.together_line() {
            note = Some(t);
        }
        if let Some(b) = self.settings.budget().filter(|_| self.brain.has_llm) {
            let spent = self.spent();
            if spent >= b * 0.8 {
                note = Some(format!("${spent:.2} of ${b:.2} spent"));
            }
        }
        if self.budget_paused {
            note = Some("Budget reached: making and talking paused (Esc to change)".into());
        }
        if self.render.cpu_fallback {
            note = Some(match note {
                Some(n) => format!("{n} · CPU renderer"),
                None => "CPU renderer".into(),
            });
        }
        let s = &mut self.screen;
        shade(s, x, y, pw, PILL_H, PILL_H * 0.5);
        s.labels.push(Label { x: x + 1.7, y: mid, text: "♥".into(), size: 1.25, color: heart, face: Face::Bold, center: true });
        let bx = x + 3.0;
        s.deco(Deco::pill(bx, mid - 0.3, BAR, 0.6, TRACK));
        if health > 0.01 {
            s.deco(Deco::pill(bx, mid - 0.3, (BAR * health).max(0.6), 0.6, tint));
        }
        if let Some(n) = note {
            let n = fit(&n, (w * 0.5).max(20.0), Face::Body, SIZE);
            let nw = width(&n, Face::Body, SIZE) + 2.0;
            let nx = x - 1.0 - nw;
            shade(s, nx, y, nw, PILL_H, PILL_H * 0.5);
            words(s, nx + 1.0, mid, &n, SIZE, ACCENT, Face::Body);
        }
    }

    /// Under the crosshair: what it rests on, and the keys for it.
    fn draw_prompt(&mut self, w: f32, h: f32, p: &Prompt) {
        let cx = (w / 2.0).floor() + 0.5;
        let cy = (h / 2.0).floor();
        let mut ky = cy + 2.0;
        if let Some(name) = &p.name {
            let s = &mut self.screen;
            let name = fit(name, w * 0.6, Face::Bold, SIZE);
            let nw = width(&name, Face::Bold, SIZE) + 1.6;
            shade(s, cx - nw * 0.5, ky - 0.75, nw, 1.5, 0.75);
            s.labels.push(Label { x: cx, y: ky, text: name, size: SIZE, color: [255, 246, 214], face: Face::Bold, center: true });
            ky += 2.3;
        }
        if self.settings.show_hints && !p.keys.is_empty() {
            chips(&mut self.screen, cx, ky, &p.keys);
        }
    }

    /// Over the input line: what you are doing and, at first, its keys; at
    /// the very start, how to move.
    fn draw_state(&mut self, w: f32, h: f32, state: Option<State>) {
        let mid = h - 5.3;
        let cx = w * 0.5;
        // The kind of state now, and since when (its tips count once a time).
        let kind = state.as_ref().map(|s| s.kind);
        if self.state_since.as_ref().map(|s| s.0) != kind {
            self.state_since = kind.map(|k| (k, Instant::now()));
            if let Some(k) = kind {
                if self.tip_ok(k) {
                    self.tip_shown(k);
                    self.state_tips = true;
                } else {
                    self.state_tips = false;
                }
            }
        }
        let fresh = self.state_since.as_ref().is_some_and(|s| s.1.elapsed().as_secs_f32() < STATE_TIPS_FOR);
        if fresh {
            self.dirty = true;
        }
        let Some(st) = state else {
            // Nothing going on: at the start, how to get about.
            if !self.moved && self.tip_ok("move") && !self.sim.fallen() && self.mode == Mode::Walk {
                chips(&mut self.screen, cx, mid, &[k("W/A/S/D", "move"), k("Mouse", "look around"), k("Esc", "menu")]);
            }
            return;
        };
        let keys = st.urgent || (self.settings.show_hints && self.state_tips && fresh);
        if !st.always && !keys {
            return;
        }
        let tw = width(&st.text, Face::Bold, SIZE) + 2.0;
        let mut row: Vec<(String, String)> = Vec::new();
        if keys {
            row = st.keys;
        }
        let total = tw + row.iter().map(|(k, w)| chip_w(k, w) + 1.8 + 1.0).sum::<f32>();
        let x = cx - total * 0.5;
        let color = if st.urgent { [255, 150, 110] } else { TEXT };
        let s = &mut self.screen;
        shade(s, x, mid - PILL_H * 0.5, tw, PILL_H, PILL_H * 0.5);
        words(s, x + 1.0, mid, &st.text, SIZE, color, Face::Bold);
        if !row.is_empty() {
            let rest: f32 = row.iter().map(|(k, w)| chip_w(k, w) + 1.8 + 1.0).sum();
            chips(&mut self.screen, x + tw + 1.0 + (rest - 1.0) * 0.5, mid, &row);
        }
    }

    /// The input pill (bottom middle): do or make anything, or talk.
    fn draw_input(&mut self, w: f32, h: f32) {
        if self.sim.fallen() {
            return;
        }
        let pw = input_w(w);
        let iy = (h - 2.0).floor();
        let (x, y, ph) = ((w - pw) * 0.5, iy - 0.6, 2.2);
        let mid = iy + 0.5;
        let active = self.mode != Mode::Walk;
        let s = &mut self.screen;
        s.deco(Deco { x, y, w: pw, h: ph, fill: SHADE, alpha: if active { 0.85 } else { SHADE_A }, radius: ph * 0.5, shadow: false, border: active.then_some(ACCENT) });
        let blink = (self.start.elapsed().as_millis() / 500) % 2 == 0;
        let mut tx = x + 1.4;
        match self.mode {
            Mode::Walk => {
                tx = words(s, tx, mid, "✦", 1.0, ACCENT, Face::Bold) + 0.8;
                words(s, tx, mid, "Do or make anything…", SIZE, [150, 155, 172], Face::Body);
                let kx = x + pw - 1.2 - cap_w("/");
                keycap(s, kx, mid, "/");
                return;
            }
            Mode::Command => tx = words(s, tx, mid, "✦", 1.0, ACCENT, Face::Bold) + 0.8,
            Mode::Talk(id) => {
                let name = self.sim.cast.get(id).map(|n| n.name().to_string()).unwrap_or_default();
                tx = words(s, tx, mid, &format!("{name} ›"), SIZE, speaker_color(&name), Face::Bold) + 0.8;
            }
        }
        // The words typed, and a mark at the right (heard, listening).
        let mark = self.hearing_mark().or_else(|| self.replying.is_some().then(|| ("…".to_string(), DIM)));
        let mark_w = mark.as_ref().map_or(0.0, |(m, _)| width(m, Face::Body, SIZE) + 1.0);
        let text = match self.mode {
            Mode::Command => self.input.strip_prefix('/').unwrap_or(&self.input).to_string(),
            _ => self.input.clone(),
        };
        let fx = tx.ceil();
        let room = (x + pw - 1.4 - mark_w - fx).max(4.0);
        let s = &mut self.screen;
        if text.is_empty() {
            let hint = match self.mode {
                Mode::Command => "a lantern on that post, dig a hole here… then Enter",
                _ => "Say something · Esc to leave",
            };
            words(s, fx + 0.3, mid, &fit(hint, room - 0.3, Face::Body, SIZE), SIZE, [110, 114, 132], Face::Body);
        }
        s.field = Some(crate::term::Field { x: fx as u16, y: iy as u16, w: room as u16, text, color: TEXT, caret: blink.then_some(ACCENT) });
        if let Some((m, fg)) = mark {
            words(s, x + pw - 1.2 - width(&m, Face::Body, SIZE), mid, &m, SIZE, fg, Face::Body);
        }
    }
}
