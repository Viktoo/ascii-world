//! The night hunt (docs/night-hunt-plan.md).
//!
//! Nothing here is written for one monster. Beings keep hours
//! (`Species::active`) and are away outside them; some go after something
//! (`want`) and their touch does something to it (`touch`); some won't come
//! near a property (`shuns`) or move only unwatched (`moves_unseen`); some
//! mean harm and think about how (`hostile`: see `phantom`). The world's
//! difficulty says how many of the dark's own come each night: on peaceful
//! worlds none. Their touch and their blows take health, the traveler's
//! too, less what is worn softens.

use super::props::{P_HEALTH, P_LIGHT, P_PROTECT};
use super::{ActorId, Note, Sim, Target};
use crate::render::sky::{self, DAY_SECONDS};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

/// One difficulty's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub name: &'static str,
    /// Night walkers out each night.
    pub walkers: u32,
    /// Phantoms out each night.
    pub phantoms: u32,
}

pub const LEVELS: [Level; 4] = [
    Level { name: "peaceful", walkers: 0, phantoms: 0 },
    Level { name: "easy", walkers: 1, phantoms: 1 },
    Level { name: "normal", walkers: 1, phantoms: 1 },
    Level { name: "hard", walkers: 2, phantoms: 2 },
];

/// After its touch lands, a night walker draws back this long (s).
pub const TOUCH_GAP: f64 = 25.0;
/// Gap between its body and the traveler's within which a watched being
/// still reaches them (m): arm's reach, from its nearest edge.
const WATCHED_REACH: f32 = 1.0;
/// The middle part of the view that counts as looking at something (of
/// its half-width): what moves unseen creeps in the rest.
const WATCHING: f32 = 0.4;
/// A hunter comes back this far from the traveler, out of sight (m).
pub const ARRIVE_AT: f32 = 48.0;
/// The most what someone wears can soften a blow.
const MAX_PROTECT: f32 = 0.7;
/// The traveler's health back in a game day.
const MEND_PER_DAY: f32 = super::body::MEND_PER_DAY;

/// When something that hunts the traveler arrives ({} is where it is,
/// told truly: "behind you", "off to your left").
const ARRIVAL: &[&str] = &[
    "You hear a rustle {}.",
    "Something has come out of the dark, {}.",
    "A twig snaps somewhere {}.",
    "The dark gets a little thicker, {}.",
];
/// The night walker's species.
pub const WALKER: &str = "night walker";
/// The night walker's pace, whatever its shape: tuned to be the right
/// amount of frightening (it walks, then runs the last stretch).
const WALKER_WALK: f32 = 1.1;
const WALKER_RUN: f32 = 3.4;
/// Words that say where something is: signs may not (the game tells it).
const PLACE_WORDS: &[&str] = &["behind", "left", "right", "ahead", "in front", "above", "overhead", "below", "beside", "nearby", "far off", "off to"];

/// What one phantom did and met tonight, for what it learns at dawn.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Tally {
    /// How much of the traveler's health it took.
    pub hurt_traveler: f32,
    /// Whom else it struck, and killed.
    pub struck: Vec<String>,
    pub killed: Vec<String>,
    /// Who hurt it, and with what ("the traveler, with the iron sword").
    pub hurt_by: Vec<String>,
    /// Who struck it down.
    pub killed_by: Option<String>,
    /// What the traveler was seen holding to fight with.
    pub traveler_arms: Option<String>,
    /// It came near the traveler.
    pub reached: bool,
    /// It struck the traveler down.
    pub killed_traveler: bool,
    /// What it made.
    pub made: Vec<String>,
}

/// The night's own state, saved with the world (kv `sim.night`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NightState {
    /// How much of the traveler's health is gone (0: whole, 1: none left).
    pub wounds: f32,
    /// When the traveler was last hurt (s of world time).
    #[serde(skip)]
    pub hurt_at: f64,
    /// The traveler has fallen (health gone): what struck them down. They
    /// lie where they fell, and the world goes on, till they wake.
    pub fallen: Option<String>,
    /// When they fell (s of world time).
    pub fallen_at: f64,
    /// The traveler fell tonight.
    pub fell_tonight: bool,
    /// A glow someone's touch left on the traveler (0..1), fading.
    #[serde(skip)]
    pub glow: f32,
    /// Night at the last check (None: not checked yet this session).
    #[serde(skip)]
    pub was_night: Option<bool>,
    /// Nights begun in this world.
    pub nights: u32,
    /// Nights the dark came and the traveler got through.
    pub survived: u32,
    /// The dark comes tonight.
    pub hunting: bool,
    /// Times the dark hurt the traveler tonight.
    pub wounded: u32,
    /// Something actually came tonight.
    pub came: bool,
    /// Night walkers (kept as characters: away by day). The ones killed
    /// tonight are counted till dusk.
    #[serde(alias = "horrors")]
    pub walkers: Vec<i64>,
    /// Phantoms: they never truly die, and come back each night.
    pub phantoms: Vec<i64>,
    /// Phantoms that made their one thing tonight.
    pub made: Vec<i64>,
    /// What each phantom did tonight.
    pub tally: BTreeMap<i64, Tally>,
    #[serde(skip)]
    pub last_dread: f64,
    /// How wide the traveler's view is: tan of half its width (the app
    /// sets it each frame; 0 until then).
    #[serde(skip)]
    pub view_slope: f32,
}

/// When the fallen traveler wakes (of the day).
pub const MORNING: f32 = 0.30;
/// When the dark is about: nightfall to dawn (the sky's night).
pub const DARK_FROM: f32 = 0.80;
pub const DARK_UNTIL: f32 = 0.21;

