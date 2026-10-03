//! Lives over time: pairs who love each other have young, the young grow up
//! and follow their parents, and over generations a line drifts with what
//! happens to it (fed by people: tamer; hunted: warier) until it is a
//! variety of its own, and then a species. All in numbers; no LLM.

use super::{ActorId, Note, Sim};
use crate::render::sky::DAY_SECONDS;
use crate::world::characters::SavedState;
use crate::world::species::{Mind, Temper, Variety};
use glam::Vec3;
use serde_json::json;

/// Game days from one litter to the next, and to grow up.
const BIRTH_GAP_DAYS: f64 = 1.0;
const GROW_DAYS: f64 = 2.0;
/// How far a line's temper must drift from its species' to be a variety,
/// and then a species of its own.
const VARIETY_DRIFT: f32 = 0.1;
const SPECIES_DRIFT: f32 = 0.3;

const SYLLABLES: [&str; 16] = ["ka", "ri", "mo", "ta", "len", "si", "bo", "ru", "na", "fe", "dor", "li", "pa", "vin", "to", "ash"];

impl Sim {
    /// Grown-up fraction for someone born at `born`.
    pub fn growth_at(&self, born: f64) -> f32 {
        if born <= 0.0 {
            return 1.0;
        }
        let days = (self.t - born) / DAY_SECONDS * self.cfg.life_speed as f64;
        (0.3 + 0.7 * (days / GROW_DAYS)).clamp(0.3, 1.0) as f32
    }

    /// The young grow (their bodies refitted as they do).
    pub fn grow(&mut self, cid: i64) {
        let Some(n) = self.cast.get(cid) else { return };
        if n.born <= 0.0 || n.growth >= 1.0 {
            return;
        }
        let g = self.growth_at(n.born);
        if g - n.growth > 0.03 || (g >= 1.0 && n.growth < 1.0) {
            let snap = self.snap.clone();
            let seed = self.seed;
            if let Some(n) = self.cast.get_mut(cid) {
                n.growth = g;
                super::npc::fit(n, &snap, seed);
            }
        }
    }

