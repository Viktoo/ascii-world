//! The live world as a guest sees it. The host tells each guest what
//! changed near them (things, beings, burning ground, flashes, the weather),
//! remembering what it last told them; the guest's copy takes it in and is
//! drawn from it, never simulated itself.
//!
//! Actors are named on the wire by the host's codes: on a guest's copy the
//! host's own traveler is traveler 0 and the guest is the player.

use super::things::{Origin, Shape, Thing};
use super::{ActorId, Sim};
use crate::net::wire::{CellState, NpcFull, NpcMove, ThingFull, ThingMove, WorldNear};
use glam::Vec3;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// How far from a guest things and burning ground are kept up to date (m).
pub const THINGS_NEAR: f32 = crate::render::VIEW_DIST;
/// Beings are drawn farther off.
pub const BEINGS_NEAR: f32 = 200.0;

/// What a guest was last told, to tell them only what changed.
#[derive(Default)]
pub struct Seen {
    things: HashMap<i64, (u64, u64)>,
    npcs: HashMap<i64, (u64, u64, u64)>,
    cells: HashMap<(i32, i32), u64>,
    spent: u64,
    flashes_to: f64,
    blasts_to: f64,
    conjuring: u64,
    weather: u64,
}

/// A guest's copy's own: what it can't draw yet, and the host's conjurings.
#[derive(Default)]
pub struct Mirror {
    /// This copy is a guest's (the host runs the world).
    pub on: bool,
    /// The guest's number in the host's world.
    pub you: u8,
    /// Things whose type hasn't come yet (the world is being made anew).
    pending: BTreeMap<i64, ThingFull>,
    /// Where the host's world is conjuring things now (and whether by this guest).
    pub conjuring: Vec<(Vec3, bool)>,
    /// Where each being is going, as last heard (they are drawn moving
    /// there smoothly between messages).
    npc_to: HashMap<i64, crate::net::wire::Body>,
}

fn hash_of<T: serde::Serialize>(v: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::net::encode(v).hash(&mut h);
    h.finish()
}

/// A host's actor as the guest `you` knows them.
pub fn from_host(code: i64, you: u8) -> ActorId {
    match ActorId::from_code(code) {
        ActorId::Player => ActorId::Traveler(0),
        ActorId::Traveler(n) if n == you => ActorId::Player,
        a => a,
    }
}


fn thing_move(t: &Thing) -> ThingMove {
    ThingMove { id: t.id, pos: t.pos.to_array(), vel: t.vel.to_array(), yaw: t.yaw, state: t.state, tilt: t.shape.tilt.unwrap_or([0.0; 4]), asleep: t.asleep }
}

fn thing_full(t: &Thing) -> ThingFull {
    let mut shape = t.shape.clone();
    shape.tilt = None;
    let origin = Origin { made_by: None, made_from: Vec::new(), ..t.origin.clone() };
    ThingFull {
        id: t.id,
        type_id: t.type_id,
        scale: t.scale,
        params: t.params,
        look: t.look.clone(),
        props: t.props.clone(),
        holder: t.holder.map(|a| a.code()),
        co_holder: t.co_holder.map(|a| a.code()),
        worn: t.worn.map(|a| a.code()),
        anchored: t.anchored,
        point_first: t.point_first,
        born: t.born,
        origin: serde_json::to_string(&origin).unwrap_or_default(),
        shape: serde_json::to_string(&shape).unwrap_or_default(),
        m: ThingMove::default(),
    }
}

fn npc_move(n: &super::npc::Npc) -> NpcMove {
    NpcMove { id: n.def.id, body: crate::net::wire::Body::of(&n.a, 0.0), growth: n.growth, away: n.away, dead: n.dead, held: n.a.held }
}

