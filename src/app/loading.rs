//! The loading screen: you step into the world only once it is made around
//! you (genesis, then every region within sight). Meanwhile a map of the land
//! forms outward from where you will stand, a log names what is being made,
//! and a bar shows how far along it all is. There is no entering early.

use super::{ACCENT, App, DIM, TEXT, speaker_color};
use crate::brain::Event;
use crate::pace::Task;
use crate::term::Cell;
use crate::terrain::WATER_LEVEL;
use crate::world::species::Mind;
use crate::world::{REGION, region_of};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use glam::Vec3;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// You enter once every region within this distance is made: about as far as you see clearly.
const SETTLE_DIST: f32 = 110.0;
/// Metres from you to the map's top and bottom edges.
const MAP_REACH: f32 = 150.0;
/// Genesis counts as this many regions on the bar.
const GENESIS_WEIGHT: f32 = 2.0;
const BG: [u8; 3] = [10, 11, 15];
const FOG: [u8; 3] = [40, 43, 56];
const FAINT: [u8; 3] = [64, 68, 84];

pub(super) struct Loading {
    start: Instant,
    /// Genesis was still to come when loading began (it weighs on the bar).
    genesis: bool,
    /// How far along each piece of work is.
    work: HashMap<Task, f32>,
    lines: VecDeque<Line>,
    /// The land's name, once genesis names it.
    title: Option<String>,
    /// What the world was asked to be (typed or random), shown under the name.
    prompt: String,
    /// `q` was pressed: asking whether to leave.
    leaving: bool,
    /// What the bar shows: eases towards the truth and never goes back.
    shown: f32,
    done_at: Option<Instant>,
    last_draw: Instant,
    /// Things and beings already on the map, and when new ones appeared (they glow).
    seen: HashSet<i64>,
    seen_beings: HashSet<String>,
    born: HashMap<i64, Instant>,
    born_beings: HashMap<String, Instant>,
    map: Option<MapCache>,
}

struct Line {
    verb: &'static str,
    text: String,
    at: Instant,
}

struct MapCache {
    key: (u32, u16, u16, [i32; 3]),
    cells: Vec<Ground>,
}

#[derive(Clone, Copy)]
struct Ground {
    h: f32,
    color: [u8; 3],
    steep: bool,
    /// Trees per map cell, from the biome's scatter.
    trees: f32,
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8)
}

fn scale(c: [u8; 3], k: f32) -> [u8; 3] {
    c.map(|v| (v as f32 * k).clamp(0.0, 255.0) as u8)
}

fn hash01(i: u32, j: u32, s: u32) -> f32 {
    crate::noise::pcg(i.wrapping_mul(7919) ^ j.wrapping_mul(104_729) ^ s.wrapping_mul(1_299_709)) as f32 / u32::MAX as f32
}

impl App {
    /// Show the loading screen until the world around you is made (no-op when it already is).
    pub(crate) fn start_loading(&mut self) {
        let seen = self.snap.instances.iter().map(|p| p.id).collect();
        let seen_beings = self.sim.cast.npcs.iter().map(|n| n.name().to_string()).collect();
        self.loading = Some(Loading {
            start: Instant::now(),
            genesis: self.genesis_pending,
            work: HashMap::new(),
            lines: VecDeque::new(),
            title: None,
            prompt: self.db.universe().map(|u| u.bible).unwrap_or_default(),
            leaving: false,
            shown: 0.0,
            done_at: None,
            last_draw: Instant::now(),
            seen,
            seen_beings,
            born: HashMap::new(),
            born_beings: HashMap::new(),
            map: None,
        });
        if self.settled() {
            self.loading = None;
        }
    }

