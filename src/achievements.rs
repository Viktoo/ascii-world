//! Achievements: things a player can do in a world, noticed as they
//! happen. Each one is a line in `ALL` and an arm in `check`.
//!
//! They are yours, not the world's: kept in ~/.pocket/achievements.json by
//! world id, so sharing a world never shares them. Only ids are kept (and
//! small progress notes), so rewording, re-tiering, changing the rule or
//! dropping one never breaks anything. Never rename an id.
//!
//! Notes are hints and the world is the truth: a note may point at someone
//! the world no longer has (the world was shared and changed), and checks
//! simply skip what is gone.

use crate::db::Db;
use crate::sim::actor::GestureKind;
use crate::sim::{ActorId, Sim, SimEvent};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Tier {
    Bronze,
    Silver,
    Gold,
    Diamond,
}

pub struct Def {
    pub id: &'static str,
    pub group: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    pub tier: Tier,
}

const fn a(id: &'static str, group: &'static str, title: &'static str, text: &'static str, tier: Tier) -> Def {
    Def { id, group, title, text, tier }
}

use Tier::*;

pub const ALL: &[Def] = &[
    a("word_made_real", "First steps", "Word Made Real", "Type something with / and watch it fade into the world.", Bronze),
    a("second_draft", "First steps", "Second Draft", "Change something that's already there: punch a hole in it, add a part, or paint it.", Bronze),
    a("fresh_start", "First steps", "Fresh Start", "Use /undo to take back something you made.", Bronze),
    a("tinkerer", "First steps", "Tinkerer", "Use what you're holding on something and see what happens.", Bronze),
    a("lost_and_found", "First steps", "Lost and Found", "Throw something through a hoop, a well or any opening, and have the world notice.", Silver),
    a("first_spark", "Fire, water and stuff", "First Spark", "Set something alight.", Bronze),
    a("doused", "Fire, water and stuff", "Doused", "Put out a fire with water before it burns itself out.", Silver),
    a("chain_reaction", "Fire, water and stuff", "Chain Reaction", "Light one thing and watch the fire spread to three others by itself.", Silver),
    a("ashes_to_saplings", "Fire, water and stuff", "Ashes to Saplings", "Come back later and find the plants you burned growing again.", Gold),
    a("mad_scientist", "Fire, water and stuff", "Mad Scientist", "Combine two things in a way nobody planned, so the world has to decide what happens.", Gold),
    a("hello_world", "People", "Hello, World", "Talk to someone who brings up your last visit without being reminded.", Gold),
    a("ask_and_receive", "People", "Ask and Receive", "Ask someone to make something, and they make it and hand it to you.", Gold),
    a("accepted", "People", "Accepted", "Someone agrees to a hug, because they like you enough.", Silver),
    a("rejected", "People", "Rejected", "Someone says no.", Bronze),
    a("new_move", "People", "New Move", "Invent a gesture and later see someone else do it.", Gold),
    a("tagalong", "Creatures", "Tagalong", "An animal decides to follow you.", Silver),
    a("good_boy", "Creatures", "Good Boy", "Throw something and have it brought back.", Silver),
    a("stampede", "Creatures", "Stampede", "See a whole group bolt together.", Silver),
    a("dressed_up", "Creatures", "Dressed Up", "A being agrees to wear something you give it.", Silver),
    a("makeover", "Creatures", "Makeover", "Change a being with an action: its looks, a new trick, or what it is.", Gold),
    a("trust_issues", "Creatures", "Trust Issues", "Do something to a being that makes it trust you less.", Bronze),
    a("saddled", "Creatures", "Saddled", "Ride something much bigger than you.", Gold),
    a("thrown", "Creatures", "Thrown", "Get bucked off by a scared mount.", Silver),
    a("heavy_lifting", "Together", "Heavy Lifting", "Carry something with someone because it's too heavy for one.", Silver),
    a("word_travels", "Together", "Word Travels", "Someone tells you something they only heard from someone else.", Gold),
    a("living_legend", "Together", "Living Legend", "A stranger in another village has heard a story about you.", Diamond),
    a("unlikely_friends", "Together", "Unlikely Friends", "Two beings who started out cold become friends.", Gold),
    a("new_life", "Generations", "New Life", "Be there when a young one is born.", Silver),
    a("all_grown_up", "Generations", "All Grown Up", "Watch that young one become an adult.", Gold),
    a("kin_of_kin", "Generations", "Kin of Kin", "Meet the grandchild of a being you know.", Gold),
    a("domesticated", "Generations", "Domesticated", "Feed a family for generations until it's tamer than its ancestors.", Diamond),
    a("a_name_of_their_own", "Generations", "A Name of Their Own", "That family gets a name of its own.", Diamond),
    a("origin_of_species", "Generations", "Origin of Species", "It becomes a whole new species.", Diamond),
    a("life_goes_on", "Emergence", "Life Goes On", "Be gone a whole day and come back to a birth, a fire or something new.", Gold),
    a("nobody_touched_it", "Emergence", "Nobody Touched It", "A character makes something new for their own reasons.", Gold),
    a("pocket_universe", "Emergence", "Pocket Universe", "Earn every challenge.", Diamond),
];

