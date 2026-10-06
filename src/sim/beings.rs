//! Beings that are not people: what their species and body change about the
//! shared verbs (strength, sounds instead of words, contact distance), and
//! the cheap mind of animals (follow their person, fetch, graze, rest).
//! Everything reads the taxonomy (`world::species`), never a species' name.

use super::actions::Action;
use super::props::*;
use super::things::ThingId;
use super::{ActorId, Note, Sim, Target};
use crate::world::species::{Mind, Speech};
use glam::Vec3;
use serde_json::json;

/// How much one harmless scare gets an animal used to what scared it.
const GET_USED: f32 = 0.15;
/// Used to it: it no longer frightens them.
const USED_TO: f32 = 0.6;

impl Sim {
    /// How much an actor can lift alone (kg): a person's strength, scaled
    /// with body mass (a dog carries a stick, a horse a sack, a cat a mouse).
    pub fn strength(&self, who: ActorId) -> f32 {
        let m = self.actor(who).map(|a| a.dims.mass).unwrap_or(70.0);
        (STRENGTH * (m / 70.0).powf(2.0 / 3.0)).clamp(0.3, 4000.0)
    }

    /// The species' mind for an actor (the traveler is a person).
    pub fn mind_of(&self, who: ActorId) -> Mind {
        match who {
            ActorId::Player => Mind::Sapient,
            ActorId::Npc(c) => self.cast.get(c).map(|n| n.species.mind).unwrap_or(Mind::Sapient),
        }
    }

    /// Whether an actor speaks in words.
    pub fn speaks(&self, who: ActorId) -> bool {
        match who {
            ActorId::Player => true,
            ActorId::Npc(c) => self.cast.get(c).is_none_or(|n| n.species.speech == Speech::Words),
        }
    }

    /// One of an animal's noises: `happy` picks a friendly one, otherwise
    /// the last (a growl, a hiss).
    pub fn noise(&mut self, cid: i64, happy: bool) -> Option<String> {
        let n = self.cast.get_mut(cid)?;
        if n.species.speech == Speech::None || n.species.sounds.is_empty() {
            return None;
        }
        let s = &n.species.sounds;
        let i = if happy || s.len() == 1 {
            n.rng = crate::noise::pcg(n.rng);
            (n.rng as usize) % (s.len() - (s.len() > 2) as usize).max(1)
        } else {
            s.len() - 1
        };
        let text = s[i].clone();
        self.cue_noise(cid, i, if happy { 0.8 } else { 1.0 });
        Some(text)
    }

    /// An animal makes a noise: heard nearby, remembered like a line.
    pub fn make_noise(&mut self, cid: i64, happy: bool) {
        let who = ActorId::Npc(cid);
        let Some(at) = self.actor(who).map(|a| a.pos) else { return };
        let Some(noise) = self.noise(cid, happy) else { return };
        let name = self.actor_name(who);
        self.note_near(at, 22.0, Note::Ambient(format!("{} gives {}.", super::physics::cap(&name), with_article(&noise))));
        self.event("sound", Some(who), None, format!("{name}: {noise}"), Some(at), json!({ "sound": noise }));
    }

    /// Words said by someone who has no words come out as a noise: the
    /// friendly kinds of line as a friendly one.
    pub fn speak_as_animal(&mut self, cid: i64, text: &str) {
        let l = text.to_lowercase();
        let unhappy = ["no", "not now", "rather not", "fire", "get back", "burning"].iter().any(|w| l.starts_with(w) || l.contains(w));
        self.make_noise(cid, !unhappy);
    }

    /// Extra room a contact gesture needs between two bodies bigger than a
    /// person's: the distance is kept between their sides.
    pub fn contact_gap(&self, a: ActorId, b: ActorId) -> f32 {
        let r = |x: ActorId| self.actor(x).map(|a| (a.dims.radius - 0.35).max(0.0)).unwrap_or(0.0);
        r(a) + r(b)
    }

    /// How much more an animal must like someone before it lets them close.
    pub fn wariness(&self, cid: i64) -> f32 {
        match self.cast.get(cid) {
            Some(n) if n.species.mind != Mind::Sapient => (n.temper.wary - 0.5) * 0.5 - (n.temper.tame - 0.5) * 0.2,
            _ => 0.0,
        }
    }

