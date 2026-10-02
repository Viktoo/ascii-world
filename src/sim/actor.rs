//! A body in the world: the player and every character are actors with the
//! same powers. Actors walk, hold one thing, strike poses and play gestures.

use super::things::ThingId;
use super::{ActorId, Target};
use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const PLAYER_SPEED: f32 = 5.0;
pub const NPC_WALK: f32 = 1.3;
pub const NPC_RUN: f32 = 3.2;
/// How far an actor can reach to pick something up or use it.
pub const REACH: f32 = 2.4;
pub const EYE: f32 = 1.65;

/// Figure pose channels (k.s0 … k.s7 of the built-in figure).
pub type Pose = [f32; 8];
pub const L_RAISE: usize = 0;
pub const R_RAISE: usize = 1;
pub const L_FWD: usize = 2;
pub const R_FWD: usize = 3;
pub const LEAN: usize = 4;
pub const NOD: usize = 5;
pub const CROUCH: usize = 6;

/// Gestures every body knows. New kinds are written as data (pose keyframes)
/// and registered by name: `Custom` indexes that registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum GestureKind {
    Wave,
    Bow,
    Nod,
    Point,
    Cheer,
    Shrug,
    Dance,
    Sit,
    Handshake,
    HighFive,
    Hug,
    Kiss,
    Custom(u16),
}

/// One key pose of a custom gesture: progress `t` (0..1) and channel values
/// by name (l_raise, r_raise, l_fwd, r_fwd, lean, nod, crouch).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct KeyPose {
    pub t: f32,
    #[serde(default)]
    pub pose: std::collections::BTreeMap<String, f32>,
}

/// A gesture written as data.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CustomGesture {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_duration")]
    pub duration: f32,
    /// Needs the other person close and willing.
    #[serde(default)]
    pub contact: bool,
    #[serde(default = "default_distance")]
    pub distance: f32,
    /// Affection needed to say yes (contact gestures).
    #[serde(default)]
    pub intimacy: f32,
    pub frames: Vec<KeyPose>,
}

fn default_duration() -> f32 {
    2.0
}

fn default_distance() -> f32 {
    3.0
}

pub const CHANNELS: [&str; 7] = ["l_raise", "r_raise", "l_fwd", "r_fwd", "lean", "nod", "crouch"];

static CUSTOM: std::sync::RwLock<Vec<CustomGesture>> = std::sync::RwLock::new(Vec::new());

/// Check and register a custom gesture (replacing one of the same name).
pub fn register_gesture(mut g: CustomGesture) -> Result<GestureKind, String> {
    g.name = g.name.trim().to_lowercase().replace(['-', ' '], "_");
    if g.name.is_empty() || g.name.len() > 24 || !g.name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return Err(format!("'{}' is not a gesture name", g.name));
    }
    if g.frames.is_empty() || g.frames.len() > 12 {
        return Err("a gesture needs 1 to 12 key poses".into());
    }
    for f in &mut g.frames {
        f.t = if f.t.is_finite() { f.t.clamp(0.0, 1.0) } else { 0.0 };
        for (k, v) in f.pose.iter_mut() {
            if !CHANNELS.contains(&k.as_str()) {
                return Err(format!("unknown pose channel '{k}' (use {})", CHANNELS.join(", ")));
            }
            *v = if v.is_finite() { if k == "lean" { v.clamp(-0.3, 0.6) } else { v.clamp(-1.0, 1.0) } } else { 0.0 };
        }
    }
    g.frames.sort_by(|a, b| a.t.total_cmp(&b.t));
    g.duration = if g.duration.is_finite() { g.duration.clamp(0.4, 20.0) } else { 2.0 };
    g.distance = if g.distance.is_finite() { g.distance.clamp(0.3, 6.0) } else { 3.0 };
    g.intimacy = if g.intimacy.is_finite() { g.intimacy.clamp(-1.0, 1.0) } else { 0.0 };
    let mut reg = CUSTOM.write().unwrap_or_else(|e| e.into_inner());
    let i = match reg.iter().position(|x| x.name == g.name) {
        Some(i) => {
            reg[i] = g;
            i
        }
        None => {
            reg.push(g);
            reg.len() - 1
        }
    };
    Ok(GestureKind::Custom(i as u16))
}

fn custom(i: u16) -> Option<CustomGesture> {
    CUSTOM.read().unwrap_or_else(|e| e.into_inner()).get(i as usize).cloned()
}

