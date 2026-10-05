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
    let q = t.hold_tilt.unwrap_or_else(|| t.tilt());
    if q != glam::Quat::IDENTITY {
        g.set_tilt(q);
    }
    super::joint::pose(&mut g, t, ty);
    g
}

/// A worn layer: the wearer's instance (place, size, look sliders, pose)
/// drawing the layer's shape instead.
pub fn layer_inst(wearer: &GpuInst, body: &TypeEntry, layer: &TypeEntry, fx: [f32; 4]) -> GpuInst {
    let factor = wearer.rot[2] / body.sphere_r.max(1e-3);
    let mut g = *wearer;
    g.info = layer.gpu_info();
    // It lies on the body: centred where the body is, at least as big.
    g.info[1] = wearer.info[1];
    g.rot[2] = (layer.sphere_r * factor).max(wearer.rot[2] * 1.1);
    g.fx = fx;
    g.fx[FX_HIGHLIGHT] = wearer.fx[FX_HIGHLIGHT];
    g.cuts = Default::default();
    g
}

/// `FX_CHAR` for the dark's own: blackened, and what glows on it glows red.
pub const DARK_OWN: f32 = 2.0;

/// Generic looks from properties.
pub fn look(p: &Props) -> [f32; 4] {
    let mut fx = [0.0; 4];
    fx[FX_CHAR] = p[P_CHAR].clamp(0.0, 1.0);
    fx[FX_WET] = (p[P_WET] * 0.6).clamp(0.0, 1.0);
    fx[FX_GLOW] = (p[P_LIGHT] * 0.9 + p[P_FIRE] * 0.4 + ((p[P_TEMP] - 400.0) / 800.0).max(0.0)).min(2.0);
    fx[FX_HIGHLIGHT] = -p[P_CORRUPT].clamp(0.0, 1.0);
    fx
}

/// Largest thing (bounding radius, m) that glows all over when its type
/// doesn't mark lit parts with glow(); a building would glare as one block.
const WHOLE_GLOW_MAX_R: f32 = 2.0;