impl Sim {
    /// What changed near `at` (a guest, number `you`) since `seen` was last
    /// brought up to date.
    pub fn world_near(&self, seen: &mut Seen, at: Vec3, you: u8) -> WorldNear {
        let mut w = WorldNear { t: self.t, ..Default::default() };
        let flat = |p: Vec3| ((p.x - at.x).powi(2) + (p.z - at.z).powi(2)).sqrt();
        // Things.
        let mut here = HashSet::new();
        for t in self.things.live() {
            if flat(t.pos) > THINGS_NEAR && t.holder.is_none_or(|h| h != ActorId::Traveler(you)) {
                continue;
            }
            here.insert(t.id);
            let full = thing_full(t);
            let m = thing_move(t);
            let (hf, hm) = (hash_of(&full), hash_of(&m));
            match seen.things.get(&t.id) {
                Some((f, _)) if *f != hf => w.things.push(ThingFull { m, ..full }),
                None => w.things.push(ThingFull { m, ..full }),
                Some((_, mm)) if *mm != hm => w.moved.push(m),
                _ => {}
            }
            seen.things.insert(t.id, (hf, hm));
        }
        seen.things.retain(|id, _| {
            let keep = here.contains(id);
            if !keep {
                w.things_gone.push(*id);
            }
            keep
        });
        // Beings.
        let mut here = HashSet::new();
        for n in &self.cast.npcs {
            if flat(n.a.pos) > BEINGS_NEAR || (n.away && !seen.npcs.contains_key(&n.def.id)) {
                continue;
            }
            here.insert(n.def.id);
            let m = npc_move(n);
            let full = (serde_json::to_string(&n.def.persona).unwrap_or_default(), n.def.home.to_array(), n.def.version);
            let (hf, hm, hp) = (hash_of(&full), hash_of(&m), hash_of(&n.props));
            match seen.npcs.get(&n.def.id) {
                Some((f, _, _)) if *f == hf => {
                    let (_, mm, pp) = seen.npcs[&n.def.id];
                    if mm != hm {
                        w.npcs_moved.push(m);
                    }
                    if pp != hp {
                        w.npcs_props.push((n.def.id, n.props.clone()));
                    }
                }
                _ => w.npcs.push(NpcFull { id: n.def.id, persona: full.0, home: full.1, version: full.2, props: n.props.clone(), m }),
            }
            seen.npcs.insert(n.def.id, (hf, hm, hp));
        }
        seen.npcs.retain(|id, _| {
            let keep = here.contains(id);
            if !keep {
                w.npcs_gone.push(*id);
            }
            keep
        });
        // Ground alive with something (burning, wet, charred).
        let mut here = HashSet::new();
        for (k, c) in &self.field.cells {
            if flat(c.pos) > THINGS_NEAR {
                continue;
            }
            here.insert(*k);
            let s = CellState { at: *k, type_id: c.type_id, pos: c.pos.to_array(), size: c.size, small: c.small, water: c.water, active: c.active, props: c.props.clone() };
            let h = hash_of(&s);
            if seen.cells.get(k) != Some(&h) {
                w.cells.push(s);
                seen.cells.insert(*k, h);
            }
        }
        seen.cells.retain(|k, _| {
            let keep = here.contains(k);
            if !keep {
                w.cells_gone.push(*k);
            }
            keep
        });
        let spent: Vec<((i32, i32), f64)> = self.things.spent_cells.iter().map(|(k, v)| (*k, *v)).collect();
        let h = hash_of(&spent);
        if h != seen.spent {
            seen.spent = h;
            w.spent = Some(spent);
        }
        // Flashes and blasts not yet told.
        w.flashes = self.flashes.iter().filter(|(_, t)| *t > seen.flashes_to).map(|(p, t)| (p.to_array(), *t)).collect();
        seen.flashes_to = self.flashes.iter().map(|f| f.1).fold(seen.flashes_to, f64::max);
        w.blasts = self.blasts.iter().filter(|(_, t, _)| *t > seen.blasts_to).map(|(p, t, r)| (p.to_array(), *t, *r)).collect();
        seen.blasts_to = self.blasts.iter().map(|b| b.1).fold(seen.blasts_to, f64::max);
        // What is being conjured, and where.
        let conj: Vec<([f32; 3], bool)> = self.work().into_iter().filter(|x| x.appears).filter_map(|x| x.at.map(|a| (a.to_array(), x.who == Some(ActorId::Traveler(you))))).collect();
        let h = hash_of(&conj);
        if h != seen.conjuring {
            seen.conjuring = h;
            w.conjuring = Some(conj);
        }
        let weather = serde_json::to_string(&self.weather_st).unwrap_or_default();
        let h = hash_of(&weather);
        if h != seen.weather {
            seen.weather = h;
            w.weather = Some(weather);
        }
        w
    }