impl GestureKind {
    pub fn parse(s: &str) -> Option<GestureKind> {
        let s = s.trim().to_lowercase().replace(['-', ' '], "_");
        if let Some(k) = Self::builtin(&s) {
            return Some(k);
        }
        let reg = CUSTOM.read().unwrap_or_else(|e| e.into_inner());
        reg.iter().position(|g| g.name == s).map(|i| GestureKind::Custom(i as u16))
    }

    fn builtin(s: &str) -> Option<GestureKind> {
        Some(match s {
            "wave" | "greet" => GestureKind::Wave,
            "bow" => GestureKind::Bow,
            "nod" => GestureKind::Nod,
            "point" => GestureKind::Point,
            "cheer" | "celebrate" => GestureKind::Cheer,
            "shrug" => GestureKind::Shrug,
            "dance" => GestureKind::Dance,
            "sit" | "crouch" | "kneel" => GestureKind::Sit,
            "handshake" | "shake_hands" | "shake" => GestureKind::Handshake,
            "high_five" | "highfive" => GestureKind::HighFive,
            "hug" | "embrace" => GestureKind::Hug,
            "kiss" => GestureKind::Kiss,
            _ => return None,
        })
    }

    pub fn name(self) -> String {
        if let GestureKind::Custom(i) = self {
            return custom(i).map(|g| g.name).unwrap_or_else(|| "gesture".into());
        }
        match self {
            GestureKind::Wave => "wave",
            GestureKind::Bow => "bow",
            GestureKind::Nod => "nod",
            GestureKind::Point => "point",
            GestureKind::Cheer => "cheer",
            GestureKind::Shrug => "shrug",
            GestureKind::Dance => "dance",
            GestureKind::Sit => "sit",
            GestureKind::Handshake => "handshake",
            GestureKind::HighFive => "high_five",
            GestureKind::Hug => "hug",
            GestureKind::Kiss => "kiss",
            GestureKind::Custom(_) => "gesture",
        }
        .to_string()
    }

    /// Gestures that need the other person close and willing.
    pub fn contact(self) -> bool {
        if let GestureKind::Custom(i) = self {
            return custom(i).is_some_and(|g| g.contact);
        }
        matches!(self, GestureKind::Handshake | GestureKind::HighFive | GestureKind::Hug | GestureKind::Kiss)
    }

    /// Distance between the two bodies for a contact gesture (m).
    pub fn distance(self) -> f32 {
        if let GestureKind::Custom(i) = self {
            return custom(i).map(|g| g.distance).unwrap_or(3.0);
        }
        match self {
            GestureKind::Hug => 0.42,
            GestureKind::Kiss => 0.5,
            GestureKind::Handshake | GestureKind::HighFive => 0.85,
            _ => 3.0,
        }
    }

    pub fn duration(self) -> f32 {
        if let GestureKind::Custom(i) = self {
            return custom(i).map(|g| g.duration).unwrap_or(2.0);
        }
        match self {
            GestureKind::Wave => 2.2,
            GestureKind::Bow => 2.0,
            GestureKind::Nod => 1.2,
            GestureKind::Point => 2.0,
            GestureKind::Cheer => 2.0,
            GestureKind::Shrug => 1.4,
            GestureKind::Dance => 6.0,
            GestureKind::Sit => 20.0,
            GestureKind::Handshake => 2.2,
            GestureKind::HighFive => 1.2,
            GestureKind::Hug => 3.5,
            GestureKind::Kiss => 2.2,
            GestureKind::Custom(_) => 2.0,
        }
    }

    /// How intimate it is: the affection a character needs to say yes.
    pub fn intimacy(self) -> f32 {
        if let GestureKind::Custom(i) = self {
            return custom(i).map(|g| g.intimacy).unwrap_or(0.0);
        }
        match self {
            GestureKind::Kiss => 0.75,
            GestureKind::Hug => 0.45,
            GestureKind::HighFive => 0.1,
            GestureKind::Handshake => -0.2,
            _ => -1.0,
        }
    }

