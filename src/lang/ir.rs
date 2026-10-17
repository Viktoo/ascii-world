//! Typed IR produced by the allowlist checker. Both translators consume this;
//! nothing downstream ever sees the JS AST.

use super::api::{Api, Ty};

/// Instance parameter fields readable as `k.<name>`: 8 static parameters,
/// then the 8 live state slots (`s0`..`s7`, written by behaviour code),
/// then what the engine poses on a body (never written by code): `s8` its
/// grip, `s9` how open its mouth is (talking), `s10` how shut its eyes are
/// (a blink, sleep), `s11` a clock in seconds (hair and cloth stir with it);
/// then look sliders six onward, `g` … `z` (the first five are `a` … `e`).
pub const K_FIELDS: [&str; 40] = [
    "seed", "scale", "a", "b", "c", "d", "e", "f", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
];
/// Where look slider `i` (0 = the first) is in k: a … e, then g … z.
pub const fn look_k(i: usize) -> usize {
    if i < 5 { 2 + i } else { 20 + (i - 5) }
}
/// State slots writable as `s.<name>` in behaviour functions.
pub const STATE_FIELDS: [&str; 8] = ["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7"];
/// What a behaviour function knows about its situation, read as `w.<name>`.
/// Every other `w.<name>` is one of the thing's own properties.
pub const CTX_FIELDS: [&str; 10] = ["dt", "hour", "age", "held", "near", "speed", "ground", "water", "impact", "on"];
pub const MAX_PROPS: usize = 32;

pub fn k_field(name: &str) -> Option<u8> {
    K_FIELDS.iter().position(|f| *f == name).map(|i| i as u8)
}

pub fn state_field(name: &str) -> Option<u8> {
    STATE_FIELDS.iter().position(|f| *f == name).map(|i| i as u8)
}

pub fn ctx_field(name: &str) -> Option<u8> {
    CTX_FIELDS.iter().position(|f| *f == name).map(|i| i as u8)
}

/// A property name: lower-case letters, digits and underscores, starting with a letter.
pub fn valid_prop_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 24 && n.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Ty,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Num(f32),
    Local(u32),
    /// 0 = x, 1 = y, 2 = z
    Param(u8),
    KField(u8),
    /// Behaviour: `s.sN`.
    State(u8),
    /// Behaviour: `w.<ctx field>`.
    Ctx(u8),
    /// Behaviour: `w.<prop>` (other = false) or `o.<prop>` (other = true);
    /// `idx` indexes `Module::prop_names`.
    Prop { other: bool, idx: u16 },
    /// Component 0..2 of a vec3 expression.
    Comp(Box<Expr>, u8),
    /// Arithmetic negation (number or vec3).
    Neg(Box<Expr>),
    Not(Box<Expr>),
    /// Number → boolean (`x != 0`), JS truthiness for conditions.
    Truthy(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
    /// true = &&, false = ||
    Logic(bool, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Api, Vec<Expr>),
}

/// Things behaviour code can ask the world to do. They are queued, never run
/// directly; the simulation applies them with per-tick caps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    /// say(i): speak `meta.says[i]`.
    Say,
    /// sound(i): make the sound `meta.sounds[i]`.
    Sound,
    /// spawn(i): make a new `meta.spawns[i]` next to it.
    Spawn,
    /// transform(i): turn into `meta.spawns[i]`.
    Transform,
    /// remove(): vanish.
    Remove,
}

impl Effect {
    pub fn from_name(n: &str) -> Option<Effect> {
        Some(match n {
            "say" => Effect::Say,
            "sound" => Effect::Sound,
            "spawn" => Effect::Spawn,
            "transform" => Effect::Transform,
            "remove" => Effect::Remove,
            _ => return None,
        })
    }
    pub fn takes_index(self) -> bool {
        self != Effect::Remove
    }
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let { id: u32, init: Expr },
    Assign { id: u32, value: Expr },
    If { cond: Expr, then: Vec<Stmt>, els: Vec<Stmt> },
    /// `for (let i = start; i < end; i += step)`, unrolled count known statically.
    For { id: u32, start: f32, step: f32, count: u32, body: Vec<Stmt> },
    Return(Expr),
    /// Behaviour: `s.sN = value`.
    SetState { idx: u8, value: Expr },
    /// Behaviour: `w.<prop> = value` / `o.<prop> = value`.
    SetProp { other: bool, idx: u16, value: Expr },
    /// Behaviour: an effect call statement.
    Effect { effect: Effect, arg: Option<Expr> },
    /// Behaviour: `return;`
    End,
}

