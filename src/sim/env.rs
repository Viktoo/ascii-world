//! The rules pass. A few times a second every live thing, every changed
//! scatter cell (a tuft of grass, a tree) and every placed object next to
//! something that is happening runs the rules. Effects are computed from the
//! state before the pass and applied together, so order never matters.
//! Crossing thresholds (catching fire, burning out, breaking, growing up)
//! become events that characters see and remember.

use super::props::*;
use super::props::Crossing;
use super::rules::{self, EntView, Rule, RuleSpec};
use super::things::{Origin, Thing, ThingId};
use super::{Note, Sim};
use crate::terrain::WATER_LEVEL;
use glam::Vec3;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// A scatter cell whose item changed (or is changing).
#[derive(Clone, Debug)]
pub struct Cell {
    pub props: Props,
    pub type_id: u32,
    pub pos: Vec3,
    /// Size of the item (bounding radius × scale).
    pub size: f32,
    /// Small items (grass, sticks) vanish when burnt out; big ones char.
    pub small: bool,
    pub water: bool,
    pub active: bool,
    pub fired: Vec<(String, f64)>,
    /// When it burnt out (regrows later if it was alive).
    pub spent_at: Option<f64>,
}

#[derive(Default)]
pub struct Field {
    pub cells: BTreeMap<(i32, i32), Cell>,
    /// Cells present at the last save, with what they were then (to write
    /// only the ones that changed, and delete the ones that healed).
    pub saved: std::collections::HashMap<(i32, i32), u64>,
}

impl Cell {
    /// A fingerprint of what a save writes for this cell.
    pub fn save_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (i, p) in self.props.iter().enumerate() {
            // A cooling cell's exact heat isn't worth a write every save.
            let p = if i == P_TEMP { (p / 25.0).round() } else { *p };
            p.to_bits().hash(&mut h);
        }
        self.active.hash(&mut h);
        h.finish()
    }
}

impl Field {
    pub fn active(&self) -> usize {
        self.cells.values().filter(|c| c.active).count()
    }
}

/// Something burning, for flames, light and minds.
#[derive(Clone, Copy, Debug)]
pub struct Burning {
    pub pos: Vec3,
    pub size: f32,
    pub fire: f32,
    pub seed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Key {
    Thing(ThingId),
    Cell(i32, i32),
    Inst(i64),
}

impl Key {
    fn subject(self) -> String {
        match self {
            Key::Thing(id) => format!("thing:{id}"),
            Key::Cell(x, z) => format!("cell:{x},{z}"),
            Key::Inst(i) => format!("instance:{i}"),
        }
    }
}

struct Ent {
    key: Key,
    pos: Vec3,
    props: Props,
    next: Props,
    water: f32,
    held: f32,
    ground: f32,
    /// Runs its own rules this pass (otherwise only a neighbour).
    acting: bool,
    name: String,
    size: f32,
    fired: Vec<String>,
}

impl Sim {
    /// Built-in rules plus the universe's own (from the `rules` table).
    pub fn load_universe_rules(&mut self) {
        let vocab = super::persist::world_vocab(&self.db);
        let mut all = rules::builtin_rules(&vocab);
        let specs = super::persist::universe_rules(&self.db);
        let mut kept = Vec::new();
        for s in specs.into_iter().take(rules::MAX_UNIVERSE_RULES) {
            match rules::compile(&s, &vocab, false) {
                Ok(r) => {
                    all.push(r);
                    kept.push(s);
                }
                Err(e) => crate::log::error(format!("universe rule skipped: {e}")),
            }
        }
        self.watch = Arc::new(watches(&vocab));
        self.vocab = Arc::new(vocab);
        self.rules = Arc::new(all);
        self.universe_rules = kept;
        self.type_props.clear();
        // Resize every live property vector to the vocabulary.
        let n = self.vocab.len();
        let defaults = self.vocab.defaults.clone();
        for t in self.things.map.values_mut() {
            while t.props.len() < n {
                t.props.push(defaults[t.props.len()]);
            }
        }
        for c in self.field.cells.values_mut() {
            while c.props.len() < n {
                c.props.push(defaults[c.props.len()]);
            }
        }
    }

