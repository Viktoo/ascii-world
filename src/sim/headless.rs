//! Running the world without a screen.
//!
//! - `pocket sim`: fast-forward a universe headless with a seed, writing every
//!   event to JSONL; `--replay` reads a log back, `--verify` runs twice and
//!   checks both histories match.
//! - `pocket act`: an agent acts as the player or any character, through the
//!   same actions the game uses.
//! - `pocket inspect`: the raw truth about anything, as JSON.
//!
//! The bridge between the simulation and the brain (`forward` / `apply`) is
//! shared with the game.

use super::{ActorId, Note, Request, Sim, SimEvent, Target};
use crate::brain::{Brain, Cmd, Event};
use crate::db::Db;
use crate::llm::Llm;
use crate::model::{Live, LiveRef, WorldModel};
use anyhow::{Context, Result, bail};
use glam::Vec3;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Send a simulation request to the brain. Returns true if an answer will come.
pub fn forward(sim: &Sim, brain: &Brain, req: Request) -> bool {
    match req {
        Request::Decide { cid, event, context } => {
            brain.send(Cmd::Decide { cid, event, context });
            true
        }
        Request::Witness { cid, text, importance } => {
            brain.send(Cmd::Witness { cid, text, importance });
            false
        }
        Request::Interpret { id, actor, text, context } => {
            brain.send(Cmd::Interpret { id, actor: sim.actor_name(actor), text, context });
            true
        }
        Request::Create { id, by, text, target, yaw, view } => {
            let by = match by {
                ActorId::Npc(c) => Some((c, sim.actor_name(by))),
                ActorId::Player => None,
            };
            brain.send(Cmd::Create { id: Some(id), by, text, view: *view, target, yaw });
            true
        }
        Request::Chat { a, b, context } => {
            brain.send(Cmd::Chat { a, b, context });
            true
        }
        Request::BuildType { id, name, description, size, props, fits } => {
            brain.send(Cmd::BuildType { id, name, description, size, props, fits });
            true
        }
        Request::Talk { cid, text, context } => {
            brain.send(Cmd::Talk { cid, text, context, history: vec![] });
            true
        }
        Request::BuildGesture { id, name, body } => {
            brain.send(Cmd::BuildGesture { id, name, body });
            true
        }
        Request::EditType { id, name, source, change, spot, cuts, with } => {
            brain.send(Cmd::EditType { id, name, source, change, spot, cuts, with });
            true
        }
        Request::NewSpecies { id, brief, fixed } => {
            brain.send(Cmd::NewSpecies { id, brief, fixed });
            true
        }
        Request::SpeciesBody { id, species, template } => {
            brain.send(Cmd::SpeciesBody { id, species, template });
            true
        }
        Request::ReshapeBody { id, name, from, source, change } => {
            brain.send(Cmd::ReshapeBody { id, name, from, source, change });
            true
        }
    }
}

/// Apply a brain event to the simulation. Returns true if it answers a request.
pub fn apply(sim: &mut Sim, ev: Event, talk: &mut HashMap<i64, String>) -> bool {
    match ev {
        Event::Flip { snap, .. } => {
            sim.flip(snap);
            false
        }
        Event::Decision { cid, decision } => {
            if let Some(d) = decision {
                sim.on_decision(cid, d);
            }
            true
        }
        Event::Interpreted { id, result } => {
            sim.on_interpreted(id, result);
            true
        }
        Event::Created { id, instances } => {
            if let Some(id) = id {
                sim.on_created(id, &instances);
            }
            id.is_some()
        }
        Event::ChatLines { a, b, lines } => {
            sim.on_chat(a, b, lines);
            true
        }
        Event::TypeBuilt { id, type_id } => {
            sim.on_type_built(id, type_id);
            true
        }
        Event::GestureBuilt { id, result } => {
            sim.on_gesture_built(id, result);
            true
        }
        Event::SpeciesMade { id, name } => {
            sim.on_species_made(id, name);
            true
        }
        Event::Token { cid, text } => {
            talk.entry(cid).or_default().push_str(&text);
            false
        }
        Event::ReplyDone { cid, ok } => {
            if let Some(line) = talk.remove(&cid) {
                if ok {
                    let line = line.trim().to_string();
                    sim.npc_said(cid, &line);
                    sim.notes.push(Note::Line { id: Some(ActorId::Npc(cid)), who: sim.actor_name(ActorId::Npc(cid)), text: line });
                }
            }
            true
        }
        Event::RulesChanged => {
            sim.load_universe_rules();
            false
        }
        Event::Log(s) => {
            sim.notes.push(Note::Info(s));
            false
        }
        _ => false,
    }
}

