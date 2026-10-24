//! Settings the player changes in the game (Settings, from the title or Esc), kept in
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
    /// Characters heard speaking in their own voices (needs sound).
    pub voices: bool,
    /// 0..1: how loud voices are against the world's other sounds.
    pub voice_volume: f32,
    /// Speak to talk (off until turned on in the menu): `/`, or Enter with
    /// someone, listens on the mic and sends what you said when you pause.
    pub speak_to_talk: bool,
    /// In a window: lines of the world drawn, scaled up to fill it; 0 = every
    /// pixel of the window, `AUTO` = as many as keep the frame rate. The
    /// terminal ignores it.
    pub window_detail: u32,
    /// In a window: frames blended over time, each drawn a fraction of a
    /// pixel apart, for about twice the detail standing still.
    pub blend_frames: bool,
    /// What other players call you when you play together.
    pub name: String,
    /// Who you are to the worlds you join (random, made once): a host
    /// knows you again by it and keeps your place, look and things.
    pub token: String,
    /// Hosting: what each guest may spend of your LLM budget in a session
    /// (dollars; 0 = nothing, guests only walk and look).
    pub guest_budget_usd: f64,
    /// Ask GitHub once at start whether a newer release is out.
    pub check_updates: bool,
    /// Where words come from: "anthropic", or "other" (any OpenAI-compatible
    /// endpoint at `base_url`, such as Ollama).
    pub provider: String,
    /// The provider's API key (an environment variable wins).
    pub api_key: String,
    pub base_url: String,
    /// The model for each job (builder, characters, choices, memories); empty: the default.
    pub models: [String; 4],
    /// The keys shown for what you look at, and tips the first few times.
    pub show_hints: bool,
    /// How many times each tip has been shown (they stop after a few).
    pub tips: std::collections::BTreeMap<String, u32>,
    /// The welcome (asking for a key) was seen and answered or skipped.
    pub welcomed: bool,
}

/// Window detail that follows the frame rate.
pub const AUTO: u32 = 1;

/// The window detail steps, in lines (0 = the window's own resolution).
pub const DETAILS: &[u32] = &[AUTO, 180, 240, 360, 480, 720, 1080, 0];

