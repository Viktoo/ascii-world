//! A body in the world: the player and every character are actors with the
//! same powers. Actors walk, hold one thing, strike poses and play gestures.

use super::things::ThingId;
use super::{ActorId, Target};
use crate::world::species::Dims;
use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const PLAYER_SPEED: f32 = 5.0;
pub const NPC_WALK: f32 = 1.3;
pub const NPC_RUN: f32 = 3.2;
/// How far an actor can reach to pick something up or use it.
pub const REACH: f32 = 2.4;

/// Pose roles (k.s0 … k.s7 of every body). Each body decides what a role
/// moves: arms on a person, paws on a dog, wings on a dragon.
pub type Pose = [f32; 8];
pub const L_RAISE: usize = 0;
pub const R_RAISE: usize = 1;
pub const L_FWD: usize = 2;
pub const R_FWD: usize = 3;
pub const LEAN: usize = 4;
pub const NOD: usize = 5;
pub const CROUCH: usize = 6;
pub const SPREAD: usize = 7;

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
    /// Tail up and swinging (bodies with `spread`); others nod.
    Wag,
    /// Stroking an animal: a hand down and along, leaning in.
    Pet,
    Custom(u16),
}

/// One key pose of a custom gesture: progress `t` (0..1) and role values by
/// name (raise_l, raise_r, reach_l, reach_r, lean, head, crouch, spread; the
/// old names l_raise, r_raise, l_fwd, r_fwd and nod still work).
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

pub const CHANNELS: [&str; 8] = crate::lang::ir::ROLES;

static CUSTOM: std::sync::RwLock<Vec<CustomGesture>> = std::sync::RwLock::new(Vec::new());