pub fn is_dark(t: f64) -> bool {
    !(DARK_UNTIL..DARK_FROM).contains(&sky::day_phase(t))
}

impl Sim {
    /// From nightfall to dawn: when night beings are about and the dark may come.
    pub fn dark(&self) -> bool {
        is_dark(self.t)
    }

    pub fn level(&self) -> Level {
        LEVELS[(self.cfg.difficulty as usize).min(LEVELS.len() - 1)]
    }

    // ------------------------------------------------------------ health

    /// The traveler's health (0..1).
    pub fn health(&self) -> f32 {
        (1.0 - self.night.wounds).clamp(0.0, 1.0)
    }

    /// How much what someone wears (and their own hide) softens a blow (0..`MAX_PROTECT`).
    pub fn protection(&self, who: ActorId) -> f32 {
        let worn: f32 = self.worn_by(who).iter().filter_map(|id| self.things.get(*id)).map(|t| t.props.get(P_PROTECT).copied().unwrap_or(0.0)).sum();
        let own = match who {
            ActorId::Npc(c) => self.cast.get(c).and_then(|n| n.props.get(P_PROTECT).copied()).unwrap_or(0.0),
            ActorId::Player => 0.0,
        };
        (worn + own).clamp(0.0, MAX_PROTECT)
    }

    /// `who` loses `hurt` of their health at `by`'s hand (what they wear
    /// already taken off); `how` is what happened to them, as it ends
    /// "they were …" ("struck by the first phantom with the rusty sword",
    /// "broken by a fall of 7 m from the oak"): it is what is remembered when
    /// it kills. True when a being died of it. At none left the
    /// traveler only stays at none, for now.
    pub fn wound(&mut self, who: ActorId, hurt: f32, by: ActorId, how: &str) -> bool {
        if hurt <= 0.0 {
            return false;
        }
        match who {
            ActorId::Player => {
                if self.fallen() {
                    return false;
                }
                self.night.wounds = (self.night.wounds + hurt).min(1.0);
                self.night.hurt_at = self.t;
                if self.dark() {
                    self.night.wounded += 1;
                }
                if let ActorId::Npc(c) = by {
                    if let Some(t) = self.night.tally.get_mut(&c) {
                        t.hurt_traveler += hurt;
                        t.reached = true;
                    }
                }
                let pct = (self.health() * 100.0).round();
                let text = format!("the traveler was {how}");
                self.event("hurt", Some(by), Some("player".into()), text, Some(self.player.pos), json!({ "hurt": (hurt * 100.0).round() / 100.0, "health": pct / 100.0 }));
                if self.night.wounds >= 1.0 {
                    self.traveler_falls(by, how);
                    return true;
                }
                false
            }
            ActorId::Npc(c) => {
                // The dark's own can always be struck down; others only where things die.
                let dark = self.cast.get(c).is_some_and(|n| n.species.touch.harms() || n.species.hostile);
                let floor = if self.cfg.hunting || dark { 0.0 } else { 0.1 };
                let Some(n) = self.cast.get_mut(c) else { return false };
                if n.props.len() <= P_HEALTH {
                    return false;
                }
                n.props[P_HEALTH] = (n.props[P_HEALTH] - hurt).max(floor);
                n.props[P_HEALTH] <= 0.0
            }
        }
    }

    /// Whether the traveler lies fallen.
    pub fn fallen(&self) -> bool {
        self.night.fallen.is_some()
    }

    /// The traveler's health is gone: they fall where they stand, letting go
    /// of what they held. Nothing else is decided here: whoever saw it
    /// remembers it, the thinking ones are asked what they do, the killer
    /// counts it, and the dark turns to whoever else is about.
    fn traveler_falls(&mut self, by: ActorId, how: &str) {
        let name = self.actor_name(by);
        let at = self.player.pos;
        self.night.fallen = Some(how.to_string());
        self.night.fallen_at = self.t;
        self.night.fell_tonight |= self.dark();
        if let Some(h) = self.player.held {
            self.release(h);
            if let Some(t) = self.things.get_mut(h) {
                t.pos = at + self.player.forward() * 0.6 + Vec3::Y * 0.4;
                t.asleep = false;
                t.dirty = true;
            }
        }
        self.player_plan.clear();
        self.player.task = None;
        self.player.motion = None;
        self.player.riding = None;
        self.player.aboard = None;
        let msg = format!("the traveler was {how}, and fell and lay still");
        self.event("traveler_fell", Some(by), Some("player".into()), msg.clone(), Some(at), json!({ "by": by.key(), "how": how }));
        self.notes.push(Note::Notable(if by == ActorId::Player { "Everything goes dark.".into() } else { format!("You fall. {} stands over you.", super::physics::cap(&name)) }));
        self.hurt_by(ActorId::Player, by, 1.0, true);
        self.witness(at, 35.0, &msg, 0.95, &[by]);
        if self.has_llm {
            let seen: Vec<(i64, Vec3)> = self
                .cast
                .npcs
                .iter()
                .filter(|n| n.here() && !n.a.asleep && n.species.mind == crate::world::species::Mind::Sapient && !n.species.hostile && (n.a.pos - at).length() < 30.0)
                .map(|n| (n.def.id, n.a.pos))
                .collect();
            for (c, p) in seen {
                let ctx = format!("You just saw the traveler fall: they were {how}. The traveler lies on the ground {:.0} m from you and doesn't move.", (p - at).length());
                let context = self.decide_context(c, &ctx);
                self.request_weighted(super::Request::Decide { cid: c, event: "traveler_fell".into(), context }, p, 1.0);
            }
        }
    }