/// A simulation with its brain, outside the game.
pub struct Session {
    pub sim: Sim,
    pub brain: Brain,
    pub rx: crossbeam_channel::Receiver<Event>,
    pub live: LiveRef,
    /// Requests sent and not yet answered.
    pub outstanding: usize,
    /// Wait for every answer before the next step (reproducible with a scripted LLM).
    pub deterministic: bool,
    talk: HashMap<i64, String>,
}

impl Session {
    pub fn with_db(db: Arc<Db>, seed: Option<u64>, llm: Option<Arc<Llm>>) -> Result<Session> {
        let live = Arc::new(Mutex::new(Live::default()));
        let mut model = WorldModel::load(db.clone(), None, live.clone())?;
        let snap = model.snapshot().map_err(|d| anyhow::anyhow!(crate::lang::format_diags(&d)))?;
        let (pos, yaw, t) = match db.player() {
            Some(p) => (Vec3::new(p.x, snap.terrain.height(p.x, p.z), p.z), p.yaw, p.t_game),
            None => (snap.spawn, 0.0, 0.33 * crate::render::sky::DAY_SECONDS),
        };
        let seed = seed.unwrap_or(model.seed as u64);
        let has_llm = llm.is_some();
        let (tx, rx) = crossbeam_channel::unbounded();
        let brain = Brain::start(model, llm, tx, false);
        let mut sim = Sim::new(db, snap, pos, yaw, t, seed);
        sim.has_llm = has_llm;
        Ok(Session { sim, brain, rx, live, outstanding: 0, deterministic: true, talk: HashMap::new() })
    }

