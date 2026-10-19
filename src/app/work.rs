//! Work in progress, top left under where you are (clear of what the
//! traveler holds, on the right): slow work
//! (a deed the world is deciding, a thing being made or reshaped, a new
//! gesture), from when it starts until it is done, so there is always a sign
//! that something is happening. Its kinds are the sim's `WorkKind`s; the land
//! being made is the one kind of its own here. Settings choose whose work.
//! Work with a place says how far and which way it is, and in the view a
//! conjuring shows in the world as a violet sigil where the thing will
//! appear (see `sim::render`), and from far off as a mark to find it by.

use super::{App, DIM, TEXT, hud};
use crate::settings::ShowWork;
use crate::sim::ActorId;
use crate::sim::interp::WorkKind;
use crate::term::Cell;
use glam::Vec3;
use std::time::Instant;

/// How long a finished line stays (seconds).
const DONE_FOR: f32 = 3.0;
const MAX_ROWS: usize = 5;
/// Lines one entry may wrap to before it is cut.
const MAX_LINES: usize = 4;
const DONE: [u8; 3] = [130, 200, 140];
/// The land's own row (no request id).
const LAND: u64 = u64::MAX;
const SPARKLE: [u8; 3] = [205, 165, 255];
/// How far off the traveler's own conjuring is still marked (nearer, the
/// sigil in the world shows it; anyone else's only shows there).
const SPARKLE_MINE: f32 = 150.0;
const SIGIL_SEEN: f32 = 30.0;
/// How near a thing being changed shows a little dust.
const DUST_NEAR: f32 = 25.0;

struct Row {
    id: u64,
    verb: &'static str,
    color: [u8; 3],
    /// Someone else's work: whose.
    who: Option<String>,
    what: String,
    /// Where it is happening, or where it will appear.
    at: Option<Vec3>,
    /// Conjuring (sparkles) rather than a change (a little dust).
    conjure: bool,
    mine: bool,
    since: Instant,
    done: Option<Instant>,
}

#[derive(Default)]
pub struct WorkBox {
    rows: Vec<Row>,
    /// The elapsed-time label last drawn, to redraw once a second.
    drawn_secs: u64,
}

fn kind_color(k: WorkKind) -> [u8; 3] {
    match k {
        WorkKind::Doing => [150, 200, 255],
        WorkKind::Conjuring => [205, 165, 255],
        WorkKind::Making | WorkKind::Reshaping => [255, 200, 120],
        WorkKind::Learning => [160, 230, 170],
    }
}

fn elapsed(secs: u64) -> String {
    if secs < 100 { format!("{secs}s") } else { format!("{}m", secs / 60) }
}

