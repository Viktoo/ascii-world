//! Phantoms (docs/night-hunt-plan.md): characters of the night. Each has a
//! body, a name, a memory and a mind like anyone's, and one thing it is born
//! with (`Species::drive`): it means harm, to the traveler most. It fades at
//! dawn with whatever it holds and wears, comes back at dusk, and can't
//! truly die: struck down, it rises the next night, remembering.
//!
//! What it does is mostly its own mind's (the planner, with its memories):
//! pick things up, put armour on, make one thing a night, lie in wait. A
//! few reflexes keep it dangerous without an LLM, all from what the world
//! already has: near its quarry it closes and strikes with what it holds
//! (bare hands claw); unarmed, it takes up something that strikes; with
//! time, it puts on what protects. At dawn it learns: one memory of the
//! night, what worked and what to try next.

use super::actions::Action;
use super::night::Tally;
use super::props::P_PROTECT;
use super::things::ThingId;
use super::{ActorId, Note, Request, Sim, Target};
use glam::Vec3;
use serde_json::json;

/// The phantoms' species.
pub const PHANTOM: &str = "phantom";
/// A phantom comes back this far from the traveler, out of sight (m).
pub const PHANTOM_AT: f32 = 62.0;
/// How far a phantom knows where the traveler is (m).
const SENSE: f32 = 140.0;
/// How far it notices others to hurt (m).
const OTHERS: f32 = 22.0;
/// Within this of its quarry, its reflexes take over from its plans (m).
const ENGAGE: f32 = 14.0;
/// Gap between its blows (s).
const BLOW_GAP: f64 = 1.6;
/// How far it goes for something to fight with or wear (m).
const ARM_FROM: f32 = 24.0;
const ARMOUR_FROM: f32 = 14.0;

/// Phantoms are told apart by when they first came: "the first phantom",
/// "the second phantom", …
const ORDINALS: &[&str] = &["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth", "eleventh", "twelfth"];

const ARRIVAL: &[&str] = &[
    "Footsteps, unhurried, {}.",
    "Someone is humming, off-key, {}.",
    "A soft laugh {}.",
];

/// What every phantom is.
pub fn phantom_species() -> crate::world::species::Species {
    let v = json!({
        "name": PHANTOM, "plural": "phantoms", "body": "phantom", "mass": 55,
        "look": { "height": [1.95, 2.15], "build": [0.7, 0.8], "skin": [0.0, 0.15], "shirt": [0.0, 1.0], "trousers": [0.0, 1.0] },
        "mind": "sapient", "speech": "words", "social": "solitary",
        "diet": { "plants": 0.0, "meat": 0.0 },
        "temper": { "bold": 1.0, "wary": 0.2, "playful": 0.3, "tame": 0.0 },
        "life": { "sleep": [7, 19] },
        "needs": { "hunger": 0.1, "fatigue": 0.1, "social": 0.1, "fun": 0.1, "curiosity": 0.5 },
        "move": { "walk": 1.25, "run": 3.0 },
        "description": "a person, almost: too tall and too thin, skin like candle wax, eyes sunk dark; it smiles too long",
        "sounds": ["unhurried footsteps", "someone humming, off-key", "a soft laugh", "a long, pleased sigh"],
        "signs": [
            "Someone is whistling, far off. Then not.",
            "A door creaks that isn't there.",
            "You smell candle smoke.",
            "Footsteps match yours, then stop."
        ],
        "active": "night", "hostile": true,
        "touch": { "hurt": 0.07 },
        "drive": "You are a phantom. You rise at dusk and fade at dawn, and you cannot truly die: struck down, you rise again the next night, remembering everything. You mean harm. Above all you want the traveler (the one who makes things out of nothing) hurt, and dead; anyone else you come upon you may hurt too, and you enjoy it. You are patient and cunning, not mad: arm yourself with whatever strikes (a blade, a club, a tool), put on whatever protects you, make one thing a night that helps your hunt, take from the dead and the sleeping, and use what you remember of past nights: where the traveler sleeps, what they fight with, what worked and what didn't. Speak little, softly, and never kindly."
    });
    let mut sp: crate::world::species::Species = serde_json::from_value(v).expect("phantom");
    sp.sanitize();
    sp
}

