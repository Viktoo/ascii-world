//! Character behaviour: a local state machine on the main thread's sim tick.
//! No LLM per tick; optional intents arrive as `Decision`s from the decider.

use super::collide::{NPC_RADIUS, Obstacles, move_body};
use super::{CharacterDef, Solid, WorldSnapshot};
use crate::noise::{pcg, u2f};
use crate::render::GpuInst;
use crate::terrain::{Terrain, WATER_LEVEL};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub const WALK_SPEED: f32 = 1.3;
pub const TALK_RANGE: f32 = 4.0;
/// Only characters this close to the player are simulated.
const SIM_RANGE: f32 = 220.0;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SavedState {
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    #[serde(default)]
    pub asleep: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Idle { until: f64 },
    Walk { target: Vec3 },
    GoHome,
    Sleep,
    Talk,
    Approach { line: Option<String> },
    Watch { until: f64 },
}

/// An intent from the decider.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Decision {
    pub action: String,
    #[serde(default)]
    pub line: Option<String>,
}

pub struct Npc {
    pub def: Arc<CharacterDef>,
    pub pos: Vec3,
    pub yaw: f32,
    pub mode: Mode,
    pub phase: f32,
    rng: u32,
}

impl Npc {
    fn rand(&mut self) -> f32 {
        self.rng = pcg(self.rng);
        u2f(self.rng)
    }

    pub fn name(&self) -> &str {
        &self.def.persona.name
    }

    pub fn saved(&self) -> SavedState {
        SavedState { x: self.pos.x, z: self.pos.z, yaw: self.yaw, asleep: self.mode == Mode::Sleep }
    }

    pub fn gpu(&self, figure: &super::TypeEntry) -> GpuInst {
        let l = self.def.persona.look;
        // The figure faces local +z; rotating by `yaw` makes it face the same way as a camera with that yaw.
        let rot = self.yaw;
        GpuInst {
            pos_scale: [self.pos.x, self.pos.y, self.pos.z, 1.0],
            rot: [rot.cos(), rot.sin(), figure.sphere_r * l.height.max(1.75) / 1.75, 1.0],
            k0: [self.def.id as f32 % 97.0, 1.0, l.height, l.build],
            k1: [l.skin, l.shirt_hue, l.trousers_hue, self.phase],
            info: figure.gpu_info(),
        }
    }
}

#[derive(Default)]
pub struct Cast {
    pub npcs: Vec<Npc>,
    index: HashMap<i64, usize>,
}

pub enum CastEvent {
    /// A character walked up to the player and speaks.
    Says { id: i64, line: String },
    /// The player came close for the first time in a while.
    PlayerNear { id: i64 },
}

impl Cast {
    /// Add characters that appeared in a new snapshot.
    pub fn sync(&mut self, snap: &WorldSnapshot) {
        for def in &snap.characters {
            if self.index.contains_key(&def.id) {
                continue;
            }
            let s = &def.state;
            let pos = if s.x == 0.0 && s.z == 0.0 { def.home } else { Vec3::new(s.x, 0.0, s.z) };
            let pos = Vec3::new(pos.x, snap.terrain.height(pos.x, pos.z), pos.z);
            let mode = if s.asleep { Mode::Sleep } else { Mode::Idle { until: 0.0 } };
            self.index.insert(def.id, self.npcs.len());
            self.npcs.push(Npc { def: def.clone(), pos, yaw: s.yaw, mode, phase: 0.0, rng: pcg(def.id as u32 ^ 0xC0FFEE) });
        }
    }

    pub fn get(&self, id: i64) -> Option<&Npc> {
        self.index.get(&id).map(|i| &self.npcs[*i])
    }

    pub fn get_mut(&mut self, id: i64) -> Option<&mut Npc> {
        self.index.get(&id).copied().map(move |i| &mut self.npcs[i])
    }

    pub fn instances(&self, figure: Option<&Arc<super::TypeEntry>>, player: Vec3) -> Vec<GpuInst> {
        let Some(f) = figure else { return Vec::new() };
        self.npcs.iter().filter(|n| (n.pos - player).length() < 200.0).map(|n| n.gpu(f)).collect()
    }