    /// Everything currently burning (for flames, light and fear).
    pub fn burning(&self) -> Vec<Burning> {
        let mut out = Vec::new();
        for t in self.things.live() {
            if t.props[P_FIRE] > 0.0 {
                let r = self.snap.type_of(t.type_id).map(|ty| ty.ct.meta.bounds[0].max(ty.ct.meta.bounds[2]) * t.scale).unwrap_or(0.5);
                out.push(Burning { pos: t.pos, size: r.clamp(0.2, 4.0), fire: t.props[P_FIRE], seed: t.id as f32 });
            }
        }
        for ((x, z), c) in &self.field.cells {
            if c.props[P_FIRE] > 0.0 {
                out.push(Burning { pos: c.pos, size: c.size.clamp(0.2, 4.0), fire: c.props[P_FIRE], seed: (*x * 31 + *z) as f32 });
            }
        }
        out
    }

    /// "thing:3", "cell:4,5", … (as events and incidents name parts).
    pub fn part_key(t: &super::Target) -> String {
        match t {
            super::Target::Thing(id) => format!("thing:{id}"),
            super::Target::Cell(c) => format!("cell:{},{}", c[0], c[1]),
            super::Target::Instance(i) => format!("instance:{i}"),
            super::Target::Actor(a) => a.key(),
            _ => String::new(),
        }
    }

    /// A part's properties, where it is and its name: a live thing, or a
    /// scatter cell (changed or as it grew).
    pub fn part_props(&mut self, t: &super::Target) -> Option<(Props, Vec3, String)> {
        match t {
            super::Target::Thing(id) => self.things.get(*id).map(|th| (th.props.clone(), th.pos, self.thing_name(th.id))),
            super::Target::Instance(i) => {
                let id = self.promote_instance(*i)?;
                self.part_props(&super::Target::Thing(id))
            }
            super::Target::Cell(c) => {
                let cell = (c[0], c[1]);
                if self.things.taken.contains_key(&cell) {
                    return None;
                }
                let snap = self.snap.clone();
                if let Some(x) = self.field.cells.get(&cell) {
                    let name = snap.type_of(x.type_id).map(|t| t.name().to_string()).unwrap_or_default();
                    return Some((x.props.clone(), x.pos, name));
                }
                let it = self.cache.item_at(&snap, cell)?;
                let ty = snap.type_of(it.inst.info[0])?;
                Some((scaled((*self.type_props.get(&self.vocab, ty)).clone(), it.inst.pos_scale[3]), it.inst.pos(), ty.name().to_string()))
            }
            _ => None,
        }
    }

    /// Write a part's properties back (the rules carry on from there).
    pub fn set_part_props(&mut self, t: &super::Target, p: Props) {
        match t {
            super::Target::Thing(id) => {
                if let Some(th) = self.things.get_mut(*id) {
                    th.props = p;
                    th.dirty = true;
                    th.asleep = false;
                }
            }
            super::Target::Instance(i) => {
                if let Some(id) = self.promote_instance(*i) {
                    self.set_part_props(&super::Target::Thing(id), p);
                }
            }
            super::Target::Cell(c) => {
                let cell = (c[0], c[1]);
                if let Some(x) = self.field.cells.get_mut(&cell) {
                    x.props = p;
                    x.active = true;
                } else {
                    let snap = self.snap.clone();
                    let Some(it) = self.cache.item_at(&snap, cell) else { return };
                    let size = snap.type_of(it.inst.info[0]).map(|t| t.radius() * it.inst.pos_scale[3]).unwrap_or(0.5);
                    let pos = it.inst.pos();
                    let water = snap.terrain.height(pos.x, pos.z) < WATER_LEVEL;
                    self.field.cells.insert(cell, Cell { props: p, type_id: it.inst.info[0], pos, size, small: size < 0.9, water, active: true, fired: Vec::new(), spent_at: None });
                }
                self.sync_overlay();
            }
            _ => {}
        }
    }

