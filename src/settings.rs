//! Settings the player changes in the game (Esc / F10), kept in
//! ~/.pocket/settings.json. An environment variable still wins for its run,
//! and the screen shows those as set by env. World settings (creature limits,
//! life speed…) belong to one world and live in its own `kv` table instead.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Spending cap per session in dollars; 0 = no limit.
    pub budget_usd: f64,
    pub fps: f32,
    pub shadows: bool,
    /// Regions loaded around you (each way).
    pub region_radius: i32,
    /// Whose slow work (deeds, makings) the "working on" box lists.
    pub show_work: ShowWork,
    /// Sound on (it turns itself off when there is no output device).
    pub sound: bool,
    /// 0..1.
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { budget_usd: 0.0, fps: 60.0, shadows: true, region_radius: 2, show_work: ShowWork::Mine, sound: true, volume: 0.8 }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShowWork {
    /// What you started.
    Mine,
    /// Yours, the characters' and the land's.
    Everyone,
    Off,
}

impl ShowWork {
    pub fn next(self, dir: i32) -> ShowWork {
        const ALL: [ShowWork; 3] = [ShowWork::Mine, ShowWork::Everyone, ShowWork::Off];
        let i = ALL.iter().position(|x| *x == self).unwrap_or(0) as i32;
        ALL[(i + dir).rem_euclid(ALL.len() as i32) as usize]
    }

    pub fn name(self) -> &'static str {
        match self {
            ShowWork::Mine => "mine",
            ShowWork::Everyone => "everyone's",
            ShowWork::Off => "off",
        }
    }
}

fn path() -> std::path::PathBuf {
    crate::log::dir().join("settings.json")
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Which settings an environment variable fixes for this run.
pub fn env_budget() -> bool {
    env("POCKET_BUDGET_USD").is_some()
}
pub fn env_fps() -> bool {
    env("POCKET_FPS").is_some()
}
pub fn env_shadows() -> bool {
    std::env::var_os("POCKET_NO_SHADOWS").is_some()
}
pub fn env_region_radius() -> bool {
    env("POCKET_REGION_RADIUS").is_some()
}
pub fn env_sound() -> bool {
    std::env::var_os("POCKET_NO_SOUND").is_some()
}

impl Settings {
    /// The saved settings, with environment variables on top.
    pub fn load() -> Settings {
        // Tests never see (or write) the player's own settings.
        let saved = if cfg!(test) { None } else { std::fs::read_to_string(path()).ok() };
        let mut s: Settings = saved.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if let Some(b) = env("POCKET_BUDGET_USD").and_then(|v| v.trim().parse::<f64>().ok()) {
            s.budget_usd = b.max(0.0);
        }
        if let Some(f) = env("POCKET_FPS").and_then(|v| v.trim().parse::<f32>().ok()) {
            s.fps = f;
        }
        if env_shadows() {
            s.shadows = false;
        }
        if let Some(r) = env("POCKET_REGION_RADIUS").and_then(|v| v.trim().parse::<i32>().ok()) {
            s.region_radius = r;
        }
        if env_sound() {
            s.sound = false;
        }
        s.volume = if s.volume.is_finite() { s.volume.clamp(0.0, 1.0) } else { 0.8 };
        s.fps = s.fps.clamp(5.0, 240.0);
        s.region_radius = s.region_radius.clamp(0, 4);
        s
    }

    pub fn budget(&self) -> Option<f64> {
        (self.budget_usd > 0.0).then_some(self.budget_usd)
    }

    /// Save, leaving values an environment variable set this run as they were
    /// in the file.
    pub fn save(&self) {
        if cfg!(test) {
            return;
        }
        let mut out: Settings = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if !env_budget() {
            out.budget_usd = self.budget_usd;
        }
        if !env_fps() {
            out.fps = self.fps;
        }
        if !env_shadows() {
            out.shadows = self.shadows;
        }
        if !env_region_radius() {
            out.region_radius = self.region_radius;
        }
        out.show_work = self.show_work;
        if !env_sound() {
            out.sound = self.sound;
        }
        out.volume = self.volume;
        let _ = std::fs::create_dir_all(crate::log::dir());
        if let Ok(t) = serde_json::to_string_pretty(&out) {
            let _ = std::fs::write(path(), t);
        }
    }
}