/// Check and register a custom gesture (replacing one of the same name).
pub fn register_gesture(mut g: CustomGesture) -> Result<GestureKind, String> {
    g.name = g.name.trim().to_lowercase().replace(['-', ' '], "_");
    let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    let valid = match g.name.split_once('@') {
        Some((a, b)) => ok(a) && ok(b),
        None => ok(&g.name),
    };
    if !valid || g.name.len() > 48 {
        return Err(format!("'{}' is not a gesture name", g.name));
    }
    if g.frames.is_empty() || g.frames.len() > 12 {
        return Err("a gesture needs 1 to 12 key poses".into());
    }
    for f in &mut g.frames {
        f.t = if f.t.is_finite() { f.t.clamp(0.0, 1.0) } else { 0.0 };
        let mut pose = std::collections::BTreeMap::new();
        for (k, v) in f.pose.iter() {
            let k = crate::lang::check::role_alias(&k.trim().to_lowercase());
            let names: Vec<String> = match k.as_str() {
                "reach" | "raise" => vec![format!("{k}_l"), format!("{k}_r")],
                _ => vec![k.clone()],
            };
            for k in names {
                if !CHANNELS.contains(&k.as_str()) {
                    return Err(format!("unknown pose role '{k}' (use {})", CHANNELS.join(", ")));
                }
                let v = if v.is_finite() { if k == "lean" { v.clamp(-0.3, 0.6) } else { v.clamp(-1.0, 1.0) } } else { 0.0 };
                pose.insert(k, v);
            }
        }
        f.pose = pose;
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
            "wag" | "wag_tail" | "tail" => GestureKind::Wag,
            "pet" | "stroke" | "pat" | "scratch" => GestureKind::Pet,
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
            GestureKind::Wag => "wag",
            GestureKind::Pet => "pet",
            GestureKind::Custom(_) => "gesture",
        }
        .to_string()
    }

    /// This gesture as a body of `species` does it: its own version
    /// (`name@species`) when one was written, else the shared one.
    pub fn for_body(self, species: &str) -> GestureKind {
        let name = self.name();
        let base = name.split('@').next().unwrap_or(&name).to_string();
        if species.is_empty() {
            return if name.contains('@') { GestureKind::parse(&base).unwrap_or(self) } else { self };
        }
        GestureKind::parse(&format!("{base}@{species}")).unwrap_or(self)
    }

    /// Whether this gesture moves roles a body lacks (`roles` bit mask).
    pub fn needs_roles(self, roles: u8) -> bool {
        (0..8).any(|i| roles & (1 << i) == 0 && [0.3f32, 0.5, 0.7].iter().any(|u| self.pose(*u, 0.0)[i].abs() > 0.15))
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
            GestureKind::Wag => 2.0,
            GestureKind::Pet => 2.4,
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
            GestureKind::Wag => {
                p[SPREAD] = (t * 14.0).sin() * 0.9 * ease;
                p[NOD] = -0.2 * ease;
            }
            GestureKind::Pet => {
                p[R_FWD] = (0.7 + (t * 5.0).sin() * 0.12) * ease;
                p[LEAN] = 0.3 * ease;
                p[CROUCH] = 0.3 * ease;
                p[NOD] = 0.25 * ease;
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
    /// The body's size numbers.
    pub dims: Dims,
    /// Species name for gestures written for it ("" for people).
    pub species: String,
    /// Roles the body answers to (bit i = k.s<i>).
    pub roles: u8,
    /// In the air (bodies that fly), this high above the ground (m).
    pub alt: f32,
    /// Sitting on another body that carries it.
    pub riding: Option<ActorId>,
    /// In or on a thing it steers (a cart, a car, a boat).
    pub aboard: Option<ThingId>,
    /// Held in someone's arms (a cat, a lamb), until it wants down.
    pub carried_by: Option<ActorId>,
    pub carried_until: f64,
    /// Up-and-down speed (m/s): jumping, falling.
    pub vy: f32,
    /// Standing on something (the ground, a floor, a stair).
    pub grounded: bool,
    /// How far down it is crouched (0 standing … 1 fully), and whether it wants to be.
    pub crouch: f32,
    pub crouching: bool,
    /// Wants to run rather than walk.
    pub running: bool,
    /// Working a held tool: a swing, a chop, a dig (see `tool`).
    pub motion: Option<super::tool::MotionRun>,
    /// The support its last step found under it (where, and how high).
    pub support: Option<(Vec3, f32)>,
}

/// Bit mask of the roles a body lists.
pub fn role_mask(b: &crate::lang::ir::Body) -> u8 {
    let mut m = 0u8;
    for (i, r) in crate::lang::ir::ROLES.iter().enumerate() {
        if b.has_role(r) {
            m |= 1 << i;
        }
    }
    m
}

/// A person's roles: arms, lean, head, crouch (no spread).
pub const HUMAN_ROLES: u8 = 0b0111_1111;

impl Actor {
    pub fn new(pos: Vec3, yaw: f32) -> Actor {
        Actor { pos, yaw, held: None, pose: [0.0; 8], gesture: None, task: None, phase: 0.0, moved: 0.0, asleep: false, catching: 0.0, stuck: 0.0, dims: Dims::default(), roles: HUMAN_ROLES, species: String::new(), alt: 0.0, riding: None, aboard: None, carried_by: None, carried_until: 0.0, vy: 0.0, grounded: true, crouch: 0.0, crouching: false, running: false, motion: None, support: None }
    }

    /// Sitting on something that carries it (a mount, a cart).
    pub fn seated(&self) -> bool {
        self.riding.is_some() || self.aboard.is_some()
    }

    /// Off its own feet: seated, or carried.
    pub fn carried(&self) -> bool {
        self.seated() || self.carried_by.is_some()
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }

    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    /// Where a held thing sits. Bodies with arms hold it between both hands
    /// (big things) or in the right hand, following the pose: arms built
    /// like the figure's follow its arm maths; other arms swing from the
    /// grip (the right hand at rest). Bodies without arms hold it at their
    /// grip (a mouth, a claw), pushed forward a little by the reach roles.
    pub fn hand(&self, big: bool) -> Vec3 {
        let w = if self.dims.arms && !self.dims.human_arms {
            let r = self.grip_hand(1.0, R_RAISE, R_FWD);
            if big {
                let m = (self.grip_hand(-1.0, L_RAISE, L_FWD) + r) * 0.5;
                Vec3::new(m.x, m.y, m.z + 0.08 * self.dims.ratio)
            } else {
                r
            }
        } else if self.dims.arms {
            let r = self.hand_local(1.0, R_RAISE, R_FWD);
            let local = if big {
                let l = self.hand_local(-1.0, L_RAISE, L_FWD);
                let m = (l + r) * 0.5;
                Vec3::new(m.x, m.y, m.z + 0.08)
            } else {
                r
            };
            local * (self.dims.height / 1.75)
        } else {
            let g = Vec3::from_array(self.dims.grip);
            let fwd = self.pose[L_FWD].max(self.pose[R_FWD]).clamp(0.0, 1.0);
            let head = self.pose[NOD].clamp(0.0, 1.0);
            g + Vec3::new(0.0, -head * g.y * 0.6, fwd * self.dims.radius * 0.5)
        };
        self.pos + self.right() * w.x + Vec3::Y * w.y + self.forward() * w.z
    }

    /// Where a rider sits (world space), if this body can be ridden.
    pub fn seat(&self) -> Option<Vec3> {
        let s = Vec3::from_array(self.dims.seat?);
        Some(self.pos + self.right() * s.x + Vec3::Y * s.y + self.forward() * s.z)
    }

    /// A hand of an arm that hangs from above the grip (local, scaled):
    /// raised sideways and swung forward as the figure's arm is.
    fn grip_hand(&self, side: f32, raise: usize, fwd: usize) -> Vec3 {
        let p = &self.pose;
        let g = Vec3::from_array(self.dims.grip);
        let len = (self.dims.height * 0.32).min(g.y.max(0.05));
        let shoulder = Vec3::new(g.x.abs() * side, g.y + len, g.z);
        let a = p[raise].clamp(0.0, 1.0) * 3.0;
        let f = p[fwd].clamp(0.0, 1.0) * 1.45;
        shoulder + Vec3::new(side * len * a.sin(), -len * a.cos() * f.cos(), len * a.cos() * f.sin())
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
        self.pos + Vec3::Y * self.eye_height()
    }

    /// How far crouching lowers a body (a fraction of its height).
    pub const CROUCH_DROP: f32 = 0.42;

    /// How tall it stands right now.
    pub fn stand_height(&self) -> f32 {
        self.dims.height * (1.0 - Self::CROUCH_DROP * self.crouch)
    }

    /// Eye height above the feet right now.
    pub fn eye_height(&self) -> f32 {
        self.dims.eye * (1.0 - Self::CROUCH_DROP * self.crouch)
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
        // Hovering bodies keep drifting (their phase bobs them) at rest.
        if !self.asleep {
            self.phase += dt * self.dims.idle;
        }
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
        if self.seated() {
            goal[CROUCH] = 0.55;
            goal[L_FWD] = goal[L_FWD].max(0.35);
            goal[R_FWD] = goal[R_FWD].max(0.35);
        }
        if self.alt > 0.5 && self.dims.flies {
            // Wings (or what serves) beat in flight.
            let beat = (t as f32 * 5.0).sin();
            goal[L_RAISE] = 0.5 + 0.45 * beat;
            goal[R_RAISE] = 0.5 + 0.45 * beat;
            goal[SPREAD] = 0.8;
        }
        if self.crouch > 0.01 {
            goal[CROUCH] = goal[CROUCH].max(self.crouch * 0.8);
            goal[LEAN] = goal[LEAN].max(self.crouch * 0.2);
        }
        if let Some(g) = &self.gesture {
            let u = ((t - g.t0) as f32 / g.dur).clamp(0.0, 1.0);
            let gp = g.kind.for_body(&self.species).pose(u, t as f32);
            for i in 0..8 {
                if gp[i].abs() > goal[i].abs() || g.kind.contact() {
                    goal[i] = gp[i];
                }
            }
            if u >= 1.0 {
                self.gesture = None;
            }
        }
        // Roles this body lacks fall back to ones it has: a raised arm
        // becomes a reached paw, a reach becomes a dip of the head.
        let has = |i: usize| self.roles & (1 << i) != 0;
        for (raise, fwd) in [(L_RAISE, L_FWD), (R_RAISE, R_FWD)] {
            if !has(raise) && goal[raise].abs() > goal[fwd].abs() {
                goal[fwd] = goal[raise];
            }
            if !has(fwd) && goal[fwd] > goal[NOD] {
                goal[NOD] = goal[fwd] * 0.5;
            }
        }
        if !has(SPREAD) && goal[SPREAD] != 0.0 && goal[NOD] == 0.0 {
            goal[NOD] = goal[SPREAD].abs() * 0.3;
        }
        for i in 0..8 {
            if !has(i) {
                goal[i] = 0.0;
            }
        }
        let k = (dt * 8.0).min(1.0);
        for i in 0..8 {
            self.pose[i] += (goal[i] - self.pose[i]) * k;
        }
        // A tool's motion moves the arm at its own pace.
        if let Some(m) = &self.motion {
            super::tool::arm_pose(m, t, &mut self.pose);
        }
    }
}