    /// Parts within `r` of a point, as targets: live things and scatter
    /// items (changed or not), nearest first.
    pub fn parts_near(&mut self, p: Vec3, r: f32) -> Vec<(super::Target, Vec3)> {
        let mut v: Vec<(super::Target, Vec3)> = self.things.live().filter(|t| !t.held() && t.worn.is_none() && (t.pos - p).length() <= r).map(|t| (super::Target::Thing(t.id), t.pos)).collect();
        let snap = self.snap.clone();
        for it in self.cache.items_near(&snap, p, r) {
            if self.things.taken.contains_key(&it.cell) {
                continue;
            }
            let at = self.field.cells.get(&it.cell).map(|c| c.pos).unwrap_or_else(|| it.inst.pos());
            if (at - p).length() <= r {
                v.push((super::Target::Cell([it.cell.0, it.cell.1]), at));
            }
        }
        v.sort_by(|a, b| (a.1 - p).length().total_cmp(&(b.1 - p).length()));
        v
    }

    /// What `secs` of working `tool` against `part` would do, by the world's
    /// rules (both ways, plus each one's own rules meanwhile). Nothing is
    /// changed: this is how anyone imagines a deed before doing it.
    pub fn foresee(&self, tool: &Props, part: &Props, secs: f32) -> (Props, Props) {
        let rules = self.rules.clone();
        let (hour, night) = (self.hour(), self.night() as u32 as f32);
        let (mut a, mut b) = (tool.clone(), part.clone());
        let dt = 0.25;
        let mut left = secs.clamp(0.0, 10.0);
        while left > 1e-3 {
            let d = left.min(dt);
            left -= d;
            let va = rules::EntView { props: &a, water: 0.0, held: 1.0, ground: 0.0 };
            let vb = rules::EntView { props: &b, water: 0.0, held: 0.0, ground: 1.0 };
            let (mut na, mut nb) = (a.clone(), b.clone());
            let mut fired = Vec::new();
            rules::run_single(&rules, &vb, &mut nb, d, hour, night, &mut fired);
            for r in rules.iter().filter(|r| r.near.is_some()) {
                if rules::pair_self_ok(r, &va, d, hour, night) {
                    rules::run_pair(r, &va, &vb, 0.1, d, hour, night, &mut na, &mut nb);
                }
                if rules::pair_self_ok(r, &vb, d, hour, night) {
                    rules::run_pair(r, &vb, &va, 0.1, d, hour, night, &mut nb, &mut na);
                }
            }
            sanitize(&mut na);
            sanitize(&mut nb);
            a = na;
            b = nb;
        }
        (a, b)
    }

    /// One rules pass over everything within the medium range.
    pub fn step_rules(&mut self, dt: f32) {
        self.rules_pass(dt, None);
    }