    /// A guest's copy takes in what changed near it.
    pub fn take_world(&mut self, w: WorldNear) {
        let you = self.mirror.you;
        let mut overlay = false;
        for id in w.things_gone {
            self.unhold(id);
            self.things.forget(id);
            self.mirror.pending.remove(&id);
            overlay = true;
        }
        for f in w.things {
            overlay |= self.things.get(f.id).is_none_or(|t| t.origin.instance.is_none() && t.origin.cell.is_none());
            self.mirror_thing(f, you);
        }
        for m in w.moved {
            if let Some(t) = self.things.get_mut(m.id) {
                apply_move(t, &m);
            } else if let Some(p) = self.mirror.pending.get_mut(&m.id) {
                p.m = m;
            }
        }
        let snap = self.snap.clone();
        let seed = self.seed;
        for f in w.npcs {
            let persona: crate::world::Persona = serde_json::from_str(&f.persona).unwrap_or_default();
            match self.cast.get_mut(f.id) {
                Some(n) => {
                    if serde_json::to_string(&n.def.persona).unwrap_or_default() != f.persona {
                        let mut def = (*n.def).clone();
                        def.persona = persona;
                        n.def = std::sync::Arc::new(def);
                        super::npc::fit(n, &snap, seed);
                    }
                }
                None => {
                    let state = crate::world::characters::SavedState { x: f.m.body.pos[0], z: f.m.body.pos[2], yaw: f.m.body.yaw, ..Default::default() };
                    let def = std::sync::Arc::new(crate::world::CharacterDef { id: f.id, persona, home: Vec3::from_array(f.home), state, version: f.version });
                    self.cast.add(def, &snap, seed);
                }
            }
            if let Some(n) = self.cast.get_mut(f.id) {
                n.props = f.props;
            }
            self.mirror_npc(f.m);
        }
        for m in w.npcs_moved {
            self.mirror_npc(m);
        }
        for (id, p) in w.npcs_props {
            if let Some(n) = self.cast.get_mut(id) {
                n.props = p;
            }
        }
        for id in w.npcs_gone {
            if let Some(n) = self.cast.get_mut(id) {
                n.away = true;
            }
        }
        for c in w.cells {
            overlay = true;
            self.field.cells.insert(c.at, super::env::Cell { props: c.props, type_id: c.type_id, pos: Vec3::from_array(c.pos), size: c.size, small: c.small, water: c.water, active: c.active, fired: Vec::new(), spent_at: None });
        }
        for k in w.cells_gone {
            overlay = true;
            self.field.cells.remove(&k);
        }
        if let Some(spent) = w.spent {
            overlay = true;
            self.things.spent_cells = spent.into_iter().collect();
            for k in self.things.spent_cells.keys() {
                self.things.taken.entry(*k).or_insert(-1);
            }
        }
        self.flashes.extend(w.flashes.into_iter().map(|(p, t)| (Vec3::from_array(p), t)));
        self.blasts.extend(w.blasts.into_iter().map(|(p, t, r)| (Vec3::from_array(p), t, r)));
        // Only the recent are drawn.
        let t = self.t;
        self.flashes.retain(|f| t - f.1 < 10.0);
        self.blasts.retain(|b| t - b.1 < 10.0);
        if let Some(c) = w.conjuring {
            self.mirror.conjuring = c.into_iter().map(|(p, mine)| (Vec3::from_array(p), mine)).collect();
        }
        if let Some(st) = w.weather.and_then(|j| serde_json::from_str(&j).ok()) {
            self.weather_st = st;
        }
        if overlay {
            self.sync_overlay();
        }
    }