    /// A parent to stay near, for the young.
    pub fn parent_of(&self, cid: i64) -> Option<ActorId> {
        let n = self.cast.get(cid)?;
        if n.growth >= 0.8 {
            return None;
        }
        let pos = n.a.pos;
        n.parents
            .iter()
            .filter_map(|p| self.cast.get(*p).filter(|m| !m.dead).map(|m| (m.def.id, (m.a.pos - pos).length())))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| ActorId::Npc(x.0))
    }

    /// Once a game minute: pairs that may have young.
    pub fn step_life(&mut self) {
        if self.cfg.life_speed <= 0.0 {
            return;
        }
        let t = self.t;
        let gap = BIRTH_GAP_DAYS * DAY_SECONDS / self.cfg.life_speed as f64;
        let ready: Vec<(i64, Vec3, u32, bool)> = self
            .cast
            .npcs
            .iter()
            .filter(|n| !n.dead && !n.a.asleep && n.growth >= 0.99 && n.needs.hunger < 0.6 && t - n.last_birth > gap && n.a.riding.is_none())
            .filter(|n| self.dist_to_player(n.a.pos) <= self.cfg.medium)
            .map(|n| (n.def.id, n.a.pos, n.body_ty, n.species.mind == Mind::Sapient))
            .collect();
        let mut pairs = Vec::new();
        for (i, (a, pa, ba, sa)) in ready.iter().enumerate() {
            for (b, pb, bb, sb) in ready.iter().skip(i + 1) {
                // Mixing is open to any two of one body.
                if ba != bb || sa != sb || (*pa - *pb).length() > 8.0 {
                    continue;
                }
                let (x, y) = (ActorId::Npc(*a), ActorId::Npc(*b));
                let Some(r) = self.social.rel(x, y) else { continue };
                if r.family && !r.partner {
                    continue;
                }
                let bonded = if *sa { r.partner && r.affection > 0.6 } else { r.affection > 0.6 };
                if bonded {
                    pairs.push((*a, *b));
                }
            }
        }
        for (a, b) in pairs {
            // Kinds that descend from one another share the limit.
            let sp = self.cast.get(a).map(|n| n.species.root().to_string()).unwrap_or_default();
            let at = self.cast.get(a).map(|n| n.a.pos).unwrap_or_default();
            // A neighbourhood (about a region across) holds only so many.
            let here = self.cast.npcs.iter().filter(|n| !n.dead && n.species.root() == sp && (n.a.pos - at).length() < 150.0).count();
            let cap = self.cfg.max_creatures.max(2);
            if here >= cap {
                continue;
            }
            // Fewer births as a place fills up.
            let p = 0.12 * self.cfg.life_speed.min(10.0) * (1.0 - here as f32 / cap as f32);
            if self.rand() < p {
                self.birth(a, b);
            }
        }
    }

    /// The best feeling a being has for any person (being fed and handled).
    fn handled(&self, cid: i64) -> f32 {
        let me = ActorId::Npc(cid);
        self.actor_ids()
            .into_iter()
            .filter(|o| *o != me && self.mind_of(*o) == Mind::Sapient)
            .map(|o| self.social.affection(me, o))
            .fold(-1.0, f32::max)
    }

    fn name_for(&mut self) -> String {
        let n = 2 + (self.rand() * 1.5) as usize;
        let mut s = String::new();
        for _ in 0..n {
            s.push_str(SYLLABLES[(self.rand() * SYLLABLES.len() as f32) as usize % SYLLABLES.len()]);
        }
        let mut c = s.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or(s)
    }

    /// Young for a pair: their looks blended with a little drift, their
    /// temper blended and nudged by what their parents lived through.
    pub fn birth(&mut self, a: i64, b: i64) -> Option<i64> {
        let coin = self.rand();
        let (Some(na), Some(nb)) = (self.cast.get(a), self.cast.get(b)) else { return None };
        let species = if coin < 0.5 { na.def.persona.species.clone() } else { nb.def.persona.species.clone() };
        let variety = na.def.persona.variety.clone();
        let body = self.snap.type_of(na.body_ty).and_then(|t| t.ct.meta.body.clone()).unwrap_or_default();
        let (sa, sb) = (na.sliders, nb.sliders);
        let (ta, tb) = (na.temper, nb.temper);
        let lineage = na.lineage;
        let at = na.a.pos;
        let owner = self.owner_of(a).or_else(|| self.owner_of(b));
        let frights = na.frights + nb.frights;
        let sapient = na.species.mind == Mind::Sapient;
        let (an, bn) = (na.name().to_string(), nb.name().to_string());
        let mut look = crate::world::species::LookMap::new();
        for (i, (name, lo, hi)) in body.look.iter().enumerate().take(5) {
            let mid = (sa[i] + sb[i]) * 0.5 + (self.rand() - 0.5) * 0.1 * (hi - lo);
            look.insert(name.clone(), mid.clamp(lo.min(*hi), hi.max(*lo)));
        }
        let handled = self.handled(a).max(self.handled(b));
        let j = |s: &mut Sim| (s.rand() - 0.5) * 0.06;
        let mut temper = Temper {
            bold: (ta.bold + tb.bold) * 0.5 + j(self),
            wary: (ta.wary + tb.wary) * 0.5 + j(self),
            playful: (ta.playful + tb.playful) * 0.5 + j(self),
            tame: (ta.tame + tb.tame) * 0.5 + j(self),
        };
        if handled > 0.5 {
            temper.tame += 0.06;
            temper.wary -= 0.03;
        }
        if frights >= 3 {
            temper.wary += 0.05;
            temper.tame -= 0.03;
        }
        for v in [&mut temper.bold, &mut temper.wary, &mut temper.playful, &mut temper.tame] {
            *v = v.clamp(0.0, 1.0);
        }
        let name = self.name_for();
        let persona = crate::world::Persona {
            name: name.clone(),
            age: 0,
            species,
            variety,
            look,
            temper: Some(temper),
            personality: if sapient { "a small child, curious and playful".into() } else { String::new() },
            ..Default::default()
        };
        let side = Vec3::new((self.rand() - 0.5) * 2.0, 0.0, (self.rand() - 0.5) * 2.0);
        let p = at + side;
        let state = SavedState { x: p.x, z: p.z, born: self.t, parents: vec![a, b], lineage, ..Default::default() };
        let id = self.add_being(persona, p, state)?;
        let kid = ActorId::Npc(id);
        for parent in [a, b] {
            let r = self.social.rel_mut(kid, ActorId::Npc(parent));
            r.family = true;
            r.affection = r.affection.max(0.85);
            r.trust = r.trust.max(0.8);
            r.familiarity = 1.0;
        }
        // Brothers and sisters.
        let sibs: Vec<i64> = self.cast.npcs.iter().filter(|n| n.def.id != id && !n.dead && n.parents.iter().any(|p| *p == a || *p == b)).map(|n| n.def.id).collect();
        for s in sibs {
            let r = self.social.rel_mut(kid, ActorId::Npc(s));
            r.family = true;
            r.affection = r.affection.max(0.6);
            r.familiarity = r.familiarity.max(0.8);
        }
        // Their people's animals' young are theirs too.
        if let Some(o) = owner {
            let r = self.social.rel_mut(kid, o);
            r.owner = Some(o.code());
            r.affection = r.affection.max(0.5);
        }
        let t = self.t;
        for parent in [a, b] {
            if let Some(n) = self.cast.get_mut(parent) {
                n.last_birth = t;
                n.needs.hunger = (n.needs.hunger + 0.2).min(1.0);
            }
        }
        let msg = format!("{an} and {bn} have a young one, {name}");
        self.event("born", Some(kid), Some(ActorId::Npc(a).key()), msg.clone(), Some(p), json!({ "parents": [a, b], "lineage": lineage, "tame": temper.tame, "wary": temper.wary }));
        self.note_near(p, 30.0, Note::Notable(format!("{msg}.")));
        self.witness(p, 30.0, &msg, 0.6, &[]);
        self.drift(lineage);
        Some(id)
    }

    /// A line far enough from its species becomes a variety, then a species.
    fn drift(&mut self, lineage: i64) {
        let mut line: Vec<(f64, i64)> = self.cast.npcs.iter().filter(|n| !n.dead && n.born > 0.0 && n.lineage == lineage).map(|n| (n.born, n.def.id)).collect();
        if line.len() < 2 {
            return;
        }
        // What the line is now: its youngest.
        line.sort_by(|a, b| b.0.total_cmp(&a.0));
        let members: Vec<i64> = line.iter().take(3).map(|x| x.1).collect();
        let Some(first) = self.cast.get(members[0]) else { return };
        let species = first.species.clone();
        let current = first.def.persona.variety.clone();
        let base = species.temper;
        let k = members.len() as f32;
        let mean = |f: &dyn Fn(&Temper) -> f32| members.iter().filter_map(|m| self.cast.get(*m)).map(|n| f(&n.temper)).sum::<f32>() / k;
        let (tame, wary) = (mean(&|t| t.tame), mean(&|t| t.wary));
        let (dt, dw) = (tame - base.tame, wary - base.wary);
        let drift = dt.abs().max(dw.abs());
        if drift < VARIETY_DRIFT {
            return;
        }
        let word = if dt.abs() >= dw.abs() { if dt > 0.0 { "tame" } else { "wild" } } else if dw > 0.0 { "shy" } else { "bold" };
        let plural = species.plural();
        let new_species = drift >= SPECIES_DRIFT && species.mind != Mind::Sapient;
        let name = format!("{word} {}", if new_species { species.name.clone() } else { plural.clone() });
        if (!new_species && current == name) || (new_species && species.name == name) {
            return;
        }
        let parent_name = self.cast.get(members[0]).and_then(|n| n.parents.first().copied()).map(|p| self.actor_name(ActorId::Npc(p))).unwrap_or_default();
        let at = self.cast.get(members[0]).map(|n| n.a.pos);
        if new_species {
            // Same body, its own numbers and name; everyone of the line takes it.
            let mut sp = (*species).clone();
            sp.kin_of = Some(species.root().to_string());
            sp.name = name.clone();
            sp.plural = String::new();
            sp.temper = Temper { tame, wary, ..base };
            sp.description = format!("{} descended from {}'s line, {word} now", plural, parent_name);
            sp.varieties.clear();
            if let Ok(j) = serde_json::to_string(&sp) {
                let _ = self.db.with(|c| crate::db::put_species(c, &sp.name, &j));
            }
            let mut book = (*self.snap.species).clone();
            book.add(sp.clone());
            self.set_species_book(book);
            for m in &members {
                let n = name.clone();
                self.change_persona(*m, |p| {
                    p.species = n;
                    p.variety.clear();
                });
            }
            let msg = format!("{}'s line are {} now, a kind of their own", parent_name, sp.plural());
            self.event("new_species", None, None, msg.clone(), at, json!({ "species": name, "from": species.name, "lineage": lineage }));
            if let Some(p) = at {
                self.note_near(p, 40.0, Note::Notable(format!("{msg}.")));
                self.witness(p, 40.0, &msg, 0.6, &[]);
            }
            return;
        }
        // A variety of the species: the line's looks and temper, by name.
        let body = self.snap.type_of(first.body_ty).and_then(|t| t.ct.meta.body.clone()).unwrap_or_default();
        let mut look = std::collections::BTreeMap::new();
        for (i, (sname, _, _)) in body.look.iter().enumerate().take(5) {
            let vals: Vec<f32> = members.iter().filter_map(|m| self.cast.get(*m)).map(|n| n.sliders[i]).collect();
            let (lo, hi) = (vals.iter().copied().fold(f32::MAX, f32::min), vals.iter().copied().fold(f32::MIN, f32::max));
            look.insert(sname.clone(), [lo, hi]);
        }
        let mut sp = (*species).clone();
        sp.varieties.retain(|v| v.name != name);
        sp.varieties.push(Variety { name: name.clone(), look, layers: Vec::new() });
        if let Ok(j) = serde_json::to_string(&sp) {
            let _ = self.db.with(|c| crate::db::put_species(c, &sp.name, &j));
        }
        let mut book = (*self.snap.species).clone();
        book.add(sp);
        self.set_species_book(book);
        for m in &members {
            let n = name.clone();
            self.change_persona(*m, |p| p.variety = n);
        }
        let msg = format!("people have started calling {}'s line the {name}", parent_name);
        self.event("new_variety", None, None, msg.clone(), at, json!({ "variety": name, "species": species.name, "lineage": lineage, "tame": tame, "wary": wary }));
        if let Some(p) = at {
            self.note_near(p, 40.0, Note::Notable(format!("{}.", super::physics::cap(&msg))));
            self.witness(p, 40.0, &msg, 0.5, &[]);
        }
    }

    /// Use a changed species book now (the world's next version brings it from the save too).
    pub fn set_species_book(&mut self, book: crate::world::species::SpeciesBook) {
        let mut snap = (*self.snap).clone();
        snap.species = std::sync::Arc::new(book);
        self.snap = std::sync::Arc::new(snap);
    }
}
