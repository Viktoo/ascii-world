//! What is stored about a character between sessions, and the shape of a
//! decision about what to do next. The characters themselves live in
//! `sim::npc`.

use serde::{Deserialize, Serialize};

pub const TALK_RANGE: f32 = 4.0;

/// Needs, 0 = satisfied … 1 = urgent.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Needs {
    #[serde(default)]
    pub hunger: f32,
    #[serde(default)]
    pub fatigue: f32,
    #[serde(default)]
    pub social: f32,
    #[serde(default)]
    pub fun: f32,
    #[serde(default)]
    pub curiosity: f32,
}

impl Default for Needs {
    fn default() -> Self {
        Needs { hunger: 0.2, fatigue: 0.1, social: 0.3, fun: 0.3, curiosity: 0.2 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SavedState {
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    #[serde(default)]
    pub asleep: bool,
    #[serde(default)]
    pub needs: Option<Needs>,
    #[serde(default)]
    pub held: Option<i64>,
    #[serde(default)]
    pub goal: String,
    /// Game time of the last simulation step (for catching up).
    #[serde(default)]
    pub t: f64,
    /// Killed (by a predator, when the universe hunts): no longer in the world.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dead: bool,
    /// Their layers were put on once (they may have taken them off since).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dressed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tricks: Vec<String>,
    /// Game time of birth (born in the world), 0 for everyone else.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub born: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<i64>,
    /// The founder of their line (their own id for founders; 0 = unknown).
    #[serde(default, skip_serializing_if = "is_zero_i")]
    pub lineage: i64,
    /// When they last had young.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub last_birth: f64,
    /// How often they have run for their lives.
    #[serde(default, skip_serializing_if = "is_zero_u")]
    pub frights: u32,
    /// How used to wonders they are (fades with time).
    #[serde(default, skip_serializing_if = "is_zero_f")]
    pub habit: f32,
    /// What they were in the middle of (plan, mission, favours), in the
    /// simulation's own shape (`sim::needs::Work`), so it carries on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<serde_json::Value>,
}

fn is_zero_i(v: &i64) -> bool {
    *v == 0
}

fn is_zero_u(v: &u32) -> bool {
    *v == 0
}

fn is_zero_f(v: &f32) -> bool {
    *v == 0.0
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// What a character decided to do about an event: either a legacy single
/// action ("approach", "watch", "go_home", "ignore" + a line), or a goal with
/// steps (actions of the shared set).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Decision {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub line: Option<String>,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub say: Option<String>,
    #[serde(default)]
    pub steps: Vec<serde_json::Value>,
}