    /// A rules pass; with `region`, only things in that region act
    /// (catching up a region the player returns to).
    pub fn rules_pass(&mut self, dt: f32, region: Option<(i32, i32)>) {
        let rules = self.rules.clone();
        if rules.is_empty() {
            return;
        }
        let max_near = rules.iter().filter_map(|r| r.near).fold(0.0f32, f32::max);
        let range = self.cfg.medium;
        let player = self.player.pos;
        let in_range = |p: Vec3| match region {
            Some(r) => crate::world::region_of(p.x, p.z) == r,
            None => {
                let d = p - player;
                d.x * d.x + d.z * d.z <= range * range
            }
        };
        let snap = self.snap.clone();
        let mut ents: Vec<Ent> = Vec::new();
        let mut index: HashMap<Key, usize> = HashMap::new();
        // Acting entities: live things and active cells.
        for t in self.things.live() {
            if !in_range(t.pos) {
                continue;
            }
            let Some(ty) = snap.type_of(t.type_id) else { continue };
            let (c, _) = t.proxy(ty);
            let key = Key::Thing(t.id);
            index.insert(key, ents.len());
            ents.push(Ent {
                key,
                pos: c,
                props: t.props.clone(),
                next: t.props.clone(),
                water: (c.y < WATER_LEVEL) as u32 as f32,
                held: t.held() as u32 as f32,
                ground: (t.asleep && !t.held()) as u32 as f32,
                acting: true,
                name: ty.name().to_string(),
                size: (ty.radius() * t.scale).max(0.1),
                fired: Vec::new(),
            });
        }
        for (k, c) in &self.field.cells {
            if !c.active || !in_range(c.pos) {
                continue;
            }
            let key = Key::Cell(k.0, k.1);
            index.insert(key, ents.len());
            ents.push(Ent {
                key,
                pos: c.pos,
                props: c.props.clone(),
                next: c.props.clone(),
                water: c.water as u32 as f32,
                held: 0.0,
                ground: 1.0,
                acting: true,
                name: snap.type_of(c.type_id).map(|t| t.name().to_string()).unwrap_or_default(),
                size: c.size,
                fired: Vec::new(),
            });
        }
        if ents.is_empty() {
            return;
        }
        let acting = ents.len();
        let hour = self.hour();
        let night = self.night() as u32 as f32;
        // Neighbours: other entities, untouched scatter cells and placed objects.
        if max_near > 0.0 {
            let mut extra: Vec<Ent> = Vec::new();
            let mut extra_keys: std::collections::HashSet<Key> = std::collections::HashSet::new();
            for i in 0..acting {
                let e = &ents[i];
                // Only look around if some pair rule could fire for it.
                let v = EntView { props: &e.props, water: e.water, held: e.held, ground: e.ground };
                let r = rules.iter().filter(|r| r.near.is_some() && rules::pair_self_ok(r, &v, dt, hour, night)).filter_map(|r| r.near).fold(0.0f32, f32::max);
                if r <= 0.0 {
                    continue;
                }
                let p = e.pos;
                for it in self.cache.items_near(&snap, p, r + 2.0) {
                    let key = Key::Cell(it.cell.0, it.cell.1);
                    if index.contains_key(&key) || extra_keys.contains(&key) || self.things.taken.contains_key(&it.cell) {
                        continue;
                    }
                    let Some(ty) = snap.type_of(it.inst.info[0]) else { continue };
                    let props = match self.field.cells.get(&it.cell) {
                        Some(c) => c.props.clone(),
                        None => scaled((*self.type_props.get(&self.vocab, ty)).clone(), it.inst.pos_scale[3]),
                    };
                    let pos = it.inst.pos();
                    extra_keys.insert(key);
                    extra.push(Ent {
                        key,
                        pos,
                        next: props.clone(),
                        props,
                        water: (snap.terrain.height(pos.x, pos.z) < WATER_LEVEL) as u32 as f32,
                        held: 0.0,
                        ground: 1.0,
                        acting: false,
                        name: ty.name().to_string(),
                        size: (ty.radius() * it.inst.pos_scale[3]).max(0.1),
                        fired: Vec::new(),
                    });
                }
                for pl in snap.near_chunk(crate::world::chunk_of(p.x, p.z)) {
                    if self.things.by_instance.contains_key(&pl.id) || (pl.pos - p).length() > r + 30.0 {
                        continue;
                    }
                    let key = Key::Inst(pl.id);
                    if extra_keys.contains(&key) {
                        continue;
                    }
                    let Some(ty) = snap.type_of(pl.type_id) else { continue };
                    let props = scaled((*self.type_props.get(&self.vocab, ty)).clone(), pl.scale);
                    extra_keys.insert(key);
                    extra.push(Ent {
                        key,
                        pos: pl.pos + Vec3::Y * ty.sphere_cy.min(2.0) * pl.scale,
                        next: props.clone(),
                        props,
                        water: (pl.pos.y < WATER_LEVEL) as u32 as f32,
                        held: 0.0,
                        ground: 1.0,
                        acting: false,
                        name: ty.name().to_string(),
                        size: ty.radius() * pl.scale,
                        fired: Vec::new(),
                    });
                }
            }
            extra.sort_by_key(|e| e.key);
            for e in extra {
                index.insert(e.key, ents.len());
                ents.push(e);
            }
        }
        // Spatial buckets for neighbour queries.
        let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let bcell = |p: Vec3| ((p.x / 8.0).floor() as i32, (p.z / 8.0).floor() as i32);
        for (i, e) in ents.iter().enumerate() {
            buckets.entry(bcell(e.pos)).or_default().push(i);
        }
        // Single rules.
        for e in ents.iter_mut().take(acting) {
            let v = EntView { props: &e.props, water: e.water, held: e.held, ground: e.ground };
            let mut fired = Vec::new();
            rules::run_single(&rules, &v, &mut e.next, dt, hour, night, &mut fired);
            e.fired.extend(fired.into_iter().map(str::to_string));
        }
        // Pair rules. Big things reach further (their surface, not their centre).
        // What spread to each entity, and from where (the hottest source):
        // a crossing it makes is part of what that source started.
        let mut from: Vec<Option<usize>> = vec![None; ents.len()];
        for i in 0..acting {
            for r in rules.iter().filter(|r| r.near.is_some()) {
                let near = r.near.unwrap_or(0.0);
                let (ok, p, size_i) = {
                    let e = &ents[i];
                    let v = EntView { props: &e.props, water: e.water, held: e.held, ground: e.ground };
                    (rules::pair_self_ok(r, &v, dt, hour, night), e.pos, e.size)
                };
                if !ok {
                    continue;
                }
                let reach = near + size_i.min(4.0);
                let n = (reach / 8.0).ceil() as i32 + 1;
                let c = bcell(p);
                let mut cands: Vec<usize> = Vec::new();
                for dz in -n..=n {
                    for dx in -n..=n {
                        if let Some(v) = buckets.get(&(c.0 + dx, c.1 + dz)) {
                            cands.extend(v.iter().copied());
                        }
                    }
                }
                cands.sort_unstable();
                for j in cands {
                    if j == i {
                        continue;
                    }
                    let gap = ((ents[j].pos - p).length() - (size_i.min(4.0) + ents[j].size.min(4.0)) * 0.5).max(0.05);
                    if gap > near {
                        continue;
                    }
                    let mut other_next = std::mem::take(&mut ents[j].next);
                    let fired = {
                        let (ei, ej) = (&ents[i], &ents[j]);
                        let vi = EntView { props: &ei.props, water: ei.water, held: ei.held, ground: ei.ground };
                        let vj = EntView { props: &ej.props, water: ej.water, held: ej.held, ground: ej.ground };
                        let mut my_next = ents[i].next.clone();
                        let f = rules::run_pair(r, &vi, &vj, gap, dt, hour, night, &mut my_next, &mut other_next);
                        if f {
                            ents[i].next = my_next;
                        }
                        f
                    };
                    ents[j].next = other_next;
                    if fired {
                        ents[i].fired.push(r.spec.name.clone());
                        if r.effects.iter().any(|e| e.other) && from[j].is_none_or(|k| ents[k].props[P_TEMP] < ents[i].props[P_TEMP]) {
                            from[j] = Some(i);
                        }
                    }
                }
            }
        }
        // Apply.
        let mut overlay_changed = false;
        let mut events: Vec<(Key, super::incident::Crossed, Vec3, String, Option<super::incident::Cause>)> = Vec::new();
        for e in ents.iter_mut() {
            sanitize(&mut e.next);
            self.vocab.clamp(&mut e.next);
        }
        let watch = self.watch.clone();
        let crosses = |e: &Ent| !phenomena(&watch, &e.props, &e.next, &e.name).is_empty();
        let causes: Vec<Option<super::incident::Cause>> = (0..ents.len()).map(|ix| from[ix].filter(|_| crosses(&ents[ix])).map(|k| self.cause_named(&ents[k].key.subject(), format!("the {}", ents[k].name)))).collect();
        for (ix, e) in ents.iter_mut().enumerate() {
            let changed = e.next.iter().zip(&e.props).any(|(a, b)| (a - b).abs() > 1e-4);
            if !changed && !e.acting {
                continue;
            }
            for (c, text) in phenomena(&watch, &e.props, &e.next, &e.name) {
                events.push((e.key, c, e.pos, text, causes[ix].clone()));
            }
            let t = self.t;
            match e.key {
                Key::Thing(id) => {
                    if let Some(th) = self.things.get_mut(id) {
                        if changed {
                            th.props = e.next.clone();
                            th.dirty = true;
                        }
                        for f in &e.fired {
                            th.note_rule(f, t);
                        }
                    }
                }
                Key::Cell(x, z) => {
                    // Nothing changing and nothing spreading from it: it can rest.
                    let at_rest = !changed && self.vocab.spreading().all(|i| e.next[i] <= self.vocab.meta[i].at);
                    let base_like = {
                        let snap_ty = snap.type_of(self.field.cells.get(&(x, z)).map(|c| c.type_id).unwrap_or(u32::MAX)).cloned();
                        match snap_ty {
                            Some(ty) => {
                                let b = self.type_props.get(&self.vocab, &ty);
                                e.next.iter().zip(b.iter()).enumerate().all(|(i, (a, b))| i == P_MASS || (a - b).abs() < 0.5)
                            }
                            None => false,
                        }
                    };
                    let entry = self.field.cells.entry((x, z));
                    match entry {
                        std::collections::btree_map::Entry::Occupied(mut o) => {
                            let c = o.get_mut();
                            let fx_before = cell_look(&c.props);
                            if c.spent_at.is_none() && e.props[P_FIRE] > 0.0 && e.next[P_FIRE] <= 0.0 && e.next[P_FUEL] <= 0.0 {
                                c.spent_at = Some(t);
                            }
                            c.props = e.next.clone();
                            c.active = !at_rest;
                            for f in &e.fired {
                                note(&mut c.fired, f, t);
                            }
                            if cell_look(&c.props) != fx_before || (c.small && c.props[P_FUEL] <= 0.0) {
                                overlay_changed = true;
                            }
                            if at_rest && base_like {
                                o.remove();
                                overlay_changed = true;
                            }
                        }
                        std::collections::btree_map::Entry::Vacant(v) => {
                            if changed {
                                let snap_ref = &snap;
                                let item = self.cache.item_at(snap_ref, (x, z));
                                if let Some(it) = item {
                                    let ty = snap.type_of(it.inst.info[0]);
                                    let size = ty.map(|t| t.radius() * it.inst.pos_scale[3]).unwrap_or(0.5);
                                    v.insert(Cell {
                                        props: e.next.clone(),
                                        type_id: it.inst.info[0],
                                        pos: it.inst.pos(),
                                        size,
                                        small: size < 0.9,
                                        water: e.water > 0.0,
                                        active: true,
                                        fired: Vec::new(),
                                        spent_at: None,
                                    });
                                    overlay_changed = true;
                                }
                            }
                        }
                    }
                }
                Key::Inst(i) => {
                    if changed {
                        if let Some(id) = self.promote_instance(i) {
                            // It is a live thing now: what it does is told as that.
                            for ev in events.iter_mut().filter(|ev| ev.0 == e.key) {
                                ev.0 = Key::Thing(id);
                            }
                            if let Some(th) = self.things.get_mut(id) {
                                th.props = e.next.clone();
                                th.dirty = true;
                            }
                        }
                    }
                }
            }
        }
        if overlay_changed {
            self.sync_overlay();
        }
        for (key, c, pos, text, cause) in events {
            self.phenomenon(key.subject(), &c, pos, &text, cause);
        }
    }

