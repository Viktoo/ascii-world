//! What the live world adds to a frame: characters (posed), live things
//! (with their state and generic looks: charred, wet, glowing, highlighted),
//! flames over everything burning, and point lights from fires and lamps.

use super::props::*;
use super::things::Thing;
use super::{ActorId, Sim, Target};
use crate::render::{FX_CHAR, FX_GLOW, FX_HIGHLIGHT, FX_WET, GpuInst, PointLight};
use crate::world::TypeEntry;
use glam::Vec3;

/// Instance for a live thing. Living things that are still growing are
/// drawn smaller, with their base kept on the ground.
pub fn thing_inst(t: &Thing, ty: &TypeEntry, fx: [f32; 4]) -> GpuInst {
    let grow = if t.props[P_ALIVE] > 0.0 || t.props[P_GROWTH] < 1.0 { 0.25 + 0.75 * t.props[P_GROWTH].clamp(0.0, 1.0) } else { 1.0 };
    let scale = (t.scale * grow).max(0.01);
    let y = t.pos.y + ty.bottom * (t.scale - scale);
    let mut k = t.params;
    k[1] = scale;
    let mut g = GpuInst {
        pos_scale: [t.pos.x, y, t.pos.z, scale],
        rot: [t.yaw.cos(), t.yaw.sin(), ty.sphere_r * scale, 1.0],
        k0: [k[0], k[1], k[2], k[3]],
        k1: [k[4], k[5], k[6], k[7]],
        fx,
        info: ty.gpu_info(),
        ..Default::default()
    };
    g.set_state(&t.state);
    g.cuts = t.gpu_cuts();
    g
}

/// Generic looks from properties.
pub fn look(p: &Props) -> [f32; 4] {
    let mut fx = [0.0; 4];
    fx[FX_CHAR] = p[P_CHAR].clamp(0.0, 1.0);
    fx[FX_WET] = (p[P_WET] * 0.6).clamp(0.0, 1.0);
    fx[FX_GLOW] = (p[P_LIGHT] * 0.9 + p[P_FIRE] * 0.4 + ((p[P_TEMP] - 400.0) / 800.0).max(0.0)).min(2.0);
    fx
}

pub struct Drawn {
    pub insts: Vec<GpuInst>,
    pub lights: Vec<PointLight>,
}

impl Sim {
    /// Everything live to draw this frame, near `cam`.
    pub fn draw(&self, cam: Vec3, view: f32) -> Drawn {
        self.draw_from(cam, view, None)
    }

    /// As `draw`, seen through the player's eyes: what they hold is shown
    /// low and to the right of the view, where a hand would bring it.
    pub fn draw_first_person(&self, cam: &crate::render::Camera, view: f32) -> Drawn {
        self.draw_from(cam.pos, view, Some(cam))
    }

    fn draw_from(&self, cam: Vec3, view: f32, eye: Option<&crate::render::Camera>) -> Drawn {
        let mut insts = Vec::new();
        let mut lights: Vec<(f32, PointLight)> = Vec::new();
        let near = |p: Vec3| (p - cam).length() < view;
        let hover_thing = match &self.hover {
            Some(Target::Thing(id)) => Some(*id),
            _ => None,
        };
        let hover_actor = match &self.hover {
            Some(Target::Actor(a)) => Some(*a),
            _ => None,
        };
        let figure = self.snap.figure_type.and_then(|f| self.snap.type_of(f));
        for n in &self.cast.npcs {
            let Some(body) = self.snap.type_of(n.body_ty).or(figure) else { continue };
            if n.dead {
                continue;
            }
            {
                if (n.a.pos - cam).length() < 200.0 {
                    let mut g = n.gpu(body);
                    if hover_actor == Some(ActorId::Npc(n.def.id)) {
                        g.fx[FX_HIGHLIGHT] = 0.5;
                    }
                    insts.push(g);
                }
            }
        }
        for t in self.things.live() {
            if !near(t.pos) {
                continue;
            }
            let Some(ty) = self.snap.type_of(t.type_id) else { continue };
            let mut fx = look(&t.props);
            if hover_thing == Some(t.id) {
                fx[FX_HIGHLIGHT] = 0.7;
            }
            match eye.filter(|_| t.holder == Some(ActorId::Player) && t.co_holder.is_none()) {
                Some(c) => {
                    // The view-model: in front, low right, scaled to fit.
                    let (f, r, u) = c.basis();
                    let size = (ty.radius() * t.scale).max(0.05);
                    let k = (0.1 / size).min(1.0);
                    let mut shown = t.clone();
                    shown.scale = t.scale * k;
                    let centre = c.pos + f * 0.6 + r * 0.27 - u * 0.21;
                    let (cc, _) = shown.proxy(ty);
                    shown.pos += centre - cc;
                    shown.yaw = c.yaw + 0.6;
                    insts.push(thing_inst(&shown, ty, fx));
                }
                None => insts.push(thing_inst(t, ty, fx)),
            }
            if t.props[P_LIGHT] > 0.05 {
                let p = t.pos + Vec3::Y * (ty.sphere_cy * t.scale).max(0.2);
                let l = t.props[P_LIGHT].min(2.0);
                lights.push(((p - cam).length(), PointLight { pos: p, color: Vec3::new(1.0, 0.82, 0.55), intensity: 0.9 * l, reach: 6.0 + 6.0 * l }));
            }
        }
        // Flames over everything burning.
        if let Some(flame) = self.flame_type() {
            let mut fires: Vec<(f32, super::env::Burning)> = self.burning().into_iter().map(|b| ((b.pos - cam).length(), b)).filter(|(d, _)| *d < view).collect();
            fires.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (d, b) in fires.iter().take(self.cfg.max_flames) {
                let s = (0.35 + 0.75 * b.fire) * (b.size / 0.5).clamp(0.6, 6.0);
                let p = b.pos + Vec3::Y * 0.7 * s;
                let mut g = GpuInst {
                    pos_scale: [p.x, p.y, p.z, s],
                    rot: [1.0, 0.0, flame.sphere_r * s, 1.0],
                    k0: [b.seed % 97.0, s, 0.5, 0.5],
                    k1: [0.5; 4],
                    fx: { let mut f = [0.0; 4]; f[FX_GLOW] = 1.6; f },
                    info: flame.gpu_info(),
                    ..Default::default()
                };
                g.s0[0] = self.t as f32 + b.seed * 0.37;
                insts.push(g);
                let flicker = 0.85 + 0.15 * ((self.t as f32 * 9.0 + b.seed).sin() * (self.t as f32 * 5.3 + b.seed * 0.3).cos());
                lights.push((*d, PointLight { pos: p + Vec3::Y * 0.5 * s, color: Vec3::new(1.0, 0.55, 0.22), intensity: (0.5 + 0.7 * b.fire * (b.size / 1.5).min(2.0)) * flicker, reach: 6.0 + 5.0 * s.min(4.0) }));
            }
        }
        lights.sort_by(|a, b| a.0.total_cmp(&b.0));
        Drawn { insts, lights: lights.into_iter().map(|x| x.1).take(crate::render::MAX_LIGHTS).collect() }
    }

    fn flame_type(&self) -> Option<&std::sync::Arc<TypeEntry>> {
        self.snap.scene.types.values().find(|t| t.builtin && t.has_tag("flame"))
    }
}