    /// Exchange requests and answers with the brain. In deterministic mode,
    /// wait (up to `timeout`) until every request is answered.
    pub fn pump(&mut self, timeout: Duration) {
        let end = Instant::now() + timeout;
        loop {
            for r in self.sim.drain_requests() {
                if forward(&self.sim, &self.brain, r) {
                    self.outstanding += 1;
                }
            }
            while let Ok(ev) = self.rx.try_recv() {
                if apply(&mut self.sim, ev, &mut self.talk) {
                    self.outstanding = self.outstanding.saturating_sub(1);
                }
            }
            if !self.deterministic || self.outstanding == 0 || Instant::now() > end {
                break;
            }
            if let Ok(ev) = self.rx.recv_timeout(Duration::from_millis(20)) {
                if apply(&mut self.sim, ev, &mut self.talk) {
                    self.outstanding = self.outstanding.saturating_sub(1);
                }
            }
        }
        // Answers may have produced new requests (a decision that makes something).
        for r in self.sim.drain_requests() {
            if forward(&self.sim, &self.brain, r) {
                self.outstanding += 1;
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.sim.step(dt);
        {
            let mut l = self.live.lock();
            l.player = self.sim.player.pos;
            l.characters = self.sim.cast.npcs.iter().map(|n| n.a.pos).collect();
            l.types = self.sim.types_in_use();
        }
        self.pump(Duration::from_secs(60));
    }

    /// Run `secs` of game time in steps of `dt`.
    pub fn run(&mut self, secs: f32, dt: f32) {
        let n = (secs / dt).ceil() as usize;
        for _ in 0..n {
            self.step(dt);
        }
    }

    pub fn save(&mut self) {
        super::persist::save(&mut self.sim);
        let p = self.sim.player.clone();
        let _ = self.sim.db.save_player(crate::db::PlayerRow { x: p.pos.x, z: p.pos.z, yaw: p.yaw, t_game: self.sim.t });
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// A private copy of a universe file (so a dry run leaves the original alone).
fn scratch_copy(path: &Path) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("pocket-sim-{}-{}", std::process::id(), crate::db::now() as u64));
    std::fs::create_dir_all(&dir)?;
    let p = dir.join(path.file_name().unwrap_or_default());
    std::fs::copy(path, &p).with_context(|| format!("copying {}", path.display()))?;
    Ok(p)
}

/// Parse "player", "npc:3", "thing:12", "instance:7", "cell:10,20", "x,y,z" or a name.
pub fn parse_target(s: &str) -> Target {
    let s = s.trim();
    if let Some(a) = ActorId::parse(s) {
        if s == "player" || s.starts_with("npc:") {
            return Target::Actor(a);
        }
    }
    let num = |x: &str| x.trim().parse::<i64>().ok();
    if let Some(r) = s.strip_prefix("thing:").and_then(num) {
        return Target::Thing(r);
    }
    if let Some(r) = s.strip_prefix("instance:").and_then(num) {
        return Target::Instance(r);
    }
    if let Some(r) = s.strip_prefix("cell:") {
        let v: Vec<i32> = r.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() == 2 {
            return Target::Cell([v[0], v[1]]);
        }
    }
    let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    if v.len() == 3 && s.split(',').count() == 3 {
        return Target::Point([v[0], v[1], v[2]]);
    }
    Target::Name(s.to_string())
}

/// Who `--as` names: "player", a character id, or a character's name.
fn parse_actor(sim: &Sim, s: &str) -> Result<ActorId> {
    if let Some(a) = ActorId::parse(s) {
        if sim.actor(a).is_some() {
            return Ok(a);
        }
    }
    let l = s.trim().to_lowercase();
    for n in &sim.cast.npcs {
        if n.name().to_lowercase() == l || n.name().to_lowercase().split_whitespace().next() == Some(l.as_str()) {
            return Ok(ActorId::Npc(n.def.id));
        }
    }
    bail!("no one called '{s}' (try player, a character id, or a name)")
}

fn llm_for(db: &Arc<Db>, args: &[String]) -> Option<Arc<Llm>> {
    if has(args, "--no-llm") {
        return None;
    }
    Llm::from_env(db.clone())
}

// ------------------------------------------------------------ pocket sim

/// What happened over a run, in numbers: if these stay flat, nothing is emerging.
#[derive(Debug, serde::Serialize)]
pub struct Metrics {
    pub game_hours: f32,
    pub events: u64,
    pub by_kind: BTreeMap<String, u64>,
    pub things_made_per_hour: f32,
    pub thing_travel_m: f32,
    pub longest_chain: usize,
    pub talk_about_things: u64,
    pub affection_change: f32,
    pub live_things: usize,
    pub active_cells: usize,
    pub invariants_broken: usize,
    /// Events between beings of different species (a chase, a hug with a dog).
    pub cross_species: u64,
    pub chases: u64,
    pub pets_following: usize,
    pub births: u64,
    /// Beings alive, by species.
    pub population: BTreeMap<String, usize>,
    /// More stories, not just more activity (see `Emergence`).
    pub emergence: Emergence,
}

/// The numbers each emergence phase must move: how many kinds of thing
/// happen, how many happenings lead on to others, and whether people still
/// get what they need.
#[derive(Debug, Default, serde::Serialize)]
pub struct Emergence {
    /// Kinds of event seen, on average per game day (a short run counts as a day).
    pub kinds_per_day: f32,
    /// Runs of cause and effect three or more steps long (see `longest_chain`).
    pub chains_3plus: usize,
    /// Events that name what caused them (an incident, a goal, a belief).
    pub with_cause: u64,
    /// Events where one character's goal needed another (a promise, an ask).
    pub goals_crossing: u64,
    /// Share of needs not pressing (under 0.8) at the end, over those awake.
    pub needs_met: f32,
    /// Awake beings that went nowhere and did nothing in the last game hour.
    pub stuck: usize,
    /// Which needs are pressing (0.8 or more) at the end, and how many.
    pub pressing: BTreeMap<String, usize>,
    /// What the stuck were doing.
    pub stuck_doing: BTreeMap<String, usize>,
}

/// The longest run of events where each follows the last within 15 s and
/// 15 m, by a different subject: cause and effect, roughly.
pub fn longest_chain(events: &[SimEvent]) -> usize {
    chains(events).into_iter().max().unwrap_or(0)
}

/// The length of the chain each chain-worthy event ends.
fn chains(events: &[SimEvent]) -> Vec<usize> {
    let interesting = |e: &SimEvent| !matches!(e.kind.as_str(), "said" | "chatted" | "plan_failed" | "decided" | "caught_up" | "proposed" | "accepted" | "declined");
    let ev: Vec<&SimEvent> = events.iter().filter(|e| interesting(e) && e.pos.is_some()).collect();
    let mut best = vec![1usize; ev.len()];
    for i in 0..ev.len() {
        let pi = Vec3::from(ev[i].pos.unwrap_or_default());
        let mut j = i;
        while j > 0 {
            j -= 1;
            if ev[i].t - ev[j].t > 15.0 {
                break;
            }
            let pj = Vec3::from(ev[j].pos.unwrap_or_default());
            if (pi - pj).length() <= 15.0 && ev[i].subject != ev[j].subject && ev[i].kind != ev[j].kind {
                best[i] = best[i].max(best[j] + 1);
            }
        }
    }
    best
}

/// Chains of three or more that nothing later carries on: one per story.
fn stories(events: &[SimEvent]) -> usize {
    let best = chains(events);
    // A chain is carried on when a longer one ends later; count each run's peak.
    let mut n = 0;
    let mut i = 0;
    while i < best.len() {
        if best[i] >= 3 {
            n += 1;
            while i + 1 < best.len() && best[i + 1] > 1 {
                i += 1;
            }
        }
        i += 1;
    }
    n
}

pub fn emergence(sim: &Sim, events: &[SimEvent], hours: f32, hour_ago: &HashMap<i64, Vec3>) -> Emergence {
    // Distinct kinds per game day.
    let day = crate::render::sky::DAY_SECONDS;
    let t0 = events.first().map(|e| e.t).unwrap_or(0.0);
    let mut days: BTreeMap<i64, std::collections::BTreeSet<&str>> = BTreeMap::new();
    for e in events {
        days.entry(((e.t - t0) / day).floor() as i64).or_default().insert(e.kind.as_str());
    }
    let kinds_per_day = if days.is_empty() { 0.0 } else { days.values().map(|k| k.len() as f32).sum::<f32>() / days.len() as f32 };
    let has = |e: &SimEvent, k: &str| e.data.get(k).is_some_and(|v| !v.is_null());
    let with_cause = events.iter().filter(|e| ["cause", "incident", "goal", "belief"].iter().any(|k| has(e, k))).count() as u64;
    let goals_crossing = events.iter().filter(|e| e.data.get("crosses").and_then(|v| v.as_bool()) == Some(true)).count() as u64;
    // Beyond `medium` time stands still: only those living count.
    let awake: Vec<&super::npc::Npc> = sim.cast.npcs.iter().filter(|n| n.here() && !n.a.asleep && sim.dist_to_player(n.a.pos) <= sim.cfg.medium).collect();
    let mut met = 0;
    let mut all = 0;
    let mut pressing: BTreeMap<String, usize> = BTreeMap::new();
    for n in &awake {
        let x = &n.needs;
        for (k, v) in [("hunger", x.hunger), ("fatigue", x.fatigue), ("social", x.social), ("fun", x.fun), ("curiosity", x.curiosity)] {
            all += 1;
            met += (v < 0.8) as usize;
            if v >= 0.8 {
                *pressing.entry(k.to_string()).or_default() += 1;
            }
        }
    }
    let acted: std::collections::HashSet<ActorId> = events.iter().filter(|e| e.t > sim.t - hour_s()).filter_map(|e| e.actor).collect();
    let stuck_ones: Vec<&&super::npc::Npc> = if hours < 1.5 { Vec::new() } else { awake.iter().filter(|n| hour_ago.get(&n.def.id).is_some_and(|p| (*p - n.a.pos).length() < 0.5) && !acted.contains(&ActorId::Npc(n.def.id))).collect() };
    let mut stuck_doing: BTreeMap<String, usize> = BTreeMap::new();
    for n in &stuck_ones {
        *stuck_doing.entry(format!("{}: {}", n.species.name, n.doing)).or_default() += 1;
    }
    Emergence { kinds_per_day, chains_3plus: stories(events), with_cause, goals_crossing, needs_met: if all == 0 { 1.0 } else { met as f32 / all as f32 }, stuck: stuck_ones.len(), pressing, stuck_doing }
}

pub fn metrics(sim: &Sim, events: &[SimEvent], hours: f32, start_pos: &HashMap<i64, Vec3>, start_rels: &BTreeMap<(i64, i64), f32>, hour_ago: &HashMap<i64, Vec3>) -> Metrics {
    let made = events.iter().filter(|e| matches!(e.kind.as_str(), "made" | "spawned" | "transformed")).count() as f32;
    // How far things moved from where they started (or from where they lay
    // before someone picked them up).
    let travel: f32 = sim
        .things
        .live()
        .filter_map(|t| {
            let from = start_pos.get(&t.id).copied().or_else(|| {
                let c = t.origin.cell?;
                Some(Vec3::new((c.0 as f32 + 0.5) * 4.0, t.pos.y, (c.1 as f32 + 0.5) * 4.0))
            })?;
            Some(Vec3::new(t.pos.x - from.x, 0.0, t.pos.z - from.z).length())
        })
        .sum();
    let names: Vec<String> = sim.snap.scene.types.values().filter(|t| !t.builtin || t.has_tag("stick") || t.has_tag("stone")).map(|t| t.name().to_lowercase()).collect();
    let talk = events.iter().filter(|e| e.kind == "said" && names.iter().any(|n| e.text.to_lowercase().contains(n.as_str()))).count() as u64;
    let aff: f32 = sim.social.rels.iter().map(|(k, r)| (r.affection - start_rels.get(k).copied().unwrap_or(0.0)).abs()).sum();
    let mut by_kind = BTreeMap::new();
    for e in events {
        *by_kind.entry(e.kind.clone()).or_insert(0) += 1;
    }
    let species_of = |a: ActorId| match a {
        ActorId::Player => Some("human".to_string()),
        ActorId::Npc(c) => sim.cast.get(c).map(|n| n.species.name.clone()),
    };
    let cross = events
        .iter()
        .filter(|e| {
            let other = e.subject.as_deref().and_then(ActorId::parse);
            match (e.actor, other) {
                (Some(a), Some(b)) => species_of(a).zip(species_of(b)).is_some_and(|(x, y)| x != y),
                _ => false,
            }
        })
        .count() as u64;
    let mut population = BTreeMap::new();
    for n in sim.cast.npcs.iter().filter(|n| n.here()) {
        *population.entry(n.species.name.clone()).or_insert(0) += 1;
    }
    let following = sim.cast.npcs.iter().filter(|n| n.here() && matches!(n.aim, super::npc::Aim::Follow(_))).count();
    Metrics {
        game_hours: hours,
        events: events.len() as u64,
        by_kind,
        things_made_per_hour: if hours > 0.0 { made / hours } else { 0.0 },
        thing_travel_m: travel,
        longest_chain: longest_chain(events),
        talk_about_things: talk,
        affection_change: aff,
        live_things: sim.things.len(),
        active_cells: sim.field.active(),
        invariants_broken: sim.check_invariants().len(),
        cross_species: cross,
        chases: events.iter().filter(|e| e.kind == "chase").count() as u64,
        pets_following: following,
        births: events.iter().filter(|e| e.kind == "born").count() as u64,
        population,
        emergence: emergence(sim, events, hours, hour_ago),
    }
}

/// One game hour in game seconds.
pub fn hour_s() -> f64 {
    crate::render::sky::DAY_SECONDS / 24.0
}

pub fn run_sim_cli(args: &[String]) -> Result<()> {
    if let Some(log) = flag(args, "--replay") {
        return replay(Path::new(&log), args);
    }
    let file = args.first().filter(|a| !a.starts_with("--")).context("usage: pocket sim FILE [--hours H] [--seed S] [--events out.jsonl] [--save] [--no-llm]")?;
    let path = PathBuf::from(file);
    let hours: f32 = flag(args, "--hours").and_then(|h| h.parse().ok()).unwrap_or(1.0);
    let seed: Option<u64> = flag(args, "--seed").and_then(|s| s.parse().ok());
    let dt: f32 = flag(args, "--dt").and_then(|s| s.parse().ok()).unwrap_or(0.1f32).clamp(0.02, 0.25);
    if has(args, "--verify") {
        let a = run_once(&path, hours, seed, dt, args, None)?;
        let b = run_once(&path, hours, seed, dt, args, None)?;
        let ja: Vec<String> = a.iter().map(|e| serde_json::to_string(e).unwrap_or_default()).collect();
        let jb: Vec<String> = b.iter().map(|e| serde_json::to_string(e).unwrap_or_default()).collect();
        if ja == jb {
            println!("deterministic: both runs produced the same {} events", ja.len());
            return Ok(());
        }
        let i = ja.iter().zip(&jb).position(|(x, y)| x != y).unwrap_or(ja.len().min(jb.len()));
        bail!("runs differ at event {i}:\n  {}\n  {}", ja.get(i).cloned().unwrap_or_default(), jb.get(i).cloned().unwrap_or_default());
    }
    let sink = flag(args, "--events").map(PathBuf::from);
    run_once(&path, hours, seed, dt, args, sink).map(|_| ())
}

fn run_once(path: &Path, hours: f32, seed: Option<u64>, dt: f32, args: &[String], sink: Option<PathBuf>) -> Result<Vec<SimEvent>> {
    let target = if has(args, "--save") { path.to_path_buf() } else { scratch_copy(path)? };
    let db = Db::open(&target)?;
    let llm = llm_for(&db, args);
    let mut s = Session::with_db(db, seed, llm)?;
    if let Some(at) = flag(args, "--at") {
        let v: Vec<f32> = at.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() >= 2 {
            let h = s.sim.snap.terrain.height(v[0], v[1]);
            s.sim.player.pos = Vec3::new(v[0], h, v[1]);
        }
    }
    let collected: Arc<Mutex<Vec<SimEvent>>> = Arc::new(Mutex::new(Vec::new()));
    struct Tee {
        file: Option<std::io::BufWriter<std::fs::File>>,
        buf: Vec<u8>,
        out: Arc<Mutex<Vec<SimEvent>>>,
    }
    impl Write for Tee {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            if let Some(f) = self.file.as_mut() {
                f.write_all(b)?;
            }
            self.buf.extend_from_slice(b);
            while let Some(i) = self.buf.iter().position(|c| *c == b'\n') {
                let line: Vec<u8> = self.buf.drain(..=i).collect();
                if let Ok(e) = serde_json::from_slice::<SimEvent>(&line) {
                    self.out.lock().push(e);
                }
            }
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if let Some(f) = self.file.as_mut() {
                f.flush()?;
            }
            Ok(())
        }
    }
    let file = match &sink {
        Some(p) => Some(std::io::BufWriter::new(std::fs::File::create(p).with_context(|| format!("creating {}", p.display()))?)),
        None => None,
    };
    s.sim.log.sink = Some(Box::new(Tee { file, buf: Vec::new(), out: collected.clone() }));
    let start_pos: HashMap<i64, Vec3> = s.sim.things.live().map(|t| (t.id, t.pos)).collect();
    let start_rels: BTreeMap<(i64, i64), f32> = s.sim.social.rels.iter().map(|(k, r)| (*k, r.affection)).collect();
    let secs = hours as f64 * hour_s();
    let t0 = Instant::now();
    let quiet = has(args, "--quiet");
    let steps = (secs / dt as f64).ceil() as u64;
    // Where everyone was an hour before the end (to find who got stuck).
    let mark = steps.saturating_sub((hour_s() / dt as f64).ceil() as u64);
    let mut hour_ago: HashMap<i64, Vec3> = HashMap::new();
    for i in 0..steps {
        if i == mark {
            hour_ago = s.sim.cast.npcs.iter().map(|n| (n.def.id, n.a.pos)).collect();
        }
        s.step(dt);
        if !quiet && i % 2000 == 0 && i > 0 {
            eprintln!("  {:.1} game h, {} events", (i as f64 * dt as f64) / hour_s(), s.sim.log.total);
        }
    }
    if let Some(w) = s.sim.log.sink.as_mut() {
        let _ = w.flush();
    }
    let real = t0.elapsed().as_secs_f64();
    if has(args, "--save") {
        s.save();
    }
    let events = collected.lock().clone();
    let broken = s.sim.check_invariants();
    if !broken.is_empty() {
        eprintln!("invariants broken:\n  {}", broken.join("\n  "));
    }
    if !quiet {
        let m = metrics(&s.sim, &events, hours, &start_pos, &start_rels, &hour_ago);
        println!("simulated {hours:.1} game hours ({secs:.0} s) in {real:.1} s real: {:.0}× speed", secs / real.max(1e-3));
        println!("{}", serde_json::to_string_pretty(&m)?);
        if let Some(p) = &sink {
            println!("events written to {}", p.display());
        }
    }
    Ok(events)
}

fn replay(path: &Path, args: &[String]) -> Result<()> {
    let from: f64 = flag(args, "--from").and_then(|s| s.parse().ok()).unwrap_or(f64::MIN);
    let kinds: Option<Vec<String>> = flag(args, "--kinds").map(|k| k.split(',').map(|x| x.trim().to_string()).collect());
    let f = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut events = Vec::new();
    for line in std::io::BufReader::new(f).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let e: SimEvent = serde_json::from_str(&line).with_context(|| format!("bad event line: {line}"))?;
        events.push(e);
    }
    let t0 = events.first().map(|e| e.t).unwrap_or(0.0);
    for e in &events {
        if e.t < from || kinds.as_ref().is_some_and(|k| !k.contains(&e.kind)) {
            continue;
        }
        let h = (e.t - t0) / hour_s();
        println!("{:>7.2}h  {:<12} {}", h, e.kind, e.text);
    }
    println!("{} events, {:.1} game hours, longest cause-and-effect chain {}", events.len(), (events.last().map(|e| e.t).unwrap_or(t0) - t0) / hour_s(), longest_chain(&events));
    Ok(())
}