impl Default for Settings {
    fn default() -> Self {
        Settings { budget_usd: 0.0, fps: 60.0, shadows: true, region_radius: 2, show_work: ShowWork::Mine, sound: true, volume: 0.8, voices: false, voice_volume: 0.5, speak_to_talk: false, window_detail: AUTO, blend_frames: true, name: String::new(), token: String::new(), guest_budget_usd: 10.0, check_updates: true, provider: "anthropic".into(), api_key: String::new(), base_url: String::new(), models: Default::default(), show_hints: true, tips: Default::default(), welcomed: false }
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
pub fn env_blend() -> bool {
    std::env::var_os("POCKET_NO_TAA").is_some()
}
pub fn env_sound() -> bool {
    std::env::var_os("POCKET_NO_SOUND").is_some()
}
pub fn env_voices() -> bool {
    std::env::var_os("POCKET_NO_VOICE").is_some()
}
pub fn env_mic() -> bool {
    std::env::var_os("POCKET_NO_MIC").is_some()
}
/// The provider is chosen by the environment (a base URL, or a key).
pub fn env_provider() -> bool {
    env("POCKET_LLM_BASE_URL").is_some() || env("ANTHROPIC_API_KEY").is_some()
}
/// The key comes from the environment for this provider.
pub fn env_key(provider: &str) -> bool {
    if provider == "other" { env("POCKET_LLM_API_KEY").or_else(|| env("OPENAI_API_KEY")).is_some() } else { env("ANTHROPIC_API_KEY").is_some() }
}
/// A job's model is fixed by the environment.
pub fn env_model(i: usize) -> bool {
    const NAMES: [&str; 4] = ["POCKET_MODEL_BUILDER", "POCKET_MODEL_CHARACTER", "POCKET_MODEL_DECIDER", "POCKET_MODEL_SUMMARIZER"];
    env(NAMES[i]).is_some() || env("POCKET_MODEL").is_some()
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
        if env_voices() {
            s.voices = false;
        }
        if env_mic() {
            s.speak_to_talk = false;
        }
        if let Some(d) = env("POCKET_WINDOW_DETAIL").and_then(|v| v.trim().parse::<u32>().ok()) {
            s.window_detail = d;
        }
        if env_blend() {
            s.blend_frames = false;
        }
        // Two players on one machine (trying it out): each its own name and token.
        if let Some(n) = env("POCKET_NAME") {
            s.name = n;
        }
        if let Some(t) = env("POCKET_TOKEN") {
            s.token = t;
        }
        s.guest_budget_usd = if s.guest_budget_usd.is_finite() { s.guest_budget_usd.max(0.0) } else { 10.0 };
        s.volume = if s.volume.is_finite() { s.volume.clamp(0.0, 1.0) } else { 0.8 };
        s.voice_volume = if s.voice_volume.is_finite() { s.voice_volume.clamp(0.0, 1.0) } else { 0.5 };
        s.fps = s.fps.clamp(5.0, 240.0);
        s.region_radius = s.region_radius.clamp(0, 4);
        s
    }

    /// Who you are when playing together: your name (your login's, until
    /// you choose one) and your token (made and saved the first time).
    pub fn player(&mut self) -> (String, String) {
        if self.token.trim().is_empty() {
            self.token = new_token();
            self.save();
        }
        let name = if self.name.trim().is_empty() { default_name() } else { self.name.trim().to_string() };
        (name, self.token.clone())
    }

    /// An LLM can be reached: a key (or another provider's address) is set, here or in the environment.
    pub fn has_llm(&self) -> bool {
        if env("POCKET_LLM_BASE_URL").is_some_and(|b| !b.contains("anthropic.com")) || env("ANTHROPIC_API_KEY").is_some() {
            return true;
        }
        if self.provider == "other" { !self.base_url.trim().is_empty() } else { !self.api_key.trim().is_empty() }
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
        out.voice_volume = self.voice_volume;
        if !env_voices() {
            out.voices = self.voices;
        }
        if !env_mic() {
            out.speak_to_talk = self.speak_to_talk;
        }
        if env("POCKET_WINDOW_DETAIL").is_none() {
            out.window_detail = self.window_detail;
        }
        if !env_blend() {
            out.blend_frames = self.blend_frames;
        }
        if env("POCKET_NAME").is_none() {
            out.name = self.name.clone();
        }
        if env("POCKET_TOKEN").is_none() {
            out.token = self.token.clone();
        }
        out.guest_budget_usd = self.guest_budget_usd;
        out.check_updates = self.check_updates;
        out.provider = self.provider.clone();
        out.api_key = self.api_key.clone();
        out.base_url = self.base_url.clone();
        out.models = self.models.clone();
        out.show_hints = self.show_hints;
        out.tips = self.tips.clone();
        out.welcomed = self.welcomed;
        let _ = std::fs::create_dir_all(crate::log::dir());
        if let Ok(t) = serde_json::to_string_pretty(&out) {
            let _ = std::fs::write(path(), t);
            // It may hold an API key: for this user's eyes only.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(path(), std::fs::Permissions::from_mode(0o600));
            }
        }
    }
}

/// A name until one is chosen: the login's, capitalised, or "Traveler".
pub fn default_name() -> String {
    let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
    let user: String = user.chars().filter(|c| c.is_alphanumeric()).take(20).collect();
    let mut c = user.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => "Traveler".into(),
    }
}

/// 128 random bits, as hex.
pub fn new_token() -> String {
    let mut seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    seed ^= std::process::id() as u64;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut out = String::new();
    for i in 0..2u64 {
        use std::hash::{BuildHasher, Hash, Hasher};
        let mut s = std::collections::hash_map::RandomState::new().build_hasher();
        (seed, i).hash(&mut s);
        (seed, i).hash(&mut h);
        out.push_str(&format!("{:016x}", s.finish() ^ h.finish()));
    }
    out
}