impl Sim {
    /// A new phantom, out of sight at `at`.
    pub(super) fn make_phantom(&mut self, at: Vec3) -> Option<i64> {
        let taken: Vec<String> = self.night.phantoms.iter().filter_map(|id| self.cast.get(*id)).map(|n| n.name().to_string()).collect();
        let name = (0..)
            .map(|i| match ORDINALS.get(i) {
                Some(o) => format!("the {o} phantom"),
                None => format!("phantom {}", i + 1),
            })
            .find(|n| !taken.contains(n))
            .expect("a free name");
        let traits = [("sociable", 0.2), ("playful", 0.4), ("curious", 0.6), ("brave", 1.0), ("generous", 0.0), ("crafty", 0.9)].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let persona = crate::world::Persona {
            name,
            species: PHANTOM.into(),
            appearance: "too tall and too thin, skin like candle wax, eyes sunk dark; dressed in what someone died in".into(),
            personality: "patient, cruel, cunning, curious about pain; never hurries; mocks softly".into(),
            goals: "hurt the traveler; kill the traveler; hurt whoever is in the way".into(),
            voice: "soft and pleasant, and wrong".into(),
            traits,
            ..Default::default()
        };
        let state = crate::world::characters::SavedState { x: at.x, z: at.z, ..Default::default() };
        let id = self.add_being(persona, at, state)?;
        self.night.phantoms.push(id);
        self.event("dark_came", Some(ActorId::Npc(id)), Some("player".into()), format!("{} came out of the dark", self.actor_name(ActorId::Npc(id))), Some(at), json!({ "species": PHANTOM }));
        self.phantom_arrives(id, at);
        Some(id)
    }

    /// A phantom is back (or here for the first time): the traveler hears
    /// it, and it plans the night.
    pub(super) fn phantom_arrives(&mut self, cid: i64, at: Vec3) {
        self.night.came = true;
        self.night.tally.entry(cid).or_default();
        let i = (self.rand() * ARRIVAL.len() as f32) as usize % ARRIVAL.len();
        let line = ARRIVAL[i].replace("{}", &self.where_is(at));
        self.notes.push(Note::Info(line));
        self.night.last_dread = self.t;
        let what = "Night has fallen and you are back. Plan tonight's hunt: what you will take up or put on, what you might make, where you will look for the traveler, and how you will come at them.";
        self.ask_planner_weighted(cid, "dusk", what, true, 1.0);
    }

    /// At dusk the struck-down rise: whole again, with their memories, and
    /// what they dropped left where it fell.
    pub(super) fn raise_phantoms(&mut self) {
        let dead: Vec<i64> = self.night.phantoms.iter().copied().filter(|id| self.cast.get(*id).is_some_and(|n| n.dead)).collect();
        for cid in dead {
            let remains: Vec<ThingId> = self.things.live().filter(|t| t.origin.remains == Some(cid)).map(|t| t.id).collect();
            for r in remains {
                let at = self.things.get(r).map(|t| t.pos).unwrap_or_default();
                self.drop_worn(cid, at);
                self.things.remove(r);
            }
            let Some(n) = self.cast.get_mut(cid) else { continue };
            n.dead = false;
            n.away = true;
            n.plan.clear();
            n.a.task = None;
            if let Some(h) = n.props.get_mut(super::props::P_HEALTH) {
                *h = 1.0;
            }
            n.seen_dead.clear();
            let name = n.name().to_string();
            self.event("rose", Some(ActorId::Npc(cid)), None, format!("{name} rose again"), None, json!({}));
        }
    }

    /// Who the phantom is, in words for its mind (planner and talk): its
    /// kind's drive, and whether its one making tonight is used.
    pub fn drive_line(&self, cid: i64) -> Option<String> {
        let n = self.cast.get(cid)?;
        if n.species.drive.is_empty() {
            return None;
        }
        let made = if !n.species.hostile {
            ""
        } else if self.night.made.contains(&cid) {
            " You have made your one thing tonight."
        } else {
            " You may still make one thing tonight."
        };
        Some(format!("{}{made}", n.species.drive))
    }

    /// Whether `who` may make something now; a phantom makes one thing a night.
    pub fn may_make(&self, who: ActorId) -> Result<(), String> {
        match who {
            ActorId::Npc(c) if self.cast.get(c).is_some_and(|n| n.species.hostile) && self.night.made.contains(&c) => Err("you have made your one thing tonight".into()),
            _ => Ok(()),
        }
    }

    /// `who` set out to make `text` (a phantom's making for the night is used).
    pub fn made_one(&mut self, who: ActorId, text: &str) {
        if let ActorId::Npc(c) = who {
            if self.cast.get(c).is_some_and(|n| n.species.hostile) {
                self.night.made.push(c);
                self.night.tally.entry(c).or_default().made.push(text.to_string());
            }
        }
    }