    /// Tell the crossings a change made (outside the rules pass: a deed, a
    /// tool worked against something), caused by `by`.
    pub fn tell_change(&mut self, part: &super::Target, name: &str, pos: Vec3, before: &Props, after: &Props, by: Option<super::ActorId>) {
        let watch = self.watch.clone();
        let subject = Self::part_key(part);
        let cause = by.map(|a| super::incident::Cause { subject: a.key(), name: String::new(), by: Some(self.actor_name(a)) });
        for (c, text) in phenomena(&watch, before, after, name) {
            self.phenomenon(subject.clone(), &c, pos, &text, cause.clone());
        }
    }

    fn phenomenon(&mut self, subject: String, c: &super::incident::Crossed, pos: Vec3, text: &str, cause: Option<super::incident::Cause>) {
        let kind = c.kind.as_str();
        // Part of something bigger (a fire, a spreading curse): counted on
        // its incident, told as one story.
        if let Some(j) = self.file_incident(&subject, c, pos, cause) {
            let (id, part) = match j {
                super::incident::Joined::Began(id) => (id, false),
                super::incident::Joined::Part(id) => (id, true),
            };
            self.event(kind, None, Some(subject), text, Some(pos), json!({ "incident": id, "part": part }));
            self.incident_crossing(j, c, pos);
            return;
        }
        let first = self.log.recent.iter().rev().take(60).filter(|e| e.kind == kind && e.pos.is_some_and(|p| (Vec3::from(p) - pos).length() < 12.0) && self.t - e.t < 20.0).count() == 0;
        self.event(kind, None, Some(subject), text, Some(pos), json!({}));
        // People notice the first of a kind nearby, not every tuft.
        if first {
            self.note_near(pos, 40.0, Note::Ambient(format!("{}.", super::physics::cap(text))));
            self.witness(pos, 45.0, &format!("{}.", super::physics::cap(text)), 0.3, &[]);
        }
    }