    pub fn apply(&mut self, id: i64, d: &Decision, now: f64) {
        let Some(n) = self.get_mut(id) else { return };
        if n.mode == Mode::Talk {
            return;
        }
        n.mode = match d.action.as_str() {
            "approach" | "approach_player" | "greet" => Mode::Approach { line: d.line.clone() },
            "go_home" | "home" => Mode::GoHome,
            "sleep" => Mode::GoHome,
            "watch" | "look" => Mode::Watch { until: now + 8.0 },
            _ => Mode::Idle { until: now + 2.0 },
        };
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        dt: f32,
        now: f64,
        night: bool,
        player: Vec3,
        talking_to: Option<i64>,
        terrain: &Terrain,
        solids_for: &mut dyn FnMut(Vec3) -> Vec<Solid>,
        events: &mut Vec<CastEvent>,
        near_flags: &mut HashMap<i64, f64>,
    ) {
        let positions: Vec<Vec3> = self.npcs.iter().map(|n| n.pos).collect();
        for (idx, n) in self.npcs.iter_mut().enumerate() {
            let to_player = player - n.pos;
            let dp = Vec3::new(to_player.x, 0.0, to_player.z).length();
            if dp > SIM_RANGE {
                continue;
            }
            if Some(n.def.id) == talking_to {
                n.mode = Mode::Talk;
            } else if n.mode == Mode::Talk {
                n.mode = Mode::Idle { until: now + 4.0 };
            }
            if dp < 9.0 {
                let last = near_flags.get(&n.def.id).copied().unwrap_or(f64::MIN);
                if now - last > 240.0 {
                    near_flags.insert(n.def.id, now);
                    events.push(CastEvent::PlayerNear { id: n.def.id });
                }
            }
            if night && !matches!(n.mode, Mode::Sleep | Mode::GoHome | Mode::Talk | Mode::Approach { .. }) {
                n.mode = Mode::GoHome;
            }
            if !night && n.mode == Mode::Sleep {
                n.mode = Mode::Idle { until: now + 1.0 + n.rand() as f64 * 4.0 };
            }
            let mut target: Option<(Vec3, f32)> = None;
            match n.mode.clone() {
                Mode::Talk => face(&mut n.yaw, to_player, dt * 4.0),
                Mode::Watch { until } => {
                    face(&mut n.yaw, to_player, dt * 3.0);
                    if now > until {
                        n.mode = Mode::Idle { until: now + 2.0 };
                    }
                }
                Mode::Sleep => {}
                Mode::Idle { until } => {
                    if now > until {
                        // Pick somewhere near home that is dry land.
                        for _ in 0..6 {
                            let a = n.rand() * std::f32::consts::TAU;
                            let r = 3.0 + n.rand() * 14.0;
                            let t = n.def.home + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
                            if terrain.height(t.x, t.z) > WATER_LEVEL + 0.3 {
                                n.mode = Mode::Walk { target: t };
                                break;
                            }
                        }
                        if matches!(n.mode, Mode::Idle { .. }) {
                            n.mode = Mode::Idle { until: now + 3.0 };
                        }
                    }
                }
                Mode::Walk { target: t } => target = Some((t, 0.4)),
                Mode::GoHome => target = Some((n.def.home, 0.6)),
                Mode::Approach { .. } => target = Some((player, 2.4)),
            }
            if let Some((t, stop)) = target {
                let d = Vec3::new(t.x - n.pos.x, 0.0, t.z - n.pos.z);
                let len = d.length();
                if len <= stop {
                    n.mode = match std::mem::replace(&mut n.mode, Mode::Sleep) {
                        Mode::GoHome if night => Mode::Sleep,
                        Mode::Approach { line } => {
                            if let Some(l) = line {
                                events.push(CastEvent::Says { id: n.def.id, line: l });
                            }
                            Mode::Watch { until: now + 10.0 }
                        }
                        _ => Mode::Idle { until: now + 2.0 + n.rand() as f64 * 8.0 },
                    };
                    continue;
                }
                let dir = d / len;
                face(&mut n.yaw, dir, dt * 5.0);
                let step = dir * (WALK_SPEED * dt).min(len);
                let others: Vec<Vec3> = positions.iter().enumerate().filter(|(i, _)| *i != idx).map(|(_, p)| *p).chain(std::iter::once(player)).collect();
                let solids = solids_for(n.pos);
                let obs = Obstacles { solids: &solids, bodies: &others };
                let before = n.pos;
                n.pos = move_body(terrain, &obs, n.pos, step, NPC_RADIUS);
                let moved = (n.pos - before).length();
                n.phase += moved * 3.2;
                if moved < WALK_SPEED * dt * 0.2 && !matches!(n.mode, Mode::Approach { .. }) {
                    // Stuck: give up on this target.
                    n.mode = Mode::Idle { until: now + 1.0 };
                }
            } else {
                n.pos.y = terrain.height(n.pos.x, n.pos.z);
            }
        }
    }
}

fn face(yaw: &mut f32, dir: Vec3, rate: f32) {
    if dir.x.abs() + dir.z.abs() < 1e-4 {
        return;
    }
    let want = dir.x.atan2(dir.z);
    let mut diff = want - *yaw;
    while diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    }
    while diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    }
    *yaw += diff.clamp(-rate, rate);
}