    /// The pose at progress u (0..1) through the gesture. `t` is game time
    /// (for rhythmic gestures).
    pub fn pose(self, u: f32, t: f32) -> Pose {
        let mut p = [0.0; 8];
        let ease = (u * 4.0).min(1.0).min((1.0 - u) * 4.0).max(0.0);
        match self {
            GestureKind::Custom(i) => {
                let Some(g) = custom(i) else { return p };
                // Interpolate between the key poses around u.
                let frames = &g.frames;
                let (a, b) = match frames.iter().position(|f| f.t >= u) {
                    Some(0) => (&frames[0], &frames[0]),
                    Some(j) => (&frames[j - 1], &frames[j]),
                    None => (frames.last().unwrap_or(&frames[0]), frames.last().unwrap_or(&frames[0])),
                };
                let w = if (b.t - a.t).abs() < 1e-4 { 0.0 } else { ((u - a.t) / (b.t - a.t)).clamp(0.0, 1.0) };
                for (ci, name) in CHANNELS.iter().enumerate() {
                    let va = a.pose.get(*name).copied().unwrap_or(0.0);
                    let vb = b.pose.get(*name).copied().unwrap_or(0.0);
                    p[ci] = (va + (vb - va) * w) * ease.max(if frames.len() > 1 { 1.0 } else { 0.0 }).min(1.0);
                }
            }
            GestureKind::Wave => {
                p[R_RAISE] = 0.85 * ease;
                p[R_FWD] = 0.1 * ease;
                p[LEAN] = -0.05 * ease;
                p[R_RAISE] += (t * 9.0).sin() * 0.08 * ease;
            }
            GestureKind::Bow => {
                p[LEAN] = 0.55 * ease;
                p[NOD] = 0.6 * ease;
            }
            GestureKind::Nod => {
                p[NOD] = (u * std::f32::consts::TAU * 2.0).sin().abs() * 0.9;
            }
            GestureKind::Point => {
                p[R_FWD] = 1.0 * ease;
                p[R_RAISE] = 0.12 * ease;
            }
            GestureKind::Cheer => {
                p[L_RAISE] = 0.95 * ease;
                p[R_RAISE] = 0.95 * ease;
                p[LEAN] = -0.08 * ease;
            }
            GestureKind::Shrug => {
                p[L_RAISE] = 0.25 * ease;
                p[R_RAISE] = 0.25 * ease;
                p[L_FWD] = 0.3 * ease;
                p[R_FWD] = 0.3 * ease;
            }
            GestureKind::Dance => {
                let s = (t * 5.0).sin();
                p[L_RAISE] = (0.5 + 0.4 * s) * ease;
                p[R_RAISE] = (0.5 - 0.4 * s) * ease;
                p[LEAN] = 0.1 * (t * 2.5).cos() * ease;
                p[CROUCH] = 0.15 * (0.5 + 0.5 * (t * 10.0).sin()) * ease;
            }
            GestureKind::Sit => {
                p[CROUCH] = ease;
                p[LEAN] = 0.15 * ease;
            }
            GestureKind::Handshake => {
                p[R_FWD] = 0.75 * ease;
                p[R_RAISE] = (0.05 + (t * 12.0).sin() * 0.03) * ease;
            }
            GestureKind::HighFive => {
                p[R_RAISE] = 0.8 * ease;
                p[R_FWD] = 0.45 * ease;
            }
            GestureKind::Hug => {
                p[L_FWD] = 0.9 * ease;
                p[R_FWD] = 0.9 * ease;
                p[L_RAISE] = 0.12 * ease;
                p[R_RAISE] = 0.12 * ease;
                p[LEAN] = 0.08 * ease;
            }
            GestureKind::Kiss => {
                p[LEAN] = 0.24 * ease;
                p[NOD] = 0.2 * ease;
                p[L_FWD] = 0.35 * ease;
                p[R_FWD] = 0.35 * ease;
            }
        }
        p
    }
}

#[derive(Clone, Debug)]
pub struct GestureRun {
    pub kind: GestureKind,
    pub with: Option<ActorId>,
    pub t0: f64,
    pub dur: f32,
}

/// Something that takes time: walking somewhere, waiting, following.
#[derive(Clone, Debug, PartialEq)]
pub enum Task {
    Goto { target: Target, stop: f32, run: bool, deadline: f64 },
    Move { dir: Vec3, until: f64 },
    Wait { until: f64 },
    Follow { who: ActorId, dist: f32, until: f64 },
    /// Stand facing someone (talking, watching, waiting to catch).
    Face { target: Target, until: f64 },
}

