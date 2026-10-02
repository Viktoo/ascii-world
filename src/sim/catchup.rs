//! Regions as the player moves: placed objects with behaviour or active
//! properties join the live layer when the player comes near, and (with
//! `far: catchup`) a region the player returns to runs the time it missed,
//! quickly, with rules only.

use super::config::FarMode;
use super::props::*;
use super::{Request, Sim};
use crate::world::{REGION, region_center, region_of};
use glam::Vec3;
use serde_json::json;

/// Coarse steps for catching up (game seconds).
const CATCHUP_STEP: f32 = 2.0;
const CATCHUP_MAX_STEPS: usize = 400;

impl Sim {
    pub fn step_regions(&mut self) {
        let t = self.t;
        let p = self.player.pos;
        let pr = region_of(p.x, p.z);
        let n = (self.cfg.medium / REGION).ceil() as i32;
        let mut entering = Vec::new();
        for dz in -n..=n {
            for dx in -n..=n {
                let r = (pr.0 + dx, pr.1 + dz);
                let c = region_center(r);
                if (Vec3::new(c.x - p.x, 0.0, c.z - p.z)).length() - REGION * 0.7 > self.cfg.medium {
                    continue;
                }
                if let Some(seen) = self.region_seen.get(&r) {
                    if t - seen > 30.0 {
                        entering.push((r, t - seen));
                    }
                }
                self.region_seen.insert(r, t);
            }
        }
        if let FarMode::CatchUp { max_hours } = self.cfg.far {
            let max_s = max_hours as f64 * crate::render::sky::DAY_SECONDS / 24.0;
            for (r, missed) in entering {
                self.catch_up(r, missed.min(max_s) as f32);
            }
        }
        self.promote_active_near();
        self.regrow();
        self.notice_fresh();
        self.expire_pending();
    }

    /// Living scatter that was eaten or burnt grows back after a while (small
    /// plants in a day, big ones in a few).
    pub fn regrow(&mut self) {
        let t = self.t;
        let day = crate::render::sky::DAY_SECONDS;
        let mut changed = false;
        let mut regrown = 0;
        let snap = self.snap.clone();
        let mut back = Vec::new();
        for (c, at) in &self.things.spent_cells {
            if t - at > day {
                back.push(*c);
            }
        }
        for c in back {
            let alive = self.cache.item_at(&snap, c).and_then(|it| snap.type_of(it.inst.info[0]).cloned()).is_some_and(|ty| self.type_props.get(&self.vocab, &ty)[P_ALIVE] > 0.0);
            if alive {
                self.things.spent_cells.remove(&c);
                if self.things.taken.get(&c).is_some_and(|id| *id < 0) {
                    self.things.taken.remove(&c);
                }
                changed = true;
                regrown += 1;
            }
        }
        let healed: Vec<(i32, i32)> = self
            .field
            .cells
            .iter()
            .filter(|(_, cell)| cell.spent_at.is_some_and(|at| t - at > day * if cell.small { 1.0 } else { 3.0 }))
            .filter(|(_, cell)| snap.type_of(cell.type_id).is_some_and(|ty| self.type_props.get(&self.vocab, ty)[P_ALIVE] > 0.0))
            .map(|(c, _)| *c)
            .collect();
        regrown += healed.len();
        for c in healed {
            self.field.cells.remove(&c);
            changed = true;
        }
        if changed {
            self.sync_overlay();
        }
        if regrown > 0 {
            self.event("regrown", None, None, format!("{regrown} plants grew back"), None, json!({ "count": regrown }));
        }
    }

    /// Placed objects near the player that act on their own join the live layer.
    fn promote_active_near(&mut self) {
        let p = self.player.pos;
        let near = self.cfg.near;
        let mut todo = Vec::new();
        for pl in &self.snap.instances {
            if self.things.by_instance.contains_key(&pl.id) || (pl.pos - p).length() > near {
                continue;
            }
            let Some(ty) = self.snap.type_of(pl.type_id).cloned() else { continue };
            let base = self.type_props.get(&self.vocab, &ty);
            let active = ty.ct.tick.is_some() || base[P_HEAT] > 0.0 || base[P_FIRE] > 0.0 || base[P_LIGHT] > 0.0 || (BUILTIN.len()..base.len()).any(|i| (base[i] - self.vocab.defaults[i]).abs() > 1e-4);
            if active {
                todo.push(pl.id);
            }
        }
        for i in todo {
            self.promote_instance(i);
        }
    }

    /// Run the time a region missed: rules in coarse steps, and its people
    /// living their day off-screen.
    pub fn catch_up(&mut self, r: (i32, i32), missed: f32) {
        if missed < 1.0 {
            return;
        }
        let steps = ((missed / CATCHUP_STEP) as usize).min(CATCHUP_MAX_STEPS);
        let dt = missed / steps.max(1) as f32;
        let before = self.log.total;
        let t0 = self.t;
        for _ in 0..steps {
            self.rules_pass(dt, Some(r));
        }
        // What happened there while nobody watched.
        let happened: Vec<String> = self.log.recent.iter().rev().take((self.log.total - before) as usize).filter(|e| matches!(e.kind.as_str(), "ignited" | "burnt_out" | "doused" | "grown" | "died")).map(|e| e.text.clone()).collect();
        let night = self.night();
        let ids: Vec<i64> = self.cast.npcs.iter().filter(|n| !n.dead).filter(|n| region_of(n.a.pos.x, n.a.pos.z) == r || region_of(n.def.home.x, n.def.home.z) == r).map(|n| n.def.id).collect();
        for cid in ids {
            let Some(n) = self.cast.get_mut(cid) else { continue };
            n.needs.hunger = (n.needs.hunger + missed / 900.0).min(1.0);
            n.needs.social = (n.needs.social + missed / 600.0).min(1.0);
            n.needs.fun = (n.needs.fun + missed / 700.0).min(1.0);
            n.plan.clear();
            n.a.task = None;
            let home = n.def.home;
            if night {
                n.a.pos = home;
                n.a.asleep = true;
            } else {
                let a = crate::noise::u2f(crate::noise::pcg(n.rng ^ (t0 as u32))) * std::f32::consts::TAU;
                let d = 3.0 + crate::noise::u2f(crate::noise::pcg(n.rng ^ 77)) * 12.0;
                let q = home + Vec3::new(a.cos() * d, 0.0, a.sin() * d);
                if self.snap.terrain.height(q.x, q.z) > crate::terrain::WATER_LEVEL + 0.3 {
                    n.a.pos = q;
                }
                n.a.asleep = false;
            }
            let h = self.snap.terrain.height(n.a.pos.x, n.a.pos.z);
            if let Some(n) = self.cast.get_mut(cid) {
                n.a.pos.y = h;
                n.last_sim = t0;
            }
            let text = if happened.is_empty() {
                "While the traveller was away, the days went on as usual.".to_string()
            } else {
                format!("While the traveller was away: {}.", happened.iter().take(4).cloned().collect::<Vec<_>>().join("; "))
            };
            self.out.push(Request::Witness { cid, text, importance: if happened.is_empty() { 0.2 } else { 0.5 } });
        }
        self.event("caught_up", None, Some(format!("region:{},{}", r.0, r.1)), format!("region ({}, {}) ran {:.0} missed seconds", r.0, r.1, missed), Some(region_center(r)), json!({ "steps": steps, "events": happened.len() }));
    }
}
