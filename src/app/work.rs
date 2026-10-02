//! The "working on" box, above the log on the right: slow work in progress
//! (a deed the world is deciding, a thing being made or reshaped, a new
//! gesture), from when it starts until it is done, so there is always a sign
//! that something is happening. Its kinds are the sim's `WorkKind`s; the land
//! being made is the one kind of its own here. Settings choose whose work.

use super::{App, DIM, TEXT};
use crate::settings::ShowWork;
use crate::sim::ActorId;
use crate::sim::interp::WorkKind;
use crate::term::Cell;
use std::time::Instant;

/// How long a finished line stays (seconds).
const DONE_FOR: f32 = 3.0;
const MAX_ROWS: usize = 5;
/// Lines one entry may wrap to before it is cut.
const MAX_LINES: usize = 4;
const BG: [u8; 3] = [20, 20, 30];
const DONE: [u8; 3] = [130, 200, 140];
/// The land's own row (no request id).
const LAND: u64 = u64::MAX;

struct Row {
    id: u64,
    verb: &'static str,
    color: [u8; 3],
    /// Someone else's work: whose.
    who: Option<String>,
    what: String,
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
        let mut now: Vec<(u64, &'static str, [u8; 3], Option<String>, String)> = Vec::new();
        for w in self.sim.work() {
            let mine = w.who == Some(ActorId::Player);
            if !mine && show == ShowWork::Mine {
                continue;
            }
            let who = (!mine).then(|| w.who.map(|a| self.sim.actor_name(a)).unwrap_or_else(|| "the world".into()));
            now.push((w.id, w.kind.verb(), kind_color(w.kind), who, w.what));
        }
        if show == ShowWork::Everyone && !self.regions_inflight.is_empty() {
            let n = self.regions_inflight.len();
            now.push((LAND, "shaping", [150, 190, 150], None, format!("the land ahead ({n} region{})", if n == 1 { "" } else { "s" })));
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
        for (id, verb, color, who, what) in now {
            match rows.iter_mut().find(|r| r.id == id) {
                Some(r) => {
                    if r.what != what {
                        r.what = what;
                        changed = true;
                    }
                }
                None => {
                    rows.push(Row { id, verb, color, who, what, since: t, done: None });
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

    /// The box itself, floating bottom right of the view, a little off the
    /// edge and the log. Each line is written out in full, wrapped.
    pub(super) fn draw_work(&mut self, w: u16, vh: u16) {
        if self.work.rows.is_empty() || self.log_view == 2 || self.menu.is_some() {
            return;
        }
        // Cells are about twice as tall as wide: two columns and one row
        // look like the same gap.
        let (mx, my) = (2u16, 1u16);
        let bw = (w as usize).saturating_sub(2 * mx as usize).min(60);
        if bw < 24 {
            return;
        }
        // The text sits in its own column after the verb, before the time.
        const TEXT_X: usize = 13;
        let room = bw - TEXT_X - 6;
        let entries: Vec<(String, [u8; 3], Vec<String>, String, bool)> = self
            .work
            .rows
            .iter()
            .map(|r| {
                let what = match &r.who {
                    Some(who) => format!("{who}: {}", r.what),
                    None => r.what.clone(),
                };
                let mut lines = crate::term::wrap(&what, room);
                if lines.len() > MAX_LINES {
                    lines.truncate(MAX_LINES);
                    let last = &mut lines[MAX_LINES - 1];
                    let keep = last.chars().count().min(room - 1);
                    *last = last.chars().take(keep).collect::<String>() + "…";
                }
                match r.done {
                    Some(_) => ("✓ done".to_string(), DONE, lines, String::new(), true),
                    None => (format!("◌ {}", r.verb), r.color, lines, elapsed(r.since.elapsed().as_secs()), false),
                }
            })
            .collect();
        // Newest at the bottom, nearest the log; older ones give way first.
        let budget = (vh as usize).saturating_sub(my as usize + 4);
        let mut used = 1;
        let mut first = entries.len();
        while first > 0 && entries.len() - first < MAX_ROWS && used + entries[first - 1].2.len() <= budget {
            first -= 1;
            used += entries[first].2.len();
        }
        if first == entries.len() {
            return;
        }
        let more = first;
        let height = used as u16;
        let x0 = w - mx - bw as u16;
        let y0 = vh - my - height;
        for row in 0..height {
            for col in 0..bw {
                self.screen.set(x0 + col as u16, y0 + row, Cell { ch: ' ', fg: TEXT, bg: BG, bold: false });
            }
        }
        let head = if more > 0 { format!("Working on  (+{more} earlier)") } else { "Working on".to_string() };
        self.screen.text(x0 + 1, y0, &head, DIM, BG, true);
        let mut y = y0 + 1;
        for (head, color, lines, time, done) in entries.into_iter().skip(first) {
            self.screen.text(x0 + 1, y, &head, color, BG, !done);
            let tx = x0 + bw as u16 - 1 - time.chars().count() as u16;
            self.screen.text(tx, y, &time, DIM, BG, false);
            for line in lines {
                self.screen.text(x0 + TEXT_X as u16, y, &line, if done { DIM } else { TEXT }, BG, false);
                y += 1;
            }
        }
    }
}