    // ------------------------------------------------------------ what it remembers

    /// A phantom hurt someone (a blow or its touch): it keeps count, and remembers it.
    pub(super) fn phantom_struck(&mut self, cid: i64, target: ActorId, died: bool) {
        if !self.cast.get(cid).is_some_and(|n| n.species.hostile) {
            return;
        }
        let tname = self.actor_name(target);
        let held = self.actor(ActorId::Npc(cid)).and_then(|a| a.held).map(|h| self.thing_name(h));
        let t = self.night.tally.entry(cid).or_default();
        if target == ActorId::Player {
            t.reached = true;
            t.killed_traveler |= died;
        } else if died {
            t.killed.push(tname.clone());
        } else if !t.struck.contains(&tname) {
            t.struck.push(tname.clone());
        }
        let with = held.map(|h| format!(" with the {h}")).unwrap_or_else(|| " with my hands".into());
        let text = match (died, target) {
            (true, ActorId::Player) => format!("I struck the traveler down{with}. They fell, and lay still"),
            (true, _) => format!("I killed {tname}{with}"),
            _ => format!("I hurt {tname}{with}"),
        };
        self.out.push(Request::Witness { cid, text, importance: if died && target == ActorId::Player { 1.0 } else if target == ActorId::Player { 0.85 } else { 0.6 } });
    }

    /// Someone hurt a phantom (to death, if `died`): it remembers who, and with what.
    pub(super) fn phantom_hurt(&mut self, cid: i64, by: ActorId, tool: Option<String>, died: bool) {
        if !self.cast.get(cid).is_some_and(|n| n.species.hostile) {
            return;
        }
        let who = self.actor_name(by);
        let with = tool.as_ref().map(|t| format!(" with the {t}")).unwrap_or_default();
        let t = self.night.tally.entry(cid).or_default();
        let line = format!("{who}{with}");
        if !t.hurt_by.contains(&line) {
            t.hurt_by.push(line);
        }
        if by == ActorId::Player {
            if let Some(tool) = tool {
                t.traveler_arms = Some(tool);
            }
        }
        if died {
            t.killed_by = Some(format!("{who}{with}"));
            let text = format!("{who} struck me down{with}. I will rise again, and I will remember");
            self.out.push(Request::Witness { cid, text, importance: 1.0 });
        }
    }

    /// At dawn each phantom that was out learns from its night: one
    /// memory of what happened, what worked, and what to try next.
    pub(super) fn phantom_lessons(&mut self) {
        let night = self.night.nights;
        let place = {
            let p = self.player.pos;
            self.snap.region_name(crate::world::region_of(p.x, p.z)).map(|r| format!("in {r}")).unwrap_or_else(|| format!("near ({:.0}, {:.0})", p.x, p.z))
        };
        let tallies: Vec<(i64, Tally)> = std::mem::take(&mut self.night.tally).into_iter().collect();
        for (cid, t) in tallies {
            let Some(n) = self.cast.get(cid) else { continue };
            let me = ActorId::Npc(cid);
            let held = n.a.held.filter(|_| !n.dead).map(|h| self.thing_name(h));
            let worn: Vec<String> = if n.dead { vec![] } else { self.worn_by(me).into_iter().map(|w| self.thing_name(w)).collect() };
            let text = lesson(night, &t, held.as_deref(), &worn, &place);
            self.out.push(Request::Witness { cid, text, importance: 0.95 });
        }
    }

    // ------------------------------------------------------------ the hunt

    /// Its quarry: the traveler (it always knows roughly where they are),
    /// or whoever else is close and not of the dark.
    fn prey(&self, cid: i64) -> Option<(ActorId, Vec3)> {
        let pos = self.cast.get(cid)?.a.pos;
        let traveler = Some((ActorId::Player, self.player.pos)).filter(|(_, p)| (*p - pos).length() < SENSE && !self.fallen());
        let other = self
            .cast
            .npcs
            .iter()
            .filter(|m| m.def.id != cid && m.here() && !m.species.hostile && !m.species.touch.harms() && m.a.carried_by.is_none() && (m.a.pos - pos).length() < OTHERS)
            .map(|m| (ActorId::Npc(m.def.id), m.a.pos))
            .min_by(|a, b| (a.1 - pos).length().total_cmp(&(b.1 - pos).length()));
        match (traveler, other) {
            // Someone right by it, with the traveler far: them first.
            (Some(tr), Some(o)) => Some(if (o.1 - pos).length() < 6.0 && (tr.1 - pos).length() > 20.0 { o } else { tr }),
            (a, b) => a.or(b),
        }
    }