// ------------------------------------------------------------ pocket act

/// Do one action as someone, let the world run a little, report what happened.
pub fn act_once(s: &mut Session, who: ActorId, action: super::Action, run: f32) -> Value {
    let before = s.sim.log.total;
    let r = s.sim.act(who, action.clone());
    let walked = matches!(r, Err(super::ActErr::TooFar { .. }));
    let (mut ok, mut msg, pending) = match &r {
        Ok(o) => (o.ok, o.msg.clone(), o.pending),
        Err(super::ActErr::TooFar { .. }) => {
            // Walk there first, then do it.
            s.sim.plan(who, vec![action.clone()], "", false);
            (true, format!("walking over to {}", action.verb()), None)
        }
        Err(e) => (false, e.to_string(), None),
    };
    s.pump(Duration::from_secs(120));
    let dt = 0.05;
    let mut t = 0.0;
    while t < run {
        s.step(dt);
        t += dt;
        let busy = s.sim.actor(who).is_some_and(|a| a.task.is_some()) || match who {
            ActorId::Player => !s.sim.player_plan.is_empty(),
            ActorId::Npc(c) => s.sim.cast.get(c).is_some_and(|n| !n.plan.is_empty()),
        };
        if !busy && t >= run.min(1.0) && s.outstanding == 0 && pending.is_none() {
            // Let thrown things land.
            if s.sim.things.live().all(|x| x.asleep || x.held()) {
                break;
            }
        }
    }
    let n = (s.sim.log.total - before) as usize;
    if walked {
        // How the walk-then-act ended.
        let mine: Vec<&SimEvent> = s.sim.log.recent.iter().rev().take(n).filter(|e| e.actor == Some(who)).collect();
        if let Some(f) = mine.iter().find(|e| e.kind == "plan_failed") {
            ok = false;
            msg = f.text.clone();
        } else if let Some(last) = mine.first() {
            msg = last.text.clone();
        }
    }
    // What this actor did, and what happened near them.
    let here = s.sim.actor(who).map(|a| a.pos).unwrap_or_default();
    let events: Vec<SimEvent> = s.sim.log.recent.iter().rev().take(n).rev().filter(|e| e.actor == Some(who) || e.pos.is_some_and(|p| (Vec3::from(p) - here).length() < 30.0)).cloned().collect();
    let notes: Vec<String> = s.sim.drain_notes().into_iter().filter(|n| !matches!(n, Note::Far { .. } | Note::Incident { near: false, .. })).map(|n| n.text()).collect();
    let me = s.sim.inspect(&Target::Actor(who));
    json!({ "ok": ok, "msg": msg, "events": events, "heard": notes, "actor": me })
}

