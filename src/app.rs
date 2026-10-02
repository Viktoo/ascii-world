//! The main thread: input, simulation tick, compositing and terminal output.
//! Never blocks on rendering or generation: frames and events arrive on channels.

use crate::brain::{Brain, Cmd, Event};
use crate::db::{Db, PlayerRow};
use crate::model::LiveRef;
use crate::render::{self, Camera, Frame, FrameRequest, RenderHandle, RenderMsg, SceneParams, sky};
use crate::term::{self, Cell, Screen};
use crate::world::characters::{Cast, CastEvent, Mode as NpcMode, TALK_RANGE};
use crate::world::collide::{Obstacles, PLAYER_RADIUS, move_body};
use crate::world::describe::{NpcView, describe};
use crate::world::scatter::ScatterCache;
use crate::world::{WorldSnapshot, cull, region_center, region_of};
use crossterm::event::{self, Event as TEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use glam::Vec3;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const WALK_SPEED: f32 = 5.0;
pub const TURN_SPEED: f32 = 1.9;
const EYE: f32 = 1.65;
const FOV_Y: f32 = 1.05;
const STATUS_BG: [u8; 3] = [38, 40, 52];
const PANEL_BG: [u8; 3] = [14, 15, 20];
const DIM: [u8; 3] = [130, 134, 150];
const TEXT: [u8; 3] = [222, 224, 230];
const ACCENT: [u8; 3] = [255, 200, 110];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Walk,
    Talk(i64),
    Create,
}

#[derive(Clone)]
struct LogLine {
    speaker: Option<String>,
    text: String,
    color: [u8; 3],
    /// A streaming reply from this character.
    streaming: Option<i64>,
}

#[derive(Default)]
struct Keys {
    /// Keys currently held (terminals with key-release reporting).
    down: HashSet<char>,
    /// Fallback: a key counts as held until this time (one step per press/repeat).
    until: HashMap<char, Instant>,
}