    /// The fallen traveler wakes where the world began, in the morning,
    /// whole. What they dropped stays where they fell.
    pub fn wake_traveler(&mut self) {
        let Some(by) = self.night.fallen.take() else { return };
        let spawn = self.snap.spawn;
        self.player.pos = Vec3::new(spawn.x, self.snap.terrain.height(spawn.x, spawn.z), spawn.z);
        self.night.wounds = 0.0;
        let day = (self.t / DAY_SECONDS).floor();
        let morning = MORNING as f64;
        let phase = sky::day_phase(self.t) as f64;
        self.t = (day + if phase >= morning { 1.0 } else { 0.0 } + morning) * DAY_SECONDS;
        self.event("traveler_woke", Some(ActorId::Player), None, format!("the traveler woke where the world began, after {by} struck them down"), Some(self.player.pos), json!({}));
        self.notes.push(Note::Notable("You wake where you first came into this world. It is morning.".into()));
    }

    /// The traveler mends, slowly (once a second).
    fn mend(&mut self) {
        if self.fallen() {
            return;
        }
        self.night.wounds = (self.night.wounds - MEND_PER_DAY / DAY_SECONDS as f32).max(0.0);
    }

    // ------------------------------------------------------------ sight

    /// Whether `p` is anywhere on the traveler's screen (or just off it),
    /// near enough for the hour: nothing may appear or vanish there.
    pub fn player_sees(&self, p: Vec3) -> bool {
        self.in_view(p, 0.0, 1.1)
    }

    /// Whether the traveler is looking at a body at `p`, `r` wide: any of
    /// it in the middle of the view. The outer part on each side doesn't
    /// count, so what moves unseen can still creep in from the edge of the eye.
    pub fn player_watches(&self, p: Vec3, r: f32) -> bool {
        self.in_view(p, r, WATCHING)
    }

    /// Any of a body at `p`, `r` wide, within `part` of the view's
    /// half-width (1: its edge).
    fn in_view(&self, p: Vec3, r: f32, part: f32) -> bool {
        let d = p - self.player.pos;
        let dist = Vec3::new(d.x, 0.0, d.z).length();
        let range = if self.night() { 40.0 } else { 90.0 };
        if dist > range {
            return false;
        }
        if dist < 0.1 + r {
            return true;
        }
        let f = self.player.forward();
        let ahead = d.x * f.x + d.z * f.z;
        let side = (d.x * f.z - d.z * f.x).abs();
        let slope = if self.night.view_slope > 0.0 { self.night.view_slope } else { 1.0 };
        // The body's near edge against the view's edge at its distance.
        let edge = ahead * slope * part;
        ahead + r > 0.0 && side - r * (1.0 + slope * part * slope * part).sqrt() < edge
    }