pub fn act_cli(args: &[String]) -> Result<()> {
    let file = args.first().filter(|a| !a.starts_with("--")).context("usage: pocket act FILE --as player|ID|NAME '<action json>' [--run SECS] [--dry] | --stdin")?;
    let db = Db::open(Path::new(file))?;
    let llm = llm_for(&db, args);
    let mut s = Session::with_db(db, None, llm)?;
    let dry = has(args, "--dry");
    if has(args, "--stdin") {
        let stdin = std::io::stdin();
        let mut out = std::io::stdout();
        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    writeln!(out, "{}", json!({ "error": e.to_string() }))?;
                    continue;
                }
            };
            let reply = stdin_command(&mut s, &v);
            writeln!(out, "{reply}")?;
            out.flush()?;
            if !dry {
                s.save();
            }
        }
        return Ok(());
    }
    let who = parse_actor(&s.sim, &flag(args, "--as").unwrap_or_else(|| "player".into()))?;
    let json_arg = args.iter().skip(1).find(|a| a.trim_start().starts_with('{')).context("give the action as JSON, e.g. '{\"do\": \"hold\", \"target\": {\"name\": \"stick\"}}'")?;
    let action: super::Action = serde_json::from_str(json_arg).or_else(|_| super::npc::parse_step(&serde_json::from_str(json_arg)?).context("not an action"))?;
    let run: f32 = flag(args, "--run").and_then(|r| r.parse().ok()).unwrap_or(3.0);
    let r = act_once(&mut s, who, action, run);
    if !dry {
        s.save();
    }
    println!("{}", serde_json::to_string_pretty(&r)?);
    Ok(())
}