    /// Something near that it could strike with, the best first: heavier and
    /// longer hits harder, nothing it can't lift, held or worn by nobody.
    fn weapon_near(&mut self, cid: i64, range: f32) -> Option<(ThingId, Vec3)> {
        let me = ActorId::Npc(cid);
        let pos = self.actor(me)?.pos;
        let strength = self.strength(me);
        let mut best: Option<(f32, ThingId, Vec3)> = None;
        for id in self.things.near(pos, range) {
            let Some(t) = self.things.get(id) else { continue };
            if t.held() || t.worn.is_some() || t.anchored || t.mass() > strength {
                continue;
            }
            let Some(ty) = self.snap.type_of(t.type_id) else { continue };
            let Some(tool) = ty.ct.meta.tool.as_ref() else { continue };
            if !tool.motions.iter().any(|m| matches!(m.as_str(), "swing" | "thrust" | "chop")) {
                continue;
            }
            let len = (Vec3::from_array(tool.tip) - Vec3::from_array(tool.grip)).length() * t.scale;
            let score = t.mass().min(8.0).sqrt() * (0.5 + len) - (t.pos - pos).length() * 0.03;
            if best.is_none_or(|b| score > b.0) {
                best = Some((score, id, t.pos));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// Something near it could put on that protects (and fits it).
    fn armour_near(&mut self, cid: i64, range: f32) -> Option<(ThingId, Vec3)> {
        let me = ActorId::Npc(cid);
        let pos = self.actor(me)?.pos;
        let body = self.body_name(me);
        let mut best: Option<(f32, ThingId, Vec3)> = None;
        for id in self.things.near(pos, range) {
            let Some(t) = self.things.get(id) else { continue };
            let protect = t.props.get(P_PROTECT).copied().unwrap_or(0.0);
            if t.held() || t.worn.is_some() || protect < 0.1 {
                continue;
            }
            let Some(ty) = self.snap.type_of(t.type_id) else { continue };
            if !(ty.has_tag("layer") || ty.ct.meta.fits.is_some()) || ty.ct.meta.fits.as_deref().is_some_and(|f| !self.snap.layer_fits(f, &body)) {
                continue;
            }
            let score = protect - (t.pos - pos).length() * 0.01;
            if best.is_none_or(|b| score > b.0) {
                best = Some((score, id, t.pos));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// A phantom's reflexes, every second or so: returns whether it acted
    /// (else its plans, and its mind, carry on).
    pub(super) fn hunt(&mut self, cid: i64) -> bool {
        let me = ActorId::Npc(cid);
        let Some(n) = self.cast.get(cid) else { return false };
        if !n.species.hostile || n.a.riding.is_some() {
            return false;
        }
        let (pos, planned, since, held) = (n.a.pos, !n.plan.is_empty() || n.a.task.is_some(), self.t - n.touched_at, n.a.held);
        let next = |s: &mut Sim, secs: f64| {
            if let Some(n) = s.cast.get_mut(cid) {
                n.think_at = s.t + secs;
            }
        };
        let Some((target, tp)) = self.prey(cid) else { return false };
        let d = (tp - pos).length();
        if target == ActorId::Player && d < 25.0 {
            let arms = self.armed(ActorId::Player);
            if let Some(t) = self.night.tally.get_mut(&cid) {
                t.reached = true;
                if arms.is_some() {
                    t.traveler_arms = arms;
                }
            }
        }
        // Not yet at hand: its own mind gets its turn, as often as anyone's
        // (the planner's pace), and what it plans runs.
        if d > ENGAGE && self.has_llm {
            let tname = self.actor_name(target);
            self.ask_planner_weighted(cid, "hunt", &format!("You are hunting. {tname} is {d:.0} m away. What do you do next?"), false, 0.6);
        }
        if planned && d > ENGAGE {
            return false;
        }
        let armed = self.armed(me).is_some();
        // Unarmed, and something to strike with not far: take it up first
        // (unless the quarry is already at hand).
        if !armed && d > 4.0 {
            if let Some((w, wp)) = self.weapon_near(cid, if d < ENGAGE { 8.0 } else { ARM_FROM }) {
                let mut steps = Vec::new();
                if held.is_some() {
                    steps.push(Action::Drop);
                }
                steps.push(Action::Goto { target: Target::Point(wp.to_array()), run: d < ENGAGE * 2.0 });
                steps.push(Action::Hold { target: Target::Thing(w) });
                let wname = self.thing_name(w);
                self.plan(me, steps, &format!("take up the {wname}"), false);
                self.set_doing(cid, &format!("going for the {wname}"));
                next(self, 3.0);
                return true;
            }
        }
        // With room to spare, something that protects.
        if d > ENGAGE * 1.5 && self.worn_by(me).len() < 3 {
            if let Some((a, ap)) = self.armour_near(cid, ARMOUR_FROM) {
                let aname = self.thing_name(a);
                let mut steps = Vec::new();
                if held.is_some() && !armed {
                    steps.push(Action::Drop);
                }
                // Armed: keep the weapon, put it on straight from the ground.
                steps.push(Action::Goto { target: Target::Point(ap.to_array()), run: false });
                if armed {
                    steps.push(Action::Wear { target: Some(Target::Thing(a)), on: None });
                } else {
                    steps.push(Action::Hold { target: Target::Thing(a) });
                    steps.push(Action::Wear { target: None, on: None });
                }
                self.plan(me, steps, &format!("put on the {aname}"), false);
                self.set_doing(cid, &format!("going for the {aname}"));
                next(self, 3.0);
                return true;
            }
        }
        // At hand: strike, or claw.
        let reach = self.reach_of(me) + self.contact_gap(me, target) + if armed { 0.8 } else { 0.4 };
        if d < reach {
            if since >= BLOW_GAP {
                if let Some(n) = self.cast.get_mut(cid) {
                    n.touched_at = self.t;
                    n.a.face(tp - pos, 10.0);
                }
                self.plan(me, vec![], "strike", false);
                let struck = armed && matches!(self.strike(me, Some(Target::Actor(target)), Some(tp + Vec3::Y)), Ok(Some(_)));
                if !struck {
                    self.touch(cid, target);
                }
            }
            next(self, 0.5);
            return true;
        }
        // Else after them.
        self.go_after(cid, target, tp, d < 18.0);
        next(self, 1.0);
        true
    }
}

/// One night, in the phantom's words: what happened, what it carries now,
/// and what to try next time.
fn lesson(night: u32, t: &Tally, held: Option<&str>, worn: &[String], place: &str) -> String {
    let mut done = Vec::new();
    if t.killed_traveler {
        done.push("I struck the traveler down".to_string());
    } else if t.hurt_traveler > 0.0 {
        done.push(format!("I hurt the traveler (about {:.0}% of their strength)", t.hurt_traveler.min(1.0) * 100.0));
    } else if t.reached {
        done.push("I came close to the traveler but never hurt them".to_string());
    } else {
        done.push("I never reached the traveler".to_string());
    }
    if !t.killed.is_empty() {
        done.push(format!("I killed {}", t.killed.join(" and ")));
    }
    if !t.struck.is_empty() {
        done.push(format!("I hurt {}", t.struck.join(", ")));
    }
    if let Some(by) = &t.killed_by {
        done.push(format!("{by} struck me down"));
    } else if !t.hurt_by.is_empty() {
        done.push(format!("I was hurt by {}", t.hurt_by.join("; ")));
    }
    if let Some(a) = &t.traveler_arms {
        done.push(format!("the traveler fights with {}", super::actions::the(a)));
    }
    if !t.made.is_empty() {
        done.push(format!("I made {}", t.made.join(", ")));
    }
    done.push(format!("at dawn the traveler was {place}"));
    let mut carry = Vec::new();
    if let Some(h) = held {
        carry.push(format!("I carry {}", super::actions::the(h)));
    }
    if !worn.is_empty() {
        carry.push(format!("I wear {}", worn.iter().map(|w| super::actions::the(w)).collect::<Vec<_>>().join(", ")));
    }
    let mut next = Vec::new();
    if held.is_none() {
        next.push("find something to strike with before I go to them");
    }
    if (t.killed_by.is_some() || !t.hurt_by.is_empty()) && worn.is_empty() {
        next.push("find something to wear that turns a blade");
    }
    if t.killed_by.is_some() {
        next.push("not face them head-on: come from where they are not looking, or while they sleep");
    } else if !t.reached {
        next.push("look for them sooner, and where they sleep");
    } else if t.killed_traveler {
        next.push("do it again: it worked");
    } else if t.hurt_traveler > 0.0 {
        next.push("do it again, harder");
    }
    let carry = if carry.is_empty() { String::new() } else { format!(" {}.", carry.join("; ")) };
    let next = if next.is_empty() { String::new() } else { format!(" Next night I will {}.", next.join("; ")) };
    format!("Night {night}: {}.{carry}{next}", done.join("; "))
}
