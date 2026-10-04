//! Night, corruption and the power of creation (docs/night-plan.md).
//!
//! Nothing here is written for one monster. Beings keep hours
//! (`Species::active`) and are away outside them; some go after something
//! (`want`) and their touch does something to it (`touch`); some won't come
//! near a property (`shuns`) or move only unwatched (`moves_unseen`).
//! Corruption is a number on beings and a property on things. The world's
//! difficulty scales the harm: on peaceful worlds harmful beings never come.
//! What the traveler makes costs charges, topped up each dawn.

use super::props::{P_CORRUPT, P_LIGHT};
use super::{ActorId, Note, Request, Sim, Target};
use crate::render::sky::{self, DAY_SECONDS};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::world::species::Social;

/// One difficulty's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub name: &'static str,
    /// The traveler's charges at the start, topped up to at dawn (None: no limit).
    pub charges: Option<i32>,
    /// Harmful beings come one night in this many (0: never).
    pub horror_nights: u32,
    /// Harmful beings out on one night...
    pub horrors: u32,
    /// ...and one more every this many nights got through (0: never more).
    pub more_every: u32,
    /// Charges a harmful touch takes, times its species' `touch.charges`.
    pub drain: f32,
    /// Corruption a harmful touch gives, times its species' `touch.corruption`.
    pub corrupt: f32,
    /// How readily corruption passes between people (0: never).
    pub spread: f32,
    /// Corrupted minds twist (one line in their prompts).
    pub twist: bool,
}

pub const LEVELS: [Level; 4] = [
    Level { name: "peaceful", charges: None, horror_nights: 0, horrors: 0, more_every: 0, drain: 0.0, corrupt: 0.0, spread: 0.0, twist: false },
    Level { name: "easy", charges: None, horror_nights: 2, horrors: 1, more_every: 0, drain: 0.0, corrupt: 0.5, spread: 0.0, twist: true },
    Level { name: "normal", charges: Some(24), horror_nights: 1, horrors: 1, more_every: 0, drain: 1.0, corrupt: 1.0, spread: 0.3, twist: true },
    Level { name: "hard", charges: Some(12), horror_nights: 1, horrors: 1, more_every: 2, drain: 4.0, corrupt: 1.0, spread: 1.0, twist: true },
];

/// Most harmful kinds-worth out at once (a lone one, or a pack, each).
const MAX_HORRORS: u32 = 4;
/// How many of a pack kind come together, and the most out at once.
const PACK: [u32; 2] = [3, 4];
const MAX_OUT: u32 = 8;
/// Corruption from which minds twist and the glow shows plainly.
pub const TWISTED: f32 = 0.3;
/// After its touch lands, a being draws back this long (s).
pub const TOUCH_GAP: f64 = 25.0;
/// Gap between its body and the traveler's within which a watched being
/// still reaches them (m): arm's reach, from its nearest edge.
const WATCHED_REACH: f32 = 1.0;
/// A harmful being comes back this far from the traveler, out of sight (m).
const ARRIVE_AT: f32 = 48.0;
/// Corruption fading by day, and again near light (per second).
const FADE_DAY: f32 = 0.0006;
const FADE_LIGHT: f32 = 0.0008;
/// Charges for getting through a night the dark came.
const NIGHT_BONUS: i32 = 2;
/// How long a night waits for its kind to be written before the stock
/// horror comes instead (s of world time).
const WRITE_WAIT: f64 = 60.0;


/// When something that hunts the traveler arrives ({} is where it is,
/// told truly: "behind you", "off to your left").
const ARRIVAL: &[&str] = &[
    "You hear a rustle {}.",
    "Something has come out of the dark, {}.",
    "A twig snaps somewhere {}.",
    "The dark gets a little thicker, {}.",
];
/// Every night horror is called this (more kinds: "night walker 2", …).
pub const HORROR_NAME: &str = "night walker";
/// Every night horror's pace, whatever its shape: tuned to be the right
/// amount of frightening (it walks, then runs the last stretch).
const HORROR_WALK: f32 = 1.1;
const HORROR_RUN: f32 = 3.4;
/// Words that say where something is: signs may not (the game tells it).
const PLACE_WORDS: &[&str] = &["behind", "left", "right", "ahead", "in front", "above", "overhead", "below", "beside", "nearby", "far off", "off to"];