    /// Spawn a live thing of a type at a point (on the ground unless `y` is given).
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_thing(&mut self, type_id: u32, at: Vec3, yaw: f32, scale: f32, origin: Origin, on_ground: bool) -> Option<ThingId> {
        if self.things.len() >= self.cfg.max_things {
            return None;
        }
        let (ty, base) = self.type_info(type_id)?;
        let id = self.things.alloc();
        let seed = (self.rand() * 97.0).floor();
        let params = [seed, scale, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5];
        let mut th = Thing::new(id, &ty, at, yaw, scale, params, scaled((*base).clone(), scale), self.t);
        if on_ground {
            let g = self.snap.terrain.height(at.x, at.z);
            th.pos.y = th.rest_y(&ty, g);
        }
        th.origin = origin;
        th.asleep = on_ground;
        self.things.insert(th);
        Some(id)
    }
}

fn note(v: &mut Vec<(String, f64)>, name: &str, t: f64) {
    if let Some(last) = v.last_mut() {
        if last.0 == name {
            last.1 = t;
            return;
        }
    }
    v.push((name.to_string(), t));
    if v.len() > 8 {
        v.remove(0);
    }
}

fn cell_look(p: &Props) -> [i32; 2] {
    [(p[P_CHAR] * 20.0) as i32, (p[P_WET] * 10.0) as i32]
}

/// What to watch for on one property: its crossings, compiled.
pub struct Watch {
    i: usize,
    at: f32,
    rises: Vec<(Crossing, Option<rules::RExpr>)>,
    falls: Vec<(Crossing, Option<rules::RExpr>)>,
    quiet: Option<rules::RExpr>,
}

/// The crossings every property's metadata asks to be told about.
pub fn watches(vocab: &Vocab) -> Vec<Watch> {
    let cond = |src: &Option<String>| -> Result<Option<rules::RExpr>, String> {
        match src {
            Some(s) if !s.trim().is_empty() => rules::parse_expr(s, vocab, false).map(Some),
            _ => Ok(None),
        }
    };
    let mut out = Vec::new();
    for (i, m) in vocab.meta.iter().enumerate() {
        if m.rises.is_empty() && m.falls.is_empty() {
            continue;
        }
        let compile = |v: &[Crossing]| v.iter().filter_map(|c| cond(&c.when).map_err(|e| crate::log::error(format!("{}: {}: {e}", vocab.names[i], c.kind))).ok().map(|w| (c.clone(), w))).collect::<Vec<_>>();
        let quiet = cond(&m.quiet).unwrap_or_else(|e| {
            crate::log::error(format!("{}: quiet: {e}", vocab.names[i]));
            None
        });
        out.push(Watch { i, at: m.at, rises: compile(&m.rises), falls: compile(&m.falls), quiet });
    }
    out
}

/// Threshold crossings worth telling about, as the vocabulary describes them.
fn phenomena(watch: &[Watch], before: &Props, after: &Props, name: &str) -> Vec<(super::incident::Crossed, String)> {
    let mut v = Vec::new();
    let n = if name.is_empty() { "something".to_string() } else { format!("the {name}") };
    let env = rules::Env { me: after, other: &[], dt: 0.0, dist: 0.0, hour: 12.0, night: 0.0, water: 0.0, held: 0.0, ground: 1.0 };
    let holds = |e: &Option<rules::RExpr>| e.as_ref().is_none_or(|e| e.eval(&env) != 0.0);
    for w in watch {
        let (Some(b), Some(a)) = (before.get(w.i), after.get(w.i)) else { continue };
        let (list, rose) = if *b <= w.at && *a > w.at {
            (&w.rises, true)
        } else if *b > w.at && *a <= w.at {
            (&w.falls, false)
        } else {
            continue;
        };
        if w.quiet.as_ref().is_some_and(|q| q.eval(&env) != 0.0) {
            continue;
        }
        if let Some((c, _)) = list.iter().find(|(_, when)| holds(when)) {
            v.push((super::incident::Crossed { kind: c.kind.clone(), prop: w.i, rose, saved: c.saved }, format!("{n} {}", c.text)));
        }
    }
    v
}

/// Run a set of universe rules in a small test scene and reject ones that
/// blow up (non-finite values) or spread to everything at once.
pub fn probe_rules(specs: &[RuleSpec], vocab: &Vocab) -> Result<Vec<Rule>, String> {
    let mut compiled = Vec::new();
    for s in specs {
        compiled.push(rules::compile(s, vocab, false)?);
    }
    let mut all = rules::builtin_rules(vocab);
    all.extend(compiled.iter().cloned());
    // 7×7 grid, 2.5 m apart; the middle one has every universe property at 1.
    let n = 7;
    let mut props: Vec<Props> = Vec::new();
    let mut pos = Vec::new();
    for z in 0..n {
        for x in 0..n {
            let mut p = vocab.defaults.clone();
            p[P_BURNS] = if (x + z) % 3 == 0 { 0.5 } else { 0.0 };
            if x == n / 2 && z == n / 2 {
                for i in 0..p.len() {
                    if !vocab.is_builtin(i) {
                        p[i] = 1.0;
                    }
                }
            }
            props.push(p);
            pos.push(Vec3::new(x as f32 * 2.5, 0.0, z as f32 * 2.5));
        }
    }
    let dt = 0.25;
    for step in 0..80 {
        let mut next = props.clone();
        for i in 0..props.len() {
            let v = EntView { props: &props[i], water: 0.0, held: 0.0, ground: 1.0 };
            let mut fired = Vec::new();
            rules::run_single(&all, &v, &mut next[i], dt, 12.0, 0.0, &mut fired);
            for r in all.iter().filter(|r| r.near.is_some()) {
                if !rules::pair_self_ok(r, &v, dt, 12.0, 0.0) {
                    continue;
                }
                for j in 0..props.len() {
                    let d = (pos[i] - pos[j]).length();
                    if i == j || d > r.near.unwrap_or(0.0) {
                        continue;
                    }
                    let vj = EntView { props: &props[j], water: 0.0, held: 0.0, ground: 1.0 };
                    let mut mine = next[i].clone();
                    let mut theirs = next[j].clone();
                    if rules::run_pair(r, &v, &vj, d, dt, 12.0, 0.0, &mut mine, &mut theirs) {
                        next[i] = mine;
                        next[j] = theirs;
                    }
                }
            }
        }
        for (k, p) in next.iter().enumerate() {
            if let Some(i) = p.iter().position(|v| !v.is_finite() || v.abs() > 1e6) {
                return Err(format!("the rules make {} run away to {} in a test scene", vocab.names[i], p[i]));
            }
            let _ = k;
        }
        props = next;
        for p in props.iter_mut() {
            sanitize(p);
        }
        // Spreading to every thing within 5 seconds is too fast to be a world.
        if step == 20 {
            for i in 0..vocab.len() {
                if vocab.is_builtin(i) {
                    continue;
                }
                let touched = props.iter().filter(|p| (p[i] - vocab.defaults[i]).abs() > 0.05).count();
                if touched == props.len() {
                    return Err(format!("'{}' spreads to everything within five seconds; slow it down (smaller effects or a shorter \"near\")", vocab.names[i]));
                }
            }
        }
    }
    Ok(compiled)
}
