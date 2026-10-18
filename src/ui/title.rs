//! Behind every menu: a land made of words. Hills drift by in layers, their
//! ridges spelled out in letters from places a world could be, a lake
//! shimmers below, and now and then a word lifts off the land and fades into
//! the sky, the way a world here begins: from words.

use crate::term::{Cell, Deco, Face, Label, Screen};

/// What the backdrop keeps between frames: the lines its ridges are written in.
pub struct Backdrop {
    lines: Vec<Vec<char>>,
    words: Vec<String>,
}

fn hash(i: u32, j: u32) -> f32 {
    crate::noise::pcg(i.wrapping_mul(7919) ^ j.wrapping_mul(104_729)) as f32 / u32::MAX as f32
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8)
}

/// A smooth bumpy line: the sum of a few waves.
fn ridge(x: f32, seed: f32) -> f32 {
    (x * 0.031 + seed).sin() * 0.55 + (x * 0.067 + seed * 2.3).sin() * 0.3 + (x * 0.151 + seed * 4.1).sin() * 0.15
}

/// The hills, far to near: (how high, how much they rise and fall, cells a second, colours).
struct Layer {
    base: f32,
    amp: f32,
    speed: f32,
    seed: f32,
    fill: [u8; 3],
    letters: [u8; 3],
}

const LAYERS: [Layer; 3] = [
    Layer { base: 0.50, amp: 0.10, speed: 0.5, seed: 1.3, fill: [36, 34, 62], letters: [92, 86, 140] },
    Layer { base: 0.60, amp: 0.08, speed: 1.1, seed: 4.7, fill: [28, 40, 52], letters: [86, 128, 140] },
    Layer { base: 0.70, amp: 0.07, speed: 2.2, seed: 8.2, fill: [22, 32, 30], letters: [150, 176, 120] },
];