#[derive(Clone, Debug)]
pub struct Local {
    pub name: String,
    pub ty: Ty,
    pub mutable: bool,
}

#[derive(Clone, Debug)]
pub struct Func {
    pub locals: Vec<Local>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Default)]
pub struct Meta {
    pub name: String,
    pub bounds: [f32; 3],
    pub tags: Vec<String>,
    /// Initial property values, e.g. `props: { mass: 2, burns: 0.6 }`.
    #[serde(default)]
    pub props: Vec<(String, f32)>,
    /// Lines for `say(i)`.
    #[serde(default)]
    pub says: Vec<String>,
    /// Sounds for `sound(i)`.
    #[serde(default)]
    pub sounds: Vec<String>,
    /// Type names for `spawn(i)` / `transform(i)`.
    #[serde(default)]
    pub spawns: Vec<String>,
    /// How it sounds struck, brushed or stepped on, when its properties
    /// don't say: `sound: { hard: 0.2, dry: 0.3, ring: 0 }` (each 0–1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sound: Vec<(String, f32)>,
    /// Present when the type is a body that a being can live in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    /// For a layer (clothing, armour, gear): the body it is written for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fits: Option<String>,
    /// Named points on the shape: doorways, shelves, seats, beds, stairs,
    /// fixed parts. What fits together is told by these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<Anchor>,
    /// A hinge it swings on (a door, a gate, a lid, a trapdoor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint: Option<Joint>,
    /// How it is held and worked: where the hand goes, where it strikes,
    /// and the motions it knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<Tool>,
    /// Ground it takes away under itself: a cellar, a burrow, a house set
    /// into a hill. The terrain inside the box is lowered to `floor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hollow: Option<Hollow>,
    /// It carries a rider who steers it (a cart, a car, a boat, a flying
    /// carpet): its top speed and what it goes on. The rider sits at its
    /// first `seat` anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive: Option<Drive>,
    /// It goes off: a blast that hurts and throws what is near, blows out
    /// the ground and holes in what stands close, maybe sets things alight
    /// (a grenade, a stick of dynamite, a powder keg, a shell, a firework).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<Burst>,
    /// Where it grows when scattered over the land, by the water over the
    /// ground (reeds at the edge, lilies on a pond, kelp in the sea).
    /// None: on dry ground, clear of the water.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grows: Option<Grows>,
}

/// Where a scattered thing grows: `depth` [lo, hi] metres of water over the
/// ground (a negative lo lets it grow on dry ground up to that high above
/// the water), standing on the bed or floating at the surface.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Grows {
    pub depth: [f32; 2],
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub surface: bool,
}

impl Grows {
    /// Things that say nothing grow on dry ground, this far over the water.
    pub const DRY: Grows = Grows { depth: [f32::NEG_INFINITY, -0.35], surface: false };

    /// Whether it grows where the water stands `wd` metres over the ground
    /// (negative: the ground is that far above the water).
    pub fn fits(&self, wd: f32) -> bool {
        wd >= self.depth[0] && wd <= self.depth[1]
    }
}

impl Meta {
    /// Where it grows (dry ground unless it says otherwise).
    pub fn grows(&self) -> &Grows {
        self.grows.as_ref().unwrap_or(&Grows::DRY)
    }
}

/// What a vehicle goes on.
pub const DRIVE_ON: [&str; 3] = ["land", "water", "air"];

/// How a vehicle moves: top speed (m/s), land, water or air, and what
/// drives it, in a word: "legs" (pedals, a scooter), "oars" (a paddle, a
/// pole), "wind" (a sail), "beast" (a horse or ox pulling it), "engine"
/// (and anything else: magic, steam). Muscle tires the one driving and goes
/// at their strength; wind goes with the weather; a beast's vehicle needs
/// one harnessed to it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Drive {
    pub speed: f32,
    pub on: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub by: String,
}