pub fn def(id: &str) -> Option<&'static Def> {
    ALL.iter().find(|d| d.id == id)
}

/// Families the player has fed: lineage → how tame the one fed was.
const FED: &str = "_fed";
/// How often the world itself is looked at (sim seconds).
const STATE_EVERY: f64 = 1.0;

/// What you have done in one world.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Record {
    /// id → when earned (unix seconds).
    earned: BTreeMap<String, f64>,
    /// id → progress notes, for the few that need to remember something.
    notes: BTreeMap<String, Value>,
    /// When you were last in this world (unix seconds).
    last_seen: Option<f64>,
}

/// Your achievements in every world, by world id.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Profile {
    worlds: BTreeMap<String, Record>,
}

fn profile_path(world: &str) -> PathBuf {
    // Tests never see (or write) the player's own.
    if cfg!(test) {
        return std::env::temp_dir().join(format!("pocket-test-achievements-{world}.json"));
    }
    crate::log::dir().join("achievements.json")
}

fn read_profile(path: &PathBuf) -> Profile {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub struct Tracker {
    world: String,
    path: PathBuf,
    earned: BTreeMap<String, f64>,
    notes: BTreeMap<String, Value>,
    /// Notes changed since the profile was last written.
    unsaved: bool,
    last_write: f64,
    /// Events already looked at (`sim.log.total`).
    seen: u64,
    next_state: f64,
    /// This session began after a whole game day away.
    away: bool,
    started: f64,
}

/// What a check can see and remember.
struct Cx<'a> {
    sim: &'a Sim,
    note: &'a mut Value,
    changed: bool,
    fed: &'a Value,
    away: bool,
    started: f64,
}

impl Cx<'_> {
    fn note(&mut self) -> &mut Value {
        self.changed = true;
        self.note
    }
}

impl Tracker {
    pub fn load(db: &Db, sim: &Sim) -> Tracker {
        let world = db.world_id();
        let path = profile_path(&world);
        let rec = read_profile(&path).worlds.remove(&world).unwrap_or_default();
        let away = rec.last_seen.is_some_and(|l| crate::db::now() - l >= crate::render::sky::DAY_SECONDS);
        Tracker { world, path, earned: rec.earned, notes: rec.notes, unsaved: false, last_write: 0.0, seen: sim.log.total, next_state: sim.t, away, started: sim.t }
    }