impl App {
    /// Follow the sim's work in progress: new work gets a row, finished work
    /// shows as done for a moment.
    pub(super) fn update_work(&mut self) {
        let show = self.settings.show_work;
        if show == ShowWork::Off {
            if !self.work.rows.is_empty() {
                self.work.rows.clear();
                self.dirty = true;
            }
            return;
        }
        let mut now: Vec<(u64, &'static str, [u8; 3], Option<String>, String, Option<Vec3>, bool)> = Vec::new();
        for w in self.sim.work() {
            let mine = w.who == Some(ActorId::Player);
            if !mine && show == ShowWork::Mine {
                continue;
            }
            let who = (!mine).then(|| w.who.map(|a| self.sim.actor_name(a)).unwrap_or_else(|| "the world".into()));
            let at = w.at.filter(|_| w.kind != WorkKind::Learning);
            now.push((w.id, w.kind.verb(), kind_color(w.kind), who, w.what, at, w.appears));
        }
        if show == ShowWork::Everyone && !self.regions_inflight.is_empty() {
            let n = self.regions_inflight.len();
            now.push((LAND, "shaping", [150, 190, 150], None, format!("the land ahead ({n} region{})", if n == 1 { "" } else { "s" }), None, false));
        }
        let t = Instant::now();
        let rows = &mut self.work.rows;
        let mut changed = false;
        // Who starts something new right now: a deed going on to a making
        // isn't done yet, it just changes row.
        let starting: Vec<Option<String>> = now.iter().filter(|n| n.0 != LAND && !rows.iter().any(|r| r.id == n.0)).map(|n| n.3.clone()).collect();
        rows.retain_mut(|r| {
            if r.done.is_some() || now.iter().any(|n| n.0 == r.id) {
                return true;
            }
            changed = true;
            if starting.contains(&r.who) {
                return false;
            }
            r.done = Some(t);
            true
        });
        for (id, verb, color, who, what, at, conjure) in now {
            match rows.iter_mut().find(|r| r.id == id) {
                Some(r) => {
                    if r.what != what {
                        r.what = what;
                        changed = true;
                    }
                    r.at = at;
                    r.conjure = conjure;
                }
                None => {
                    let mine = who.is_none() && id != LAND;
                    rows.push(Row { id, verb, color, who, what, at, conjure, mine, since: t, done: None });
                    changed = true;
                }
            }
        }
        let before = rows.len();
        rows.retain(|r| r.done.is_none_or(|d| d.elapsed().as_secs_f32() < DONE_FOR));
        changed |= rows.len() != before;
        let secs = self.start.elapsed().as_secs();
        if changed || (!rows.is_empty() && secs != self.work.drawn_secs) {
            self.work.drawn_secs = secs;
            self.dirty = true;
        }
    }

    /// The work, top left under where you are: a small card each, newest
    /// at the bottom, each written out in full, wrapped.
    pub(super) fn draw_work(&mut self, w: u16, vh: u16) {
        if self.work.rows.is_empty() {
            return;
        }
        use crate::term::Face;
        const SIZE: f32 = 0.9;
        const LINE: f32 = 1.2;
        let (me, yaw) = (self.pos(), self.yaw());
        let cw = (w as f32 * 0.35).clamp(30.0, 44.0).min(w as f32 - 4.0);
        let (x, mut y) = (1.6, 3.4);
        let budget = vh as f32 * 0.45;
        let text_x = 1.0 + ["doing", "conjuring", "making", "reshaping", "learning", "shaping"].iter().map(|v| hud::width(v, Face::Bold, SIZE)).fold(0.0, f32::max) + 1.0;
        let entries: Vec<(String, [u8; 3], String, String, bool)> = self
            .work
            .rows
            .iter()
            .map(|r| {
                let what = match &r.who {
                    Some(who) => format!("{who}: {}", r.what),
                    None => r.what.clone(),
                };
                match r.done {
                    Some(_) => ("✓ done".to_string(), DONE, what, String::new(), true),
                    None => {
                        let mut time = elapsed(r.since.elapsed().as_secs());
                        if let Some(at) = r.at {
                            time = format!("{} {time}", where_from(me, yaw, at));
                        }
                        (r.verb.to_string(), r.color, what, time, false)
                    }
                }
            })
            .collect();
        // Newest last; older ones give way when there is no room.
        let laid: Vec<(Vec<String>, f32)> = entries
            .iter()
            .map(|(_, _, what, time, _)| {
                let tw = if time.is_empty() { 0.0 } else { hud::width(time, Face::Body, SIZE) + 1.0 };
                let room = cw - text_x - 1.0;
                let mut lines = hud::wrap(what, room - tw, room, Face::Body, SIZE);
                if lines.len() > MAX_LINES {
                    lines.truncate(MAX_LINES);
                    let last = lines[MAX_LINES - 1].clone();
                    lines[MAX_LINES - 1] = hud::fit(&format!("{last}…"), room - 1.0, Face::Body, SIZE);
                }
                let h = lines.len() as f32 * LINE + 0.6;
                (lines, h)
            })
            .collect();
        let mut first = entries.len();
        let mut used = 0.0;
        while first > 0 && entries.len() - first < MAX_ROWS && used + laid[first - 1].1 + 0.4 <= budget {
            first -= 1;
            used += laid[first].1 + 0.4;
        }
        let s = &mut self.screen;
        if first > 0 {
            hud::words(s, x + 1.0, y + 0.5, &format!("+{first} earlier"), 0.8, DIM, Face::Body);
            y += 1.2;
        }
        for (i, (head, color, _, time, done)) in entries.into_iter().enumerate().skip(first) {
            let (lines, h) = &laid[i];
            hud::shade(s, x, y, cw, *h, 0.55);
            let mid = y + 0.3 + LINE * 0.5;
            hud::words(s, x + 1.0, mid, &head, SIZE, color, if done { Face::Body } else { Face::Bold });
            if !time.is_empty() {
                hud::words(s, x + cw - 1.0 - hud::width(&time, Face::Body, SIZE), mid, &time, SIZE, DIM, Face::Body);
            }
            for (j, line) in lines.iter().enumerate() {
                hud::words(s, x + text_x, mid + j as f32 * LINE, line, SIZE, if done { DIM } else { TEXT }, Face::Body);
            }
            y += h + 0.4;
        }
    }
}

/// How far and which way a spot is from someone facing `yaw`: "24m ↖".
fn where_from(me: Vec3, yaw: f32, at: Vec3) -> String {
    let d = Vec3::new(at.x - me.x, 0.0, at.z - me.z);
    let dist = d.length();
    if dist < 2.0 {
        return "here".into();
    }
    let (f, r) = (Vec3::new(yaw.sin(), 0.0, yaw.cos()), Vec3::new(yaw.cos(), 0.0, -yaw.sin()));
    // Clockwise from straight ahead, in eighths.
    let a = d.dot(r).atan2(d.dot(f));
    const ARROWS: [char; 8] = ['↑', '↗', '→', '↘', '↓', '↙', '←', '↖'];
    let arrow = ARROWS[((a / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8) as usize];
    let dist = if dist < 1000.0 { format!("{}m", dist.round() as i32) } else { format!("{:.1}km", dist / 1000.0) };
    format!("{dist} {arrow}")
}

impl App {
    /// In the view: the traveler's own conjuring, when it is too far off to
    /// see its sigil, as one small violet mark breathing over the spot (seen
    /// through walls, so it can be found again); and a little dust where
    /// something near is being changed.
    pub(super) fn draw_work_marks(&mut self, w: u16, vh: u16) {
        if self.menu.is_some() || self.log_view == 2 {
            return;
        }
        let cam = self.camera();
        let (f, r, u) = cam.basis();
        let tan = (cam.fov_y * 0.5).tan();
        let (pw, ph, pa) = self.pixel_size();
        let aspect = pw as f32 / ph as f32 * pa;
        let secs = self.start.elapsed().as_secs_f32();
        let marks: Vec<(Vec3, bool, bool, u64)> = self.work.rows.iter().filter(|r| r.done.is_none()).filter_map(|r| r.at.map(|a| (a, r.conjure, r.mine, r.id))).collect();
        for (at, conjure, mine, id) in marks {
            let v = at + Vec3::Y * if conjure { 0.6 } else { 0.0 } - cam.pos;
            let dist = v.length();
            let shown = match (conjure, mine) {
                (true, true) => dist > SIGIL_SEEN && dist < SPARKLE_MINE,
                (true, false) => false,
                (false, _) => dist < DUST_NEAR,
            };
            if !shown {
                continue;
            }
            let z = v.dot(f);
            if z < 0.3 {
                continue;
            }
            let sx = v.dot(r) / z / (tan * aspect);
            let sy = v.dot(u) / z / tan;
            if sx.abs() > 1.05 || sy.abs() > 1.05 {
                continue;
            }
            let col = ((sx + 1.0) * 0.5 * w as f32) as i32;
            let row = ((1.0 - sy) * 0.5 * vh as f32) as i32;
            if conjure {
                if col < 0 || row < 0 || col >= w as i32 || row >= vh as i32 {
                    continue;
                }
                let k = 0.65 + 0.35 * (secs * 2.2 + id as f32).sin();
                let fg = SPARKLE.map(|c| (c as f32 * k) as u8);
                self.screen.over(col as u16, row as u16, |c| Cell { ch: '✦', fg, bold: true, ..c });
                continue;
            }
            for i in 0..2u32 {
                // Each mote rises and fades at its own pace, from its own place.
                let seed = id.wrapping_mul(2654435761).wrapping_add(i as u64 * 40503) as u32;
                let rnd = |k: u32| ((seed.wrapping_mul(k).wrapping_add(k >> 3)) % 1000) as f32 / 1000.0;
                let life = (secs * (0.35 + 0.5 * rnd(7919)) + rnd(104729)).fract();
                let x = col + ((rnd(1301) * 2.0 - 1.0)).round() as i32;
                let y = row - (life * 2.0) as i32;
                if x < 0 || y < 0 || x >= w as i32 || y >= vh as i32 {
                    continue;
                }
                let glow = 1.0 - (life * 2.0 - 1.0).abs() * 0.6;
                let fg = [235u8, 225, 200].map(|c| (c as f32 * glow) as u8);
                self.screen.over(x as u16, y as u16, |c| Cell { ch: '·', fg, ..c });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::where_from;
    use glam::Vec3;

    #[test]
    fn which_way_and_how_far() {
        // Facing +z (yaw 0): +x is to the right.
        assert_eq!(where_from(Vec3::ZERO, 0.0, Vec3::new(0.0, 5.0, 24.0)), "24m ↑");
        assert_eq!(where_from(Vec3::ZERO, 0.0, Vec3::new(10.0, 0.0, 0.0)), "10m →");
        assert_eq!(where_from(Vec3::ZERO, 0.0, Vec3::new(-10.0, 0.0, -10.0)), "14m ↙");
        assert_eq!(where_from(Vec3::ZERO, 0.0, Vec3::new(1.0, 0.0, 0.0)), "here");
        assert_eq!(where_from(Vec3::ZERO, std::f32::consts::FRAC_PI_2, Vec3::new(10.0, 0.0, 0.0)), "10m ↑");
    }
}