/// The night's own state, saved with the world (kv `sim.night`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NightState {
    /// The traveler's charges (only counted when the difficulty limits them).
    pub charges: i32,
    /// The limit the charges were last counted against (None: unlimited).
    pub limit: Option<i32>,
    /// How much darkness has got into the traveler (0..1).
    pub corruption: f32,
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
    /// Harmful touches on the traveler tonight.
    pub touched: u32,
    /// Something actually came tonight.
    pub came: bool,
    /// Beings the night brought (kept as characters: away by day).
    pub horrors: Vec<i64>,
    /// Their species.
    pub kinds: Vec<String>,
    /// A species being written (request id; not kept: a write cut short by
    /// quitting is simply asked for again).
    #[serde(skip)]
    pub writing: Option<u64>,
    /// When the night started waiting for that writing (asked for, or nightfall).
    #[serde(skip)]
    pub wait_from: f64,
    /// Charges kindness earned today, and which day.
    pub kind_today: (i64, i32),
    #[serde(skip)]
    pub last_dread: f64,
    /// How wide the traveler's view is: tan of half its width (the app
    /// sets it each frame; 0 until then).
    #[serde(skip)]
    pub view_slope: f32,
}

impl NightState {
    pub fn day(t: f64) -> i64 {
        (t / DAY_SECONDS).floor() as i64
    }
}

