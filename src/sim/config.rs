//! How much of the world is alive at once. Every distance and budget is a
//! setting: turning them down makes the world quieter, never different in
//! kind, and nothing needs rebuilding to turn them up again.
//!
//! Defaults live here; environment variables override them, and so does the
//! universe's `kv` table (keys `sim.<field>`), so one world can be tuned alone.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum FarMode {
    /// Beyond `medium`, time stands still.
    Frozen,
    /// When the player comes back, run the missed time quickly with rules
    /// only (no LLM), up to `max_hours` game hours.
    CatchUp { max_hours: f32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SimConfig {
    /// Full simulation every frame within this distance of the player (m).
    pub near: f32,
    /// Reduced-rate simulation up to here (m).
    pub medium: f32,
    /// Ticks per second for characters at medium distance.
    pub medium_hz: f32,
    /// May characters at medium distance ask the LLM?
    pub medium_llm: bool,
    /// Global budget of LLM decisions per real minute, shared by all tiers.
    pub llm_per_min: f32,
    pub far: FarMode,
    /// Rule passes per second.
    pub rules_hz: f32,
    /// Behaviour (tick) calls per second for near things.
    pub behavior_hz: f32,
    /// At most this many things move under physics at once.
    pub max_awake: usize,
    /// At most this many flames drawn.
    pub max_flames: usize,
    /// Characters within this distance of the player speak via the LLM; further away they use short lines.
    pub chat_llm_range: f32,
    /// Hard cap on live things (spawning stops beyond it).
    pub max_things: usize,
    /// A character turns to their craft (make, fix, improve) at most this often (s).
    pub work_secs: f32,
    /// ...and across the whole world, at most one character this often (s):
    /// each piece of work may add a type, and every type slows shader builds.
    pub work_gap_secs: f32,
    /// Predators kill what they catch (off: they only chase, and give up).
    pub hunting: bool,
    /// At most this many beings of one species are born in a region.
    pub max_creatures: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            near: 220.0,
            medium: 512.0,
            medium_hz: 2.0,
            medium_llm: true,
            llm_per_min: 12.0,
            far: FarMode::Frozen,
            rules_hz: 4.0,
            behavior_hz: 4.0,
            max_awake: 300,
            max_flames: 48,
            chat_llm_range: 20.0,
            max_things: 5000,
            work_secs: 600.0,
            work_gap_secs: 120.0,
            hunting: false,
            max_creatures: 24,
        }
    }
}

fn env_f(name: &str) -> Option<f32> {
    std::env::var(name).ok().and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite())
}

impl SimConfig {
    /// Defaults, then the universe's kv overrides, then environment variables.
    pub fn load(kv: impl Fn(&str) -> Option<String>) -> SimConfig {
        let mut c = SimConfig::default();
        let set = |key: &str, env: &str, c: &mut SimConfig| {
            let v = kv(&format!("sim.{key}")).and_then(|s| s.trim().parse::<f32>().ok()).or_else(|| env_f(env));
            if let Some(v) = v {
                c.set(key, v);
            }
        };
        for (key, env) in [
            ("near", "POCKET_SIM_NEAR"),
            ("medium", "POCKET_SIM_MEDIUM"),
            ("medium_hz", "POCKET_SIM_MEDIUM_HZ"),
            ("llm_per_min", "POCKET_LLM_PER_MIN"),
            ("rules_hz", "POCKET_SIM_RULES_HZ"),
            ("behavior_hz", "POCKET_SIM_BEHAVIOR_HZ"),
            ("max_awake", "POCKET_SIM_MAX_AWAKE"),
            ("max_flames", "POCKET_SIM_MAX_FLAMES"),
            ("chat_llm_range", "POCKET_SIM_CHAT_RANGE"),
            ("max_things", "POCKET_SIM_MAX_THINGS"),
            ("work_secs", "POCKET_SIM_WORK_SECS"),
            ("work_gap_secs", "POCKET_SIM_WORK_GAP"),
            ("max_creatures", "POCKET_SIM_MAX_CREATURES"),
        ] {
            set(key, env, &mut c);
        }
        let far = kv("sim.far").or_else(|| std::env::var("POCKET_SIM_FAR").ok());
        if let Some(f) = far {
            let f = f.trim().to_lowercase();
            if f == "frozen" {
                c.far = FarMode::Frozen;
            } else if let Some(h) = f.strip_prefix("catchup").or_else(|| f.strip_prefix("catch_up")) {
                let h = h.trim_start_matches([':', '=', ' ']).parse::<f32>().unwrap_or(24.0);
                c.far = FarMode::CatchUp { max_hours: h.clamp(0.1, 24.0 * 30.0) };
            }
        }
        if let Some(v) = kv("sim.hunting").or_else(|| std::env::var("POCKET_SIM_HUNTING").ok()) {
            c.hunting = matches!(v.trim(), "1" | "true" | "yes" | "on");
        }
        let ml = kv("sim.medium_llm").or_else(|| std::env::var("POCKET_SIM_MEDIUM_LLM").ok());
        if let Some(v) = ml {
            c.medium_llm = matches!(v.trim(), "1" | "true" | "yes" | "on");
        }
        c.medium = c.medium.max(c.near);
        c
    }

    pub fn set(&mut self, key: &str, v: f32) {
        match key {
            "near" => self.near = v.clamp(10.0, 5000.0),
            "medium" => self.medium = v.clamp(10.0, 20000.0),
            "medium_hz" => self.medium_hz = v.clamp(0.05, 60.0),
            "llm_per_min" => self.llm_per_min = v.clamp(0.0, 600.0),
            "rules_hz" => self.rules_hz = v.clamp(0.25, 30.0),
            "behavior_hz" => self.behavior_hz = v.clamp(0.25, 30.0),
            "max_awake" => self.max_awake = v.clamp(1.0, 100_000.0) as usize,
            "max_flames" => self.max_flames = v.clamp(0.0, 1000.0) as usize,
            "chat_llm_range" => self.chat_llm_range = v.clamp(0.0, 1000.0),
            "max_things" => self.max_things = v.clamp(10.0, 1_000_000.0) as usize,
            "work_secs" => self.work_secs = v.clamp(10.0, 1e7),
            "work_gap_secs" => self.work_gap_secs = v.clamp(0.0, 1e7),
            "max_creatures" => self.max_creatures = v.clamp(1.0, 10_000.0) as usize,
            "hunting" => self.hunting = v > 0.5,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv_overrides_and_far_modes() {
        let c = SimConfig::load(|k| match k {
            "sim.near" => Some("100".into()),
            "sim.medium" => Some("50".into()),
            "sim.far" => Some("catchup:6".into()),
            "sim.medium_llm" => Some("false".into()),
            _ => None,
        });
        assert_eq!(c.near, 100.0);
        assert_eq!(c.medium, 100.0, "medium is never nearer than near");
        assert_eq!(c.far, FarMode::CatchUp { max_hours: 6.0 });
        assert!(!c.medium_llm);
        let d = SimConfig::load(|_| None);
        assert_eq!(d.far, FarMode::Frozen);
        assert_eq!(d.near, 220.0);
    }
}