    /// Regions within sight of you: the world opens once they are made.
    fn settle_regions(&self) -> Vec<(i32, i32)> {
        if !self.brain.has_llm || self.settings.region_radius == 0 {
            return Vec::new();
        }
        let p = self.pos();
        let pr = region_of(p.x, p.z);
        let mut out = Vec::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                let r = (pr.0 + dx, pr.1 + dz);
                let (ox, oz) = (r.0 as f32 * REGION, r.1 as f32 * REGION);
                let near = Vec3::new(p.x.clamp(ox, ox + REGION), 0.0, p.z.clamp(oz, oz + REGION));
                if near.distance(Vec3::new(p.x, 0.0, p.z)) <= SETTLE_DIST {
                    out.push(r);
                }
            }
        }
        out
    }

    fn region_settled(&self, r: (i32, i32)) -> bool {
        self.snap.regions.contains_key(&r) || self.regions_failed.contains(&r)
    }

    /// Nothing more is coming for the land around you (or nothing can).
    fn settled(&self) -> bool {
        if !self.brain.has_llm || self.budget_paused {
            return true;
        }
        !self.genesis_pending && self.settle_regions().into_iter().all(|r| self.region_settled(r))
    }

    fn load_progress(&self, l: &Loading) -> f32 {
        let (mut total, mut done) = (0.0, 0.0);
        if l.genesis {
            total += GENESIS_WEIGHT;
            done += GENESIS_WEIGHT * if self.genesis_pending { l.work.get(&Task::Genesis).copied().unwrap_or(0.0) } else { 1.0 };
        }
        for r in self.settle_regions() {
            total += 1.0;
            done += if self.region_settled(r) { 1.0 } else { l.work.get(&Task::Region(r)).copied().unwrap_or(0.0) };
        }
        if total == 0.0 { 1.0 } else { done / total }
    }

    /// Progress, names and news for the loading screen. True when the event was only for it.
    pub(super) fn loading_event(&mut self, ev: &Event) -> bool {
        match ev {
            Event::Progress { task, frac } => {
                if let Some(l) = self.loading.as_mut() {
                    let w = l.work.entry(*task).or_insert(0.0);
                    *w = w.max(*frac);
                }
                true
            }
            Event::Made { task, verb, name } => {
                if let Some(l) = self.loading.as_mut() {
                    if *task == Task::Genesis && *verb == "naming" && l.title.is_none() {
                        l.title = Some(name.clone());
                    }
                    l.push(verb, name);
                }
                true
            }
            Event::Log(s) => {
                if let Some(l) = self.loading.as_mut() {
                    l.push("", s);
                }
                false
            }
            _ => false,
        }
    }

    pub(super) fn loading_key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Press {
            return;
        }
        if self.menu.is_some() {
            self.menu_key(k);
            self.dirty = true;
            return;
        }
        let quit = matches!(k.code, KeyCode::Char('q') | KeyCode::Char('Q'));
        if let Some(l) = self.loading.as_mut().filter(|l| l.leaving) {
            l.leaving = false;
            self.quit = quit;
            self.dirty = true;
            return;
        }
        let done = self.loading.as_ref().is_some_and(|l| l.done_at.is_some());
        match k.code {
            KeyCode::Esc | KeyCode::F(10) => self.toggle_menu(),
            _ if quit => {
                if let Some(l) = self.loading.as_mut() {
                    l.leaving = true;
                }
            }
            _ if done => self.finish_loading(),
            _ => {}
        }
        self.dirty = true;
    }

    fn finish_loading(&mut self) {
        self.loading = None;
        // The session's spend starts once you are in: making the world is not counted
        // (it stays in the world's all-time total).
        if let Some(l) = &self.llm {
            *l.spent.lock() = 0.0;
        }
        self.budget_paused = false;
        self.dirty = true;
    }

    /// Instead of `tick` while loading: keep regions coming, and nobody moves.
    pub(super) fn tick_loading(&mut self, dt: f32) {
        if self.last_region_check.elapsed() > Duration::from_millis(500) {
            self.last_region_check = Instant::now();
            {
                let mut live = self.live.lock();
                live.player = self.pos();
                live.types = self.sim.types_in_use();
            }
            self.schedule_regions();
        }
        let settled = self.settled();
        let Some(mut l) = self.loading.take() else { return };
        let target = if settled { 1.0 } else { self.load_progress(&l).min(0.99) };
        if target > l.shown {
            l.shown = (l.shown + (target - l.shown) * (dt * 2.5).min(1.0) + dt * 0.02).min(target);
        }
        if settled && l.done_at.is_none() {
            l.done_at = Some(Instant::now());
        }
        if l.last_draw.elapsed() > Duration::from_millis(66) {
            self.dirty = true;
        }
        // The world waits for a key: whoever started it may have walked away.
        self.loading = Some(l);
    }

    pub(super) fn compose_loading(&mut self) {
        let Some(mut l) = self.loading.take() else { return };
        l.last_draw = Instant::now();
        let (w, h) = (self.screen.w, self.screen.h);
        for y in 0..h {
            self.screen.fill_row(y, Cell { bg: BG, ..Cell::BLANK });
        }
        let t = l.start.elapsed().as_secs_f32();
        let cw = w.saturating_sub(4).clamp(10, 104);
        let x0 = (w - cw.min(w)) / 2;
        let top = if h >= 24 { 2 } else { 1 };
        let done = l.done_at.is_some();

        // The land's name.
        let look_name = self.snap.look.name.trim().to_string();
        let title = if !look_name.is_empty() && !self.genesis_pending { look_name } else { l.title.clone().unwrap_or_else(|| "a new world".into()) };
        let title: String = title.chars().take(cw as usize).collect();
        let tx = x0 + (cw.saturating_sub(title.chars().count() as u16)) / 2;
        self.screen.text(tx, top, &title, if done || l.title.is_some() || !self.genesis_pending { ACCENT } else { DIM }, BG, true);

        // What it was asked to be, under the name (at most three lines).
        let bar_y = h.saturating_sub(3);
        let mut lines = crate::term::wrap(&format!("“{}”", l.prompt.trim()), cw as usize);
        let room = (bar_y.saturating_sub(top + 10) as usize).min(3);
        if l.prompt.trim().is_empty() || room == 0 {
            lines.clear();
        }
        if lines.len() > room {
            lines.truncate(room);
            if let Some(last) = lines.last_mut() {
                let keep: String = last.chars().take((cw as usize).saturating_sub(2)).collect();
                *last = format!("{}…", keep.trim_end());
            }
        }
        for (i, line) in lines.iter().enumerate() {
            let lx = x0 + (cw.saturating_sub(line.chars().count() as u16)) / 2;
            self.screen.text(lx, top + 1 + i as u16, line, TEXT, BG, false);
        }

        // Map and log between the title and the bar.
        let mid_top = top + 2 + if lines.is_empty() { 0 } else { lines.len() as u16 + 1 };
        let mid_h = bar_y.saturating_sub(1).saturating_sub(mid_top);
        if cw >= 72 && mid_h >= 6 {
            let map_w = cw * 58 / 100;
            self.draw_map(&mut l, x0, mid_top, map_w, mid_h, t);
            self.draw_lines(&l, x0 + map_w + 3, mid_top, cw - map_w - 3, mid_h);
        } else if mid_h >= 3 {
            let log_h = (mid_h / 3).clamp(1, 5);
            let map_h = mid_h.saturating_sub(log_h + 1);
            if map_h >= 3 {
                self.draw_map(&mut l, x0, mid_top, cw, map_h, t);
            }
            self.draw_lines(&l, x0, mid_top + mid_h - log_h, cw, log_h);
        }

        // The bar.
        let bw = cw.saturating_sub(6);
        let filled = (l.shown * bw as f32).round() as u16;
        for i in 0..bw {
            let (ch, fg) = if i < filled {
                // A soft pulse runs along the filled part while work goes on.
                let wave = if done { 0.0 } else { ((i as f32 * 0.25 - t * 3.0).sin() * 0.5 + 0.5) * 0.25 };
                ('━', mix(ACCENT, TEXT, wave))
            } else {
                ('─', FOG)
            };
            self.screen.set(x0 + i, bar_y, Cell { ch, fg, bg: BG, bold: false });
        }
        self.screen.text(x0 + bw + 1, bar_y, &format!("{:>3}%", (l.shown * 100.0).floor() as u32), if done { ACCENT } else { TEXT }, BG, false);

        // What is happening, and for how long.
        let label = if done {
            "ready".to_string()
        } else if self.budget_paused {
            "budget reached".to_string()
        } else if self.genesis_pending {
            match l.work.get(&Task::Genesis).copied().unwrap_or(0.0) {
                f if f < 0.7 => "dreaming up the land",
                f if f < 0.96 => "giving things their shape",
                _ => "settling the land",
            }
            .to_string()
        } else {
            let rs = self.settle_regions();
            let k = rs.iter().filter(|r| self.region_settled(**r)).count();
            format!("filling in the land around you  {k}/{}", rs.len())
        };
        self.screen.text(x0, bar_y + 1, &label, DIM, BG, false);
        let secs = l.start.elapsed().as_secs();
        let mut right = format!("{}:{:02}", secs / 60, secs % 60);
        if self.brain.has_llm {
            right = format!("{right} · ${:.2}", self.spent());
        }
        if done {
            right = "any key to step in".into();
        }
        let rx = (x0 + cw).saturating_sub(right.chars().count() as u16 + 1);
        let right_fg = if done { mix(TEXT, ACCENT, (t * 2.0).sin() * 0.5 + 0.5) } else { FAINT };
        self.screen.text(rx, bar_y + 1, &right, right_fg, BG, false);
        if !done && bar_y + 2 < h {
            self.screen.text(x0, bar_y + 2, "q leave · Esc settings", FAINT, BG, false);
        }
        if l.leaving {
            self.draw_leaving();
        }

        if self.menu.is_some() {
            self.draw_menu(w, h);
        }
        self.loading = Some(l);
    }

    /// Asked after `q`: leaving keeps the world, and the rest is made on return.
    fn draw_leaving(&mut self) {
        const PANEL: [u8; 3] = [26, 28, 38];
        let lines: [(&str, [u8; 3], bool); 5] = [
            ("Leave this world?", ACCENT, true),
            ("It is saved. Whatever is still being made", TEXT, false),
            ("carries on when you open it again (pocket list).", TEXT, false),
            ("", TEXT, false),
            ("q leave    any other key stay", DIM, false),
        ];
        let (w, h) = (self.screen.w, self.screen.h);
        let bw = (lines.iter().map(|l| l.0.chars().count()).max().unwrap_or(0) as u16 + 6).min(w);
        let bh = (lines.len() as u16 + 2).min(h);
        let (bx, by) = ((w - bw) / 2, (h - bh) / 2);
        for y in by..by + bh {
            for x in bx..bx + bw {
                self.screen.set(x, y, Cell { bg: PANEL, ..Cell::BLANK });
            }
        }
        for (i, (text, fg, bold)) in lines.iter().enumerate() {
            let text: String = text.chars().take(bw.saturating_sub(2) as usize).collect();
            let x = bx + (bw.saturating_sub(text.chars().count() as u16)) / 2;
            self.screen.text(x, by + 1 + i as u16, &text, *fg, PANEL, *bold);
        }
    }

    /// The land seen from above, your view direction up, forming outward from you.
    fn draw_map(&mut self, l: &mut Loading, bx: u16, by: u16, bw: u16, bh: u16, t: f32) {
        // Cells are about twice as tall as wide: twice the columns for a square.
        let mh = bh.min(bw / 2).max(3);
        let mw = (mh * 2).min(bw);
        let mx0 = bx + (bw - mw) / 2;
        let my0 = by + (bh - mh) / 2;
        let p = self.pos();
        let yaw = self.yaw();
        let fwd = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        let sx = 2.0 * MAP_REACH / mw as f32;
        let sz = 2.0 * MAP_REACH / mh as f32;
        let world = |i: u16, j: u16| -> Vec3 {
            let u = (i as f32 + 0.5 - mw as f32 / 2.0) * sx;
            let v = (mh as f32 / 2.0 - j as f32 - 0.5) * sz;
            p + right * u + fwd * v
        };
        let terrain = self.snap.terrain.clone();
        let key = (crate::render::terrain_epoch(&terrain), mw, mh, [p.x.round() as i32, p.z.round() as i32, (yaw * 100.0) as i32]);
        if l.map.as_ref().is_none_or(|m| m.key != key) {
            let pal = &self.snap.look.palette;
            let sun = Vec3::new(-0.5, 0.8, -0.35).normalize();
            let mut cells = Vec::with_capacity(mw as usize * mh as usize);
            for j in 0..mh {
                for i in 0..mw {
                    let q = world(i, j);
                    let h = terrain.height(q.x, q.z);
                    let n = terrain.normal(q.x, q.z);
                    let c = terrain.ground_color(pal, q.x, q.z, h, n.y);
                    let shade = (0.55 + 0.55 * n.dot(sun).max(0.0)).min(1.15) * 0.85;
                    let color = [c.x, c.y, c.z].map(|v| (v * shade * 255.0).clamp(0.0, 255.0) as u8);
                    let b = &terrain.biomes[terrain.biome_at(q.x, q.z)];
                    let density: f32 = b.scatter.iter().filter(|(k, _)| ["tree", "pine", "palm"].iter().any(|t| k.contains(t))).map(|(_, d)| d).sum();
                    cells.push(Ground { h, color, steep: n.y < 0.8, trees: density * sx * sz / 100.0 });
                }
            }
            l.map = Some(MapCache { key, cells });
        }
        let water = self.snap.look.palette.water;
        let cells = &l.map.as_ref().unwrap().cells;
        // The land forms out to `reach` (in map radii), with a ragged edge.
        let reach = 0.05 + 1.6 * l.shown;
        let frame = (t * 10.0) as u32;
        for j in 0..mh {
            for i in 0..mw {
                let g = cells[j as usize * mw as usize + i as usize];
                let u = (i as f32 + 0.5 - mw as f32 / 2.0) / (mw as f32 / 2.0);
                let v = (j as f32 + 0.5 - mh as f32 / 2.0) / (mh as f32 / 2.0);
                let e = (u * u + v * v).sqrt() + hash01(i as u32, j as u32, 1) * 0.18;
                let (ch, fg) = if e > reach {
                    if i % 4 == 0 && j % 2 == 0 { ('·', FOG) } else { (' ', BG) }
                } else if e > reach - 0.07 {
                    let k = (hash01(i as u32, j as u32, frame) * 4.0) as usize;
                    (['·', ':', '+', '*'][k.min(3)], mix(FOG, ACCENT, 0.55))
                } else if g.h < WATER_LEVEL {
                    let wave = (i as f32 * 0.7 + j as f32 * 1.3 - t * 1.6).sin();
                    let deep = 1.0 - ((WATER_LEVEL - g.h) / 10.0).min(0.45);
                    (if wave > 0.55 { '≈' } else { '~' }, scale(water, (0.6 + 0.2 * wave) * deep))
                } else if hash01(i as u32, j as u32, 2) < (g.trees * 0.35).min(0.3) {
                    ('♣', scale(g.color, 1.15))
                } else if g.steep {
                    ('^', g.color)
                } else {
                    (if hash01(i as u32, j as u32, 3) < 0.5 { '.' } else { ',' }, g.color)
                };
                self.screen.set(mx0 + i, my0 + j, Cell { ch, fg, bg: BG, bold: false });
            }
        }
        // Things, people and creatures (they show through the fog: news arrives).
        let now = Instant::now();
        let cell_of = |q: Vec3| -> Option<(u16, u16)> {
            let d = q - p;
            let i = (d.dot(right) / sx + mw as f32 / 2.0).floor();
            let j = (mh as f32 / 2.0 - d.dot(fwd) / sz).floor();
            (i >= 0.0 && j >= 0.0 && i < mw as f32 && j < mh as f32).then_some((i as u16, j as u16))
        };
        let glow = |born: Option<&Instant>, base: [u8; 3]| born.map(|b| mix(ACCENT, base, b.elapsed().as_secs_f32() / 2.0)).unwrap_or(base);
        for pl in &self.snap.instances {
            if !l.seen.contains(&pl.id) {
                l.seen.insert(pl.id);
                l.born.insert(pl.id, now);
            }
            let Some((i, j)) = cell_of(pl.pos) else { continue };
            let building = self.snap.type_of(pl.type_id).is_some_and(|t| t.has_tag("building"));
            let (ch, base) = if building { ('⌂', [214, 204, 188]) } else { ('▪', [170, 176, 192]) };
            self.screen.set(mx0 + i, my0 + j, Cell { ch, fg: glow(l.born.get(&pl.id), base), bg: BG, bold: building });
        }
        for n in &self.sim.cast.npcs {
            let name = n.name().to_string();
            if !l.seen_beings.contains(&name) {
                l.seen_beings.insert(name.clone());
                l.born_beings.insert(name.clone(), now);
            }
            let Some((i, j)) = cell_of(n.a.pos) else { continue };
            let (ch, base) = if n.species.mind == Mind::Sapient { ('@', speaker_color(&name)) } else { ('•', [196, 168, 128]) };
            self.screen.set(mx0 + i, my0 + j, Cell { ch, fg: glow(l.born_beings.get(&name), base), bg: BG, bold: false });
        }
        l.born.retain(|_, b| b.elapsed().as_secs_f32() < 2.0);
        l.born_beings.retain(|_, b| b.elapsed().as_secs_f32() < 2.0);
        // You.
        let blink = (t * 2.0) as u32 % 2 == 0;
        self.screen.set(mx0 + mw / 2, my0 + mh / 2, Cell { ch: '◆', fg: if blink { ACCENT } else { TEXT }, bg: BG, bold: true });
    }

    /// What is being made, newest at the bottom, older lines fading.
    fn draw_lines(&mut self, l: &Loading, x: u16, y: u16, w: u16, rows: u16) {
        let w = w as usize;
        let shown: Vec<&Line> = l.lines.iter().rev().take(rows as usize).collect();
        for (k, line) in shown.iter().enumerate() {
            let row = y + rows - 1 - k as u16;
            let age = k as f32 / rows.max(2) as f32;
            let fresh = k == 0 && line.at.elapsed() < Duration::from_millis(700);
            let fg = if fresh { ACCENT } else { mix(TEXT, FAINT, age * 1.2) };
            if line.verb.is_empty() {
                self.screen.text(x, row, &clip_words(&line.text, w), mix(DIM, FAINT, age), BG, false);
                continue;
            }
            let verb = format!("{:>9}  ", line.verb);
            let x2 = self.screen.text(x, row, &verb, mix(DIM, FOG, age), BG, false);
            self.screen.text(x2, row, &clip_words(&line.text, w.saturating_sub(11)), fg, BG, false);
        }
    }
}

