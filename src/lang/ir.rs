//! Typed IR produced by the allowlist checker. Both translators consume this;
//! nothing downstream ever sees the JS AST.

use super::api::{Api, Ty};

/// Instance parameter fields readable as `k.<name>`: 8 static parameters,
/// then the 8 live state slots (`s0`..`s7`, written by behaviour code).
pub const K_FIELDS: [&str; 16] = ["seed", "scale", "a", "b", "c", "d", "e", "f", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7"];
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
}

/// The pose roles a body can answer to (k.s0 … k.s7).
pub const ROLES: [&str; 8] = ["raise_l", "raise_r", "reach_l", "reach_r", "lean", "head", "crouch", "spread"];

/// Look sliders a body may declare: k.a … k.e, in order.
pub const MAX_SLIDERS: usize = 5;

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
    /// Look sliders (name, lo, hi), read as k.a … k.e.
    pub look: Vec<(String, f32, f32)>,
    /// The body it was written from (a cat's body from the quadruped, one
    /// cat's reshaped body from the cat's): layers and kin follow the line.
    #[serde(default)]
    pub from: Option<String>,
}

impl Default for Body {
    fn default() -> Self {
        Body { height: 1.75, eye: 1.65, radius: 0.35, reach: 2.4, grip: [0.3, 1.0, 0.3], seat: None, roles: Vec::new(), gait: "biped".into(), flies: false, arms: false, look: Vec::new(), from: None }
    }
}

impl Body {
    pub fn has_role(&self, r: &str) -> bool {
        self.roles.iter().any(|x| x == r)
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