    /// Write this world's record into the profile (other worlds as they are
    /// on disk, in case another game is open).
    fn save(&mut self) {
        let now = crate::db::now();
        let mut p = read_profile(&self.path);
        let notes = self.notes.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.clone(), v.clone())).collect();
        p.worlds.insert(self.world.clone(), Record { earned: self.earned.clone(), notes, last_seen: Some(now) });
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("json.tmp");
        if let Ok(t) = serde_json::to_string_pretty(&p) {
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
        self.unsaved = false;
        self.last_write = now;
    }

    pub fn earned(&self, id: &str) -> Option<f64> {
        self.earned.get(id).copied()
    }

    pub fn count(&self) -> usize {
        ALL.iter().filter(|d| self.earned.contains_key(d.id)).count()
    }

    /// Keep notes, and that the player is here (for "be gone a whole day").
    pub fn touch(&mut self) {
        if self.unsaved || crate::db::now() - self.last_write > 30.0 {
            self.save();
        }
    }

    /// Earn one directly (for things the app sees, like /undo).
    pub fn grant(&mut self, id: &str) -> Vec<&'static Def> {
        let mut got = Vec::new();
        self.earn(id, &mut got);
        self.complete(&mut got);
        got
    }

    fn earn(&mut self, id: &str, got: &mut Vec<&'static Def>) {
        let Some(d) = def(id) else { return };
        if self.earned.contains_key(id) {
            return;
        }
        let now = crate::db::now();
        self.earned.insert(id.to_string(), now);
        // Earned: its progress is no longer needed.
        self.notes.remove(id);
        got.push(d);
        self.save();
    }

    fn complete(&mut self, got: &mut Vec<&'static Def>) {
        if ALL.iter().all(|d| d.id == "pocket_universe" || self.earned.contains_key(d.id)) {
            self.earn("pocket_universe", got);
        }
    }

    /// Look at what happened since last time; returns what was just earned.
    pub fn update(&mut self, sim: &Sim) -> Vec<&'static Def> {
        let mut got = Vec::new();
        let fresh = (sim.log.total.saturating_sub(self.seen) as usize).min(sim.log.recent.len());
        self.seen = sim.log.total;
        let start = sim.log.recent.len() - fresh;
        for e in sim.log.recent.range(start..) {
            if note_fed(sim, e, self.notes.entry(FED.into()).or_insert(json!({}))) {
                self.unsaved = true;
            }
            self.run(sim, Some(e), &mut got);
        }
        if sim.t >= self.next_state {
            self.next_state = sim.t + STATE_EVERY;
            self.run(sim, None, &mut got);
        }
        self.complete(&mut got);
        got
    }

    fn run(&mut self, sim: &Sim, e: Option<&SimEvent>, got: &mut Vec<&'static Def>) {
        let fed = self.notes.get(FED).cloned().unwrap_or(Value::Null);
        for d in ALL {
            if self.earned.contains_key(d.id) {
                continue;
            }
            let note = self.notes.entry(d.id.to_string()).or_insert(Value::Null);
            let mut cx = Cx { sim, note, changed: false, fed: &fed, away: self.away, started: self.started };
            let hit = check(d.id, &mut cx, e);
            self.unsaved |= cx.changed;
            if hit {
                self.earn(d.id, got);
            }
        }
    }
}

// ---------------------------------------------------------------- checks