    /// The person an animal belongs to, if any (the nearest when several).
    pub fn owner_of(&self, cid: i64) -> Option<ActorId> {
        let me = ActorId::Npc(cid);
        let pos = self.actor(me)?.pos;
        let mut best: Option<(f32, ActorId)> = None;
        for o in self.actor_ids() {
            if o == me {
                continue;
            }
            if self.social.rel(me, o).and_then(|r| r.owner) == Some(o.code()) {
                let d = self.actor(o).map(|a| (a.pos - pos).length()).unwrap_or(f32::MAX);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, o));
                }
            }
        }
        best.map(|b| b.1)
    }

    /// The traveler spoke to (or called) an animal: it answers with a noise
    /// and its body, by how it feels about them. No words, no LLM.
    pub fn animal_answers(&mut self, cid: i64) {
        let me = ActorId::Npc(cid);
        let Some(pos) = self.actor(me).map(|a| a.pos) else { return };
        let aff = self.social.affection(me, ActorId::Player) - self.wariness(cid);
        let player = self.player.pos;
        if let Some(n) = self.cast.get_mut(cid) {
            n.a.asleep = false;
            n.a.face(player - pos, 10.0);
            n.think_at = self.t + 4.0;
        }
        if aff > 0.15 {
            self.make_noise(cid, true);
            let _ = self.act(me, Action::Gesture { kind: "wag".into(), to: None });
            self.social.bond(me, ActorId::Player, 0.03, self.t);
            self.set_doing(cid, "greeting the traveler");
        } else if aff < -0.25 {
            self.make_noise(cid, false);
            let away = (pos - player).normalize_or_zero() * 4.0;
            self.plan(me, vec![Action::Goto { target: Target::Point((pos + away).to_array()), run: true }], "keep away from the traveler", false);
            self.set_doing(cid, "backing away");
        } else {
            self.make_noise(cid, true);
            let _ = self.act(me, Action::Gesture { kind: "nod".into(), to: None });
            self.social.bond(me, ActorId::Player, 0.02, self.t);
            self.set_doing(cid, "eyeing the traveler");
        }
    }

    /// Put on what characters wear (their own layers and their variety's)
    /// the first time they are in the world, once the layer types exist.
    pub fn dress_new(&mut self) {
        let todo: Vec<(i64, Vec<String>)> = self
            .cast
            .npcs
            .iter()
            .filter(|n| !n.dressed && n.here())
            .map(|n| {
                let mut v = n.def.persona.layers.clone();
                if let Some(var) = n.species.varieties.iter().find(|v| v.name == n.def.persona.variety) {
                    v.extend(var.layers.iter().cloned());
                }
                (n.def.id, v)
            })
            .collect();
        for (cid, layers) in todo {
            let who = ActorId::Npc(cid);
            let types: Vec<u32> = layers.iter().filter_map(|l| self.type_by_name(l).map(|t| t.id)).collect();
            if types.len() < layers.len() && !layers.is_empty() {
                // Not all written yet: try again after the next flip.
                continue;
            }
            if let Some(n) = self.cast.get_mut(cid) {
                n.dressed = true;
            }
            if !self.worn_by(who).is_empty() {
                continue;
            }
            let Some((p, yaw)) = self.actor(who).map(|a| (a.pos, a.yaw)) else { continue };
            for ty in types.into_iter().take(3) {
                let origin = super::things::Origin { made_by: Some(self.actor_name(who)), ..Default::default() };
                let Some(id) = self.spawn_thing(ty, p, yaw, 1.0, origin, false) else { continue };
                if let Some(t) = self.things.get_mut(id) {
                    t.worn = Some(who);
                    t.pos = p;
                    t.yaw = yaw;
                    t.vel = Vec3::ZERO;
                    t.asleep = true;
                    t.dirty = true;
                }
            }
        }
    }

    /// A gesture that moves parts this body doesn't have: ask once for this
    /// species' own version (the shared one plays meanwhile, with fallbacks).
    pub fn ask_body_gesture(&mut self, who: ActorId, k: super::actor::GestureKind) {
        let ActorId::Npc(c) = who else { return };
        let Some(n) = self.cast.get(c) else { return };
        if n.species.is_human() || !self.has_llm || k.for_body(&n.a.species) != k || !k.needs_roles(n.a.roles) {
            return;
        }
        let name = format!("{}@{}", k.name(), n.species.name.replace([' ', '-'], "_"));
        if !self.interp.variants_asked.insert(name.clone()) {
            return;
        }
        let roles: Vec<&str> = crate::lang::ir::ROLES.iter().enumerate().filter(|(i, _)| n.a.roles & (1 << i) != 0).map(|(_, r)| *r).collect();
        let body = format!("a {} ({}; {} m tall, moves by {}). Its roles: {}.", n.species.name, n.species.description, (n.a.dims.height * 10.0).round() / 10.0, if n.a.dims.flies { "flying and walking" } else { "walking" }, roles.join(", "));
        let pos = n.a.pos;
        let id = self.next_id();
        self.interp.gestures.insert(id, (who, None, name.clone()));
        self.request(super::Request::BuildGesture { id, name, body }, pos);
    }

    /// Something thrown lately by someone this animal likes, for fetching.
    pub(super) fn fetchable(&self, cid: i64, range: f32) -> Option<(ThingId, ActorId)> {
        let me = ActorId::Npc(cid);
        let pos = self.actor(me)?.pos;
        let strength = self.strength(me);
        let mut best: Option<(f32, ThingId, ActorId)> = None;
        for t in self.things.live() {
            let Some((by, when)) = t.thrown_by else { continue };
            if by == me || self.t - when > 25.0 || t.held() || t.anchored || t.mass() > strength || self.vocab.harm(&t.props) > 0.0 {
                continue;
            }
            if self.social.affection(me, by) < 0.2 {
                continue;
            }
            let d = (t.pos - pos).length();
            if d < range && best.is_none_or(|b| d < b.0) {
                best = Some((d, t.id, by));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// A thing was thrown: playful animals nearby look up from what they do.
    pub fn animals_notice_throw(&mut self, at: Vec3) {
        let t = self.t;
        for n in self.cast.npcs.iter_mut() {
            // Pottering about or tagging along, not running from or after something.
            let idle = matches!(n.aim, super::npc::Aim::Idle | super::npc::Aim::Other | super::npc::Aim::Follow(_));
            if n.here() && n.species.mind != Mind::Sapient && n.temper.playful > 0.5 && !n.a.asleep && (n.a.pos - at).length() < 30.0 && idle {
                n.think_at = n.think_at.min(t + 0.3);
                n.plan.clear();
                n.a.task = None;
            }
        }
    }

    pub(super) fn species_of(&self, who: ActorId) -> Option<&std::sync::Arc<crate::world::species::Species>> {
        match who {
            ActorId::Player => None,
            ActorId::Npc(c) => self.cast.get(c).map(|n| &n.species),
        }
    }

    /// A being's own temper (its line's, or its species').
    pub fn temper_of(&self, who: ActorId) -> Option<crate::world::species::Temper> {
        match who {
            ActorId::Npc(c) => self.cast.get(c).map(|n| n.temper),
            ActorId::Player => None,
        }
    }

    fn mass_of(&self, who: ActorId) -> f32 {
        self.actor(who).map(|a| a.dims.mass).unwrap_or(70.0)
    }

    /// Kin, pets of the same person, friends: never prey.
    fn kin(&self, a: ActorId, b: ActorId) -> bool {
        if let Some(r) = self.social.rel(a, b) {
            if r.owner.is_some() || r.family || r.partner || r.affection > 0.3 {
                return true;
            }
        }
        match (a, b) {
            // Animals of one household: one owner, or owners who are family.
            (ActorId::Npc(x), ActorId::Npc(y)) => match (self.owner_of(x), self.owner_of(y)) {
                (Some(ox), Some(oy)) => ox == oy || self.social.rel(ox, oy).is_some_and(|r| r.family || r.partner),
                _ => false,
            },
            _ => false,
        }
    }

    /// Whether `a` would hunt `b`, from the taxonomy alone: a meat eater
    /// hunts what is clearly smaller than it (and its pack), never its own
    /// kind or kin; only the boldest go after people, and nobody hunts the
    /// traveler.
    pub fn hunts(&self, a: ActorId, b: ActorId) -> bool {
        if a == b || b == ActorId::Player {
            return false;
        }
        let (Some(sa), Some(sb)) = (self.species_of(a), self.species_of(b)) else { return false };
        if sa.diet.meat <= 0.5 || sa.name == sb.name || sa.mind == Mind::Sapient {
            return false;
        }
        if sb.mind == Mind::Sapient && self.temper_of(a).map(|t| t.bold).unwrap_or(0.5) < 0.85 {
            return false;
        }
        if self.kin(a, b) {
            return false;
        }
        let pa = self.actor(a).map(|x| x.pos).unwrap_or_default();
        let pack = if sa.social == crate::world::species::Social::Pack {
            1.0 + self.kind_near(a, pa, 30.0).len() as f32
        } else {
            1.0
        };
        self.mass_of(b) < 0.8 * self.mass_of(a) * pack
    }

    /// Others of the same species near `p` (a herd, a pack).
    pub(super) fn kind_near(&self, me: ActorId, p: Vec3, range: f32) -> Vec<(ActorId, Vec3)> {
        let Some(sp) = self.species_of(me).map(|s| s.name.clone()) else { return Vec::new() };
        self.cast
            .npcs
            .iter()
            .filter(|n| n.here() && ActorId::Npc(n.def.id) != me && n.species.name == sp && (n.a.pos - p).length() < range)
            .map(|n| (ActorId::Npc(n.def.id), n.a.pos))
            .collect()
    }

    /// The nearest danger to `me`: a predator that would hunt it, or (for
    /// wary animals) any bigger stranger, within the distance it keeps.
    pub fn threat(&self, me: ActorId) -> Option<(ActorId, Vec3)> {
        let a = self.actor(me)?;
        let pos = a.pos;
        let sp = self.species_of(me);
        let wary = self.temper_of(me).map(|t| t.wary).unwrap_or(0.4);
        let animal = sp.is_some_and(|s| s.mind != Mind::Sapient);
        let my_mass = self.mass_of(me);
        let mut best: Option<(f32, ActorId, Vec3)> = None;
        for o in self.actor_ids() {
            if o == me {
                continue;
            }
            let Some(x) = self.actor(o) else { continue };
            if x.asleep {
                continue;
            }
            let d = (x.pos - pos).length();
            let ratio = (self.mass_of(o) / my_mass.max(0.1)).sqrt().min(3.0);
            let spooks = animal && sp.is_some_and(|s| s.diet.meat < 0.3 && wary >= 0.5) && self.species_of(o).is_some_and(|s| s.diet.meat > 0.5 && s.mind != Mind::Sapient) && !self.kin(me, o) && !self.used_to(me, o);
            let dark = self.species_of(o).is_some_and(|s| s.touch.harms()) && self.species_of(me).is_none_or(|s| !s.touch.harms());
            let keep = if dark {
                // Whatever's touch harms: everyone keeps away.
                10.0 + 20.0 * wary
            } else if self.hunts(o, me) {
                (6.0 + 30.0 * wary * ratio).min(60.0)
            } else if spooks {
                // Prey animals shy from any meat eater, however small.
                6.0 + 10.0 * wary
            } else if animal && wary > 0.65 && self.social.affection(me, o) < 0.3 && sp.map(|s| &s.name) != self.species_of(o).map(|s| &s.name) && self.mass_of(o) > 0.4 * my_mass && !self.used_to(me, o) {
                4.0 + 10.0 * wary
            } else {
                continue;
            };
            if d < keep && best.is_none_or(|b| d < b.0) {
                best = Some((d, o, x.pos));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// Run from `from`; a herd runs together. Each scare that comes to
    /// nothing makes the next less likely: animals get used to a meat eater
    /// that lives among them and never hunts them.
    pub fn flee(&mut self, cid: i64, from: ActorId, at: Vec3) {
        let me = ActorId::Npc(cid);
        if !self.hunts(from, me) {
            let pos = self.actor(me).map(|a| a.pos).unwrap_or(at);
            let mut herd = vec![me];
            herd.extend(self.kind_near(me, pos, 25.0).into_iter().map(|h| h.0));
            for h in herd {
                let r = self.social.rel_mut(h, from);
                r.familiarity = (r.familiarity + GET_USED).min(1.0);
            }
        }
        let what = self.actor_name(from);
        self.run_from(cid, &what, Some(from.key()), at);
    }

    /// Used enough to `o` that it no longer frightens `me` (unless it hunts).
    fn used_to(&self, me: ActorId, o: ActorId) -> bool {
        self.social.rel(me, o).is_some_and(|r| r.familiarity >= USED_TO)
    }

    /// Run from whatever is at `at` (a being, or a thing that frightened
    /// them: `what` names it); a herd runs together.
    pub fn run_from(&mut self, cid: i64, what: &str, subject: Option<String>, at: Vec3) {
        let me = ActorId::Npc(cid);
        let Some(pos) = self.actor(me).map(|a| a.pos) else { return };
        let away = (pos - at).normalize_or_zero();
        let away = if away.length() < 0.5 { Vec3::X } else { away };
        let group = self.species_of(me).is_some_and(|s| s.social == crate::world::species::Social::Herd);
        if let Some(n) = self.cast.get_mut(cid) {
            n.frights = n.frights.saturating_add(1);
        }
        let mut who = vec![(me, pos)];
        if group {
            who.extend(self.kind_near(me, pos, 25.0));
        }
        let herd: Vec<ActorId> = who.iter().map(|w| w.0).collect();
        // A herd runs to one place, so it stays a herd.
        let centre = who.iter().fold(Vec3::ZERO, |a, (_, p)| a + *p) / who.len() as f32;
        let side = Vec3::new(-away.z, 0.0, away.x);
        for (i, (x, p)) in who.into_iter().enumerate() {
            let ActorId::Npc(c) = x else { continue };
            if self.cast.get(c).is_some_and(|n| n.aim == super::npc::Aim::Avoid && x != me) {
                continue;
            }
            let dest = if group { centre + away * 20.0 + side * ((i as f32 % 3.0) - 1.0) * 2.0 + away * (i / 3) as f32 * 2.0 } else { p + away * 18.0 };
            self.plan(x, vec![Action::Goto { target: Target::Point([dest.x, 0.0, dest.z]), run: true }], &format!("get away from {what}"), false);
            if let Some(n) = self.cast.get_mut(c) {
                n.a.asleep = false;
                n.a.task = None;
                n.think_at = self.t + 2.5;
            }
            self.set_aim(c, super::npc::Aim::Avoid, &format!("fleeing {what}"));
        }
        let t = self.t;
        // One telling for the whole herd, its cry in the same breath.
        let recent = self.log.recent.iter().rev().take(30).any(|e| e.kind == "fled" && e.actor.is_some_and(|a| herd.contains(&a)) && t - e.t < 8.0);
        if !recent {
            let name = self.actor_name(me);
            let msg = if herd.len() > 1 { format!("{name} and the herd bolt from {what}") } else { format!("{name} runs from {what}") };
            self.event("fled", Some(me), subject, msg.clone(), Some(pos), json!({ "herd": group }));
            let cry = if self.speaks(me) { None } else { self.noise(cid, false) };
            let told = match &cry {
                Some(noise) => format!("{} with {}.", super::physics::cap(&msg), with_article(noise)),
                None => format!("{}.", super::physics::cap(&msg)),
            };
            self.note_near(pos, 30.0, Note::Ambient(told));
            self.witness(pos, 25.0, &msg, 0.35, &[me]);
            if let Some(noise) = cry {
                self.event("sound", Some(me), None, format!("{name}: {noise}"), Some(pos), json!({ "sound": noise }));
            }
        }
    }

    /// The nearest prey a hungry predator could chase.
    pub(super) fn prey_near(&self, me: ActorId, range: f32) -> Option<(ActorId, Vec3)> {
        let pos = self.actor(me)?.pos;
        let mut best: Option<(f32, ActorId, Vec3)> = None;
        for o in self.actor_ids() {
            let Some(x) = self.actor(o) else { continue };
            let d = (x.pos - pos).length();
            if d < range && self.hunts(me, o) && best.is_none_or(|b| d < b.0) {
                best = Some((d, o, x.pos));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// Start chasing: the prey notices, and the pack joins in.
    pub(super) fn chase(&mut self, cid: i64, prey: ActorId) {
        let me = ActorId::Npc(cid);
        let pos = self.actor(me).map(|a| a.pos).unwrap_or_default();
        let mut hunters = vec![me];
        if self.species_of(me).is_some_and(|s| s.social == crate::world::species::Social::Pack) {
            for (x, _) in self.kind_near(me, pos, 30.0) {
                if let ActorId::Npc(c) = x {
                    if self.cast.get(c).is_some_and(|n| !n.a.asleep && n.needs.hunger > 0.3 && n.plan.is_empty()) {
                        hunters.push(x);
                    }
                }
            }
        }
        let pname = self.actor_name(prey);
        for h in &hunters {
            let ActorId::Npc(c) = *h else { continue };
            self.plan(*h, vec![Action::Goto { target: Target::Actor(prey), run: true }], &format!("hunt {pname}"), false);
            self.set_aim(c, super::npc::Aim::Chase(prey), &format!("chasing {pname}"));
            if let Some(n) = self.cast.get_mut(c) {
                n.think_at = self.t + 1.0;
            }
        }
        if let ActorId::Npc(p) = prey {
            if let Some(n) = self.cast.get_mut(p) {
                n.think_at = n.think_at.min(self.t);
            }
        }
        let name = self.actor_name(me);
        let msg = if hunters.len() > 1 { format!("{name} and the pack go after {pname}") } else { format!("{name} goes after {pname}") };
        self.event("chase", Some(me), Some(prey.key()), msg.clone(), Some(pos), json!({ "pack": hunters.len() }));
        self.witness(pos, 30.0, &msg, 0.4, &[me]);
    }

    /// A predator reached its prey: in a hunting universe it is killed;
    /// otherwise the chase ends there and the predator gives up, winded.
    pub(super) fn caught(&mut self, cid: i64, prey: ActorId) {
        let me = ActorId::Npc(cid);
        let name = self.actor_name(me);
        let pname = self.actor_name(prey);
        let at = self.actor(prey).map(|a| a.pos).unwrap_or_default();
        if self.cfg.hunting {
            if let ActorId::Npc(p) = prey {
                self.kill(p, &format!("killed by {name}"), Some(me));
            }
            if let Some(n) = self.cast.get_mut(cid) {
                n.needs.hunger = 0.0;
                n.needs.fatigue = (n.needs.fatigue + 0.3).min(1.0);
            }
            self.event("killed", Some(me), Some(prey.key()), format!("{name} killed {pname}"), Some(at), json!({}));
        } else {
            if let Some(n) = self.cast.get_mut(cid) {
                n.needs.hunger = (n.needs.hunger - 0.25).max(0.0);
                n.needs.fatigue = (n.needs.fatigue + 0.3).min(1.0);
            }
            self.event("gave_up", Some(me), Some(prey.key()), format!("{name} gives up the chase after {pname}"), Some(at), json!({}));
        }
    }

    /// A being dies (at `by`'s hand, if anyone's): it leaves the living for
    /// good (it stays in the save, dead) and its body stays where it fell
    /// (see `remains`), fallen away from its killer, wearing what it wore.
    /// Those who saw come to fear the killer (see `fear`).
    pub fn kill(&mut self, cid: i64, how: &str, by: Option<ActorId>) {
        let who = ActorId::Npc(cid);
        let Some(pos) = self.actor(who).map(|a| a.pos) else { return };
        if self.cast.get(cid).is_none_or(|n| n.dead) {
            return;
        }
        let from = by.and_then(|b| self.actor(b)).map(|a| a.pos);
        if let Some(b) = by {
            self.hurt_by(who, b, 1.0, true);
        }
        if let Some(h) = self.actor(who).and_then(|a| a.held) {
            self.release(h);
        }
        // Off whatever carried it, and off it whoever rode it.
        if let Some(a) = self.actor_mut(who) {
            a.carried_by = None;
            a.riding = None;
            a.aboard = None;
        }
        for r in self.actor_ids() {
            if let Some(a) = self.actor_mut(r).filter(|a| a.riding == Some(who)) {
                a.riding = None;
            }
        }
        let remains = self.leave_remains(cid, from);
        // A phantom's gear is left for the taking.
        if self.cast.get(cid).is_some_and(|n| n.species.hostile) {
            self.drop_worn(cid, pos);
        }
        if remains.is_none() {
            for id in self.worn_by(who) {
                if let Some(t) = self.things.get_mut(id) {
                    t.worn = None;
                    t.asleep = false;
                    t.dirty = true;
                }
            }
        }
        let name = self.actor_name(who);
        self.social.joints.retain(|j| !j.has(who));
        if let Some(n) = self.cast.get_mut(cid) {
            n.dead = true;
            n.plan.clear();
            n.a.task = None;
            n.a.gesture = None;
            n.a.motion = None;
            n.a.jolt = None;
        }
        let msg = format!("{name} was {how}");
        self.note_near(pos, 40.0, Note::Notable(format!("{}.", super::physics::cap(&msg))));
        self.witness(pos, 40.0, &msg, 0.8, &[who]);
        self.event("died", Some(who), None, msg, Some(pos), json!({ "by": by.map(|b| b.key()) }));
    }

    /// Something this species eats, near `p`: food anyone eats, and for
    /// plant eaters, grass and leaves.
    pub fn food_for(&mut self, cid: i64, p: Vec3, range: f32) -> Option<(Target, Vec3)> {
        let diet = self.cast.get(cid)?.species.diet.clone();
        let strength = self.strength(ActorId::Npc(cid));
        let grazes = diet.grazes();
        self.nearest_matching(p, range, |pr| (pr[P_EDIBLE] > 0.05 && pr[P_MASS] < strength.max(1.0)) || (grazes && grazable(pr)))
    }

    /// Does this being eat plants as they grow (grass, leaves)?
    pub fn grazes(&self, who: ActorId) -> bool {
        self.species_of(who).is_some_and(|s| s.diet.grazes())
    }
}

/// A plant a grazer can eat where it grows: alive, small, not burning.
pub fn grazable(pr: &[f32]) -> bool {
    pr[P_ALIVE] > 0.0 && pr[P_MASS] < 1.0 && pr[P_FIRE] <= 0.0
}

fn with_article(s: &str) -> String {
    let l = s.to_lowercase();
    if l.starts_with("a ") || l.starts_with("an ") || l.starts_with("the ") || s.ends_with('!') {
        s.to_string()
    } else {
        format!("{} {s}", super::article(s))
    }
}

/// Doing things to beings, and bringing new ones into the world.
impl Sim {
    /// Would `b` let `actor` do this to them? Needs and feelings are free
    /// (feeding, kind words); changing what they wear or look like, or what
    /// they are, needs their trust. Ok, or the refusal to tell.
    pub fn being_consents(&mut self, actor: ActorId, b: ActorId, fx: &super::interp::BeingFx) -> Result<(), String> {
        if actor == b {
            return Ok(());
        }
        let need = if fx.turn_into.is_some() {
            0.0
        } else if !fx.look.is_empty() || fx.grow.is_some() || fx.reshape.is_some() {
            0.3
        } else if !fx.take_off.is_empty() {
            0.25
        } else if !fx.wear.is_empty() {
            0.15
        } else {
            return Ok(());
        };
        // A curse needs no consent, but it needs a world with its own forces.
        if fx.turn_into.is_some() {
            return Ok(());
        }
        let ActorId::Npc(c) = b else { return Ok(()) };
        let aff = self.social.affection(b, actor) - self.wariness(c);
        if aff >= need {
            return Ok(());
        }
        let (an, bn) = (self.actor_name(actor), self.actor_name(b));
        let at = self.actor(b).map(|x| x.pos);
        self.event("refused", Some(b), Some(actor.key()), format!("{bn} wouldn't let {an} do that"), at, json!({}));
        if self.speaks(b) {
            self.say_template(b, "no", &an);
        } else {
            self.make_noise(c, false);
        }
        Err(format!("{} won't let you.", super::physics::cap(&bn)))
    }

    /// The parts of a deed that change a being.
    /// A deed hurts `b` (themselves, when `b` is the actor): their health
    /// goes, less what they wear softens; it can kill.
    fn deed_hurts(&mut self, actor: ActorId, b: ActorId, hurt: f32, how: Option<&str>) {
        let hurt = hurt * (1.0 - self.protection(b));
        let Some(at) = self.actor(b).map(|a| a.pos) else { return };
        let (name, bname) = (self.actor_name(actor), self.actor_name(b));
        let how = match how.map(|h| h.trim().trim_end_matches('.').chars().take(90).collect::<String>()).filter(|h| !h.is_empty()) {
            Some(h) => h,
            None if actor == b => "hurt by their own hand".into(),
            None => format!("hurt by {name}"),
        };
        let died = self.wound(b, hurt, actor, &how);
        let msg = format!("{bname} was {how}");
        self.event("hurt_by_deed", Some(actor), Some(b.key()), msg.clone(), Some(at), json!({ "hurt": (hurt * 100.0).round() / 100.0 }));
        if let ActorId::Npc(c) = b {
            self.phantom_hurt(c, actor, None, died);
        }
        if let ActorId::Npc(c) = actor {
            if actor != b {
                self.phantom_struck(c, b, died);
            }
        }
        if died {
            if let ActorId::Npc(c) = b {
                self.kill(c, &format!("{how}, and died"), (actor != b).then_some(actor));
            }
            return;
        }
        let dir = self.actor(actor).map(|a| (at - a.pos).normalize_or_zero()).unwrap_or(Vec3::ZERO);
        self.jolt(b, dir * 0.2, (0.1 + hurt).min(0.4));
        if actor != b {
            self.hurt_by(b, actor, hurt, false);
            self.witness(at, 25.0, &msg, 0.6 + 0.3 * hurt, &[]);
        }
    }

    pub fn apply_being(&mut self, actor: ActorId, b: ActorId, fx: &super::interp::BeingFx) {
        let t = self.t;
        if let Some(h) = fx.hurt.filter(|h| h.is_finite() && *h > 0.0) {
            self.deed_hurts(actor, b, h.min(1.0), fx.how.as_deref());
            if matches!(b, ActorId::Npc(c) if self.cast.get(c).is_none_or(|n| n.dead)) {
                return;
            }
        }
        if let ActorId::Npc(c) = b {
            if let Some(n) = self.cast.get_mut(c) {
                for (k, v) in &fx.needs {
                    let v = if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
                    let x = match k.as_str() {
                        "hunger" => &mut n.needs.hunger,
                        "fatigue" | "tiredness" => &mut n.needs.fatigue,
                        "social" | "loneliness" => &mut n.needs.social,
                        "fun" | "boredom" => &mut n.needs.fun,
                        "curiosity" => &mut n.needs.curiosity,
                        _ => continue,
                    };
                    *x = (*x + v).clamp(0.0, 1.0);
                }
            }
        }
        if actor != b {
            for (k, v) in &fx.feel {
                let v = if v.is_finite() { v.clamp(-0.5, 0.5) } else { 0.0 };
                let r = self.social.rel_mut(b, actor);
                match k.as_str() {
                    "affection" | "love" | "liking" => r.affection = (r.affection + v).clamp(-1.0, 1.0),
                    "trust" => r.trust = (r.trust + v).clamp(-1.0, 1.0),
                    "rivalry" | "anger" => r.rivalry = (r.rivalry + v).clamp(0.0, 1.0),
                    _ => continue,
                }
                r.familiarity = (r.familiarity + 0.05).min(1.0);
                r.last = t;
            }
        }
        for name in fx.take_off.iter().take(3) {
            let n = name.trim().to_lowercase();
            if let Some(id) = self.worn_by(b).into_iter().find(|id| { let tn = self.thing_name(*id).to_lowercase(); tn.contains(&n) || n.contains(&tn) }) {
                let _ = self.take_off(actor, b, id);
            }
        }
        for m in fx.wear.iter().take(2) {
            let body = self.body_name(b);
            let at = self.actor(b).map(|a| a.pos).unwrap_or_default();
            match self.type_by_name(&m.name).filter(|ty| ty.ct.meta.fits.as_deref().is_none_or(|f| self.snap.layer_fits(f, &body))) {
                Some(ty) => {
                    let origin = super::things::Origin { made_by: Some(self.actor_name(actor)), ..Default::default() };
                    if let Some(id) = self.spawn_thing(ty.id, at, 0.0, 1.0, origin, false) {
                        if self.wear(actor, b, id).is_err() {
                            self.things.remove(id);
                        }
                    }
                }
                None => {
                    let id = self.next_id();
                    let key = m.name.trim().to_lowercase();
                    self.interp.building_names.insert(key, id);
                    self.interp.building.insert(id, super::interp::PendingBuild { name: m.name.clone(), by: Some(actor), wear_on: Some((b, actor)), ..Default::default() });
                    let props: Vec<(String, f32)> = m.props.iter().map(|(k, v)| (k.clone(), *v)).collect();
                    let desc = if m.description.is_empty() { m.name.clone() } else { m.description.clone() };
                    self.request_now(super::Request::BuildType { id, name: m.name.clone(), description: desc, size: m.size_m.unwrap_or([0.6, 0.6, 0.6]), props, fits: Some(body) });
                }
            }
        }
        if let ActorId::Npc(c) = b {
            let grow = fx.grow.filter(|g| g.is_finite() && *g > 0.0 && (*g - 1.0).abs() > 0.01);
            if !fx.look.is_empty() || grow.is_some() {
                let look = fx.look.clone();
                self.change_persona(c, |p| {
                    for (k, v) in look {
                        if v.is_finite() {
                            p.look.insert(k.trim().to_lowercase(), v);
                        }
                    }
                    if let Some(g) = grow {
                        p.size = Some((p.size_mul() * g).clamp(0.5, 4.0));
                    }
                });
            }
            if let Some(trick) = fx.learn.as_deref().map(|s| s.trim().to_lowercase().replace(['-', ' '], "_")).filter(|s: &String| !s.is_empty()) {
                if super::actor::GestureKind::parse(&trick).is_some() {
                    if let Some(n) = self.cast.get_mut(c) {
                        if !n.tricks.contains(&trick) {
                            n.tricks.push(trick.clone());
                            n.tricks.truncate(6);
                        }
                    }
                    let _ = self.act(b, Action::Gesture { kind: trick.clone(), to: None });
                    let (bn, an) = (self.actor_name(b), self.actor_name(actor));
                    self.event("learned", Some(b), Some(actor.key()), format!("{bn} learned to {} from {an}", trick.replace('_', " ")), self.actor(b).map(|a| a.pos), json!({ "trick": trick }));
                    self.social.bond(b, actor, 0.05, t);
                }
            }
            if let Some(sp) = fx.turn_into.as_deref() {
                let _ = self.transform_being(c, sp, actor);
            }
        }
        if actor != b {
            let fed = fx.needs.get("hunger").is_some_and(|v| *v < 0.0);
            let (an, bn) = (self.actor_name(actor), self.actor_name(b));
            self.event("deed_on", Some(actor), Some(b.key()), format!("{an} did something to {bn}"), self.actor(b).map(|a| a.pos), json!({ "fed": fed, "mood": !fx.feel.is_empty(), "looks": !fx.look.is_empty() || fx.grow.is_some(), "trust": fx.feel.get("trust").copied().unwrap_or(0.0) }));
        }
    }

    /// Rewrite one being's body as a deed says (a poofy tail, pointed
    /// ears): its own body from then on, written from the one it has, with
    /// the same roles, so its gestures and walk still work. Returns the
    /// build its story waits on.
    pub fn reshape_being(&mut self, by: ActorId, b: ActorId, change: &str) -> Result<Option<u64>, String> {
        let ActorId::Npc(c) = b else { return Err("the traveler's own body stays as it is".into()) };
        let change = change.trim();
        if change.is_empty() || !self.has_llm {
            return Ok(None);
        }
        let n = self.cast.get(c).ok_or("they are gone")?;
        let ty = self.snap.type_of(n.body_ty).cloned().ok_or("they have no body to change")?;
        let from = ty.name().to_string();
        let first = n.name().split_whitespace().next().unwrap_or("").to_string();
        // Type names hold 48 characters.
        let mut name = format!("{} ({first} #{c})", n.species.name);
        if name.chars().count() > 48 {
            name = format!("{} (#{c})", n.species.name);
        }
        let id = self.next_id();
        let change: String = change.chars().take(300).collect();
        self.interp.building.insert(id, super::interp::PendingBuild { name: name.clone(), by: Some(by), change: change.clone(), body_of: Some(c), ..Default::default() });
        self.request_now(super::Request::ReshapeBody { id, name, from, source: ty.ct.source.clone(), change });
        Ok(Some(id))
    }

    /// A being's reshaped body is written (or came to nothing).
    pub fn on_body_reshaped(&mut self, id: u64, cid: i64, b: super::interp::PendingBuild, type_id: Option<u32>) {
        if let Some(by) = b.by {
            self.deed_landed(by);
        }
        let who = ActorId::Npc(cid);
        let at = self.actor(who).map(|a| a.pos).unwrap_or_default();
        let name = self.actor_name(who);
        let body = type_id.and_then(|t| self.snap.type_of(t)).filter(|t| t.ct.meta.body.is_some()).map(|t| t.name().to_string());
        let Some(body) = body.filter(|_| self.cast.get(cid).is_some()) else {
            self.release_deeds(id, false);
            if self.interp.stories.remove(&id).is_some() {
                self.note_near(at, 30.0, Note::Info(format!("{} stays as they were.", super::physics::cap(&name))));
            }
            return;
        };
        let before = self.cast.get(cid).map(|n| n.a.dims.height).unwrap_or(1.0);
        self.change_persona(cid, |p| p.body = body.clone());
        // Drawn at another scale than the body it came from: same height.
        let after = self.cast.get(cid).map(|n| n.a.dims.height).unwrap_or(before);
        let r = before / after.max(0.01);
        if !(0.67..=1.5).contains(&r) {
            self.change_persona(cid, |p| p.size = Some((p.size_mul() * r).clamp(0.5, 4.0)));
        }
        self.tell_held_stories(id, at);
        let by = b.by.map(|x| self.actor_name(x)).unwrap_or_default();
        self.event("reshaped_being", b.by, Some(who.key()), format!("{by} changed {name}'s shape: {}", b.change), Some(at), json!({ "body": body, "change": b.change }));
    }

    /// Species still drawn with a generic body (a cat on the four-legged
    /// "quadruped") get their own, written from it: once each, for those
    /// about. Until it comes they keep the generic one.
    pub fn ask_species_bodies(&mut self) {
        if !self.has_llm || !self.cfg.own_bodies {
            return;
        }
        let mut asks: Vec<(String, String)> = Vec::new();
        for n in self.cast.npcs.iter().filter(|n| n.here() && !n.dead && !n.species.is_human()) {
            let sp = &n.species;
            if self.interp.bodies_asked.contains(&sp.name) || asks.iter().any(|a| a.0 == sp.name) {
                continue;
            }
            let generic = self.snap.body_named(&sp.body).is_some_and(|b| b.builtin && crate::brain::TEMPLATE_BODIES.contains(&b.name()));
            if generic {
                asks.push((sp.name.clone(), serde_json::to_string(sp.as_ref()).unwrap_or_default()));
            }
        }
        for (name, json) in asks {
            let template = self.snap.species.get(&name).map(|s| s.body.clone()).unwrap_or_default();
            self.interp.bodies_asked.insert(name);
            let id = self.next_id();
            self.request_now(super::Request::SpeciesBody { id, species: json, template });
        }
    }

    /// Whether this world has forces of its own (magic, curses…): its own
    /// properties, or the `sim.transform` setting.
    pub fn world_has_magic(&self) -> bool {
        self.cfg.transform || self.vocab.names.len() > super::props::BUILTIN.len()
    }

    /// A being turns into another species (a curse, a spell). It keeps its
    /// name, memories and relationships; what no longer fits falls off.
    pub fn transform_being(&mut self, cid: i64, species: &str, by: ActorId) -> Result<(), String> {
        let who = ActorId::Npc(cid);
        if !self.world_has_magic() {
            return Err("nothing like that happens in this world".into());
        }
        let Some(sp) = self.snap.species.get(species).cloned() else { return Err(format!("there are no {species}s here")) };
        let from = self.cast.get(cid).map(|n| n.species.name.clone()).unwrap_or_default();
        if from == sp.name {
            return Ok(());
        }
        let name = sp.name.clone();
        self.change_persona(cid, |p| {
            p.species = if name == "human" { String::new() } else { name.clone() };
            p.variety.clear();
            p.look.clear();
            p.body.clear();
        });
        // Layers made for the old body fall off.
        let body = self.body_name(who);
        for id in self.worn_by(who) {
            let fits = self.things.get(id).and_then(|t| self.snap.type_of(t.type_id)).and_then(|ty| ty.ct.meta.fits.clone());
            if fits.is_some_and(|f| !self.snap.layer_fits(&f, &body)) {
                if let Some(t) = self.things.get_mut(id) {
                    t.worn = None;
                    t.asleep = false;
                }
            }
        }
        if let Some(h) = self.actor(who).and_then(|a| a.held).filter(|h| self.things.get(*h).is_some_and(|t| t.mass() > self.strength(who))) {
            self.release(h);
        }
        let (n, b) = (self.actor_name(who), self.actor_name(by));
        let at = self.actor(who).map(|a| a.pos);
        let msg = format!("{n} turned from {} {from} into {} {}", super::article(&from), super::article(&sp.name), sp.name);
        self.event("transformed", Some(who), Some(by.key()), msg.clone(), at, json!({ "from": from, "to": sp.name, "by": b }));
        if let Some(p) = at {
            self.note_near(p, 30.0, Note::Notable(format!("{}!", super::physics::cap(&msg))));
            self.witness(p, 30.0, &msg, 0.8, &[]);
        }
        Ok(())
    }

    /// Change a character's persona (saved), then fit them to it again.
    pub fn change_persona(&mut self, cid: i64, f: impl FnOnce(&mut crate::world::Persona)) {
        let Some(n) = self.cast.get_mut(cid) else { return };
        let mut def = (*n.def).clone();
        f(&mut def.persona);
        let json = serde_json::to_string(&def.persona).unwrap_or_default();
        n.def = std::sync::Arc::new(def);
        let snap = self.snap.clone();
        let seed = self.seed;
        if let Some(n) = self.cast.get_mut(cid) {
            super::npc::fit(n, &snap, seed);
        }
        let _ = self.db.with(|c| crate::db::set_persona(c, cid, &json));
    }

    /// Bring a new being into the world (born, made, conjured) next to `at`.
    pub fn add_being(&mut self, persona: crate::world::Persona, at: Vec3, state: crate::world::characters::SavedState) -> Option<i64> {
        let home = Vec3::new(at.x, self.snap.terrain.height(at.x, at.z), at.z);
        let json = serde_json::to_string(&persona).ok()?;
        let version = self.snap.version.max(1);
        let id = self.db.with(|c| crate::db::add_character(c, crate::world::region_of(home.x, home.z), &json, home.x, home.z, version)).ok()?;
        let def = std::sync::Arc::new(crate::world::CharacterDef { id, persona, home, state, version });
        let snap = self.snap.clone();
        self.cast.add(def, &snap, self.seed);
        self.fit_bodies();
        let book = snap.species.clone();
        self.social.seed_from_personas(&self.cast, &book);
        Some(id)
    }

    /// A deed brings a being into the world (where the world allows it):
    /// it belongs to whoever made it.
    pub fn make_being(&mut self, by: ActorId, nb: &super::interp::NewBeing) -> Result<i64, String> {
        if !self.cfg.create_beings {
            return Err("beings can't be made in this world".into());
        }
        let Some(sp) = self.snap.species.get(&nb.species).cloned() else { return Err(format!("no such species: {}", nb.species)) };
        let n_of = self.cast.npcs.iter().filter(|n| n.here() && n.species.name == sp.name).count();
        if n_of >= self.cfg.max_creatures * 4 {
            return Err("there are enough of them".into());
        }
        let maker = self.actor_name(by);
        let first = maker.split_whitespace().next().unwrap_or("").to_string();
        let name = if nb.name.trim().is_empty() { format!("the {}", sp.name) } else { nb.name.trim().chars().take(32).collect() };
        let persona = crate::world::Persona {
            name: name.clone(),
            species: if sp.is_human() { String::new() } else { sp.name.clone() },
            appearance: nb.description.chars().take(160).collect(),
            look: nb.look.iter().filter(|(_, v)| v.is_finite()).map(|(k, v)| (k.trim().to_lowercase(), *v)).collect(),
            size: nb.size.filter(|s| s.is_finite() && (*s - 1.0).abs() > 0.01).map(|s| s.clamp(0.5, 4.0)),
            relationships: vec![format!("{}: owner and maker", if first == "the" { "the traveler" } else { &maker })],
            ..Default::default()
        };
        let at = self.actor(by).map(|a| a.pos + a.forward() * 2.5).unwrap_or_default();
        let state = crate::world::characters::SavedState { x: at.x, z: at.z, born: self.t, ..Default::default() };
        let id = self.add_being(persona, at, state).ok_or("it didn't take")?;
        let me = ActorId::Npc(id);
        let r = self.social.rel_mut(me, by);
        r.owner = Some(by.code());
        r.affection = r.affection.max(0.7);
        r.familiarity = 1.0;
        self.note_creation(&format!("being:{id}"), "being", &format!("{name}, {} {}", super::article(&sp.name), sp.name), Some(by), "", at);
        let msg = format!("{maker} brought {name} into the world");
        self.event("made_being", Some(by), Some(me.key()), msg.clone(), Some(at), json!({ "species": sp.name }));
        self.note_near(at, 30.0, Note::Made(format!("{}.", super::physics::cap(&msg))));
        let extent = self.actor(me).map(|a| a.dims.height).unwrap_or(1.0);
        let sight = super::surprise::Sight { how: super::surprise::Arrival::FromNowhere, extent, strange: 0.0 };
        let tell = format!("{}, near you.", super::physics::cap(&msg));
        self.startle(30.0, &super::surprise::News { at, sight, memory: &format!("{msg}."), importance: 0.7, event: "new_being", tell: &tell, what: &name }, &[me]);
        Ok(id)
    }
}