    /// One thing as the host has it.
    fn mirror_thing(&mut self, f: ThingFull, you: u8) {
        let Some((ty, _)) = self.type_info(f.type_id) else {
            // Its type is on its way (the world is being made anew).
            self.mirror.pending.insert(f.id, f);
            return;
        };
        self.mirror.pending.remove(&f.id);
        self.unhold(f.id);
        let m = f.m;
        let mut t = Thing::new(f.id, &ty, Vec3::from_array(m.pos), m.yaw, f.scale, f.params, f.props, f.born);
        t.look = f.look;
        t.holder = f.holder.map(|c| from_host(c, you));
        t.co_holder = f.co_holder.map(|c| from_host(c, you));
        t.worn = f.worn.map(|c| from_host(c, you));
        t.anchored = f.anchored;
        t.point_first = f.point_first;
        t.origin = serde_json::from_str(&f.origin).unwrap_or_default();
        t.shape = serde_json::from_str::<Shape>(&f.shape).unwrap_or_default();
        t.dirty = false;
        apply_move(&mut t, &m);
        let holders: Vec<ActorId> = [t.holder, t.co_holder].into_iter().flatten().collect();
        self.things.insert(t);
        for h in holders {
            if let Some(a) = self.actor_mut(h) {
                a.held = Some(f.id);
            }
        }
    }

    /// Things that waited for their type (after the world is made anew).
    pub fn mirror_pending(&mut self) {
        let you = self.mirror.you;
        let waiting: Vec<ThingFull> = std::mem::take(&mut self.mirror.pending).into_values().collect();
        for f in waiting {
            self.mirror_thing(f, you);
        }
    }

    /// Whoever held a thing lets go of it (as the host's word replaces it).
    fn unhold(&mut self, id: i64) {
        let holders: Vec<ActorId> = self.things.get(id).map(|t| [t.holder, t.co_holder].into_iter().flatten().collect()).unwrap_or_default();
        for h in holders {
            if let Some(a) = self.actor_mut(h) {
                if a.held == Some(id) {
                    a.held = None;
                }
            }
        }
    }

    fn mirror_npc(&mut self, m: NpcMove) {
        let snap = self.snap.clone();
        let seed = self.seed;
        let Some(n) = self.cast.get_mut(m.id) else { return };
        // Coming into sight, or a jump (carried off, put back): at once.
        let far = (Vec3::from_array(m.body.pos) - n.a.pos).length() > 4.0;
        if n.away || far || !self.mirror.npc_to.contains_key(&m.id) {
            m.body.onto(&mut n.a);
        }
        self.mirror.npc_to.insert(m.id, m.body);
        n.away = m.away;
        n.dead = m.dead;
        n.a.held = m.held;
        if (n.growth - m.growth).abs() > 0.005 {
            n.growth = m.growth;
            super::npc::fit(n, &snap, seed);
        }
    }

    /// Beings move on toward where they were last heard to be.
    pub(super) fn ease_npcs(&mut self, dt: f32) {
        let k = (dt * 14.0).min(1.0);
        for (id, to) in &self.mirror.npc_to {
            if let Some(n) = self.cast.get_mut(*id) {
                let now = crate::net::wire::Body::of(&n.a, 0.0);
                now.lerp(to, k).onto(&mut n.a);
            }
        }
    }

    /// A guest's own lot as the host keeps it.
    pub fn take_own(&mut self, wounds: f32, fallen: bool, glow: f32) {
        self.night.wounds = wounds;
        self.night.glow = glow;
        match (fallen, self.night.fallen.is_some()) {
            (true, false) => {
                self.night.fallen = Some("struck down".into());
                self.night.fallen_at = self.t;
            }
            (false, true) => {
                self.night.fallen = None;
            }
            _ => {}
        }
    }

    /// The host's lot for one traveler: (wounds, fallen, glow).
    pub fn own_of(&self, no: u8) -> Option<(f32, bool, f32)> {
        self.travelers.get(&no).map(|t| (t.wounds, t.fallen.is_some(), t.glow))
    }
}

fn apply_move(t: &mut Thing, m: &ThingMove) {
    t.pos = Vec3::from_array(m.pos);
    t.vel = Vec3::from_array(m.vel);
    t.yaw = m.yaw;
    t.state = m.state;
    t.shape.tilt = (m.tilt != [0.0; 4]).then_some(m.tilt);
    t.asleep = m.asleep;
}