/// One arm per achievement. `e` is a new event, or None for a look at the
/// world as it is (about once a second).
fn check(id: &str, cx: &mut Cx, e: Option<&SimEvent>) -> bool {
    let sim = cx.sim;
    let Some(e) = e else { return check_state(id, cx) };
    let by_player = e.actor == Some(ActorId::Player);
    let at_player = e.subject.as_deref() == Some("player");
    match id {
        "word_made_real" => e.kind == "made" && by_player,
        "second_draft" => e.kind == "reshaped" && by_player,
        "tinkerer" => (e.kind == "used" && by_player) || used_unplanned(e),
        "lost_and_found" => e.kind == "through" && by_player,
        "first_spark" => e.kind == "ignited" && near(sim, e, 30.0) && player_acted(sim, e.t, 15.0),
        "doused" => e.kind == "doused" && near(sim, e, 30.0) && player_acted(sim, e.t, 15.0),
        "chain_reaction" => e.kind == "ignited" && near(sim, e, 60.0) && spread_from_spark(sim, e) >= 4,
        "ashes_to_saplings" => {
            if e.kind == "burnt_out" && near(sim, e, 60.0) && cx.note.is_null() {
                *cx.note() = json!({ "burnt": e.t });
            }
            e.kind == "regrown" && !cx.note.is_null()
        }
        "mad_scientist" => used_unplanned(e),
        "ask_and_receive" => {
            e.kind == "gave" && to(e) == Some(ActorId::Player) && matches!(e.actor, Some(ActorId::Npc(_))) && sim.log.recent.iter().any(|x| x.kind == "agreed" && x.actor == e.actor && x.subject.as_deref() == Some("player"))
        }
        "accepted" => (e.kind == "together" && by_player && data_str(e, "kind").contains("Hug")) || (e.kind == "accepted" && at_player && e.text.contains("hug")),
        "rejected" => matches!(e.kind.as_str(), "refused" | "declined") && !by_player && (at_player || e.text.contains(&sim.actor_name(ActorId::Player))),
        "new_move" => {
            if e.kind != "gesture" || !is_custom(data_str(e, "kind")) {
                return false;
            }
            let k = data_str(e, "kind").to_string();
            if by_player {
                let n = cx.note();
                if !n.is_array() {
                    *n = json!([]);
                }
                if let Some(v) = n.as_array_mut().filter(|v| !v.iter().any(|x| x == &k)) {
                    v.push(json!(k));
                }
                return false;
            }
            cx.note.as_array().is_some_and(|v| v.iter().any(|x| x == &k))
        }
        "good_boy" => {
            e.kind == "gave" && to(e) == Some(ActorId::Player) && e.actor.is_some_and(|a| is_being(sim, a)) && sim.log.recent.iter().any(|x| x.kind == "fetch" && x.actor == e.actor && x.subject == e.subject)
        }
        "stampede" => {
            e.kind == "fled"
                && e.data.get("herd").and_then(Value::as_bool) == Some(true)
                && near(sim, e, 60.0)
                && e.pos.is_some_and(|p| sim.cast.npcs.iter().filter(|n| !n.dead && n.doing.starts_with("fleeing") && (n.a.pos - glam::Vec3::from(p)).length() < 40.0).count() >= 3)
        }
        "dressed_up" => e.kind == "wore" && by_player && e.data.get("on").and_then(actor).is_some_and(|o| o != ActorId::Player),
        "makeover" => {
            (e.kind == "deed_on" && by_player && e.data.get("looks").and_then(Value::as_bool) == Some(true))
                || (e.kind == "learned" && at_player)
                || (e.kind == "transformed" && at_player && matches!(e.actor, Some(ActorId::Npc(_))))
        }
        "trust_issues" => e.kind == "deed_on" && by_player && e.data.get("trust").and_then(Value::as_f64).is_some_and(|t| t < 0.0),
        "saddled" => {
            let bulk = |a: ActorId| sim.actor(a).map(|x| x.dims.height * x.dims.radius * x.dims.radius).unwrap_or(0.0);
            e.kind == "mounted" && by_player && e.subject.as_deref().and_then(ActorId::parse).is_some_and(|m| bulk(m) >= 3.0 * bulk(ActorId::Player))
        }
        "thrown" => e.kind == "thrown" && at_player,
        "heavy_lifting" => e.kind == "carry" && (by_player || e.data.get("with").and_then(actor) == Some(ActorId::Player)),
        "unlikely_friends" => false,
        "new_life" => e.kind == "born" && near(sim, e, 30.0),
        "all_grown_up" => {
            if let (true, Some(ActorId::Npc(kid))) = (e.kind == "born" && near(sim, e, 30.0), e.actor) {
                let n = cx.note();
                if !n.is_array() {
                    *n = json!([]);
                }
                if let Some(v) = n.as_array_mut() {
                    v.push(json!(kid));
                }
            }
            false
        }
        "domesticated" => {
            e.kind == "born"
                && fed_tame(cx.fed, e).is_some_and(|was| e.data.get("tame").and_then(Value::as_f64).is_some_and(|now| now >= was + 0.15))
        }
        "a_name_of_their_own" => e.kind == "new_variety" && fed_tame(cx.fed, e).is_some(),
        "origin_of_species" => e.kind == "new_species" && fed_tame(cx.fed, e).is_some(),
        "life_goes_on" => {
            cx.away
                && e.t - cx.started < 600.0
                && (matches!(e.kind.as_str(), "born" | "ignited" | "new_variety" | "new_species")
                    || (e.kind == "caught_up" && e.data.get("events").and_then(Value::as_u64).unwrap_or(0) > 0)
                    || (e.kind == "made" && matches!(e.actor, Some(ActorId::Npc(_)))))
        }
        "nobody_touched_it" => {
            let Some(ActorId::Npc(c)) = e.actor else { return false };
            e.kind == "made"
                && sim.cast.get(c).is_some_and(|n| n.helping.is_none() && n.asked_by.is_none())
                && !sim.log.recent.iter().any(|x| x.kind == "agreed" && x.actor == e.actor && e.t - x.t < 900.0)
        }
        // Earned by the app (fresh_start), by earning the rest
        // (pocket_universe), or not noticed yet (hello_world, word_travels,
        // living_legend: they need to know what was said).
        _ => false,
    }
}

/// Checks on the world as it is, not on one event.
fn check_state(id: &str, cx: &mut Cx) -> bool {
    let sim = cx.sim;
    let me = sim.player.pos;
    match id {
        "tagalong" => {
            let follow = format!("following {}", sim.actor_name(ActorId::Player));
            sim.cast.npcs.iter().any(|n| !n.dead && n.doing == follow && is_being(sim, ActorId::Npc(n.def.id)))
        }
        "unlikely_friends" => {
            let mut hit = false;
            let mut cold: Vec<Value> = cx.note.as_array().cloned().unwrap_or_default();
            let mut changed = false;
            for (&(a, b), r) in &sim.social.rels {
                if a == 0 || b == 0 {
                    continue;
                }
                let k = json!(format!("{a},{b}"));
                let was_cold = cold.contains(&k);
                if was_cold && r.affection > 0.4 {
                    hit = true;
                } else if !was_cold && r.affection < -0.15 && cold.len() < 300 {
                    cold.push(k);
                    changed = true;
                }
            }
            if changed {
                *cx.note() = Value::Array(cold);
            }
            hit
        }
        "all_grown_up" => {
            let Some(kids) = cx.note.as_array() else { return false };
            kids.iter().filter_map(Value::as_i64).any(|k| sim.cast.get(k).is_some_and(|n| !n.dead && n.growth >= 1.0))
        }
        "kin_of_kin" => sim.cast.npcs.iter().filter(|n| !n.dead && (n.a.pos - me).length() < 10.0).any(|n| {
            n.parents.iter().filter_map(|p| sim.cast.get(*p)).flat_map(|p| p.parents.iter()).any(|g| sim.social.rel(ActorId::Player, ActorId::Npc(*g)).is_some_and(|r| r.familiarity > 0.2))
        }),
        _ => false,
    }
}