/// When the dark is about: nightfall to dawn (the sky's night).
pub const DARK_FROM: f32 = 0.80;
/// Sunset, the start of dusk: the dark's first kind starts being written.
pub const SUNSET: f32 = 0.71;
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

    /// The traveler's charges (None: no limit).
    pub fn charges(&self) -> Option<i32> {
        self.level().charges.map(|_| self.night.charges)
    }

    /// Spend one charge on a deed in words. False (and nothing spent) when
    /// they are used up.
    pub fn spend_charge(&mut self) -> bool {
        self.sync_level();
        if self.level().charges.is_none() {
            return true;
        }
        if self.night.charges <= 0 {
            return false;
        }
        self.night.charges -= 1;
        true
    }

    pub fn refund_charge(&mut self) {
        if self.level().charges.is_some() {
            self.night.charges += 1;
        }
    }

    pub fn gain_charges(&mut self, n: i32, why: &str) {
        if self.level().charges.is_none() || n <= 0 {
            return;
        }
        self.night.charges += n;
        let left = self.night.charges;
        self.notes.push(Note::Info(format!("{why} ✦ +{n} ({left})")));
        self.event("charges", Some(ActorId::Player), None, format!("the traveler's power grew by {n}: {why}"), Some(self.player.pos), json!({ "gain": n, "left": left }));
    }

    /// Count the charges against the difficulty, when it changed.
    fn sync_level(&mut self) {
        let limit = self.level().charges;
        // Unlimited for now: remember the count, so going back is no refill.
        if limit.is_none() || limit == self.night.limit {
            return;
        }
        match (self.night.limit, limit) {
            (None, Some(n)) => self.night.charges = n,
            // A harder or easier world: the same share of power, moved by the difference.
            (Some(a), Some(b)) => self.night.charges = (self.night.charges + b - a).max(0),
            _ => {}
        }
        self.night.limit = limit;
    }

    /// Corruption of anyone (0..1).
    pub fn corruption_of(&self, who: ActorId) -> f32 {
        match who {
            ActorId::Player => self.night.corruption,
            ActorId::Npc(c) => self.cast.get(c).map(|n| n.corruption()).unwrap_or(0.0),
        }
    }

    fn add_corruption(&mut self, who: ActorId, d: f32) {
        match who {
            ActorId::Player => self.night.corruption = (self.night.corruption + d).clamp(0.0, 1.0),
            ActorId::Npc(c) => {
                if let Some(v) = self.cast.get_mut(c).and_then(|n| n.props.get_mut(P_CORRUPT)) {
                    *v = (*v + d).clamp(0.0, 1.0);
                }
            }
        }
    }

    /// The line a corrupted mind's prompts carry (None: not corrupted enough,
    /// or the world doesn't twist).
    pub fn twist_line(&self, who: ActorId) -> Option<String> {
        let c = self.corruption_of(who);
        if !self.level().twist || c < TWISTED {
            return None;
        }
        let pct = (c * 100.0).round();
        Some(match who {
            ActorId::Player => format!(
                "Darkness has got into the traveler ({pct}%): whatever they make or change comes out a little wrong (twisted, sinister or spoiled in some way that fits it), and carries some corruption (set its \"corruption\" property, about {c:.1})."
            ),
            ActorId::Npc(_) => format!(
                "Something dark has a hold on you ({pct}%). Your own nature has turned: the generous grasp, the curious pry, the sociable whisper and sow doubt, the crafty make things that harm, the brave pick fights. Let your words and deeds twist in your own way (what you make carries some corruption); never say that you are corrupted."
            ),
        })
    }

    /// Whether `p` is anywhere on the traveler's screen (or just off it),
    /// near enough for the hour: nothing may appear or vanish there.
    pub fn player_sees(&self, p: Vec3) -> bool {
        self.in_view(p, 0.0, 1.1)
    }

    /// Whether the traveler is looking at a body at `p`, `r` wide: any of
    /// it in the middle of the view. The outer tenth on each side doesn't
    /// count, so what moves unseen can still be caught creeping at the edge
    /// of the eye.
    pub fn player_watches(&self, p: Vec3, r: f32) -> bool {
        self.in_view(p, r, 0.8)
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

    /// Once a second: dusk and dawn, beings coming and going, corruption.
    pub fn step_night(&mut self) {
        self.sync_level();
        let night = self.dark();
        let first = self.night.was_night.is_none();
        match self.night.was_night {
            None => {
                // An older world's stock horrors take its current shape.
                let stock = fallback_horror();
                let old: Vec<String> = self.night.kinds.iter().filter(|k| self.snap.species.get(k).is_some_and(|s| s.body == stock.body && (s.look != stock.look || s.moves != stock.moves || s.signs != stock.signs))).cloned().collect();
                for name in old {
                    let mut sp = stock.clone();
                    sp.plural = format!("{name}s");
                    sp.name = name;
                    self.store_species(sp);
                }
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
        self.write_ahead();
        // Nothing came yet tonight (the writing is slow, or there was no
        // room behind the traveler): keep trying.
        if night && self.night.hunting && !self.night.came {
            self.call_the_dark();
        }
        self.comings_and_goings(night, first);
        self.fade(night);
        self.dread();
    }

    fn dusk(&mut self) {
        self.night.nights += 1;
        self.night.touched = 0;
        self.night.came = false;
        let lv = self.level();
        let n = self.night.nights;
        self.night.hunting = lv.horror_nights > 0
            && (n == 1 || match lv.horror_nights {
                1 => true,
                k => crate::noise::pcg(n ^ self.seed as u32 ^ 0xD00D) % k == 0,
            });
        self.event("dusk", None, None, format!("night {n} falls"), Some(self.player.pos), json!({ "night": n, "dark": self.night.hunting }));
        // Written by day and not ready yet: it gets a while more.
        self.night.wait_from = self.t;
        // Nothing tells of it until it is here.
        if self.night.hunting {
            self.call_the_dark();
        }
    }

    fn dawn(&mut self) {
        if let Some(n) = self.level().charges {
            self.night.charges = self.night.charges.max(n);
        }
        if self.night.hunting && self.night.came {
            self.night.survived += 1;
            let clean = self.night.touched == 0;
            self.event("survived_night", Some(ActorId::Player), None, format!("the traveler got through night {}", self.night.nights), Some(self.player.pos), json!({ "untouched": clean, "night": self.night.nights }));
            self.notes.push(Note::Notable("Dawn. The dark draws back.".into()));
            self.gain_charges(NIGHT_BONUS, "You got through the night.");
        }
        self.night.hunting = false;
    }

    /// The dark comes for the traveler: enough harmful beings for tonight,
    /// writing a new kind when there is none yet (or now and then).
    fn call_the_dark(&mut self) {
        let lv = self.level();
        let want = (lv.horrors + if lv.more_every > 0 { self.night.survived / lv.more_every } else { 0 }).min(MAX_HORRORS);
        self.night.horrors.retain(|id| self.cast.get(*id).is_some_and(|n| !n.dead));
        self.night.kinds.retain(|k| self.snap.species.get(k).is_some());
        let have = self.night.horrors.len() as u32;
        let pack_kind = self.night.kinds.last().and_then(|k| self.snap.species.get(k)).is_some_and(|s| s.social == Social::Pack);
        let groups = if pack_kind { have.div_ceil(PACK[0]) } else { have };
        if groups >= want || have >= MAX_OUT {
            return;
        }
        let fresh_kind = self.night.kinds.is_empty() || (self.night.nights % 5 == 0 && self.night.kinds.len() < 3);
        // Waited long enough: the stock horror (or the last kind) comes now,
        // and the written one joins the dark when it is ready.
        let waited = self.night.writing.is_some() && self.t - self.night.wait_from > WRITE_WAIT;
        if fresh_kind && self.has_llm && !waited {
            if self.night.writing.is_none() {
                self.write_horror();
            }
            return;
        }
        let kind = match self.night.kinds.last().cloned() {
            Some(k) => k,
            None => {
                let mut sp = fallback_horror();
                sp.name = self.free_horror_name();
                sp.plural = format!("{}s", sp.name);
                let name = sp.name.clone();
                self.store_species(sp);
                self.night.kinds.push(name.clone());
                name
            }
        };
        let Some(sp) = self.snap.species.get(&kind).cloned() else { return };
        // A pack comes as one: a few together, from one spot.
        let size = if sp.social == Social::Pack { PACK[0] + (self.rand() * (PACK[1] - PACK[0] + 1) as f32) as u32 } else { 1 };
        for _ in groups..want {
            let Some(at) = self.arrival_spot(ARRIVE_AT) else { break };
            let mut first = None;
            for i in 0..size.min(MAX_OUT.saturating_sub(self.night.horrors.len() as u32)) {
                let a = i as f32 * 2.4 + self.rand();
                let p = if i == 0 { at } else { at + Vec3::new(a.cos(), 0.0, a.sin()) * (1.5 + self.rand() * 2.5) };
                let p = Vec3::new(p.x, self.snap.terrain.height(p.x, p.z), p.z);
                if let Some(id) = self.bring_horror(&sp, p) {
                    first.get_or_insert(id);
                }
            }
            if first.is_some() {
                self.arrival_note(at);
            }
        }
    }

    /// Have the dark's first kind written from sunset, quietly, so it is
    /// ready at nightfall (it is stored, never shown or told of).
    fn write_ahead(&mut self) {
        if self.level().horror_nights == 0 || !self.has_llm || self.night.writing.is_some() {
            return;
        }
        // A new world gets its first day to choose its difficulty: not
        // before its first sunset. After that, as soon as it is missing.
        if self.night.nights == 0 && !self.dark() && sky::day_phase(self.t) < SUNSET {
            return;
        }
        self.night.kinds.retain(|k| self.snap.species.get(k).is_some());
        if !self.night.kinds.is_empty() {
            return;
        }
        self.write_horror();
    }

    fn write_horror(&mut self) {
        let id = self.next_id();
        self.night.writing = Some(id);
        self.night.wait_from = self.t;
        let mut fixed = horror_fixed();
        let name = self.free_horror_name();
        fixed["name"] = json!(name);
        fixed["plural"] = json!(format!("{name}s"));
        self.request_now(Request::NewSpecies { id, brief: horror_brief(), fixed });
    }

    /// "night walker", or the first "night walker N" no species has yet.
    fn free_horror_name(&self) -> String {
        (1..).map(|i| if i == 1 { HORROR_NAME.to_string() } else { format!("{HORROR_NAME} {i}") }).find(|n| self.snap.species.get(n).is_none() && !self.night.kinds.contains(n)).unwrap()
    }

    /// A species the brain wrote (for `Request::NewSpecies`) is ready.
    pub fn on_species_made(&mut self, id: u64, name: Option<String>) {
        if self.night.writing != Some(id) {
            return;
        }
        self.night.writing = None;
        let rows = self.db.with(|c| crate::db::species_rows(c)).unwrap_or_default();
        let book = crate::world::species::SpeciesBook::load(&rows, self.db.kv_get("species.world").as_deref());
        let made = name.and_then(|n| book.get(&n).map(|s| (**s).clone()));
        let sp = match made {
            Some(mut sp) => {
                merge_fixed(&mut sp, &horror_fixed());
                fit_horror(&mut sp);
                // Its own body failed: the stock one, not a stray quadruped.
                if matches!(sp.body.as_str(), "quadruped" | "figure") {
                    let stock = fallback_horror();
                    sp.body = stock.body;
                    sp.look = stock.look;
                }
                sp
            }
            None => fallback_horror(),
        };
        let name = sp.name.clone();
        self.store_species(sp);
        if !self.night.kinds.contains(&name) {
            self.night.kinds.push(name);
        }
        if self.dark() && self.night.hunting {
            self.call_the_dark();
        }
    }

    /// Keep a species in the save and use it now.
    fn store_species(&mut self, mut sp: crate::world::species::Species) {
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

    fn bring_horror(&mut self, sp: &crate::world::species::Species, at: Vec3) -> Option<i64> {
        let persona = crate::world::Persona { name: format!("the {}", sp.name), species: sp.name.clone(), appearance: sp.description.clone(), ..Default::default() };
        let state = crate::world::characters::SavedState { x: at.x, z: at.z, corruption: 1.0, ..Default::default() };
        let id = self.add_being(persona, at, state)?;
        self.night.horrors.push(id);
        self.night.came = true;
        self.event("dark_came", Some(ActorId::Npc(id)), Some("player".into()), format!("{} came out of the dark", self.actor_name(ActorId::Npc(id))), Some(at), json!({ "species": sp.name }));
        Some(id)
    }

    /// Tell the traveler, quietly, that something has arrived.
    fn arrival_note(&mut self, at: Vec3) {
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
    fn arrival_spot(&mut self, dist: f32) -> Option<Vec3> {
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
    /// before the traveler's eyes.
    /// `quiet`: the first look after loading (older saves didn't keep who was
    /// away): out-of-hours beings are simply gone, nothing told.
    fn comings_and_goings(&mut self, night: bool, quiet: bool) {
        let hunting = self.night.hunting;
        let ids: Vec<(i64, bool, bool, Vec3, bool)> = self
            .cast
            .npcs
            .iter()
            .filter(|n| !n.dead && (n.species.active != crate::world::species::Active::Always || n.species.touch.harms()))
            .map(|n| {
                let harms = n.species.touch.harms();
                let due = n.species.about(night) && (!harms || hunting);
                (n.def.id, n.away, due, n.a.pos, harms && n.species.want == "traveler")
            })
            .collect();
        for (cid, away, due, pos, hunts_you) in ids {
            if !away && !due {
                // Slip away out of sight (or, watched, now and then: gone between glances).
                if !quiet && self.player_sees(pos) && self.rand() < 0.85 {
                    continue;
                }
                let name = self.actor_name(ActorId::Npc(cid));
                if let Some(h) = self.actor(ActorId::Npc(cid)).and_then(|a| a.held) {
                    self.release(h);
                }
                self.social.joints.retain(|j| !j.has(ActorId::Npc(cid)));
                if let Some(n) = self.cast.get_mut(cid) {
                    n.away = true;
                    n.plan.clear();
                    n.a.task = None;
                    n.doing = "away".into();
                }
                if !quiet && self.dist_to_player(pos) < 60.0 && hunts_you {
                    self.notes.push(Note::Ambient(format!("{} is gone with the light.", super::physics::cap(&name))));
                }
            } else if away && due {
                let at = if hunts_you { self.arrival_spot(ARRIVE_AT) } else { Some(pos).filter(|p| !self.player_sees(*p)) };
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
                    self.arrival_note(at);
                }
            }
        }
    }

    /// Corruption fades by day, faster near light (a glow on a body fades
    /// by a rule; the traveler's here).
    fn fade(&mut self, night: bool) {
        let light = vec!["light".to_string()];
        let lit_player = self.shunned_at(self.player.pos, &light).is_some();
        let k = |lit: bool| if night { 0.0 } else { FADE_DAY } + if lit { FADE_LIGHT } else { 0.0 };
        self.night.corruption = (self.night.corruption - k(lit_player)).max(0.0);
        self.night.glow = (self.night.glow - 0.002).max(0.0);
        let ids: Vec<(i64, Vec3)> = self.cast.npcs.iter().filter(|n| n.here() && n.corruption() > 0.0 && !n.species.touch.harms()).map(|n| (n.def.id, n.a.pos)).collect();
        for (cid, pos) in ids {
            let lit = self.dist_to_player(pos) < self.cfg.medium && self.shunned_at(pos, &light).is_some();
            self.add_corruption(ActorId::Npc(cid), -k(lit));
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
            "traveler" => Some((ActorId::Player, self.player.pos)).filter(|(_, p)| (*p - pos).length() < 150.0),
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
            self.plan(me, vec![Action::Goto { target: Target::Point(p.to_array()), run: false }], "draw back", false);
            self.set_doing(cid, "drawing back");
            next(self, 3.0);
            return true;
        }
        // Inside what it shuns: out at once.
        if let Some((at, r)) = self.shunned_at(pos, &shuns) {
            let out = (pos - at).normalize_or_zero();
            let out = if out == Vec3::ZERO { Vec3::Z } else { out };
            let p = at + out * (r + 2.0);
            self.plan(me, vec![Action::Goto { target: Target::Point(p.to_array()), run: true }], "get out of the light", false);
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
                self.plan(me, vec![Action::Goto { target: Target::Point(p.to_array()), run: false }], "wait at the edge", false);
            } else {
                self.plan(me, vec![Action::Wait { secs: 2.0 }], "wait at the edge", false);
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
                self.plan(me, vec![Action::Wait { secs: 3.0 }], "stay near", false);
            }
            next(self, 1.0);
            return true;
        }
        // Blocked (a tree, a wall): step round it, one side or the other.
        if self.actor(me).is_some_and(|a| a.stuck > 0.6) {
            let dir = (tp - pos).normalize_or_zero();
            let side = Vec3::new(-dir.z, 0.0, dir.x) * if self.rand() < 0.5 { 1.0 } else { -1.0 };
            let p = pos + side * 4.0 + dir * 1.5;
            if let Some(a) = self.actor_mut(me) {
                a.stuck = 0.0;
            }
            self.plan(me, vec![Action::Goto { target: Target::Point(p.to_array()), run: false }], &format!("go after {tname}"), false);
            next(self, 2.5);
            return true;
        }
        self.plan(me, vec![Action::Goto { target: Target::Actor(target), run: d < 14.0 }], &format!("go after {tname}"), false);
        self.set_doing(cid, &format!("coming for {tname}"));
        next(self, 1.0);
        true
    }

    /// A watched being that wants the traveler close enough to touch them
    /// without a step: its body's nearest edge within arm's reach of theirs,
    /// out of what it shuns.
    pub(super) fn watched_reach(&mut self, cid: i64) -> bool {
        let Some(n) = self.cast.get(cid) else { return false };
        if n.species.want != "traveler" || !n.species.touch.any() {
            return false;
        }
        let shuns = n.species.shuns.clone();
        let d = n.a.pos - self.player.pos;
        let gap = Vec3::new(d.x, 0.0, d.z).length() - n.a.dims.radius - crate::world::collide::PLAYER_RADIUS;
        gap < WATCHED_REACH && self.shunned_at(self.player.pos, &shuns).is_none()
    }

    /// Its touch lands on `target`.
    pub fn touch(&mut self, cid: i64, target: ActorId) {
        let Some(n) = self.cast.get_mut(cid) else { return };
        n.touched_at = self.t;
        let touch = n.species.touch.clone();
        let me = ActorId::Npc(cid);
        let lv = self.level();
        let name = self.actor_name(me);
        let at = self.actor(target).map(|a| a.pos).unwrap_or_default();
        let lost = if target == ActorId::Player && lv.charges.is_some() { ((touch.charges * lv.drain).round() as i32).min(self.night.charges.max(0)) } else { 0 };
        let dark = touch.corruption * lv.corrupt * 0.5;
        if target == ActorId::Player {
            self.night.charges -= lost;
            self.night.glow = (self.night.glow + touch.glow).min(1.0);
            if touch.harms() {
                self.night.touched += 1;
            }
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
        if dark > 0.0 {
            self.add_corruption(target, dark);
        }
        let tname = self.actor_name(target);
        self.event("touched", Some(me), Some(target.key()), format!("{name} touched {tname}"), Some(at), json!({ "charges": lost, "corruption": dark, "glow": touch.glow }));
        if target == ActorId::Player {
            let mut what = Vec::new();
            if lost > 0 {
                what.push(format!("your power ebbs (✦ −{lost}, {} left)", self.night.charges));
            }
            if dark > 0.0 {
                what.push("something cold gets in".into());
            }
            if touch.glow > 0.0 {
                what.push("you glow faintly".into());
            }
            let tail = if what.is_empty() { String::new() } else { format!(": {}", what.join("; ")) };
            self.notes.push(Note::Notable(format!("{} touches you{tail}.", super::physics::cap(&name))));
        } else {
            self.witness(at, 25.0, &format!("{name} touched {tname}"), if touch.harms() { 0.8 } else { 0.3 }, &[me]);
            self.note_near(at, 30.0, Note::Ambient(format!("{} touches {tname}.", super::physics::cap(&name))));
        }
        // Whoever sees a harmful touch runs.
        if touch.harms() {
            let near: Vec<i64> = self.cast.npcs.iter().filter(|m| m.here() && m.def.id != cid && !m.species.touch.harms() && (m.a.pos - at).length() < 20.0).map(|m| m.def.id).collect();
            for c in near {
                self.flee(c, me, at);
            }
        }
    }
}

use super::actions::Action;

/// What the brain is asked for when the dark needs a new kind.
fn horror_brief() -> String {
    "Invent one night horror for this world (it is called the night walker; its name is given): what comes out of the dark at night for the traveler, because the traveler can make things out of nothing. Let this world's lore choose where it comes from and how it is felt, and its form: it may walk upright, crawl, slither, or glide low on wings; be one huge thing or a pack of lean ones. Its body must borrow nothing from this world's own people or animals: no ears, fur, whiskers, tails, muzzles or faces like theirs, nothing cute or familiar. It must be frightening to meet in a dark, low-resolution world: an unnatural silhouette (wrong proportions, joints that bend wrong, too many of something, a face that is not a face). It is black, the colour of the dark itself (never purple or any bright colour, and its body gives no light); its eyes, two or many, glow red, and are the only lit part. Give it a description of under 25 words, a body (a new body name of your own, not \"night walker\", with a body_description of how it looks and moves, which says it is black all over, borrows nothing from the world's creatures, and only its red eyes shine, marked with glow()), \"social\": \"solitary\" if it comes alone or \"pack\" if a few come together, a \"size\" (1 for something person-sized, up to 3 for something huge), look ranges that keep it near black, 2–4 \"sounds\": what the traveler hears of it, each read after \"You hear\" (\"a wet click\", \"slow, dragging steps\"), with a \"voice\" for each that is wrong to hear (breath that is too slow, a groan too low, clicks too wet), and 5–8 \"signs\": short lines, to the traveler, of what they notice when it is near but out of sight, in this world's own textures (\"The crickets stop, all at once.\", \"You feel watched.\"); signs never say where it is (no behind, ahead, left or right: the game says that), and none may name or describe it outright.".into()
}

/// What every night horror is, whatever the brain wrote.
fn horror_fixed() -> serde_json::Value {
    json!({
        "mind": "instinct", "speech": "sounds",
        "diet": { "plants": 0.0, "meat": 0.0 },
        "temper": { "bold": 1.0, "wary": 0.0, "playful": 0.0, "tame": 0.0 },
        "life": { "sleep": [7, 19] },
        "active": "night", "want": "traveler", "shuns": ["light"], "moves_unseen": true,
        "touch": { "charges": 1.0, "corruption": 0.5 }
    })
}

/// Fixed fields laid over a written species.
pub fn merge_fixed(sp: &mut crate::world::species::Species, fixed: &serde_json::Value) {
    let Ok(mut v) = serde_json::to_value(&*sp) else { return };
    if let (Some(o), Some(f)) = (v.as_object_mut(), fixed.as_object()) {
        for (k, x) in f {
            o.insert(k.clone(), x.clone());
        }
    }
    if let Ok(s) = serde_json::from_value(v) {
        *sp = s;
    }
    // Never faster than the traveler can walk away.
    sp.moves.walk = sp.moves.walk.min(1.6);
    sp.moves.run = sp.moves.run.clamp(sp.moves.walk, 3.8);
}

/// What the brain may choose for a horror, kept inside what is fair: alone
/// or a pack, no bigger than a house, never faster than the traveler.
pub fn fit_horror(sp: &mut crate::world::species::Species) {
    if sp.social != Social::Pack {
        sp.social = Social::Solitary;
    }
    sp.size = sp.size.clamp(0.5, 3.5);
    // One pace for all of them; winged ones glide low at it, never take off.
    sp.moves.walk = HORROR_WALK;
    sp.moves.run = HORROR_RUN;
    sp.moves.fly = 0.0;
}

/// A sign that says where something is (which only the game may say).
fn says_where(line: &str) -> bool {
    let l: String = line.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    let l = format!(" {} ", l.split_whitespace().collect::<Vec<_>>().join(" "));
    PLACE_WORDS.iter().any(|w| l.contains(&format!(" {w} ")))
}

/// A night horror for when no LLM writes one.
pub fn fallback_horror() -> crate::world::species::Species {
    let mut v = json!({
        "name": "night walker", "plural": "night walkers", "body": "night walker", "mass": 60,
        "look": { "height": [2.2, 2.6], "build": [0.75, 0.9], "shade": [0.0, 0.4], "eyes": [0.85, 1.0] },
        "sounds": ["a wet click", "a long, slow breath", "something saying your name, almost", "slow steps that stop when you stop"],
        "signs": [
            "The crickets stop, all at once.",
            "You feel watched.",
            "Something said your name. Or almost.",
            "The night has gone very quiet.",
            "The hair on your neck stands up."
        ],
        "description": "too tall, hunched and black as the dark it came from; two red eyes, and arms that nearly touch the ground"
    });
    if let (Some(o), Some(f)) = (v.as_object_mut(), horror_fixed().as_object()) {
        for (k, x) in f {
            o.insert(k.clone(), x.clone());
        }
    }
    let mut sp: crate::world::species::Species = serde_json::from_value(v).expect("fallback horror");
    sp.sanitize();
    fit_horror(&mut sp);
    sp
}

impl Sim {
    /// The night's part of the status line: the hour to come, charges,
    /// corruption (parts that don't apply are left out).
    pub fn night_status(&self) -> Vec<String> {
        let mut out = Vec::new();
        let lv = self.level();
        if lv.horror_nights > 0 {
            let (night, mins) = minutes_to_turn(self.t);
            let m = mins.ceil().max(1.0);
            out.push(if night { format!("dawn in {m:.0}m") } else { format!("night in {m:.0}m") });
        }
        out.push(match self.charges() {
            Some(n) => format!("✦ {n}"),
            None => "✦ ∞".into(),
        });
        if self.night.corruption >= 0.05 {
            out.push(format!("corrupted {:.0}%", self.night.corruption * 100.0));
        }
        let p = self.player.pos;
        let near = self.cast.npcs.iter().filter(|n| n.here() && n.corruption() >= TWISTED && !n.species.touch.harms() && (n.a.pos - p).length() < 60.0).count();
        if near > 0 {
            out.push(format!("{near} corrupted near"));
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