/// Fits `s` in `n` columns, cutting at the last whole word and ending in `…`.
fn clip_words(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let head: String = s.chars().take(n.saturating_sub(1)).collect();
    let cut = match head.rfind(' ') {
        Some(i) if head[..i].chars().count() * 2 >= n => &head[..i],
        _ => &head,
    };
    format!("{}…", cut.trim_end_matches([' ', ',', ';', ':', '.']))
}

impl Loading {
    fn push(&mut self, verb: &'static str, text: &str) {
        self.lines.push_back(Line { verb, text: text.to_string(), at: Instant::now() });
        while self.lines.len() > 200 {
            self.lines.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{App, Setup};
    use crate::brain::Brain;
    use crate::db::Db;
    use crate::llm::{Llm, Msg};
    use crate::model::{Live, WorldModel};
    use parking_lot::Mutex;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn clip_words_cuts_at_a_word() {
        assert_eq!(super::clip_words("cairns mend what rests near the fire", 30), "cairns mend what rests near…");
        assert_eq!(super::clip_words("short", 30), "short");
        assert_eq!(super::clip_words("abcdefghijklmnop", 8), "abcdefg…");
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)).unwrap()
    }

    fn script(_sys: &str, msgs: &[Msg]) -> String {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if user.contains("Design the base layer") {
            let look = r#"{"name": "Greywater Coast", "biomes": [{"name": "dune grass", "base": 3, "amp": 6, "rough": 0.1, "ground": [140, 150, 100], "ground2": [180, 170, 130], "scatter": {"rock": 0.3}},
                {"name": "coastal pines", "base": 8, "amp": 16, "rough": 0.3, "ground": [70, 95, 60], "ground2": [90, 100, 70], "scatter": {"tree": 1.0}}]}"#;
            return format!("```json\n{look}\n```\n```js\n{}\n```", fixture("mock/seapine.js"));
        }
        if user.contains("Plan the story layer") {
            let r = user.split("Region (").nth(1).and_then(|s| s.split(')').next()).unwrap_or("0, 0").replace(", ", "_").replace('-', "m");
            return format!(
                "```json\n{{\"name\": \"Vale {r}\", \"new_types\": [{{\"name\": \"hut {r}\", \"description\": \"hut\", \"size_m\": [6, 6, 5], \"tags\": [\"building\"]}}],\n \"landmarks\": [{{\"type\": \"hut {r}\", \"x\": 128, \"z\": 128}}], \"characters\": [{{\"name\": \"Ola {r}\", \"home_x\": 120, \"home_z\": 120}}]}}\n```"
            );
        }
        if user.contains("Object type to write") {
            let name = user.split('"').nth(1).unwrap_or("hut");
            return format!("```js\n{}\n```", fixture("mock/cottage.js").replace("keeper's cottage", name));
        }
        "ok".into()
    }

    /// A new world: the screen holds until genesis and the regions in sight
    /// are made, names them as they come, then lets you in by itself.
    #[test]
    fn loading_waits_for_the_world_around_you() {
        let dir = std::env::temp_dir().join(format!("pocket-loading-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::create(&dir.join("l.pocket"), 99, "a grey coast", &serde_json::to_string(&crate::world::Look::default()).unwrap()).unwrap();
        let live = Arc::new(Mutex::new(Live::default()));
        let mut model = WorldModel::load(db.clone(), None, live.clone()).unwrap();
        let snap = model.snapshot().unwrap();
        let llm = Llm::scripted(db.clone(), Arc::new(|s: &str, m: &[Msg]| script(s, m)));
        let (tx, rx) = crossbeam_channel::unbounded();
        let brain = Brain::start(model, Some(llm.clone()), tx, false);
        let render = crate::render::cpu::spawn();
        let mut app = App::new(Setup { db: db.clone(), brain, events: rx, render, snap, live, enhanced: false, truecolor: true, size: (110, 36), llm: Some(llm), genesis: true });
        app.start_loading();
        assert!(app.loading.is_some(), "a new world starts behind the loading screen");
        let needed = app.settle_regions();
        assert!(!needed.is_empty());
        let (mut saw_bar, mut saw_names, mut peak, mut waited) = (false, false, 0.0f32, false);
        let stop = AtomicBool::new(false);
        let mut sink = Vec::new();
        let mut t = 0.0;
        let mut f = |a: &mut App, dt: f32| {
            t += dt;
            if let Some(l) = a.loading.as_ref() {
                assert!(l.shown >= peak, "the bar never goes back");
                peak = l.shown;
                saw_names |= l.lines.iter().any(|x| x.verb == "shaping" && x.text == "sea pine") && l.lines.iter().any(|x| x.verb == "waking");
                let row: String = (0..a.screen.w).filter_map(|x| a.screen.get(x, a.screen.h - 3).map(|c| c.ch)).collect();
                saw_bar |= row.contains('━') && row.contains('%');
            }
            // Ready, it waits for you; a key steps in.
            if a.loading.as_ref().and_then(|l| l.done_at).is_some_and(|d| d.elapsed() > std::time::Duration::from_secs(2)) {
                waited = true;
                *a.llm.as_ref().unwrap().spent.lock() = 1.5;
                a.loading_key(crossterm::event::KeyEvent::new(crossterm::event::KeyCode::Enter, crossterm::event::KeyModifiers::NONE));
            }
            t < 90.0 && a.loading.is_some()
        };
        app.run(false, &mut sink, &stop, Some(&mut f)).unwrap();
        assert!(app.loading.is_none(), "the world opened (after {t:.1} s)");
        assert!(waited, "a ready world waits for a key");
        assert_eq!(app.spent(), 0.0, "making the world is not counted in the session's spend");
        assert!(saw_bar && saw_names, "bar {saw_bar}, names {saw_names}");
        assert_eq!(app.snap.look.name, "Greywater Coast");
        for r in needed {
            assert!(app.snap.regions.contains_key(&r) || app.regions_failed.contains(&r), "region {r:?} made before entering");
        }
        assert!(!app.sim.cast.npcs.is_empty(), "people are there when you arrive");
        app.shutdown();
    }

    /// `q` asks first, says what happens on return, and only a second `q` leaves.
    #[test]
    fn leaving_asks_first() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let dir = std::env::temp_dir().join(format!("pocket-leave-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::create(&dir.join("l.pocket"), 99, "a grey coast where the keeper vanished", &serde_json::to_string(&crate::world::Look::default()).unwrap()).unwrap();
        let live = Arc::new(Mutex::new(Live::default()));
        let mut model = WorldModel::load(db.clone(), None, live.clone()).unwrap();
        let snap = model.snapshot().unwrap();
        let llm = Llm::scripted(db.clone(), Arc::new(|_: &str, _: &[Msg]| "?".into()));
        let (tx, rx) = crossbeam_channel::unbounded();
        let brain = Brain::start(model, Some(llm.clone()), tx, false);
        let render = crate::render::cpu::spawn();
        let mut app = App::new(Setup { db: db.clone(), brain, events: rx, render, snap, live, enhanced: false, truecolor: true, size: (110, 36), llm: Some(llm), genesis: true });
        app.start_loading();
        let screen = |a: &mut App| {
            a.compose_loading();
            (0..a.screen.h).map(|y| (0..a.screen.w).filter_map(|x| a.screen.get(x, y).map(|c| c.ch)).collect::<String>()).collect::<Vec<_>>().join("\n")
        };
        let key = |c: char| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        let shown = screen(&mut app);
        assert!(shown.contains("a grey coast where the keeper vanished") && shown.contains("q leave · Esc settings"), "{shown}");
        app.loading_key(key('q'));
        assert!(!app.quit && screen(&mut app).contains("Leave this world?"));
        app.loading_key(key('x'));
        assert!(!app.quit && !screen(&mut app).contains("Leave this world?"), "any other key stays");
        app.loading_key(key('q'));
        app.loading_key(key('q'));
        assert!(app.quit);
        app.shutdown();
    }
}