/// Families the player feeds (a deed that feeds, or food handed over).
fn note_fed(sim: &Sim, e: &SimEvent, fed: &mut Value) -> bool {
    if e.actor != Some(ActorId::Player) {
        return false;
    }
    let to = match e.kind.as_str() {
        "deed_on" if e.data.get("fed").and_then(Value::as_bool) == Some(true) => e.subject.as_deref().and_then(ActorId::parse),
        "gave" => to(e),
        _ => None,
    };
    let Some(ActorId::Npc(c)) = to.filter(|a| is_being(sim, *a)) else { return false };
    let Some(n) = sim.cast.get(c) else { return false };
    let key = n.lineage.to_string();
    let Some(m) = fed.as_object_mut() else { return false };
    if m.contains_key(&key) {
        return false;
    }
    m.insert(key, json!(n.temper.tame));
    true
}

/// How tame the family of this event was when the player first fed it.
fn fed_tame(fed: &Value, e: &SimEvent) -> Option<f64> {
    let l = e.data.get("lineage").and_then(Value::as_i64)?;
    fed.get(l.to_string()).and_then(Value::as_f64)
}

/// The player used one thing on another and the world had to work it out.
fn used_unplanned(e: &SimEvent) -> bool {
    e.kind == "interpreted" && e.actor == Some(ActorId::Player) && e.data.get("effect").is_some() && e.data.get("came_to_nothing").is_none() && e.text.contains(": use the ") && e.text.contains(" on ")
}

/// How many things caught fire near a fire the player started in the last
/// two minutes (that one included).
fn spread_from_spark(sim: &Sim, e: &SimEvent) -> usize {
    let fires: Vec<&SimEvent> = sim.log.recent.iter().rev().take(600).filter(|x| x.kind == "ignited" && x.t >= e.t - 120.0 && x.t <= e.t).collect();
    let Some(spark) = fires.iter().rev().find(|x| player_acted(sim, x.t, 15.0)) else { return 0 };
    let Some(sp) = spark.pos.map(glam::Vec3::from) else { return 0 };
    let mut subjects: Vec<&str> = fires.iter().filter(|x| x.t >= spark.t && x.pos.is_some_and(|p| (glam::Vec3::from(p) - sp).length() < 40.0)).filter_map(|x| x.subject.as_deref()).collect();
    subjects.sort();
    subjects.dedup();
    subjects.len()
}

/// The player did something with their hands (or words) just before `t`.
fn player_acted(sim: &Sim, t: f64, secs: f64) -> bool {
    sim.log.recent.iter().rev().take(300).any(|x| x.actor == Some(ActorId::Player) && x.t <= t && t - x.t <= secs && matches!(x.kind.as_str(), "used" | "threw" | "interpreted" | "placed" | "dropped" | "gave" | "made"))
}

fn near(sim: &Sim, e: &SimEvent, r: f32) -> bool {
    e.pos.is_some_and(|p| sim.dist_to_player(glam::Vec3::from(p)) <= r)
}

fn is_being(sim: &Sim, a: ActorId) -> bool {
    matches!(a, ActorId::Npc(_)) && !sim.speaks(a)
}

fn is_custom(kind: &str) -> bool {
    matches!(GestureKind::parse(kind), Some(GestureKind::Custom(_)))
}

fn actor(v: &Value) -> Option<ActorId> {
    serde_json::from_value(v.clone()).ok()
}

fn to(e: &SimEvent) -> Option<ActorId> {
    e.data.get("to").and_then(actor)
}

fn data_str<'a>(e: &'a SimEvent, k: &str) -> &'a str {
    e.data.get(k).and_then(Value::as_str).unwrap_or("")
}