impl Backdrop {
    pub fn new() -> Backdrop {
        let prompts = if cfg!(test) { vec!["salt flats where caravans trade salt for everything.".to_string(); 4] } else { crate::random_world::sample(6) };
        let lines = prompts.iter().map(|p| format!("{} ", p.trim().trim_end_matches('.').to_lowercase()).chars().collect()).collect();
        let mut words: Vec<String> = prompts.iter().flat_map(|p| p.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).collect::<Vec<_>>()).filter(|w| w.chars().count() >= 5).collect();
        words.dedup();
        if words.is_empty() {
            words.push("worlds".into());
        }
        Backdrop { lines, words }
    }

    /// Draw it over the whole screen at time `t` (seconds); `calm` 0..1
    /// darkens it under a panel.
    pub fn draw(&self, s: &mut Screen, t: f32, calm: f32) {
        s.clear_decor();
        let (w, h) = (s.w as usize, s.h as usize);
        let hf = h as f32;
        let shade = |c: [u8; 3]| mix(c, [8, 9, 14], calm * 0.55);
        // Sky: deep at the top, warmer down to the hills.
        let sky_top = [13, 14, 28];
        let sky_low = [46, 34, 58];
        let water_y = (hf * 0.84) as usize;
        for y in 0..h {
            let k = (y as f32 / (hf * 0.75)).min(1.0);
            let bg = shade(mix(sky_top, sky_low, k * k));
            s.fill_row(y as u16, Cell { bg, ..Cell::BLANK });
        }
        // Stars, twinkling.
        for y in 0..(hf * 0.55) as usize {
            for x in 0..w {
                let r = hash(x as u32, y as u32 + 17);
                if r > 0.985 {
                    let tw = ((t * (0.6 + r * 3.0) + r * 40.0).sin() * 0.5 + 0.5) * (1.0 - y as f32 / (hf * 0.6));
                    let bg = s.get(x as u16, y as u16).map_or([0; 3], |c| c.bg);
                    let ch = if r > 0.997 { '✦' } else { '·' };
                    s.set(x as u16, y as u16, Cell { ch, fg: mix(bg, shade([230, 226, 255]), tw), bg, bold: false });
                }
            }
        }
        // The moon, softly glowing.
        let mx = w as f32 * 0.84;
        let my = hf * 0.13;
        // Only where there is room beside the title.
        let room = if w >= 120 { (hf / 45.0).clamp(0.6, 1.3) } else { 0.0 };
        for (k, a) in [(5.5f32 * room, 0.05f32), (3.8 * room, 0.08), (2.4 * room, 1.0)] {
            if k <= 0.0 {
                continue;
            }
            let d = Deco { x: mx - k * 1.0, y: my - k * 0.5, w: k * 2.0, h: k, fill: shade([255, 236, 200]), alpha: a, radius: k * 0.5, shadow: false, border: None };
            s.deco(d);
        }
        // The hills, far to near, each ridge written in letters.
        for (li, l) in LAYERS.iter().enumerate() {
            let line = &self.lines[li % self.lines.len().max(1)];
            let off = t * l.speed;
            let shift = off.floor() as i64;
            for x in 0..w {
                let wx = x as f32 + off.floor();
                let top = ((l.base - ridge(wx, l.seed) * l.amp) * hf).round().max(1.0) as usize;
                for y in top..water_y.min(h) {
                    let depth = (y - top) as f32 / hf;
                    let bg = shade(mix(l.fill, [10, 12, 16], depth * 1.6));
                    let (ch, fg) = if y == top {
                        let i = (x as i64 + shift).rem_euclid(line.len().max(1) as i64) as usize;
                        (line.get(i).copied().unwrap_or(' '), shade(l.letters))
                    } else if hash(x as u32 + shift as u32, y as u32 * 31 + li as u32) > 0.93 {
                        ('.', shade(mix(l.fill, l.letters, 0.35)))
                    } else {
                        (' ', bg)
                    };
                    s.set(x as u16, y as u16, Cell { ch, fg, bg, bold: y == top && li == 2 });
                }
            }
        }
        // The lake: the sky again, rippling.
        for y in water_y..h {
            let k = (y - water_y) as f32 / (hf - water_y as f32).max(1.0);
            let bg = shade(mix([30, 30, 52], [12, 13, 22], k));
            for x in 0..w {
                let r = hash(x as u32, y as u32 * 7 + 3);
                let wave = ((x as f32 * 0.35 + t * 1.6 + y as f32 * 1.7).sin() + r).max(0.0);
                // Under the moon the ripples catch its light.
                let lit = 1.0 - ((x as f32 - mx).abs() / (2.0 + k * 6.0)).min(1.0);
                let (ch, fg) = if lit > 0.0 && wave > 0.9 - lit * 0.5 {
                    ('~', shade(mix(bg, [255, 236, 200], 0.25 + lit * 0.5)))
                } else if wave > 1.35 {
                    ('~', shade(mix(bg, [140, 130, 190], 0.55)))
                } else if wave > 1.2 {
                    ('-', shade(mix(bg, [120, 110, 170], 0.35)))
                } else {
                    (' ', bg)
                };
                s.set(x as u16, y as u16, Cell { ch, fg, bg, bold: false });
            }
        }
        self.words(s, t, calm, [sky_top, sky_low]);
    }

    /// Words lifting off the land and fading into the sky.
    fn words(&self, s: &mut Screen, t: f32, calm: f32, sky: [[u8; 3]; 2]) {
        let (w, hf) = (s.w as usize, s.h as f32);
        let shade = |c: [u8; 3]| mix(c, [8, 9, 14], calm * 0.55);
        let (sky_top, sky_low) = (sky[0], sky[1]);
        let n = 5;
        for i in 0..n {
            let period = 14.0;
            let phase = (t / period + i as f32 / n as f32).fract();
            let cycle = (t / period + i as f32 / n as f32).floor() as u32;
            let word = &self.words[(crate::noise::pcg(cycle * 13 + i as u32) as usize) % self.words.len()];
            let x = 0.08 + hash(cycle, i as u32 * 5 + 1) * 0.84;
            let y = 0.66 - phase * 0.5;
            let fade = (phase * 5.0).min(1.0) * (1.0 - phase).powf(1.5);
            let bg = mix(sky_top, sky_low, (y / 0.75).powi(2));
            s.labels.push(Label { x: x * w as f32, y: y * hf, text: word.clone(), size: 1.0 + phase * 0.4, color: shade(mix(bg, [255, 226, 170], fade * 0.8)), face: Face::Title, center: true });
        }
    }

    /// Your world behind instead (the panorama's frame shows through every
    /// cell), darkened for the words on it; `shown` 0 → 1 fades it in.
    pub fn draw_world(&self, s: &mut Screen, t: f32, calm: f32, shown: f32) {
        s.clear_decor();
        s.fill_world(s.h);
        let (w, h) = (s.w as f32, s.h as f32);
        // Dimmed for the words on it (more under a panel), and dark until it fades in.
        let base = 0.3 + calm * 0.32;
        let alpha = base + (1.0 - base) * (1.0 - shown);
        s.deco(Deco { x: 0.0, y: 0.0, w, h, fill: [6, 8, 14], alpha, radius: 0.0, shadow: false, border: None });
        let _ = t;
    }
}

/// Keep the backdrop behind what lies over it: no letters of it in panels
/// and buttons. `under` is how many labels the backdrop set (they come first).
pub fn tidy(s: &mut Screen, under: usize) {
    let solid: Vec<Deco> = s.decor.iter().filter(|d| d.alpha >= 0.9 && d.w > 2.0).copied().collect();
    let hits = |x: f32, y: f32, w: f32, h: f32| solid.iter().any(|d| x < d.x + d.w && x + w > d.x && y < d.y + d.h && y + h > d.y);
    for y in 0..s.h {
        for x in 0..s.w {
            if hits(x as f32, y as f32, 1.0, 1.0) {
                s.over(x, y, |c| Cell { ch: ' ', ..c });
            }
        }
    }
    let mut i = 0;
    s.labels.retain(|l| {
        i += 1;
        if i > under {
            return true;
        }
        let n = l.text.chars().count() as f32 * l.size * 0.9;
        let x = if l.center { l.x - n * 0.5 } else { l.x };
        let y = if l.center { l.y - l.size * 0.5 } else { l.y };
        !hits(x, y, n, l.size)
    });
}

impl Default for Backdrop {
    fn default() -> Self {
        Backdrop::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_at_any_size() {
        let b = Backdrop::new();
        for (w, h) in [(20, 10), (80, 24), (160, 50)] {
            let mut s = Screen::new(w, h, true);
            b.draw(&mut s, 3.7, 0.0);
            b.draw(&mut s, 1000.0, 1.0);
            assert!(s.cells.iter().any(|c| c.ch.is_alphabetic()), "ridges in letters");
        }
    }
}