/// One line of `pocket act --stdin`:
/// {"as": "player", "action": {...}, "run": 2} | {"inspect": "thing:3"} | {"run": 5} | {"look": true}
fn stdin_command(s: &mut Session, v: &Value) -> Value {
    if let Some(t) = v.get("inspect").and_then(|x| x.as_str()) {
        return s.sim.inspect(&parse_target(t));
    }
    if v.get("look").is_some() {
        let who = v.get("as").and_then(|x| x.as_str()).and_then(|a| parse_actor(&s.sim, a).ok()).unwrap_or(ActorId::Player);
        return look_around(&mut s.sim, who);
    }
    let run = v.get("run").and_then(|x| x.as_f64()).unwrap_or(2.0) as f32;
    match v.get("action") {
        Some(a) => {
            let who = match v.get("as").and_then(|x| x.as_str().map(str::to_string).or_else(|| x.as_i64().map(|i| i.to_string()))) {
                Some(w) => match parse_actor(&s.sim, &w) {
                    Ok(x) => x,
                    Err(e) => return json!({ "error": e.to_string() }),
                },
                None => ActorId::Player,
            };
            let action = serde_json::from_value::<super::Action>(a.clone()).ok().or_else(|| super::npc::parse_step(a));
            match action {
                Some(act) => act_once(s, who, act, run),
                None => json!({ "error": "not an action" }),
            }
        }
        None => {
            s.run(run, 0.05);
            json!({ "ok": true, "t": s.sim.t })
        }
    }
}