/// `look` for a thing of this type: unmarked big things keep only their light.
fn thing_look(p: &Props, ty: &TypeEntry, scale: f32) -> [f32; 4] {
    let mut fx = look(p);
    if !ty.ct.marks_glow() && ty.radius() * scale > WHOLE_GLOW_MAX_R {
        fx[FX_GLOW] = 0.0;
    }
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
        let mut worn: std::collections::HashMap<ActorId, Vec<&Thing>> = std::collections::HashMap::new();
        for t in self.things.live() {
            if let Some(w) = t.worn {
                worn.entry(w).or_default().push(t);
            }
        }
        for n in &self.cast.npcs {
            let Some(body) = self.snap.type_of(n.body_ty).or(figure) else { continue };
            if !n.here() {
                continue;
            }
            {
                if (n.a.pos - cam).length() < 200.0 {
                    let mut g = n.gpu(body);
                    // Corruption shows as the dark creeping over them; a touch's glow as light.
                    // The dark's own beings are black whatever colours their body has:
                    // only their eyes show (the parts marked glow()), always, and red
                    // (charred past 1 is the renderer's sign for that).
                    let of_the_dark = n.species.touch.harms();
                    g.fx[FX_GLOW] = n.glow();
                    g.fx[FX_WET] = n.props.get(super::props::P_WET).copied().unwrap_or(0.0).min(1.0) * 0.6;
                    if of_the_dark {
                        g.fx[FX_CHAR] = DARK_OWN;
                        g.fx[FX_GLOW] = if body.ct.marks_glow() { 1.6 } else { 0.0 };
                    } else {
                        g.fx[FX_HIGHLIGHT] = -n.corruption();
                    }
                    if hover_actor == Some(ActorId::Npc(n.def.id)) {
                        g.fx[FX_HIGHLIGHT] = 0.5;
                    }
                    insts.push(g);
                    if of_the_dark {
                        let p = n.a.pos + Vec3::Y * n.a.dims.eye;
                        lights.push(((p - cam).length(), PointLight { pos: p, color: Vec3::new(1.0, 0.08, 0.04), intensity: 0.12, reach: 1.6 }));
                    } else if n.glow() > 0.05 && n.glow() > n.corruption() {
                        let p = n.a.pos + Vec3::Y * n.a.dims.height * 0.6;
                        let gl = n.glow().min(1.0);
                        lights.push(((p - cam).length(), PointLight { pos: p, color: Vec3::new(0.95, 0.9, 0.5), intensity: 0.5 * gl, reach: 3.0 + 4.0 * gl }));
                    }
                    // What they wear, drawn in their frame and pose.
                    for t in worn.get(&ActorId::Npc(n.def.id)).map(|v| v.as_slice()).unwrap_or(&[]) {
                        let Some(ty) = self.snap.type_of(t.type_id) else { continue };
                        insts.push(layer_inst(&g, body, ty, look(&t.props)));
                    }
                }
            }
        }
        // A glow or the dark on the traveler lights the ground about them.
        let (pc, pg) = (self.night.corruption, self.night.glow);
        if pc > 0.15 || pg > 0.05 {
            let p = self.player.pos + Vec3::Y * 1.2;
            let (color, k) = if pc >= pg { (Vec3::new(0.6, 0.05, 0.03), pc * 0.35) } else { (Vec3::new(0.95, 0.9, 0.5), pg) };
            lights.push(((p - cam).length(), PointLight { pos: p, color, intensity: 0.4 * k, reach: 3.0 + 4.0 * k }));
        }
        for t in self.things.live() {
            if !near(t.pos) || t.worn.is_some() {
                continue;
            }
            let Some(ty) = self.snap.type_of(t.type_id) else { continue };
            let mut fx = thing_look(&t.props, ty, t.scale);
            if hover_thing == Some(t.id) {
                fx[FX_HIGHLIGHT] = 0.7;
            }
            let tool = ty.ct.meta.tool.as_ref();
            match eye.filter(|_| t.holder == Some(ActorId::Player) && t.co_holder.is_none()) {
                Some(c) if tool.is_some() => {
                    // A tool in the view: held by its grip low and right,
                    // turned through its motion as the arm would.
                    let tool = tool.expect("tool");
                    let (f, r, u) = c.basis();
                    let len = ((Vec3::from_array(tool.tip) - Vec3::from_array(tool.grip)).length() * t.scale).max(0.05);
                    let k = (0.55 / len).min(1.0);
                    let fr = match self.player.motion.as_ref() {
                        Some(m) if m.tool == t.id => super::tool::frame(m.name, m.progress(self.t)),
                        _ => super::tool::REST,
                    };
                    let mut shown = t.clone();
                    shown.scale = t.scale * k;
                    shown.yaw = c.yaw;
                    // Pitch with the view, so it stays in sight looking up or down.
                    let q = glam::Quat::from_rotation_x(-c.pitch) * super::tool::tool_turn(tool, &fr);
                    let hand = c.pos + f * (0.32 + 0.25 * fr.reach) + r * (0.22 - 0.1 * fr.reach) - u * (0.26 - 0.18 * fr.raise);
                    shown.pos = hand - glam::Quat::from_rotation_y(c.yaw) * (q * (Vec3::from_array(tool.grip) * shown.scale));
                    shown.hold_tilt = Some(q);
                    insts.push(thing_inst(&shown, ty, fx));
                }
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
        // Placed lamps and lit windows at night, and their light anchors.
        let lit = self.cache.overlay.lamps;
        if lit > 0.05 {
            let c = crate::world::chunk_of(cam.x, cam.z);
            for p in self.snap.near_chunk(c) {
                if self.cache.overlay.hidden.contains(&p.id) {
                    continue;
                }
                let Some(ty) = self.snap.type_of(p.type_id) else { continue };
                let l = ty.light();
                let anchors: Vec<Vec3> = ty.ct.meta.anchors.iter().filter(|a| a.kind == "light").map(|a| Vec3::from_array(a.at)).collect();
                if l <= 0.0 && anchors.is_empty() {
                    continue;
                }
                let g = p.gpu(ty, 1.0);
                let spots: Vec<Vec3> = if anchors.is_empty() { vec![g.from_local(Vec3::new(0.0, ty.top * 0.9, 0.0))] } else { anchors.iter().map(|a| g.from_local(*a)).collect() };
                let l = if l > 0.0 { l.min(2.0) } else { 0.8 };
                for s in spots {
                    let d = (s - cam).length();
                    if d < 60.0 {
                        lights.push((d, PointLight { pos: s, color: Vec3::new(1.0, 0.8, 0.52), intensity: 0.8 * l * lit, reach: 7.0 + 5.0 * l }));
                    }
                }
            }
        }
        // Lights dim near the corrupted.
        let dark: Vec<(Vec3, f32)> = self.cast.npcs.iter().filter(|n| n.here() && n.corruption() > 0.5).map(|n| (n.a.pos, n.corruption())).collect();
        if !dark.is_empty() {
            for (_, l) in lights.iter_mut() {
                if l.color.z > l.color.x {
                    continue;
                }
                let k = dark.iter().map(|(p, c)| c * (1.0 - (*p - l.pos).length() / 12.0).clamp(0.0, 1.0)).fold(0.0f32, f32::max);
                l.intensity *= 1.0 - 0.7 * k;
            }
        }
        lights.sort_by(|a, b| a.0.total_cmp(&b.0));
        Drawn { insts, lights: lights.into_iter().map(|x| x.1).take(crate::render::MAX_LIGHTS).collect() }
    }

    fn flame_type(&self) -> Option<&std::sync::Arc<TypeEntry>> {
        self.snap.scene.types.values().find(|t| t.builtin && t.has_tag("flame"))
    }
}