#[derive(Clone, Debug)]
pub struct Actor {
    pub pos: Vec3,
    pub yaw: f32,
    pub held: Option<ThingId>,
    pub pose: Pose,
    pub gesture: Option<GestureRun>,
    pub task: Option<Task>,
    /// Walk animation phase (radians).
    pub phase: f32,
    /// Distance moved last step (for animation and stuck detection).
    pub moved: f32,
    pub asleep: bool,
    /// Ready to catch something thrown at it until this game time.
    pub catching: f64,
    /// How long it has been trying to walk without getting anywhere (s).
    pub stuck: f32,
}

impl Actor {
    pub fn new(pos: Vec3, yaw: f32) -> Actor {
        Actor { pos, yaw, held: None, pose: [0.0; 8], gesture: None, task: None, phase: 0.0, moved: 0.0, asleep: false, catching: 0.0, stuck: 0.0 }
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }

    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    /// Where a held thing sits: between both hands (big things) or in the
    /// right hand, following the pose (the same arm the figure draws).
    pub fn hand(&self, big: bool, height: f32) -> Vec3 {
        let r = self.hand_local(1.0, R_RAISE, R_FWD);
        let local = if big {
            let l = self.hand_local(-1.0, L_RAISE, L_FWD);
            let m = (l + r) * 0.5;
            Vec3::new(m.x, m.y, m.z + 0.08)
        } else {
            r
        };
        let s = height / 1.75;
        let w = local * s;
        self.pos + self.right() * w.x + Vec3::Y * w.y + self.forward() * w.z
    }

    /// A hand in the figure's own frame (x right, y up, z forward; metres
    /// for a 1.75 m body), from the arm and lean channels of the pose.
    fn hand_local(&self, side: f32, raise: usize, fwd: usize) -> Vec3 {
        let p = &self.pose;
        let a = p[raise].clamp(0.0, 1.0) * 3.0;
        let f = p[fwd].clamp(0.0, 1.0) * 1.45;
        let crouch = p[CROUCH].clamp(0.0, 1.0);
        let hip = 0.9 - crouch * 0.38;
        let x = side * (0.29 + 0.56 * a.sin());
        let y = 1.42 - 0.56 * a.cos() * f.cos();
        let z = 0.56 * a.cos() * f.sin();
        // Lean forward around the hip.
        let lean = p[LEAN].clamp(-0.3, 0.6);
        let (sn, cs) = lean.sin_cos();
        let (uy, uz) = (y - 0.9, z);
        Vec3::new(x, hip + uy * cs - uz * sn, uy * sn + uz * cs)
    }

    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::Y * EYE
    }

    /// Turn towards a direction at most `rate` radians.
    pub fn face(&mut self, dir: Vec3, rate: f32) {
        if dir.x.abs() + dir.z.abs() < 1e-4 {
            return;
        }
        let want = dir.x.atan2(dir.z);
        let mut diff = want - self.yaw;
        while diff > std::f32::consts::PI {
            diff -= std::f32::consts::TAU;
        }
        while diff < -std::f32::consts::PI {
            diff += std::f32::consts::TAU;
        }
        self.yaw += diff.clamp(-rate, rate);
    }

    /// Ease the pose towards what the body is doing now.
    pub fn update_pose(&mut self, t: f64, dt: f32, holding_big: Option<bool>) {
        let mut goal = [0.0f32; 8];
        if let Some(big) = holding_big {
            goal[R_FWD] = 0.55;
            if big {
                goal[L_FWD] = 0.6;
                goal[R_FWD] = 0.6;
            }
        }
        if self.catching > t {
            goal[L_FWD] = 0.7;
            goal[R_FWD] = 0.7;
            goal[L_RAISE] = 0.15;
            goal[R_RAISE] = 0.15;
        }
        if self.asleep {
            goal[CROUCH] = 1.0;
            goal[NOD] = 0.8;
        }
        if let Some(g) = &self.gesture {
            let u = ((t - g.t0) as f32 / g.dur).clamp(0.0, 1.0);
            let gp = g.kind.pose(u, t as f32);
            for i in 0..8 {
                if gp[i].abs() > goal[i].abs() || g.kind.contact() {
                    goal[i] = gp[i];
                }
            }
            if u >= 1.0 {
                self.gesture = None;
            }
        }
        let k = (dt * 8.0).min(1.0);
        for i in 0..8 {
            self.pose[i] += (goal[i] - self.pose[i]) * k;
        }
    }
}
