//! Day/night cycle. One in-game day lasts 20 real minutes.

use super::Lighting;
use crate::terrain::{Palette, rgbv, smoothstep};
use glam::Vec3;

pub const DAY_SECONDS: f64 = 1200.0;

/// 0 = midnight, 0.25 = sunrise, 0.5 = noon, 0.75 = sunset.
pub fn day_phase(t_game: f64) -> f32 {
    (t_game / DAY_SECONDS).rem_euclid(1.0) as f32
}

pub fn time_label(t_game: f64) -> &'static str {
    let p = day_phase(t_game);
    match p {
        p if !(0.21..0.80).contains(&p) => "night",
        p if p < 0.29 => "dawn",
        p if p < 0.44 => "morning",
        p if p < 0.56 => "midday",
        p if p < 0.71 => "afternoon",
        _ => "dusk",
    }
}

/// How long ago something happened, as a person would say it (game time).
pub fn ago(dt_game: f64) -> &'static str {
    let h = dt_game.max(0.0) / (DAY_SECONDS / 24.0);
    match h {
        h if h < 0.25 => "just now",
        h if h < 1.0 => "a little while ago",
        h if h < 4.0 => "earlier",
        h if h < 24.0 => "earlier today",
        h if h < 48.0 => "yesterday",
        _ => "days ago",
    }
}

pub fn is_night(t_game: f64) -> bool {
    time_label(t_game) == "night"
}

pub fn lighting(t_game: f64, pal: &Palette) -> Lighting {
    let p = day_phase(t_game);
    let ang = (p - 0.25) * std::f32::consts::TAU;
    let elev = ang.sin();
    // Sun rises in the east (+x), sets in the west, tilted south.
    let sun = Vec3::new(ang.cos(), elev, -0.35).normalize();
    let daylight = smoothstep(-0.08, 0.3, elev);
    let night = smoothstep(0.05, -0.2, elev);
    let dusk = (1.0 - (elev.abs() / 0.32)).clamp(0.0, 1.0) * (1.0 - night * 0.6);

    let lerp3 = |a: [u8; 3], b: [u8; 3], c: [u8; 3]| -> Vec3 {
        let day = rgbv(a);
        let du = rgbv(b);
        let ni = rgbv(c);
        let base = ni.lerp(day, daylight);
        base.lerp(du, dusk * 0.85)
    };
    let zenith = lerp3(pal.sky_day, pal.sky_dusk, pal.sky_night);
    let horizon = lerp3(pal.horizon_day, pal.horizon_dusk, pal.horizon_night);
    let warm = Vec3::new(1.0, 0.55, 0.3);
    let mut sun_col = rgbv(pal.sun).lerp(warm, dusk * 0.8);
    let mut sun_dir = sun;
    if night > 0.5 {
        // The moon takes over, opposite the sun.
        sun_dir = Vec3::new(-sun.x, (-elev).max(0.15), 0.3).normalize();
        sun_col = Vec3::new(0.32, 0.38, 0.55);
    }
    let ambient = 0.55 + 0.45 * daylight;
    Lighting { sun_dir, daylight, night, sun_col, zenith, horizon, ambient }
}