    /// Something that `sp` shuns near `p`: where it is and how wide a berth.
    pub fn shunned_near(&mut self, p: Vec3, shuns: &[String]) -> Option<(Vec3, f32)> {
        if shuns.is_empty() {
            return None;
        }
        let idx: Vec<usize> = shuns.iter().filter_map(|s| self.vocab.id(s)).collect();
        let fires = shuns.iter().any(|s| s == "light" || s == "fire");
        let mut best: Option<(f32, Vec3, f32)> = None;
        let mut consider = |d: f32, at: Vec3, r: f32| {
            if d < r + 14.0 && best.is_none_or(|b| d - r < b.0) {
                best = Some((d - r, at, r));
            }
        };
        for id in self.things.near(p, 26.0) {
            let Some(t) = self.things.get(id) else { continue };
            let v = idx.iter().map(|i| t.props.get(*i).copied().unwrap_or(0.0)).fold(0.0f32, f32::max);
            if v > 0.3 {
                consider((t.pos - p).length(), t.pos, 3.0 + 5.0 * v.min(1.5));
            }
        }
        if fires {
            for b in self.burning() {
                consider((b.pos - p).length(), b.pos, 4.0 + 2.0 * b.size);
            }
        }
        // Lamps and lit things placed in the land (not yet live).
        let snap = self.snap.clone();
        for pl in snap.near_chunk(crate::world::chunk_of(p.x, p.z)) {
            let d = (pl.pos - p).length();
            if d > 26.0 || self.things.by_instance.contains_key(&pl.id) {
                continue;
            }
            let Some(ty) = snap.type_of(pl.type_id) else { continue };
            let props = self.type_props.get(&self.vocab, ty);
            let v = idx.iter().map(|i| props.get(*i).copied().unwrap_or(0.0)).fold(0.0f32, f32::max);
            if v > 0.3 {
                consider(d, pl.pos, 3.0 + 5.0 * v.min(1.5) + ty.radius() * pl.scale * 0.5);
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// Inside the berth of something `sp` shuns?
    fn shunned_at(&mut self, p: Vec3, shuns: &[String]) -> Option<(Vec3, f32)> {
        self.shunned_near(p, shuns).filter(|(at, r)| (p - *at).length() < *r)
    }

    /// Beings whose touch harms, about and near `p`.
    pub fn dread_near(&self, p: Vec3, range: f32) -> bool {
        self.cast.npcs.iter().any(|n| n.here() && n.species.touch.harms() && (n.a.pos - p).length() < range)
    }

    /// Whether `who` is one of the dark's own (a night walker, a phantom).
    pub fn of_the_dark(&self, who: ActorId) -> bool {
        matches!(who, ActorId::Npc(c) if self.cast.get(c).is_some_and(|n| n.species.touch.harms() || n.species.hostile))
    }

    /// Where someone sleeps tonight: at home, or with the dark about, by the
    /// nearest light near home.
    pub fn night_shelter(&mut self, home: Vec3) -> Vec3 {
        if !self.night.hunting {
            return home;
        }
        let light = vec!["light".to_string()];
        match self.shunned_near(home, &light) {
            Some((at, r)) if (at - home).length() < 40.0 => {
                let a = (home.x * 12.9898 + home.z * 78.233).sin() * 43758.545;
                let a = a.fract() * std::f32::consts::TAU;
                at + Vec3::new(a.cos(), 0.0, a.sin()) * (r * 0.6).clamp(1.5, 4.0)
            }
            _ => home,
        }
    }

    // ------------------------------------------------------------ the clock

    /// Once a second: dusk and dawn, the dark coming and going.
    pub fn step_night(&mut self) {
        let night = self.dark();
        let first = self.night.was_night.is_none();
        match self.night.was_night {
            None => {
                self.settle_night_kinds();
                if night && self.night.hunting {
                    self.call_the_dark();
                }
            }
            Some(was) if was != night => {
                if night {
                    self.dusk();
                } else {
                    self.dawn();
                }
            }
            _ => {}
        }
        self.night.was_night = Some(night);
        // Nothing came yet tonight (no room behind the traveler): keep trying.
        if night && self.night.hunting {
            self.call_the_dark();
        }
        self.comings_and_goings(night, first);
        if !night {
            self.fade_dark_remains();
        }
        self.mend();
        self.night.glow = (self.night.glow - 0.002).max(0.0);
        self.dread();
    }

    fn dusk(&mut self) {
        self.night.nights += 1;
        self.night.wounded = 0;
        self.night.fell_tonight = false;
        self.night.came = false;
        self.night.made.clear();
        self.night.tally.clear();
        let lv = self.level();
        let n = self.night.nights;
        self.night.hunting = lv.walkers + lv.phantoms > 0;
        // Walkers struck down last night are gone for good.
        self.night.walkers.retain(|id| self.cast.get(*id).is_some_and(|n| !n.dead));
        self.event("dusk", None, None, format!("night {n} falls"), Some(self.player.pos), json!({ "night": n, "dark": self.night.hunting }));
        if self.night.hunting {
            self.raise_phantoms();
            self.call_the_dark();
        }
    }

    fn dawn(&mut self) {
        if self.night.hunting && self.night.came && !self.night.fell_tonight {
            self.night.survived += 1;
            let clean = self.night.wounded == 0;
            self.event("survived_night", Some(ActorId::Player), None, format!("the traveler got through night {}", self.night.nights), Some(self.player.pos), json!({ "untouched": clean, "night": self.night.nights }));
            self.notes.push(Note::Notable("Dawn. The dark draws back.".into()));
        }
        if self.night.hunting {
            self.phantom_lessons();
        }
        self.night.hunting = false;
    }

    /// Whether a being of the dark has its place tonight: the first so many
    /// of its kind, by the difficulty (a world made easier keeps the rest away).
    fn has_place(&self, cid: i64) -> bool {
        let lv = self.level();
        match self.night.walkers.iter().position(|id| *id == cid) {
            Some(i) => (i as u32) < lv.walkers,
            None => match self.night.phantoms.iter().position(|id| *id == cid) {
                Some(i) => (i as u32) < lv.phantoms,
                None => true,
            },
        }
    }

    /// The dark comes for the traveler: as many night walkers and phantoms
    /// as the difficulty says (a walker struck down tonight isn't replaced
    /// till tomorrow).
    fn call_the_dark(&mut self) {
        let lv = self.level();
        self.settle_night_kinds();
        while (self.night.walkers.len() as u32) < lv.walkers {
            let Some(at) = self.arrival_spot(ARRIVE_AT) else { return };
            let Some(sp) = self.snap.species.get(WALKER).cloned() else { return };
            let persona = crate::world::Persona { name: format!("the {}", sp.name), species: sp.name.clone(), appearance: sp.description.clone(), ..Default::default() };
            let state = crate::world::characters::SavedState { x: at.x, z: at.z, ..Default::default() };
            let Some(id) = self.add_being(persona, at, state) else { return };
            self.night.walkers.push(id);
            self.night.came = true;
            self.event("dark_came", Some(ActorId::Npc(id)), Some("player".into()), format!("{} came out of the dark", self.actor_name(ActorId::Npc(id))), Some(at), json!({ "species": sp.name }));
            self.arrival_note(at);
        }
        while (self.night.phantoms.len() as u32) < lv.phantoms {
            let Some(at) = self.arrival_spot(super::phantom::PHANTOM_AT) else { return };
            if self.make_phantom(at).is_none() {
                return;
            }
        }
    }

    /// The night's kinds are the built-in ones: stored when missing, and set
    /// right when older worlds kept another shape of them (their LLM-written
    /// horrors become night walkers, keeping their names).
    fn settle_night_kinds(&mut self) {
        let stock = walker_species();
        let mut names: Vec<String> = self.night.walkers.iter().filter_map(|id| self.cast.get(*id)).map(|n| n.species.name.clone()).collect();
        names.push(WALKER.to_string());
        names.sort();
        names.dedup();
        for name in names {
            let mut sp = stock.clone();
            sp.plural = format!("{name}s");
            sp.name = name.clone();
            let same = self.snap.species.get(&name).is_some_and(|s| **s == sp);
            if !same {
                self.store_species(sp);
            }
        }
        let ph = super::phantom::phantom_species();
        if self.snap.species.get(&ph.name).is_none_or(|s| **s != ph) {
            self.store_species(ph);
        }
    }

    /// Keep a species in the save and use it now.
    pub(super) fn store_species(&mut self, mut sp: crate::world::species::Species) {
        sp.sanitize();
        if let Ok(j) = serde_json::to_string(&sp) {
            let _ = self.db.with(|c| crate::db::put_species(c, &sp.name, &j));
        }
        let mut book = (*self.snap.species).clone();
        book.add(sp);
        self.set_species_book(book);
        let snap = self.snap.clone();
        self.cast.sync(&snap, self.seed);
    }

    /// Tell the traveler, quietly, that something has arrived.
    pub(super) fn arrival_note(&mut self, at: Vec3) {
        let i = (self.rand() * ARRIVAL.len() as f32) as usize % ARRIVAL.len();
        let line = ARRIVAL[i].replace("{}", &self.where_is(at));
        self.notes.push(Note::Info(line));
        self.night.last_dread = self.t;
    }

    /// Where `p` is from the traveler, in words, truly.
    pub fn where_is(&self, p: Vec3) -> String {
        let d = p - self.player.pos;
        let (ahead, side) = (d.dot(self.player.forward()), d.dot(self.player.right()));
        let a = side.atan2(ahead).to_degrees();
        let lr = if a > 0.0 { "right" } else { "left" };
        let close = Vec3::new(d.x, 0.0, d.z).length() < 12.0;
        match a.abs() {
            x if x < 25.0 => "ahead of you, in the dark".into(),
            x if x < 65.0 => format!("ahead of you, off to the {lr}"),
            x if x < 115.0 => if close { format!("close, to your {lr}") } else { format!("off to your {lr}") },
            x if x < 155.0 => format!("behind you, to the {lr}"),
            _ => if close { "close behind you".into() } else { "behind you".into() },
        }
    }

    /// A spot about `dist` from the traveler, behind them, on dry land.
    pub(super) fn arrival_spot(&mut self, dist: f32) -> Option<Vec3> {
        let back = -self.player.forward();
        let base = back.z.atan2(back.x);
        for i in 0..10 {
            let spread = (self.rand() - 0.5) * 2.4 * (1.0 + i as f32 * 0.15);
            let a = base + spread;
            let d = dist * (0.9 + self.rand() * 0.25);
            let p = self.player.pos + Vec3::new(a.cos(), 0.0, a.sin()) * d;
            let h = self.snap.terrain.height(p.x, p.z);
            if h > crate::terrain::WATER_LEVEL + 0.2 && !self.player_sees(p) {
                return Some(Vec3::new(p.x, h, p.z));
            }
        }
        None
    }

    /// Beings go away outside their hours and come back in them, never
    /// before the traveler's eyes. A phantom keeps what it holds and wears
    /// while away.
    /// `quiet`: the first look after loading (older saves didn't keep who was
    /// away): out-of-hours beings are simply gone, nothing told.
    fn comings_and_goings(&mut self, night: bool, quiet: bool) {
        let hunting = self.night.hunting;
        let ids: Vec<(i64, bool, bool, Vec3, bool, bool)> = self
            .cast
            .npcs
            .iter()
            .filter(|n| !n.dead && (n.species.active != crate::world::species::Active::Always || n.species.touch.harms() || n.species.hostile || !n.species.comes_with.is_empty()))
            .map(|n| {
                let dark = n.species.touch.harms() || n.species.hostile;
                let due = n.species.about(night) && crate::world::weather::suits(&n.species.comes_with, &self.wx) && (!dark || hunting && self.has_place(n.def.id));
                (n.def.id, n.away, due, n.a.pos, dark && (n.species.want == "traveler" || n.species.hostile), n.species.hostile)
            })
            .collect();
        for (cid, away, due, pos, hunts_you, hostile) in ids {
            if !away && !due {
                // Slip away out of sight (or, watched, now and then: gone between glances).
                if !quiet && self.player_sees(pos) && self.rand() < 0.85 {
                    continue;
                }
                let name = self.actor_name(ActorId::Npc(cid));
                if !hostile {
                    if let Some(h) = self.actor(ActorId::Npc(cid)).and_then(|a| a.held) {
                        self.release(h);
                    }
                }
                self.social.joints.retain(|j| !j.has(ActorId::Npc(cid)));
                if let Some(n) = self.cast.get_mut(cid) {
                    n.away = true;
                    n.plan.clear();
                    n.a.task = None;
                    n.a.motion = None;
                    n.doing = "away".into();
                }
                if !quiet && self.dist_to_player(pos) < 60.0 && hunts_you {
                    self.notes.push(Note::Ambient(format!("{} is gone with the light.", super::physics::cap(&name))));
                }
            } else if away && due {
                let at = if hunts_you { self.arrival_spot(if hostile { super::phantom::PHANTOM_AT } else { ARRIVE_AT }) } else { Some(pos).filter(|p| !self.player_sees(*p)) };
                let Some(at) = at else { continue };
                let t = self.t;
                if let Some(n) = self.cast.get_mut(cid) {
                    n.away = false;
                    n.a.pos = at;
                    n.a.asleep = false;
                    n.think_at = t;
                    n.doing = "about".into();
                }
                if hunts_you {
                    self.night.came = true;
                    if hostile {
                        self.phantom_arrives(cid, at);
                    } else {
                        self.arrival_note(at);
                    }
                }
            }
        }
    }

    /// By day, what is left of the dark's dead is gone (out of sight).
    fn fade_dark_remains(&mut self) {
        let gone: Vec<(super::things::ThingId, i64)> = self
            .things
            .live()
            .filter_map(|t| t.origin.remains.map(|c| (t.id, c, t.pos)))
            .filter(|(_, c, p)| self.of_the_dark(ActorId::Npc(*c)) && !self.player_sees(*p))
            .map(|(id, c, _)| (id, c))
            .collect();
        for (id, c) in gone {
            self.drop_worn(c, self.things.get(id).map(|t| t.pos).unwrap_or_default());
            self.things.remove(id);
        }
    }

    /// What someone wore falls where they lie.
    pub(super) fn drop_worn(&mut self, cid: i64, at: Vec3) {
        for w in self.worn_by(ActorId::Npc(cid)) {
            if let Some(t) = self.things.get_mut(w) {
                t.worn = None;
                t.pos = at + Vec3::Y * 0.3;
                t.asleep = false;
                t.dirty = true;
            }
        }
    }

    /// Signs of beings near but out of sight, now and then: often with the
    /// dark about, rarely otherwise (an owl somewhere).
    fn dread(&mut self) {
        let p = self.player.pos;
        let near = self
            .cast
            .npcs
            .iter()
            .filter(|n| n.here() && (!n.species.signs.is_empty() || n.species.touch.harms() && !n.species.sounds.is_empty()) && (n.a.pos - p).length() < 70.0 && !self.player_sees(n.a.pos))
            .min_by(|a, b| (a.a.pos - p).length().total_cmp(&(b.a.pos - p).length()))
            .map(|n| (n.def.id, n.species.touch.harms()));
        let Some((cid, dark)) = near else { return };
        if self.t - self.night.last_dread < if dark { 45.0 } else { 150.0 } {
            return;
        }
        self.sign_of(cid);
    }

    /// One of the signs a being gives (its species wrote them).
    /// Signs never say where (a sign that does is left out); the dark's own
    /// are also heard, and then the game says truly where.
    fn sign_of(&mut self, cid: i64) {
        let Some(n) = self.cast.get(cid) else { return };
        let signs: Vec<String> = n.species.signs.iter().filter(|s| !says_where(s)).cloned().collect();
        let sounds: Vec<String> = if n.species.touch.harms() { n.species.sounds.clone() } else { vec![] };
        let at = n.a.pos;
        let heard = !sounds.is_empty() && (signs.is_empty() || self.rand() < 0.5);
        let line = if heard {
            let i = (self.rand() * sounds.len() as f32) as usize % sounds.len();
            // Heard truly, from where it is.
            self.cue_noise(cid, i, 1.0);
            format!("You hear {} {}.", sounds[i].trim().trim_end_matches('.'), self.where_is(at))
        } else if !signs.is_empty() {
            let i = (self.rand() * signs.len() as f32) as usize % signs.len();
            // A sign that is a sound ("a twig snaps") is heard too, from it.
            if let Some(n) = self.cast.get(cid) {
                if let Some(call) = crate::audio::call::guess(&signs[i], n.species.mass, false) {
                    let mass = n.species.mass;
                    self.cue(at + Vec3::Y, Some(ActorId::Npc(cid)), crate::audio::Heard::Call { call, mass, pitch: 1.0, gain: 0.8 });
                }
            }
            signs[i].clone()
        } else {
            return;
        };
        self.night.last_dread = self.t;
        self.notes.push(Note::Ambient(line));
    }

    // ------------------------------------------------------------ going after

    /// What it goes after, if anything is there: the traveler, anyone, or one of a kind.
    fn quarry(&self, cid: i64) -> Option<(ActorId, Vec3)> {
        let n = self.cast.get(cid)?;
        let pos = n.a.pos;
        let want = n.species.want.as_str();
        match want {
            "" => None,
            "traveler" => Some((ActorId::Player, self.player.pos)).filter(|(_, p)| (*p - pos).length() < 150.0 && !self.fallen()),
            _ => self
                .actor_ids()
                .into_iter()
                .filter(|o| *o != ActorId::Npc(cid))
                .filter_map(|o| self.actor(o).map(|a| (o, a.pos)))
                .filter(|(o, p)| {
                    (*p - pos).length() < 60.0
                        && match want {
                            "anyone" => self.cast.get(cid).map(|n| &n.species.name) != match o {
                                ActorId::Npc(c) => self.cast.get(*c).map(|m| &m.species.name),
                                ActorId::Player => None,
                            },
                            sp => matches!(o, ActorId::Npc(c) if self.cast.get(*c).is_some_and(|m| m.species.name == sp)),
                        }
                })
                .min_by(|a, b| (a.1 - pos).length().total_cmp(&(b.1 - pos).length())),
        }
    }

    /// Go after what it wants: keep out of what it shuns, touch on contact,
    /// draw back after. Returns whether it acted.
    pub(super) fn pursue_want(&mut self, cid: i64) -> bool {
        let Some(n) = self.cast.get(cid) else { return false };
        if n.species.want.is_empty() || n.a.riding.is_some() {
            return false;
        }
        let me = ActorId::Npc(cid);
        let pos = n.a.pos;
        let shuns = n.species.shuns.clone();
        let touch = n.species.touch.clone();
        let since = self.t - n.touched_at;
        let Some((target, tp)) = self.quarry(cid) else { return false };
        let tname = self.actor_name(target);
        let next = |s: &mut Sim, secs: f64| {
            if let Some(n) = s.cast.get_mut(cid) {
                n.think_at = s.t + secs;
            }
        };
        // Just touched: draw back a while.
        if touch.any() && since < TOUCH_GAP {
            let away = (pos - tp).normalize_or_zero();
            let away = if away == Vec3::ZERO { Vec3::X } else { away };
            let p = tp + away * 20.0;
            self.plan(me, vec![super::actions::Action::Goto { target: Target::Point(p.to_array()), run: false }], "draw back", false);
            self.set_doing(cid, "drawing back");
            next(self, 3.0);
            return true;
        }
        // Inside what it shuns: out at once.
        if let Some((at, r)) = self.shunned_at(pos, &shuns) {
            let out = (pos - at).normalize_or_zero();
            let out = if out == Vec3::ZERO { Vec3::Z } else { out };
            let p = at + out * (r + 2.0);
            self.plan(me, vec![super::actions::Action::Goto { target: Target::Point(p.to_array()), run: true }], "get out of the light", false);
            self.set_doing(cid, "shrinking back");
            next(self, 1.5);
            return true;
        }
        // Its quarry inside what it shuns: wait at the edge.
        if let Some((at, r)) = self.shunned_at(tp, &shuns) {
            let side = (pos - at).normalize_or_zero();
            let side = if side == Vec3::ZERO { Vec3::X } else { side };
            let p = at + side * (r + 1.0);
            if (p - pos).length() > 1.5 {
                self.plan(me, vec![super::actions::Action::Goto { target: Target::Point(p.to_array()), run: false }], "wait at the edge", false);
            } else {
                self.plan(me, vec![super::actions::Action::Wait { secs: 2.0 }], "wait at the edge", false);
                if let Some(n) = self.cast.get_mut(cid) {
                    n.a.face(tp - pos, 10.0);
                }
            }
            self.set_doing(cid, &format!("waiting at the edge of the light, watching {tname}"));
            next(self, 2.0);
            return true;
        }
        let d = (tp - pos).length();
        let reach = 2.2 + self.contact_gap(me, target);
        if d < reach {
            if touch.any() {
                self.touch(cid, target);
            } else {
                self.plan(me, vec![super::actions::Action::Wait { secs: 3.0 }], "stay near", false);
            }
            next(self, 1.0);
            return true;
        }
        self.go_after(cid, target, tp, d < 14.0);
        next(self, 1.0);
        true
    }

    /// Head for `target` (stepping round what blocks the way).
    pub(super) fn go_after(&mut self, cid: i64, target: ActorId, tp: Vec3, run: bool) {
        let me = ActorId::Npc(cid);
        let tname = self.actor_name(target);
        let Some(pos) = self.actor(me).map(|a| a.pos) else { return };
        // Blocked (a tree, a wall): step round it, one side or the other.
        if self.actor(me).is_some_and(|a| a.stuck > 0.6) {
            let dir = (tp - pos).normalize_or_zero();
            let side = Vec3::new(-dir.z, 0.0, dir.x) * if self.rand() < 0.5 { 1.0 } else { -1.0 };
            let p = pos + side * 4.0 + dir * 1.5;
            if let Some(a) = self.actor_mut(me) {
                a.stuck = 0.0;
            }
            self.plan(me, vec![super::actions::Action::Goto { target: Target::Point(p.to_array()), run: false }], &format!("go after {tname}"), false);
            return;
        }
        self.plan(me, vec![super::actions::Action::Goto { target: Target::Actor(target), run }], &format!("go after {tname}"), false);
        self.set_doing(cid, &format!("coming for {tname}"));
    }

    /// A watched being that wants the traveler close enough to touch them
    /// without a step: its body's nearest edge within arm's reach of theirs,
    /// out of what it shuns.
    pub(super) fn watched_reach(&mut self, cid: i64) -> bool {
        let Some(n) = self.cast.get(cid) else { return false };
        if n.species.want != "traveler" || !n.species.touch.any() || self.fallen() {
            return false;
        }
        let shuns = n.species.shuns.clone();
        let d = n.a.pos - self.player.pos;
        let gap = Vec3::new(d.x, 0.0, d.z).length() - n.a.dims.radius - crate::world::collide::PLAYER_RADIUS;
        gap < WATCHED_REACH && self.shunned_at(self.player.pos, &shuns).is_none()
    }

    /// Its touch lands on `target`: a glow, a change to their needs, and
    /// for the dark's own, a wound.
    pub fn touch(&mut self, cid: i64, target: ActorId) {
        let Some(n) = self.cast.get_mut(cid) else { return };
        n.touched_at = self.t;
        let touch = n.species.touch.clone();
        let me = ActorId::Npc(cid);
        let name = self.actor_name(me);
        let at = self.actor(target).map(|a| a.pos).unwrap_or_default();
        let hurt = touch.hurt * (1.0 - self.protection(target));
        if target == ActorId::Player {
            self.night.glow = (self.night.glow + touch.glow).min(1.0);
        } else if let ActorId::Npc(c) = target {
            if let Some(m) = self.cast.get_mut(c) {
                if let Some(l) = m.props.get_mut(P_LIGHT) {
                    *l = (*l + touch.glow).min(1.0);
                }
                for (k, v) in &touch.needs {
                    let x = match k.as_str() {
                        "hunger" => &mut m.needs.hunger,
                        "fatigue" => &mut m.needs.fatigue,
                        "social" => &mut m.needs.social,
                        "fun" => &mut m.needs.fun,
                        _ => &mut m.needs.curiosity,
                    };
                    *x = (*x + v).clamp(0.0, 1.0);
                }
            }
        }
        let tname = self.actor_name(target);
        let verb = if touch.harms() { "claws at" } else { "touches" };
        let died = self.wound(target, hurt, me, &format!("clawed by {name}"));
        self.event("touched", Some(me), Some(target.key()), format!("{name} {verb} {tname}"), Some(at), json!({ "hurt": (hurt * 100.0).round() / 100.0, "glow": touch.glow }));
        if touch.harms() {
            let from = self.actor(me).map(|a| a.pos).unwrap_or(at);
            let push = Vec3::new(at.x - from.x, 0.0, at.z - from.z).normalize_or_zero() * 0.25;
            self.jolt(target, push, 0.25);
            self.cue(at + Vec3::Y, Some(target), crate::audio::Heard::Hit { mat: crate::audio::call::Material::FLESH, mass: 60.0, speed: 3.0, by: None });
            self.phantom_struck(cid, target, died);
            if !died {
                self.hurt_by(target, me, hurt, false);
            }
        }
        if target == ActorId::Player {
            let mut what = Vec::new();
            if hurt > 0.0 {
                what.push("it hurts".to_string());
            }
            if touch.glow > 0.0 {
                what.push("you glow faintly".into());
            }
            let tail = if what.is_empty() { String::new() } else { format!(": {}", what.join("; ")) };
            self.notes.push(Note::Notable(format!("{} {verb} you{tail}.", super::physics::cap(&name))));
        } else {
            self.witness(at, 25.0, &format!("{name} {verb} {tname}"), if touch.harms() { 0.8 } else { 0.3 }, &[me]);
            self.note_near(at, 30.0, Note::Ambient(format!("{} {verb} {tname}.", super::physics::cap(&name))));
        }
        if died {
            if let ActorId::Npc(c) = target {
                self.kill(c, &format!("clawed by {name}, and died"), Some(me));
            }
            return;
        }
        // Whoever sees the dark's harm runs.
        if touch.harms() {
            let near: Vec<i64> = self.cast.npcs.iter().filter(|m| m.here() && m.def.id != cid && !m.species.touch.harms() && !m.species.hostile && (m.a.pos - at).length() < 20.0).map(|m| m.def.id).collect();
            for c in near {
                self.flee(c, me, at);
            }
        }
    }
}

/// What every night walker is.
pub fn walker_species() -> crate::world::species::Species {
    let v = json!({
        "name": WALKER, "plural": "night walkers", "body": "night walker", "mass": 60,
        "look": { "height": [2.2, 2.6], "build": [0.75, 0.9], "shade": [0.0, 0.4], "eyes": [0.85, 1.0] },
        "sounds": ["a wet click", "a long, slow breath", "something saying your name, almost", "slow steps that stop when you stop"],
        "signs": [
            "The crickets stop, all at once.",
            "You feel watched.",
            "Something said your name. Or almost.",
            "The night has gone very quiet.",
            "The hair on your neck stands up."
        ],
        "description": "too tall, hunched and black as the dark it came from; two red eyes, and arms that nearly touch the ground",
        "mind": "instinct", "speech": "sounds", "social": "solitary",
        "diet": { "plants": 0.0, "meat": 0.0 },
        "temper": { "bold": 1.0, "wary": 0.0, "playful": 0.0, "tame": 0.0 },
        "life": { "sleep": [7, 19] },
        "move": { "walk": WALKER_WALK, "run": WALKER_RUN },
        "active": "night", "want": "traveler", "shuns": ["light"], "moves_unseen": true,
        "touch": { "hurt": 0.25 }
    });
    let mut sp: crate::world::species::Species = serde_json::from_value(v).expect("night walker");
    sp.sanitize();
    sp
}

/// A sign that says where something is (which only the game may say).
fn says_where(line: &str) -> bool {
    let l: String = line.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    let l = format!(" {} ", l.split_whitespace().collect::<Vec<_>>().join(" "));
    PLACE_WORDS.iter().any(|w| l.contains(&format!(" {w} ")))
}

impl Sim {
    /// The night's part of the status line: the hour to come, and what of
    /// the dark is near (parts that don't apply are left out).
    pub fn night_status(&self) -> Vec<String> {
        let mut out = Vec::new();
        let lv = self.level();
        if lv.walkers + lv.phantoms > 0 {
            let (night, mins) = minutes_to_turn(self.t);
            let m = mins.ceil().max(1.0);
            out.push(if night { format!("dawn in {m:.0}m") } else { format!("night in {m:.0}m") });
        }
        let p = self.player.pos;
        let near = |phantom: bool| self.cast.npcs.iter().filter(|n| n.here() && n.species.hostile == phantom && (phantom || n.species.touch.harms()) && (n.a.pos - p).length() < 60.0).count();
        let (w, ph) = (near(false), near(true));
        let mut parts = Vec::new();
        if w > 0 {
            parts.push(format!("{w} walker{}", if w == 1 { "" } else { "s" }));
        }
        if ph > 0 {
            parts.push(format!("{ph} phantom{}", if ph == 1 { "" } else { "s" }));
        }
        if !parts.is_empty() {
            out.push(format!("{} near", parts.join(" · ")));
        }
        out
    }
}

/// Minutes (real, at normal speed) until the next dusk or dawn, and
/// whether it is dark now.
pub fn minutes_to_turn(t: f64) -> (bool, f64) {
    let p = sky::day_phase(t) as f64;
    let (from, until) = (DARK_FROM as f64, DARK_UNTIL as f64);
    let dark = is_dark(t);
    let to = if dark {
        if p >= from { 1.0 - p + until } else { until - p }
    } else {
        from - p
    };
    (dark, to * DAY_SECONDS / 60.0)
}