/// What an anchor is for.
pub const ANCHOR_KINDS: [&str; 7] = ["door", "slot", "seat", "bed", "stairs", "part", "light"];

/// A named point on a shape (local metres; `face` in degrees, 0 = +z, the
/// way out of a doorway or the way a seat faces).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Anchor {
    pub kind: String,
    pub at: [f32; 3],
    #[serde(default)]
    pub face: f32,
    /// An opening's width and height (doors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f32; 2]>,
    /// The top of a stair or ladder (`at` is its foot).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<[f32; 3]>,
    /// What is set there (a slot's sword, a part's rope ladder): a type name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holds: Option<String>,
}

impl Anchor {
    /// The way it faces, as a local unit vector in the ground plane.
    pub fn out(&self) -> [f32; 3] {
        let a = self.face.to_radians();
        [a.sin(), 0.0, a.cos()]
    }
}

/// A hinge: a local axis through `at`, turned `open` radians when fully open.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Joint {
    pub axis: [f32; 3],
    pub at: [f32; 3],
    pub open: f32,
}

/// Motions a held tool can make.
pub const MOTIONS: [&str; 6] = ["swing", "chop", "thrust", "dig", "pour", "shoot"];

/// A tool: hand position, striking point (local), motions it knows.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Tool {
    pub grip: [f32; 3],
    pub tip: [f32; 3],
    pub motions: Vec<String>,
    /// How keen its tip or edge is, 0..1 (a club 0, an axe 0.6, a spear or
    /// an arrowhead 0.9). A keen thing hurts more, and one that flies point
    /// first sticks where it lands hard. None: judged from its motions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharp: Option<f32>,
    /// It sends something flying from its tip (a bow, a sling, a gun, a
    /// cannon, a blowpipe): the "shoot" motion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<Launch>,
}

impl Tool {
    /// How keen it is: as said, or as its motions suggest.
    pub fn keen(&self) -> f32 {
        let has = |m: &str| self.motions.iter().any(|x| x == m);
        self.sharp.unwrap_or(if has("thrust") { 0.8 } else if has("chop") { 0.6 } else if has("swing") { 0.4 } else { 0.0 }).clamp(0.0, 1.0)
    }
}

/// What a launcher sends flying, and how.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Launch {
    /// The type name of what it shoots ("arrow", "musket ball", "stone").
    pub ammo: String,
    /// How fast it leaves, m/s (a sling 30, a bow 55, a musket 400).
    pub speed: f32,
    /// How far shots stray, degrees.
    #[serde(default)]
    pub spread: f32,
    /// How many shots it holds when full (a quiver's worth, a magazine);
    /// 0: it never runs out (a wand that shoots sparks).
    #[serde(default = "twelve")]
    pub holds: u32,
    /// How many go at once (a blunderbuss's shot: 8).
    #[serde(default = "one_u")]
    pub per_shot: u32,
    /// Seconds to make ready between shots (draw a bow 0.8, ram a musket 6).
    #[serde(default = "ready_secs")]
    pub ready: f32,
}

fn twelve() -> u32 {
    12
}
fn one_u() -> u32 {
    1
}
fn ready_secs() -> f32 {
    0.8
}

/// What happens when a thing goes off.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Burst {
    /// How far it reaches, metres.
    pub radius: f32,
    /// How badly it hurts a body at its heart, 0..1 (it fades to nothing at
    /// `radius`; what is worn softens it).
    #[serde(default)]
    pub hurt: f32,
    /// How hard it throws what is near, m/s at its heart.
    #[serde(default)]
    pub push: f32,
    /// How deep it blows out the ground, metres, and the size of the holes
    /// it tears in what stands close (0: none).
    #[serde(default)]
    pub crater: f32,
    /// How hot it sets what is near, 0..1 (1: things that burn catch).
    #[serde(default)]
    pub fire: f32,
    /// Seconds from being lit (used, or caught by fire) to going off.
    #[serde(default)]
    pub fuse: f32,
    /// It goes off when it hits something hard.
    #[serde(default)]
    pub impact: bool,
}