/// Movement keys normalised to chars: arrows are ^ v < >.
fn key_char(code: KeyCode) -> Option<char> {
    match code {
        KeyCode::Up => Some('^'),
        KeyCode::Down => Some('v'),
        KeyCode::Left => Some('<'),
        KeyCode::Right => Some('>'),
        KeyCode::Char(c) => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

pub struct Stats {
    frames: VecDeque<Instant>,
    pub max_loop_ms: f32,
    pub worst_recent: VecDeque<(Instant, f32)>,
    pub gpu_ms: f32,
    pub insts: usize,
    pub shown: u64,
    /// Longest time between two displayed frames.
    pub max_gap_ms: f32,
}

impl Stats {
    fn fps(&self) -> f32 {
        if self.frames.len() < 2 {
            return 0.0;
        }
        let dt = self.frames.back().unwrap().duration_since(*self.frames.front().unwrap()).as_secs_f32();
        if dt <= 0.0 { 0.0 } else { (self.frames.len() - 1) as f32 / dt }
    }
    fn worst(&self) -> f32 {
        self.worst_recent.iter().map(|x| x.1).fold(0.0, f32::max)
    }
}

pub struct App {
    pub db: Arc<Db>,
    brain: Brain,
    events: crossbeam_channel::Receiver<Event>,
    render: RenderHandle,
    pub snap: Arc<WorldSnapshot>,
    cache: ScatterCache,
    cast: Cast,
    live: LiveRef,
    pub pos: Vec3,
    pub yaw: f32,
    pitch: f32,
    cam_y: f32,
    pub t_game: f64,
    mode: Mode,
    input: String,
    log: VecDeque<LogLine>,
    keys: Keys,
    pub enhanced: bool,
    ascii: bool,
    debug: bool,
    pub screen: Screen,
    last_frame: Option<Frame>,
    outstanding: u32,
    next_frame_id: u64,
    last_request: Instant,
    building: i32,
    appear: HashMap<i64, Instant>,
    regions_inflight: HashSet<(i32, i32)>,
    regions_failed: HashSet<(i32, i32)>,
    near_flags: HashMap<i64, f64>,
    conv: HashMap<i64, Vec<(bool, String)>>,
    replying: Option<i64>,
    recent_actions: VecDeque<String>,
    pub stats: Stats,
    start: Instant,
    last_save: Instant,
    last_char_save: Instant,
    last_region_check: Instant,
    talk_hint: Option<(i64, String)>,
    quit: bool,
    pub noclip: bool,
    llm_note: Option<String>,
    budget_paused: bool,
    target_fps: f32,
    dirty: bool,
    llm: Option<Arc<crate::llm::Llm>>,
    /// Region plans wait until the universe's look (and so its terrain) exists.
    genesis_pending: bool,
}

pub struct Setup {
    pub db: Arc<Db>,
    pub brain: Brain,
    pub events: crossbeam_channel::Receiver<Event>,
    pub render: RenderHandle,
    pub snap: Arc<WorldSnapshot>,
    pub live: LiveRef,
    pub enhanced: bool,
    pub truecolor: bool,
    pub size: (u16, u16),
    pub llm: Option<Arc<crate::llm::Llm>>,
    pub genesis: bool,
}

impl App {
    pub fn new(s: Setup) -> App {
        let player = s.db.player();
        let (pos, yaw, t_game) = match player {
            Some(p) => (Vec3::new(p.x, s.snap.terrain.height(p.x, p.z), p.z), p.yaw, p.t_game),
            None => (s.snap.spawn, 0.0, 0.33 * sky::DAY_SECONDS),
        };
        // Saved under water (an older save, or a spawn on a lake bed): back to dry land.
        let pos = if pos.y < crate::terrain::WATER_LEVEL + 0.3 { s.snap.spawn } else { pos };
        let mut cast = Cast::default();
        cast.sync(&s.snap);
        let mut app = App {
            db: s.db,
            brain: s.brain,
            events: s.events,
            render: s.render,
            snap: s.snap,
            cache: ScatterCache::default(),
            cast,
            live: s.live,
            pos,
            yaw,
            pitch: -0.06,
            cam_y: pos.y + EYE,
            t_game,
            mode: Mode::Walk,
            input: String::new(),
            log: VecDeque::new(),
            keys: Keys::default(),
            enhanced: s.enhanced,
            ascii: false,
            debug: false,
            screen: Screen::new(s.size.0, s.size.1, s.truecolor),
            last_frame: None,
            outstanding: 0,
            next_frame_id: 1,
            last_request: Instant::now(),
            building: 0,
            appear: HashMap::new(),
            regions_inflight: HashSet::new(),
            regions_failed: HashSet::new(),
            near_flags: HashMap::new(),
            conv: HashMap::new(),
            replying: None,
            recent_actions: VecDeque::new(),
            stats: Stats { frames: VecDeque::new(), max_loop_ms: 0.0, worst_recent: VecDeque::new(), gpu_ms: 0.0, insts: 0, shown: 0, max_gap_ms: 0.0 },
            start: Instant::now(),
            last_save: Instant::now(),
            last_char_save: Instant::now(),
            last_region_check: Instant::now() - Duration::from_secs(5),
            talk_hint: None,
            quit: false,
            noclip: false,
            llm_note: None,
            budget_paused: false,
            target_fps: std::env::var("POCKET_FPS").ok().and_then(|v| v.parse().ok()).unwrap_or(60.0f32).clamp(5.0, 240.0),
            dirty: true,
            llm: s.llm.clone(),
            genesis_pending: false,
        };
        app.unstick();
        let name = app.snap.look.name.clone();
        app.say(None, &format!("Welcome{}. ↑↓ walk, ←→ turn, A/D strafe, Enter talk, / create, Tab style, q quit.", if name.is_empty() { String::new() } else { format!(" to {name}") }), DIM);
        match s.llm.as_ref().map(|l| l.describe()) {
            Some(d) => crate::log::info(format!("LLM: {d}")),
            None => {
                app.llm_note = Some("no LLM key: story & dialogue off".into());
                app.say(None, "No LLM configured (set ANTHROPIC_API_KEY or POCKET_LLM_BASE_URL): the land is here, but its stories and people are not.", DIM);
            }
        }
        if app.render.cpu_fallback {
            app.say(None, "No GPU adapter found: using the slower CPU renderer at reduced resolution.", DIM);
        }
        if s.genesis && app.brain.has_llm {
            app.genesis_pending = true;
            app.say(None, "The world is taking shape around you…", DIM);
            app.brain.send(Cmd::Genesis);
        }
        app
    }

    /// Nobody starts a frame inside a solid: the player or a character caught
    /// inside one (an old save, a building appearing) moves to the nearest free spot.
    fn unstick(&mut self) {
        use crate::world::collide::{NPC_RADIUS, free_spot};
        let snap = self.snap.clone();
        let solids = self.cache.solids_near(&snap, self.pos, 6.0);
        if !solids.is_empty() {
            let obs = Obstacles { solids: &solids, bodies: &[] };
            if obs.dist(&snap.terrain, self.pos.x, self.pos.z) < PLAYER_RADIUS {
                self.pos = free_spot(&snap.terrain, &obs, self.pos, PLAYER_RADIUS, 40.0);
                self.cam_y = self.pos.y + EYE;
            }
        }
        for i in 0..self.cast.npcs.len() {
            let p = self.cast.npcs[i].pos;
            let solids = self.cache.solids_near(&snap, p, 6.0);
            if solids.is_empty() {
                continue;
            }
            let obs = Obstacles { solids: &solids, bodies: &[] };
            if obs.dist(&snap.terrain, p.x, p.z) < NPC_RADIUS {
                self.cast.npcs[i].pos = free_spot(&snap.terrain, &obs, p, NPC_RADIUS, 40.0);
            }
        }
    }

    fn say(&mut self, speaker: Option<&str>, text: &str, color: [u8; 3]) {
        self.log.push_back(LogLine { speaker: speaker.map(str::to_string), text: text.to_string(), color, streaming: None });
        while self.log.len() > 300 {
            self.log.pop_front();
        }
        self.dirty = true;
    }

    fn held(&self, c: char) -> bool {
        if self.enhanced {
            self.keys.down.contains(&c)
        } else {
            self.keys.until.get(&c).is_some_and(|t| *t > Instant::now())
        }
    }

    fn camera(&self) -> Camera {
        Camera { pos: Vec3::new(self.pos.x, self.cam_y, self.pos.z), yaw: self.yaw, pitch: self.pitch, fov_y: FOV_Y }
    }

    /// Viewport size in cells.
    fn layout(&self) -> (u16, u16, u16) {
        let h = self.screen.h;
        let log_rows: u16 = if h >= 44 { 6 } else if h >= 30 { 5 } else if h >= 22 { 4 } else { 3 };
        let ui = log_rows + 3; // separator, log, input, status
        let vh = h.saturating_sub(ui).max(4);
        (self.screen.w, vh, log_rows)
    }

    fn pixel_size(&self) -> (u32, u32, f32) {
        let (w, vh, _) = self.layout();
        let (mut pw, mut ph, aspect) = if self.ascii { (w as u32, vh as u32, 0.5) } else { (w as u32, vh as u32 * 2, 1.0) };
        if self.render.cpu_fallback {
            pw = pw.div_ceil(2);
            ph = ph.div_ceil(2);
        }
        (pw.max(8), ph.max(8), aspect)
    }

    // ------------------------------------------------------------------ input

    fn on_key(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('C')) {
            if k.kind != KeyEventKind::Release {
                self.quit = true;
            }
            return;
        }
        if k.kind == KeyEventKind::Release {
            if let Some(c) = key_char(k.code) {
                self.keys.down.remove(&c);
            }
            return;
        }
        if k.code == KeyCode::F(1) {
            if k.kind == KeyEventKind::Press {
                self.debug = !self.debug;
                self.dirty = true;
            }
            return;
        }
        match self.mode {
            Mode::Walk => self.walk_key(k),
            Mode::Talk(_) | Mode::Create => self.text_key(k),
        }
    }

    fn walk_key(&mut self, k: KeyEvent) {
        if let Some(c) = key_char(k.code) {
            if matches!(c, '^' | 'v' | '<' | '>' | 'a' | 'd' | 'w' | 's') {
                if self.enhanced {
                    self.keys.down.insert(c);
                } else {
                    self.keys.until.insert(c, Instant::now() + Duration::from_millis(140));
                }
                return;
            }
        }
        if k.kind == KeyEventKind::Repeat {
            return;
        }
        match k.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,
            KeyCode::Tab => {
                self.ascii = !self.ascii;
                self.screen.resize(self.screen.w, self.screen.h);
                self.last_frame = None;
                self.dirty = true;
            }
            KeyCode::Char('/') => {
                self.set_mode(Mode::Create);
                self.input = "/".into();
            }
            KeyCode::Enter => {
                if let Some((id, name)) = self.talk_hint.clone() {
                    if !self.brain.has_llm {
                        self.say(None, &format!("{name} looks at you, but words need an LLM key."), DIM);
                        return;
                    }
                    self.set_mode(Mode::Talk(id));
                    self.say(None, &format!("You approach {name}. (Esc to leave)"), DIM);
                }
            }
            KeyCode::PageUp => self.pitch = (self.pitch + 0.08).min(0.6),
            KeyCode::PageDown => self.pitch = (self.pitch - 0.08).max(-0.6),
            _ => {}
        }
    }

    fn set_mode(&mut self, m: Mode) {
        self.mode = m;
        self.keys.down.clear();
        self.keys.until.clear();
        self.input.clear();
        self.dirty = true;
    }

    fn text_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc => self.set_mode(Mode::Walk),
            KeyCode::Enter => {
                let text = std::mem::take(&mut self.input);
                match self.mode {
                    Mode::Talk(cid) => self.send_talk(cid, text),
                    Mode::Create => {
                        self.set_mode(Mode::Walk);
                        self.command(text.trim());
                    }
                    Mode::Walk => {}
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                if self.mode == Mode::Create && self.input.is_empty() {
                    self.set_mode(Mode::Walk);
                }
            }
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => self.input.clear(),
            KeyCode::Char(c) => {
                let c = if k.modifiers.contains(KeyModifiers::SHIFT) { c.to_uppercase().next().unwrap_or(c) } else { c };
                if self.input.chars().count() < 400 {
                    self.input.push(c);
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn command(&mut self, line: &str) {
        let body = line.trim_start_matches('/').trim();
        let (cmd, rest) = body.split_once(' ').map(|(a, b)| (a, b.trim())).unwrap_or((body, ""));
        match cmd.to_lowercase().as_str() {
            "" => {}
            "help" | "?" => {
                for l in [
                    "/a <thing> — create something where you are looking, e.g. /a lighthouse on that hill",
                    "/undo — remove the last thing you created   /history — list world versions",
                    "/day, /night, /time <hour 0–23> — jump the clock forward to that time",
                    "Walk: ↑↓ move, ←→ turn, A/D strafe, PgUp/PgDn look, Tab ascii/blocks, F1 stats, q quit",
                    "Talk: walk up to someone and press Enter; Esc to leave.",
                ] {
                    self.say(None, l, DIM);
                }
            }
            "undo" => self.brain.send(Cmd::Undo),
            "history" => self.brain.send(Cmd::History),
            "day" | "noon" => self.set_hour(12.0),
            "night" | "midnight" => self.set_hour(0.0),
            "morning" | "dawn" => self.set_hour(7.0),
            "dusk" | "evening" => self.set_hour(18.5),
            "time" => match rest.parse::<f64>() {
                Ok(h) if (0.0..24.0).contains(&h) => self.set_hour(h),
                _ => self.say(None, "Usage: /time 12  (an hour from 0 to 23)", DIM),
            },
            "a" | "add" | "create" | "build" | "make" => self.create(rest, line),
            _ => self.create(body, line),
        }
    }

    /// Jump the clock forward to the next occurrence of `hour` (never backwards).
    fn set_hour(&mut self, hour: f64) {
        let day = sky::DAY_SECONDS;
        let mut t = (self.t_game / day).floor() * day + hour / 24.0 * day;
        if t <= self.t_game {
            t += day;
        }
        self.t_game = t;
        self.say(None, &format!("Time jumps to {:02}:{:02} ({}).", hour as u32, ((hour.fract()) * 60.0).round() as u32, sky::time_label(t)), DIM);
    }

    fn create(&mut self, text: &str, typed: &str) {
        if text.is_empty() {
            self.say(None, "Describe what to create: /a stone well by the path", DIM);
            return;
        }
        if !self.brain.has_llm {
            self.say(None, "Creating needs an LLM (set ANTHROPIC_API_KEY or POCKET_LLM_BASE_URL).", DIM);
            return;
        }
        if self.over_budget() {
            return;
        }
        let cam = self.camera();
        let (pw, ph, pa) = self.pixel_size();
        let aspect = pw as f32 / ph as f32 * pa;
        let npcs = self.npc_views();
        let view = describe(&self.snap, &mut self.cache, &npcs, &cam, aspect, self.t_game);
        let f = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let target = match &view.target {
            Some(t) if t.distance > 3.0 => Vec3::new(t.x, t.y, t.z),
            Some(_) | None => {
                let p = self.pos + f * 12.0;
                Vec3::new(p.x, self.snap.terrain.height(p.x, p.z), p.z)
            }
        };
        self.say(Some("you"), typed, ACCENT);
        self.recent_actions.push_back(format!("created \"{text}\""));
        if self.recent_actions.len() > 4 {
            self.recent_actions.pop_front();
        }
        self.brain.send(Cmd::Create { text: text.to_string(), view, target, yaw: self.yaw });
    }

    fn npc_views(&self) -> Vec<NpcView> {
        self.cast.npcs.iter().map(|n| NpcView { id: n.def.id, name: n.name().to_string(), pos: n.pos }).collect()
    }

    fn talk_context(&mut self) -> String {
        let cam = self.camera();
        let npcs = self.npc_views();
        let view = describe(&self.snap, &mut self.cache, &npcs, &cam, 1.6, self.t_game);
        let seen: Vec<String> = view.visible.iter().take(10).map(|s| format!("{} ({:.0} m {})", s.name, s.distance, s.third)).collect();
        let place = view.region.clone().unwrap_or_else(|| view.biome.clone());
        format!(
            "It is {} in {}. Visible around you: {}. Recently the traveller {}.",
            view.time,
            place,
            if seen.is_empty() { "open land".into() } else { seen.join(", ") },
            if self.recent_actions.is_empty() { "just arrived".into() } else { self.recent_actions.iter().cloned().collect::<Vec<_>>().join("; ") }
        )
    }

    fn send_talk(&mut self, cid: i64, text: String) {
        let text = text.trim().to_string();
        if text.is_empty() || self.replying.is_some() || self.over_budget() {
            return;
        }
        self.say(Some("you"), &text, ACCENT);
        let context = self.talk_context();
        let history = self.conv.get(&cid).cloned().unwrap_or_default();
        self.conv.entry(cid).or_default().push((true, text.clone()));
        self.replying = Some(cid);
        self.brain.send(Cmd::Talk { cid, text, context, history });
    }

    // ------------------------------------------------------------------ events

    fn on_event(&mut self, ev: Event) {
        match ev {
            Event::Flip { snap, region } => self.flip(snap, region),
            Event::Log(s) => {
                if s.starts_with("Budget reached") {
                    if self.budget_paused {
                        return;
                    }
                    self.budget_paused = true;
                }
                self.say(None, &s, TEXT)
            }
            Event::Building(d) => {
                self.building = (self.building + d).max(0);
                self.dirty = true;
            }
            Event::RegionStarted(r) => {
                self.regions_inflight.insert(r);
            }
            Event::RegionFinished(r, ok) => {
                self.regions_inflight.remove(&r);
                if !ok {
                    self.regions_failed.insert(r);
                }
            }
            Event::Token { cid, text } => {
                let name = self.cast.get(cid).map(|n| n.name().to_string()).unwrap_or_else(|| "?".into());
                match self.log.back_mut() {
                    Some(l) if l.streaming == Some(cid) => l.text.push_str(&text),
                    _ => self.log.push_back(LogLine { speaker: Some(name), text: text.trim_start().to_string(), color: TEXT, streaming: Some(cid) }),
                }
                self.dirty = true;
            }
            Event::ReplyDone { cid, ok } => {
                if let Some(l) = self.log.iter_mut().rev().find(|l| l.streaming == Some(cid)) {
                    l.streaming = None;
                    if ok {
                        let reply = l.text.clone();
                        self.conv.entry(cid).or_default().push((false, reply));
                    }
                }
                if !ok {
                    // Keep the conversation history alternating.
                    if let Some(h) = self.conv.get_mut(&cid) {
                        h.pop();
                    }
                }
                self.replying = None;
                self.dirty = true;
            }
            Event::Decision { cid, decision } => {
                let now = self.start.elapsed().as_secs_f64();
                self.cast.apply(cid, &decision, now);
            }
            Event::GenesisDone => {
                self.genesis_pending = false;
            }
            Event::History(lines) => {
                self.say(None, "World history (newest first):", DIM);
                for l in lines.iter().take(12) {
                    self.say(None, l, DIM);
                }
            }
        }
    }

    fn flip(&mut self, snap: Arc<WorldSnapshot>, region: Option<((i32, i32), String)>) {
        let old: HashSet<i64> = self.snap.instances.iter().map(|p| p.id).collect();
        let now = Instant::now();
        let mut new_near: Vec<(String, Vec3)> = Vec::new();
        for p in &snap.instances {
            if !old.contains(&p.id) {
                self.appear.insert(p.id, now);
                if (p.pos - self.pos).length() < 70.0 {
                    if let Some(t) = snap.type_of(p.type_id) {
                        new_near.push((t.name().to_string(), p.pos));
                    }
                }
            }
        }
        // Characters witness new things nearby.
        for (name, at) in &new_near {
            for n in &self.cast.npcs {
                if (n.pos - *at).length() < 60.0 {
                    let dir = compass(*at - n.pos);
                    self.brain.send(Cmd::Witness { cid: n.def.id, text: format!("A {name} appeared {dir} of me, out of nowhere, after the traveller arrived."), importance: 0.6 });
                    if self.brain.has_llm {
                        self.brain.send(Cmd::Decide { cid: n.def.id, event: "new_building".into(), context: format!("A {name} just appeared {dir} of you.") });
                    }
                }
            }
        }
        if let Some((r, name)) = region {
            let pr = region_of(self.pos.x, self.pos.z);
            if (r.0 - pr.0).abs() <= 1 && (r.1 - pr.1).abs() <= 1 && !name.is_empty() {
                self.say(None, &format!("The fog lifts over {name}."), [170, 200, 255]);
            }
        }
        let terrain_changed = render::terrain_epoch(&snap.terrain) != render::terrain_epoch(&self.snap.terrain);
        self.snap = snap;
        self.cast.sync(&self.snap);
        self.unstick();
        if terrain_changed {
            // The land reshaped itself; don't leave the player under water.
            let h = self.snap.terrain.height(self.pos.x, self.pos.z);
            if h < crate::terrain::WATER_LEVEL + 0.3 {
                self.pos = self.snap.spawn;
            }
            self.pos.y = self.snap.terrain.height(self.pos.x, self.pos.z);
            self.cam_y = self.pos.y + EYE;
        }
        self.dirty = true;
    }

    // ------------------------------------------------------------------ sim

    fn tick(&mut self, dt: f32) {
        let now_s = self.start.elapsed().as_secs_f64();
        self.t_game += dt as f64;
        let night = sky::is_night(self.t_game);
        if self.mode == Mode::Walk || self.noclip {
            let fwd = (self.held('^') || self.held('w')) as i32 - (self.held('v') || self.held('s')) as i32;
            let turn = self.held('>') as i32 - self.held('<') as i32;
            let strafe = self.held('d') as i32 - self.held('a') as i32;
            self.yaw += turn as f32 * TURN_SPEED * dt;
            let f = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
            let r = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
            let mut dir = f * fwd as f32 + r * strafe as f32;
            if dir.length() > 1.0 {
                dir = dir.normalize();
            }
            if dir != Vec3::ZERO {
                let delta = dir * WALK_SPEED * dt;
                if self.noclip {
                    let p = self.pos + delta;
                    self.pos = Vec3::new(p.x, self.snap.terrain.height(p.x, p.z), p.z);
                } else {
                    let solids = self.cache.solids_near(&self.snap, self.pos, 4.0);
                    let bodies: Vec<Vec3> = self.cast.npcs.iter().map(|n| n.pos).filter(|p| (*p - self.pos).length() < 4.0).collect();
                    let obs = Obstacles { solids: &solids, bodies: &bodies };
                    self.pos = move_body(&self.snap.terrain, &obs, self.pos, delta, PLAYER_RADIUS);
                }
            }
        }
        self.pos.y = self.snap.terrain.height(self.pos.x, self.pos.z);
        let eye = self.pos.y.max(crate::terrain::WATER_LEVEL - 0.4) + EYE;
        self.cam_y += (eye - self.cam_y) * (dt * 10.0).min(1.0);

        // Characters.
        let talking = if let Mode::Talk(id) = self.mode { Some(id) } else { None };
        let mut events = Vec::new();
        {
            let snap = self.snap.clone();
            let cache = &mut self.cache;
            let mut solids_for = |p: Vec3| cache.solids_near(&snap, p, 3.0);
            self.cast.tick(dt, now_s, night, self.pos, talking, &snap.terrain, &mut solids_for, &mut events, &mut self.near_flags);
        }
        for e in events {
            match e {
                CastEvent::Says { id, line } => {
                    let name = self.cast.get(id).map(|n| n.name().to_string()).unwrap_or_default();
                    self.say(Some(&name), &line, TEXT);
                    self.conv.entry(id).or_default().push((false, line.clone()));
                    self.brain.send(Cmd::Witness { cid: id, text: format!("I went up to the traveller and said: \"{line}\""), importance: 0.4 });
                }
                CastEvent::PlayerNear { id } => {
                    if self.brain.has_llm && !self.budget_paused && talking.is_none() {
                        let ctx = format!("The traveller has come within a few metres of you. It is {}.", sky::time_label(self.t_game));
                        self.brain.send(Cmd::Decide { cid: id, event: "player_near".into(), context: ctx });
                    }
                }
            }
        }
        // Leave talk mode if they walked off.
        if let Mode::Talk(id) = self.mode {
            if self.cast.get(id).is_none_or(|n| (n.pos - self.pos).length() > TALK_RANGE * 3.0) {
                self.set_mode(Mode::Walk);
            }
        }
        // Who could we talk to?
        let f = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let mut best: Option<(f32, i64, String)> = None;
        for n in &self.cast.npcs {
            let d = n.pos - self.pos;
            let dist = Vec3::new(d.x, 0.0, d.z).length();
            if dist > TALK_RANGE || n.mode == NpcMode::Sleep && dist > 2.0 {
                continue;
            }
            let ang = (Vec3::new(d.x, 0.0, d.z).normalize_or_zero().dot(f)).clamp(-1.0, 1.0).acos();
            if ang < 0.75 && best.as_ref().is_none_or(|b| dist < b.0) {
                best = Some((dist, n.def.id, n.name().to_string()));
            }
        }
        let hint = best.map(|b| (b.1, b.2));
        if hint != self.talk_hint {
            self.talk_hint = hint;
            self.dirty = true;
        }
        {
            let mut l = self.live.lock();
            l.player = self.pos;
            l.characters = self.cast.npcs.iter().map(|n| n.pos).collect();
        }
        if self.last_save.elapsed() > Duration::from_secs(3) {
            self.save_player();
        }
        if self.last_char_save.elapsed() > Duration::from_secs(12) {
            self.save_characters();
        }
        if self.last_region_check.elapsed() > Duration::from_millis(500) {
            self.last_region_check = Instant::now();
            self.schedule_regions();
            self.cache.trim(self.pos, render::VIEW_DIST + 80.0);
        }
    }

    pub fn save_player(&mut self) {
        self.last_save = Instant::now();
        let _ = self.db.save_player(PlayerRow { x: self.pos.x, z: self.pos.z, yaw: self.yaw, t_game: self.t_game });
    }

    pub fn save_characters(&mut self) {
        self.last_char_save = Instant::now();
        let states: Vec<(i64, String)> = self.cast.npcs.iter().map(|n| (n.def.id, serde_json::to_string(&n.saved()).unwrap_or_default())).collect();
        if !states.is_empty() {
            let _ = self.db.save_character_states(&states);
        }
    }

    /// Queue story plans for unplanned regions within 2 regions, nearest and
    /// most in the direction of travel first.
    fn schedule_regions(&mut self) {
        if !self.brain.has_llm || self.budget_paused || self.genesis_pending || self.regions_inflight.len() >= 2 {
            return;
        }
        let pr = region_of(self.pos.x, self.pos.z);
        let f = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let mut best: Option<(f32, (i32, i32))> = None;
        let rr: i32 = std::env::var("POCKET_REGION_RADIUS").ok().and_then(|v| v.parse().ok()).unwrap_or(2).clamp(0, 4);
        for dz in -rr..=rr {
            for dx in -rr..=rr {
                let r = (pr.0 + dx, pr.1 + dz);
                if self.snap.regions.contains_key(&r) || self.regions_inflight.contains(&r) || self.regions_failed.contains(&r) {
                    continue;
                }
                let c = region_center(r);
                let to = Vec3::new(c.x - self.pos.x, 0.0, c.z - self.pos.z);
                let d = to.length();
                let score = d - to.normalize_or_zero().dot(f) * crate::world::REGION * 0.6;
                if best.is_none_or(|b| score < b.0) {
                    best = Some((score, r));
                }
            }
        }
        if let Some((_, r)) = best {
            self.regions_inflight.insert(r);
            self.brain.send(Cmd::Region(r));
        }
    }

    // ------------------------------------------------------------------ render

    fn request_frame(&mut self) {
        let (pw, ph, pa) = self.pixel_size();
        let cam = self.camera();
        let aspect = pw as f32 / ph as f32 * pa;
        let now = Instant::now();
        let mut fade = HashMap::new();
        self.appear.retain(|id, t| {
            let f = now.duration_since(*t).as_secs_f32() / 1.0;
            if f < 1.0 {
                fade.insert(*id, f.max(0.001));
                true
            } else {
                false
            }
        });
        let figure = self.snap.figure_type.and_then(|f| self.snap.type_of(f)).cloned();
        let dynamic = self.cast.instances(figure.as_ref(), self.pos);
        let culled = cull::cull(&self.snap, &mut self.cache, &cam, aspect, &dynamic, &fade);
        self.stats.insts = culled.insts.len();
        let light = sky::lighting(self.t_game, &self.snap.look.palette);
        let sp = SceneParams {
            terrain: &self.snap.terrain,
            palette: &self.snap.look.palette,
            camera: cam,
            width: pw,
            height: ph,
            pixel_aspect: pa,
            light,
            time: self.start.elapsed().as_secs_f32(),
            frame: self.next_frame_id as u32,
            shadows: !self.render.cpu_fallback && std::env::var("POCKET_NO_SHADOWS").is_err(),
        };
        let globals = render::build_globals(&sp, culled.insts.len(), culled.grid.as_ref());
        let req = FrameRequest {
            id: self.next_frame_id,
            width: pw,
            height: ph,
            globals,
            instances: culled.insts,
            grid: culled.grid,
            scene: self.snap.scene.clone(),
            terrain: self.snap.terrain.clone(),
            look: self.snap.look.clone(),
        };
        self.next_frame_id += 1;
        if self.render.tx.send(RenderMsg::Frame(Box::new(req))).is_ok() {
            self.outstanding += 1;
            self.last_request = now;
        }
    }

    fn compose(&mut self) {
        let (w, vh, log_rows) = self.layout();
        // Viewport.
        if let Some(f) = &self.last_frame {
            frame_to_cells(f, &mut self.screen, w, vh, self.ascii);
        } else {
            for y in 0..vh {
                self.screen.fill_row(y, Cell { bg: [0, 0, 0], ..Cell::BLANK });
            }
        }
        self.draw_labels(w, vh);
        if self.debug {
            self.draw_debug();
        }
        // Separator with mode.
        let sep = vh;
        self.screen.fill_row(sep, Cell { ch: '─', fg: [60, 64, 80], bg: PANEL_BG, bold: false });
        let tag = match self.mode {
            Mode::Walk => " walk ".to_string(),
            Mode::Talk(id) => format!(" talking to {} ", self.cast.get(id).map(|n| n.name()).unwrap_or("?")),
            Mode::Create => " create ".to_string(),
        };
        self.screen.text(2, sep, &tag, ACCENT, PANEL_BG, true);
        // Log.
        let width = w as usize - 2;
        let mut lines: Vec<(Option<String>, String, [u8; 3], bool)> = Vec::new();
        for l in self.log.iter().rev() {
            let prefix = l.speaker.as_ref().map(|s| format!("{s}: ")).unwrap_or_default();
            let full = format!("{prefix}{}", l.text);
            let wrapped = term::wrap(&full, width);
            for (i, wl) in wrapped.into_iter().enumerate().rev() {
                lines.push((if i == 0 { l.speaker.clone() } else { None }, wl, l.color, l.streaming.is_some()));
            }
            if lines.len() >= log_rows as usize {
                break;
            }
        }
        lines.truncate(log_rows as usize);
        lines.reverse();
        for i in 0..log_rows {
            let y = sep + 1 + i;
            self.screen.fill_row(y, Cell { bg: PANEL_BG, ..Cell::BLANK });
            if let Some((speaker, text, color, _)) = lines.get(i as usize) {
                match speaker {
                    Some(s) if text.starts_with(&format!("{s}: ")) => {
                        let sc = if s == "you" { ACCENT } else { speaker_color(s) };
                        let x = self.screen.text(1, y, &format!("{s}:"), sc, PANEL_BG, true);
                        self.screen.text(x, y, &text[s.len() + 1..], *color, PANEL_BG, false);
                    }
                    _ => {
                        self.screen.text(1, y, text, *color, PANEL_BG, false);
                    }
                }
            }
        }
        // Input line.
        let iy = sep + 1 + log_rows;
        self.screen.fill_row(iy, Cell { bg: PANEL_BG, ..Cell::BLANK });
        match self.mode {
            Mode::Walk => {
                let hint = match &self.talk_hint {
                    Some((_, n)) => format!("Enter: talk to {n}   / create   Tab style   q quit"),
                    None => "↑↓ walk  ←→ turn  A/D strafe  / create  Tab style  F1 stats  q quit".into(),
                };
                self.screen.text(1, iy, &hint, DIM, PANEL_BG, false);
            }
            _ => {
                let prompt = if self.mode == Mode::Create { "" } else { "> " };
                let shown: String = {
                    let full = format!("{prompt}{}", self.input);
                    let n = full.chars().count();
                    let max = w as usize - 3;
                    if n > max { full.chars().skip(n - max).collect() } else { full }
                };
                let x = self.screen.text(1, iy, &shown, TEXT, PANEL_BG, false);
                let blink = (self.start.elapsed().as_millis() / 500) % 2 == 0;
                self.screen.set(x, iy, Cell { ch: if blink { '▏' } else { ' ' }, fg: ACCENT, bg: PANEL_BG, bold: false });
                if self.replying.is_some() {
                    self.screen.text(w.saturating_sub(12), iy, "(listening)", DIM, PANEL_BG, false);
                }
            }
        }
        // Status bar.
        let sy = iy + 1;
        self.screen.fill_row(sy, Cell { bg: STATUS_BG, ..Cell::BLANK });
        let pr = region_of(self.pos.x, self.pos.z);
        let place = self.snap.region_name(pr).map(str::to_string).unwrap_or_else(|| {
            let b = self.snap.terrain.biome_at(self.pos.x, self.pos.z);
            let n = self.snap.terrain.biomes[b].name.clone();
            let mut c = n.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        });
        let mut parts = vec![place, sky::time_label(self.t_game).to_string()];
        if self.building > 0 {
            parts.push(format!("◌ building {}", self.building));
        }
        let spent = self.spent();
        if self.brain.has_llm {
            parts.push(format!("${spent:.2}"));
        }
        let left = format!(" {}", parts.join(" · "));
        self.screen.text(0, sy, &left, TEXT, STATUS_BG, false);
        let mut right = String::new();
        if let Some(n) = &self.llm_note {
            right = n.clone();
        }
        if self.budget_paused {
            right = "budget reached: generation paused".into();
        }
        if self.render.cpu_fallback {
            right = format!("{right}{}CPU renderer", if right.is_empty() { "" } else { " · " });
        }
        if let (Mode::Walk, Some((_, n))) = (self.mode, &self.talk_hint) {
            right = format!("Enter: talk to {n}");
        }
        let rx = w.saturating_sub(right.chars().count() as u16 + 1);
        if !right.is_empty() {
            self.screen.text(rx, sy, &right, ACCENT, STATUS_BG, false);
        }
    }

    /// True (and says so) when POCKET_BUDGET_USD has been reached.
    fn over_budget(&mut self) -> bool {
        if self.llm.as_ref().is_some_and(|l| l.over_budget()) {
            self.budget_paused = true;
            self.say(None, "Budget reached (POCKET_BUDGET_USD): generation and dialogue are paused; walking still works.", ACCENT);
            return true;
        }
        false
    }

    fn spent(&self) -> f64 {
        self.llm.as_ref().map(|l| *l.spent.lock()).unwrap_or(0.0)
    }

    fn draw_labels(&mut self, w: u16, vh: u16) {
        let cam = self.camera();
        let (f, r, u) = cam.basis();
        let tan = (cam.fov_y * 0.5).tan();
        let (pw, ph, pa) = self.pixel_size();
        let aspect = pw as f32 / ph as f32 * pa;
        let mut labels = Vec::new();
        for n in &self.cast.npcs {
            let head = n.pos + Vec3::Y * (n.def.persona.look.height + 0.35);
            let v = head - cam.pos;
            let d = v.length();
            if d > 10.0 {
                continue;
            }
            let z = v.dot(f);
            if z < 0.3 {
                continue;
            }
            let sx = v.dot(r) / z / (tan * aspect);
            let sy = v.dot(u) / z / tan;
            if sx.abs() > 1.0 || sy.abs() > 1.0 {
                continue;
            }
            let col = ((sx + 1.0) * 0.5 * w as f32) as i32;
            let row = ((1.0 - sy) * 0.5 * vh as f32) as i32 - 1;
            let name = if n.mode == NpcMode::Sleep { format!("{} (asleep)", n.name()) } else { n.name().to_string() };
            labels.push((col, row, name));
        }
        for (col, row, name) in labels {
            if row < 0 || row >= vh as i32 {
                continue;
            }
            let len = name.chars().count() as i32;
            let x0 = (col - len / 2).clamp(0, (w as i32 - len).max(0));
            for (i, ch) in name.chars().enumerate() {
                let x = x0 as u16 + i as u16;
                if let Some(c) = self.screen.get(x, row as u16) {
                    let bg = [(c.bg[0] as u16 / 3) as u8, (c.bg[1] as u16 / 3) as u8, (c.bg[2] as u16 / 3) as u8];
                    self.screen.set(x, row as u16, Cell { ch: term::narrow(ch), fg: [255, 255, 255], bg, bold: true });
                }
            }
        }
    }

    fn draw_debug(&mut self) {
        let lines = vec![
            format!("fps {:5.1}   worst loop {:4.1} ms   worst gap {:4.1} ms", self.stats.fps(), self.stats.worst(), self.stats.max_gap_ms),
            format!("gpu {:5.2} ms   {}", self.stats.gpu_ms, self.render.backend),
            format!("instances {}   out {} KB/frame", self.stats.insts, self.screen.bytes_last / 1024),
            format!("pos {:.0}, {:.0}  yaw {:.0}°  v{}", self.pos.x, self.pos.z, self.yaw.to_degrees().rem_euclid(360.0), self.snap.version),
            format!("{} keys{}", if self.enhanced { "enhanced" } else { "legacy" }, if self.screen.truecolor { "  truecolor" } else { "  256 colours" }),
        ];
        for (i, l) in lines.iter().enumerate() {
            let s = format!(" {l:<44}");
            self.screen.text(0, i as u16, &s, [180, 255, 180], [0, 0, 0], false);
        }
    }

    // ------------------------------------------------------------------ loop

    pub fn run(&mut self, input: bool, out: &mut dyn Write, stop: &AtomicBool, mut script: Option<&mut dyn FnMut(&mut App, f32) -> bool>) -> std::io::Result<()> {
        let mut last = Instant::now();
        let frame_interval = Duration::from_secs_f32(1.0 / self.target_fps);
        // Test hook: prove the terminal survives a panic.
        let panic_at = std::env::var("POCKET_TEST_PANIC_MS").ok().and_then(|v| v.parse::<u64>().ok()).map(|ms| Instant::now() + Duration::from_millis(ms));
        loop {
            let loop_start = Instant::now();
            if panic_at.is_some_and(|t| loop_start > t) {
                panic!("test panic (POCKET_TEST_PANIC_MS)");
            }
            if stop.load(Ordering::Relaxed) {
                self.quit = true;
            }
            // Input (non-blocking).
            if input {
                while event::poll(Duration::ZERO)? {
                    match event::read()? {
                        TEvent::Key(k) => self.on_key(k),
                        TEvent::Resize(w, h) => {
                            self.screen.resize(w, h);
                            self.last_frame = None;
                            self.dirty = true;
                        }
                        TEvent::Paste(s) => {
                            if self.mode != Mode::Walk {
                                self.input.push_str(&s.replace(['\n', '\r'], " "));
                                self.dirty = true;
                            }
                        }
                        TEvent::FocusLost => {
                            self.keys.down.clear();
                        }
                        _ => {}
                    }
                }
            }
            if self.quit {
                break;
            }
            while let Ok(ev) = self.events.try_recv() {
                self.on_event(ev);
            }
            let now = Instant::now();
            let dt = now.duration_since(last).as_secs_f32().min(0.1);
            last = now;
            if let Some(s) = script.as_mut() {
                if !s(self, dt) {
                    break;
                }
            }
            self.tick(dt);

            // Frames in flight: at most two; keep only the newest result.
            let mut got = false;
            if self.render.rx.is_empty() && self.render.thread.as_ref().is_some_and(|t| t.is_finished()) {
                // The renderer died (e.g. a GPU fault): keep going on the CPU.
                crate::log::error("renderer thread stopped; switching to the CPU renderer");
                self.render = crate::render::cpu::spawn();
                self.outstanding = 0;
                self.say(None, "The GPU renderer stopped; continuing with the CPU renderer.", ACCENT);
            }
            while let Ok(f) = self.render.rx.try_recv() {
                self.outstanding = self.outstanding.saturating_sub(1);
                if f.width == 0 {
                    continue;
                }
                if let Some(g) = f.gpu_ms {
                    self.stats.gpu_ms = self.stats.gpu_ms * 0.8 + g * 0.2;
                }
                self.last_frame = Some(f);
                got = true;
            }
            if self.outstanding < 2 && now.duration_since(self.last_request) >= frame_interval.mul_f32(0.9) {
                self.request_frame();
            }
            if got || self.dirty {
                self.compose();
                self.screen.flush(out)?;
                self.dirty = false;
                if got {
                    self.stats.shown += 1;
                    if let Some(prev) = self.stats.frames.back() {
                        self.stats.max_gap_ms = self.stats.max_gap_ms.max(prev.elapsed().as_secs_f32() * 1000.0);
                    }
                    self.stats.frames.push_back(Instant::now());
                    while self.stats.frames.len() > 90 {
                        self.stats.frames.pop_front();
                    }
                }
            }
            let ms = loop_start.elapsed().as_secs_f32() * 1000.0;
            self.stats.max_loop_ms = self.stats.max_loop_ms.max(ms);
            self.stats.worst_recent.push_back((Instant::now(), ms));
            while self.stats.worst_recent.front().is_some_and(|x| x.0.elapsed() > Duration::from_secs(2)) {
                self.stats.worst_recent.pop_front();
            }
            // Wait for input or the next frame slot (whichever first).
            let spent = loop_start.elapsed();
            let budget = Duration::from_millis(4).max(frame_interval.saturating_sub(spent) / 2);
            if input {
                let _ = event::poll(budget);
            } else {
                std::thread::sleep(budget.min(Duration::from_millis(2)));
            }
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        self.save_player();
        self.save_characters();
        self.render.shutdown();
    }
}

fn compass(d: Vec3) -> &'static str {
    let a = d.x.atan2(d.z).to_degrees().rem_euclid(360.0);
    ["north", "north-east", "east", "south-east", "south", "south-west", "west", "north-west"][((a + 22.5) / 45.0) as usize % 8]
}

fn speaker_color(name: &str) -> [u8; 3] {
    let h = crate::noise::pcg(name.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)));
    let hue = crate::noise::u2f(h);
    let c = crate::lang::api::Api::Hsv;
    let mut out = [0.0; 3];
    crate::lang::api::eval(c, &[hue, 0.45, 1.0], &mut out);
    [(out[0] * 255.0) as u8, (out[1] * 255.0) as u8, (out[2] * 255.0) as u8]
}

const RAMP: &[u8] = b" .:-=+*#%@";

/// Monochrome text with per-frame auto-contrast (no colour to carry the image).
fn frame_to_mono(f: &Frame, ascii: bool, w: u16, vh: u16) -> String {
    let lums: Vec<f32> = f.pixels.iter().map(|p| lum(render::unpack(*p))).collect();
    let mut sorted = lums.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let lo = sorted[sorted.len() / 50];
    let hi = sorted[sorted.len() * 49 / 50].max(lo + 0.05);
    let norm = |l: f32| ((l - lo) / (hi - lo)).clamp(0.0, 1.0);
    let at = |x: u32, y: u32| lums[(y.min(f.height - 1) * f.width + x.min(f.width - 1)) as usize];
    let shades = [' ', '░', '▒', '▓', '█'];
    let mut out = String::new();
    for y in 0..vh as u32 {
        for x in 0..w as u32 {
            if ascii {
                let l = norm(at(x, y));
                out.push(RAMP[((l * (RAMP.len() - 1) as f32).round() as usize).min(RAMP.len() - 1)] as char);
            } else {
                let l = (norm(at(x, y * 2)) + norm(at(x, y * 2 + 1))) * 0.5;
                out.push(shades[((l * 4.0).round() as usize).min(4)]);
            }
        }
        out.push('\n');
    }
    out
}

fn lum(c: [u8; 3]) -> f32 {
    (0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32) / 255.0
}

/// Convert a rendered frame to cells (half-blocks or coloured ASCII), scaling
/// with nearest-neighbour if the frame size differs (CPU fallback).
pub fn frame_to_cells(f: &Frame, screen: &mut Screen, w: u16, vh: u16, ascii: bool) {
    let rows_px = if ascii { vh as u32 } else { vh as u32 * 2 };
    let px = |x: u32, y: u32| -> [u8; 3] {
        let sx = (x * f.width / w as u32).min(f.width - 1);
        let sy = (y * f.height / rows_px).min(f.height - 1);
        render::unpack(f.pixels[(sy * f.width + sx) as usize])
    };
    for y in 0..vh {
        for x in 0..w {
            let cell = if ascii {
                let c = px(x as u32, y as u32);
                let l = lum(c);
                let ch = RAMP[((l * (RAMP.len() - 1) as f32 * 1.15).round() as usize).min(RAMP.len() - 1)] as char;
                let m = c[0].max(c[1]).max(c[2]).max(1) as f32;
                let k = (255.0 / m).min(3.0);
                let fg = [(c[0] as f32 * k).min(255.0) as u8, (c[1] as f32 * k).min(255.0) as u8, (c[2] as f32 * k).min(255.0) as u8];
                Cell { ch, fg, bg: [0, 0, 0], bold: false }
            } else {
                let top = px(x as u32, y as u32 * 2);
                let bot = px(x as u32, y as u32 * 2 + 1);
                if top == bot { Cell { ch: ' ', fg: top, bg: bot, bold: false } } else { Cell { ch: '▀', fg: top, bg: bot, bold: false } }
            };
            screen.set(x, y, cell);
        }
    }
}

/// Plain-text rendering for `pocket snapshot`.
pub fn frame_to_text(f: &Frame, ascii: bool, mono: bool, truecolor: bool) -> String {
    let w = f.width as u16;
    let vh = if ascii { f.height as u16 } else { (f.height / 2) as u16 };
    if mono {
        return frame_to_mono(f, ascii, w, vh);
    }
    let mut s = Screen::new(w, vh, truecolor);
    frame_to_cells(f, &mut s, w, vh, ascii);
    let mut out = String::new();
    for y in 0..vh {
        let mut pen: Option<([u8; 3], [u8; 3])> = None;
        for x in 0..w {
            let c = s.get(x, y).unwrap_or(Cell::BLANK);
            if mono {
                let ch = if ascii {
                    c.ch
                } else {
                    let l = (lum(c.fg) + lum(c.bg)) * 0.5;
                    [' ', '░', '▒', '▓', '█'][((l * 5.0) as usize).min(4)]
                };
                out.push(ch);
            } else {
                if pen != Some((c.fg, c.bg)) {
                    if truecolor {
                        out.push_str(&format!("\x1b[38;2;{};{};{};48;2;{};{};{}m", c.fg[0], c.fg[1], c.fg[2], c.bg[0], c.bg[1], c.bg[2]));
                    } else {
                        out.push_str(&format!("\x1b[38;5;{};48;5;{}m", term::ansi256(c.fg), term::ansi256(c.bg)));
                    }
                    pen = Some((c.fg, c.bg));
                }
                out.push(c.ch);
            }
        }
        if !mono {
            out.push_str("\x1b[0m");
        }
        out.push('\n');
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{Llm, Msg};
    use crate::model::{Live, WorldModel};
    use crossterm::event::{KeyEventState, KeyModifiers};
    use parking_lot::Mutex;

    fn key(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent { code, modifiers: KeyModifiers::NONE, kind, state: KeyEventState::NONE }
    }

    fn reply(sys: &str, msgs: &[Msg]) -> String {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        let lh = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/good/lighthouse.js")).unwrap();
        if sys.contains("You are a character") {
            return "You look like you fell from the sky.".into();
        }
        if user.contains("The player is creating") {
            return format!("```json\n{{\"summary\": \"a lighthouse\", \"reuse\": null, \"placements\": [{{\"right\": 0, \"forward\": 0}}]}}\n```\n```js\n{lh}\n```");
        }
        if sys.contains("You decide what a character") {
            return r#"{"action": "ignore", "line": null}"#.into();
        }
        "ok".into()
    }

    fn make_app(enhanced: bool) -> (App, Arc<Db>) {
        let (a, d, _) = make_app_gpu(enhanced);
        (a, d)
    }

    fn make_app_gpu(enhanced: bool) -> (App, Arc<Db>, Option<Arc<crate::render::gpu::Gpu>>) {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("pocket-app-{}-{enhanced}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.pocket");
        let db = Db::create(&path, 4242, "a quiet test valley", &serde_json::to_string(&crate::world::Look::default()).unwrap()).unwrap();
        db.kv_set("genesis", "done").unwrap();
        // One character, standing right next to the spawn.
        let gpu = crate::render::gpu::Gpu::new().ok();
        let live = Arc::new(Mutex::new(Live::default()));
        let mut model = WorldModel::load(db.clone(), gpu.clone(), live.clone()).unwrap();
        let spawn = model.snapshot().unwrap().spawn;
        let persona = crate::world::Persona { name: "Mara".into(), ..Default::default() };
        db.tx(|tx| {
            let v = crate::db::add_version(tx, None, "region", "test region", None)?;
            crate::db::add_character(tx, crate::world::region_of(spawn.x, spawn.z), &serde_json::to_string(&persona)?, spawn.x, spawn.z + 2.5, v)?;
            crate::db::set_region(tx, 0, 0, "done", "{\"name\": \"Test Vale\"}", Some(v))?;
            Ok(())
        })
        .unwrap();
        let mut model = WorldModel::load(db.clone(), gpu.clone(), live.clone()).unwrap();
        let snap = model.snapshot().unwrap();
        let llm = Llm::scripted(db.clone(), Arc::new(|s: &str, m: &[Msg]| reply(s, m)));
        let (tx, rx) = crossbeam_channel::unbounded();
        let brain = Brain::start(model, Some(llm.clone()), tx, false);
        let render = match &gpu {
            Some(g) => crate::render::gpu::spawn(g.clone()),
            None => crate::render::cpu::spawn(),
        };
        let app = App::new(Setup { db: db.clone(), brain, events: rx, render, snap, live, enhanced, truecolor: true, size: (100, 34), llm: Some(llm), genesis: false });
        (app, db, gpu)
    }

    /// Run the loop with `steps(app, elapsed_seconds) -> keep going`.
    fn drive(app: &mut App, secs: f32, mut steps: impl FnMut(&mut App, f32) -> bool) {
        let stop = AtomicBool::new(false);
        let mut t = 0.0f32;
        let mut sink = Vec::new();
        let mut f = |a: &mut App, dt: f32| {
            t += dt;
            t < secs && steps(a, t)
        };
        app.run(false, &mut sink, &stop, Some(&mut f)).unwrap();
        assert!(!sink.is_empty(), "something was drawn");
    }

    fn log_has(app: &App, s: &str) -> bool {
        app.log.iter().any(|l| l.text.contains(s) || l.speaker.as_deref().is_some_and(|sp| format!("{sp}: {}", l.text).contains(s)))
    }

    #[test]
    fn talk_create_undo_via_keys() {
        let (mut app, _db) = make_app(false);
        // Keep Mara where she is, in front of us.
        app.yaw = 0.0;
        let mut phase = 0;
        drive(&mut app, 40.0, |a, _t| {
            if let Some(n) = a.cast.npcs.first_mut() {
                if phase < 3 {
                    n.mode = NpcMode::Watch { until: f64::MAX };
                }
            }
            match phase {
                0 if a.talk_hint.is_some() => {
                    a.on_key(key(KeyCode::Enter, KeyEventKind::Press));
                    assert!(matches!(a.mode, Mode::Talk(_)));
                    for c in "hello there".chars() {
                        a.on_key(key(KeyCode::Char(c), KeyEventKind::Press));
                    }
                    a.on_key(key(KeyCode::Enter, KeyEventKind::Press));
                    phase = 1;
                }
                1 if a.replying.is_none() && log_has(a, "fell from the sky") => {
                    a.on_key(key(KeyCode::Esc, KeyEventKind::Press));
                    assert_eq!(a.mode, Mode::Walk);
                    a.on_key(key(KeyCode::Char('/'), KeyEventKind::Press));
                    assert_eq!(a.mode, Mode::Create);
                    for c in "a lighthouse on that hill".chars() {
                        a.on_key(key(KeyCode::Char(c), KeyEventKind::Press));
                    }
                    a.on_key(key(KeyCode::Enter, KeyEventKind::Press));
                    phase = 2;
                }
                2 if log_has(a, "Built the lighthouse") => {
                    assert!(a.snap.instances.iter().any(|p| a.snap.type_of(p.type_id).is_some_and(|t| t.name() == "lighthouse")));
                    for c in "/undo".chars() {
                        a.on_key(key(KeyCode::Char(c), KeyEventKind::Press));
                    }
                    a.on_key(key(KeyCode::Enter, KeyEventKind::Press));
                    phase = 3;
                }
                3 if log_has(a, "Undone") => {
                    assert!(a.snap.instances.is_empty());
                    phase = 4;
                    return false;
                }
                _ => {}
            }
            true
        });
        assert_eq!(phase, 4, "log: {:?}", app.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>());
        assert!(log_has(&app, "Mara: You look like you fell from the sky."));
        app.shutdown();
    }

    fn region_reply(sys: &str, msgs: &[Msg]) -> String {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if user.contains("Plan the story layer") {
            let r = user.split("Region (").nth(1).and_then(|s| s.split(')').next()).unwrap_or("0, 0").replace(", ", "_").replace('-', "m");
            return format!(
                "```json\n{{\"name\": \"Vale {r}\", \"mood\": \"quiet\", \"facts\": [], \"new_types\": [{{\"name\": \"hut {r}\", \"description\": \"hut\", \"size_m\": [6, 6, 5], \"tags\": [\"building\"]}}],
                 \"landmarks\": [{{\"type\": \"hut {r}\", \"x\": 128, \"z\": 128, \"rot\": 0}}], \"settlement\": null, \"characters\": [{{\"name\": \"Ola {r}\", \"home_x\": 120, \"home_z\": 120}}]}}\n```"
            );
        }
        if user.contains("Object type to write") {
            let name = user.split('"').nth(1).unwrap_or("hut");
            let cottage = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/mock/cottage.js")).unwrap();
            return format!("```js\n{}\n```", cottage.replace("keeper's cottage", name));
        }
        reply(sys, msgs)
    }

    /// Walk in a straight line while regions generate and flip in; no frame may
    /// take more than 50 ms. (Slow: run with --ignored.)
    #[test]
    #[ignore]
    fn long_walk_with_generation_has_no_hitches() {
        let (mut app, db, gpu) = make_app_gpu(false);
        let llm = Llm::scripted(db.clone(), Arc::new(|s: &str, m: &[Msg]| region_reply(s, m)));
        // Swap in a brain that plans regions (same GPU device as the renderer).
        let live = app.live.clone();
        let mut model = WorldModel::load(db.clone(), gpu, live).unwrap();
        app.snap = model.snapshot().unwrap();
        let (tx, rx) = crossbeam_channel::unbounded();
        app.brain = Brain::start(model, Some(llm.clone()), tx, false);
        app.events = rx;
        app.noclip = true;
        let distance: f32 = std::env::var("POCKET_WALK_M").ok().and_then(|v| v.parse().ok()).unwrap_or(1500.0);
        let start = app.pos;
        let mut warm = 0.0;
        drive(&mut app, 900.0, |a, t| {
            if t < 2.0 {
                return true;
            }
            if warm == 0.0 {
                a.stats.max_loop_ms = 0.0;
                a.stats.max_gap_ms = 0.0;
                warm = t;
            }
            a.keys.until.insert('^', Instant::now() + Duration::from_millis(200));
            let d = Vec3::new(a.pos.x - start.x, 0.0, a.pos.z - start.z).length();
            d < distance
        });
        let regions = app.snap.regions.len();
        eprintln!("walked {distance} m: worst loop {:.1} ms, worst gap between frames {:.1} ms, {} frames, {regions} regions generated", app.stats.max_loop_ms, app.stats.max_gap_ms, app.stats.shown);
        assert!(regions >= 4, "regions kept appearing");
        assert!(app.stats.max_loop_ms < 50.0, "worst loop {:.1} ms", app.stats.max_loop_ms);
        assert!(app.stats.max_gap_ms < 50.0, "worst frame gap {:.1} ms", app.stats.max_gap_ms);
        app.shutdown();
    }

    #[test]
    fn held_key_moves_at_constant_speed_and_stops_on_release() {
        let (mut app, _db) = make_app(true);
        app.noclip = true; // isolate key handling from collisions
        let start = app.pos;
        let mut pressed_at = None;
        let mut released_pos = None;
        drive(&mut app, 3.0, |a, t| {
            if pressed_at.is_none() && t > 0.3 {
                a.on_key(key(KeyCode::Up, KeyEventKind::Press));
                pressed_at = Some(t);
            }
            if let Some(p0) = pressed_at {
                if t - p0 >= 1.0 && released_pos.is_none() {
                    a.on_key(key(KeyCode::Up, KeyEventKind::Release));
                    released_pos = Some(a.pos);
                }
            }
            true
        });
        let rp = released_pos.unwrap();
        let moved = Vec3::new(rp.x - start.x, 0.0, rp.z - start.z).length();
        assert!((moved - WALK_SPEED).abs() < WALK_SPEED * 0.2, "1 s held moved {moved} m");
        let after = Vec3::new(app.pos.x - rp.x, 0.0, app.pos.z - rp.z).length();
        assert!(after < 0.15, "kept moving {after} m after release");
        app.shutdown();
    }

    #[test]
    fn legacy_keys_step_per_event() {
        let (mut app, _db) = make_app(false);
        app.noclip = true;
        let start = app.pos;
        let mut sent = 0;
        drive(&mut app, 1.5, |a, t| {
            if t > 0.2 && sent < 1 {
                a.on_key(key(KeyCode::Up, KeyEventKind::Press));
                sent += 1;
            }
            true
        });
        let moved = Vec3::new(app.pos.x - start.x, 0.0, app.pos.z - start.z).length();
        assert!(moved > 0.2 && moved < 1.6, "one press = one short step, moved {moved}");
        app.shutdown();
    }
}