/// What someone can see and use around them (for agents).
pub fn look_around(sim: &mut Sim, who: ActorId) -> Value {
    let Some(me) = sim.actor(who).cloned() else { return json!({ "error": "no such actor" }) };
    let mut things = Vec::new();
    for id in sim.things.near(me.pos, 30.0) {
        if let Some(t) = sim.things.get(id) {
            let d = (t.pos - me.pos).length();
            things.push(json!({ "target": { "thing": id }, "name": sim.thing_name(id), "dist": (d as f64 * 10.0).round() / 10.0, "held_by": t.holder.map(|h| sim.actor_name(h)) }));
        }
    }
    let snap = sim.snap.clone();
    for p in &snap.instances {
        let d = (p.pos - me.pos).length();
        if d < 40.0 && !sim.things.by_instance.contains_key(&p.id) {
            if let Some(ty) = snap.type_of(p.type_id) {
                things.push(json!({ "target": { "instance": p.id }, "name": ty.name(), "dist": (d as f64 * 10.0).round() / 10.0 }));
            }
        }
    }
    let mut loose = Vec::new();
    for it in sim.cache.items_near(&snap, me.pos, 12.0) {
        if let Some(ty) = snap.type_of(it.inst.info[0]) {
            if ty.has_tag("small") {
                let d = (it.inst.pos() - me.pos).length();
                loose.push(json!({ "target": { "cell": [it.cell.0, it.cell.1] }, "name": ty.name(), "dist": (d as f64 * 10.0).round() / 10.0 }));
            }
        }
    }
    loose.truncate(12);
    let people: Vec<Value> = sim
        .actor_ids()
        .into_iter()
        .filter(|a| *a != who)
        .filter_map(|a| sim.actor(a).map(|x| (a, (x.pos - me.pos).length())))
        .filter(|(_, d)| *d < 60.0)
        .map(|(a, d)| json!({ "target": { "actor": a }, "name": sim.actor_name(a), "dist": (d as f64 * 10.0).round() / 10.0 }))
        .collect();
    json!({ "at": me.pos.to_array().map(|v| (v as f64 * 10.0).round() / 10.0), "yaw_deg": (me.yaw.to_degrees().rem_euclid(360.0) as f64).round(), "holding": me.held.map(|h| sim.thing_name(h)), "things": things, "loose": loose, "people": people, "time": crate::render::sky::time_label(sim.t) })
}

// ------------------------------------------------------------ pocket inspect

pub fn inspect_cli(args: &[String]) -> Result<()> {
    let file = args.first().context("usage: pocket inspect FILE <player|npc:ID|thing:ID|instance:ID|cell:X,Z|name> | --look [--as WHO]")?;
    let db = Db::open(Path::new(file))?;
    let mut s = Session::with_db(db, None, None)?;
    if has(args, "--look") {
        let who = match flag(args, "--as") {
            Some(a) => parse_actor(&s.sim, &a)?,
            None => ActorId::Player,
        };
        println!("{}", serde_json::to_string_pretty(&look_around(&mut s.sim, who))?);
        return Ok(());
    }
    let t = args.get(1).context("what to inspect?")?;
    let v = s.sim.inspect(&parse_target(t));
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}