/// Terrain cut away under a shape: |x| < half[0], |z| < half[1] (local),
/// down to local y = floor.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Hollow {
    pub half: [f32; 2],
    pub floor: f32,
}

/// The pose roles a body can answer to (k.s0 … k.s8). `grip` closes the
/// hands (0 open … 1 a fist; a claw, a fin folding in).
pub const ROLES: [&str; 9] = ["raise_l", "raise_r", "reach_l", "reach_r", "lean", "head", "crouch", "spread", "grip"];

/// Look sliders a body may declare: k.a … k.e, then k.g … k.z, in order.
pub const MAX_SLIDERS: usize = 25;

/// What a body type says about itself (`meta.body`). Lengths are metres at
/// the body's default look and scale 1; the engine scales them with size.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Body {
    /// Standing height.
    pub height: f32,
    pub eye: f32,
    /// Collision radius on the ground.
    pub radius: f32,
    pub reach: f32,
    /// Where a held thing sits (x right, y up, z forward).
    pub grip: [f32; 3],
    /// Where a rider sits, if it can be ridden.
    #[serde(default)]
    pub seat: Option<[f32; 3]>,
    /// Roles the shape answers to (names from ROLES).
    pub roles: Vec<String>,
    /// biped | quad | slither | hover
    pub gait: String,
    pub flies: bool,
    /// Two arms with hands (big things are held in both). Bodies written
    /// from the figure keep its arm maths; other arms hang from above the
    /// grip, which is the right hand at rest.
    pub arms: bool,
    /// Look sliders (name, lo, hi), read as k.a … k.e, then k.g … k.z.
    pub look: Vec<(String, f32, f32)>,
    /// Sliders that are one value unless something says otherwise (what is
    /// worn: most people are dressed), by name; the rest vary per being.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub usual: Vec<(String, f32)>,
    /// The body it was written from (a cat's body from the quadruped, one
    /// cat's reshaped body from the cat's): layers and kin follow the line.
    #[serde(default)]
    pub from: Option<String>,
    /// How it stands at rest, per role (in ROLES order): a stoop, an arm
    /// always half-raised. Added to every pose, so what it wears follows.
    #[serde(default)]
    pub rest: [f32; 8],
    /// How it lies dead, per role (in ROLES order), if not as it stands:
    /// a stoop straightened, so it lies flat once it has fallen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead: Option<[f32; 8]>,
}

impl Default for Body {
    fn default() -> Self {
        Body { height: 1.75, eye: 1.65, radius: 0.35, reach: 2.4, grip: [0.3, 1.0, 0.3], seat: None, roles: Vec::new(), gait: "biped".into(), flies: false, arms: false, look: Vec::new(), usual: Vec::new(), from: None, rest: [0.0; 8], dead: None }
    }
}

impl Body {
    pub fn has_role(&self, r: &str) -> bool {
        self.roles.iter().any(|x| x == r)
    }
    /// A slider's usual value, if it has one.
    pub fn usual(&self, name: &str) -> Option<f32> {
        self.usual.iter().find(|u| u.0 == name).map(|u| u.1)
    }
    /// Slider index (0 = k.a) by name.
    pub fn slider(&self, name: &str) -> Option<usize> {
        self.look.iter().position(|l| l.0 == name)
    }
}

/// Behaviour entry points a type may export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Behavior {
    /// Runs a few times a second while anyone is near.
    Tick,
    /// Runs when someone uses it.
    Use,
    /// Runs when something hits it.
    Touch,
}

impl Behavior {
    pub fn name(self) -> &'static str {
        match self {
            Behavior::Tick => "tick",
            Behavior::Use => "use",
            Behavior::Touch => "touch",
        }
    }
    pub fn from_name(n: &str) -> Option<Behavior> {
        Some(match n {
            "tick" => Behavior::Tick,
            "use" => Behavior::Use,
            "touch" => Behavior::Touch,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Module {
    pub meta: Meta,
    pub sdf: Func,
    pub color: Func,
    pub behaviors: Vec<(Behavior, Func)>,
    /// Property names read or written by behaviour code (`w.<prop>`, `o.<prop>`).
    pub prop_names: Vec<String>,
}
