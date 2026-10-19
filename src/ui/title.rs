//! The stock backdrop, wherever a screen has nothing of its own behind it
//! (the title until Maple Cove has come, opening a world, notices, and with
//! no GPU at all): a night sky drawn pixel by pixel, deep stars of every
//! brightness and colour, a few of them twinkling, and in the middle a soft
//! light slowly breathing, faint wisps shifting in it, like something about
//! to begin. Drawn on the CPU as a picture behind the cells; most of it is
//! worked out once per size, so a frame is a few additions a pixel.

use crate::render::Frame;
use crate::term::{Cell, Deco, Screen};

/// The widest it is drawn (the window scales it up, softly).
const MAX_W: f32 = 960.0;
/// Seconds of one breath of the light.
const BREATH: f32 = 7.0;

pub struct Backdrop {
    size: (u32, u32),
    /// The still sky: the dark, the faint stars, the haze of the far ones.
    base: Vec<[f32; 3]>,
    /// The light: its core, and its glow lit through two wisp fields that
    /// fade into each other.
    core: Vec<[f32; 3]>,
    glow: [Vec<[f32; 3]>; 2],
    /// Stars bright enough to twinkle: where, their colour and brightness.
    bright: Vec<(f32, f32, [f32; 3], f32)>,
    frame: Frame,
}

fn hash(i: u32, j: u32) -> f32 {
    crate::noise::pcg(i.wrapping_mul(7919) ^ j.wrapping_mul(104_729)) as f32 / u32::MAX as f32
}

/// A star's colour, from blue-white to deep orange (most nearly white).
fn star_color(u: f32) -> [f32; 3] {
    const C: [[f32; 3]; 5] = [[0.66, 0.76, 1.0], [0.84, 0.89, 1.0], [1.0, 0.98, 0.94], [1.0, 0.88, 0.72], [1.0, 0.72, 0.52]];
    let w = [0.12, 0.3, 0.33, 0.17, 0.08];
    let mut acc = 0.0;
    for (c, w) in C.iter().zip(w) {
        acc += w;
        if u < acc {
            return *c;
        }
    }
    C[4]
}

/// Add a point of light at (x, y), spread over the pixels round it.
fn splat(img: &mut [[f32; 3]], w: u32, h: u32, x: f32, y: f32, c: [f32; 3], b: f32, sigma: f32) {
    let r = (sigma * 2.5).ceil() as i32;
    let (cx, cy) = (x.floor() as i32, y.floor() as i32);
    for dy in -r..=r {
        for dx in -r..=r {
            let (px, py) = (cx + dx, cy + dy);
            if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                continue;
            }
            let (ex, ey) = (px as f32 + 0.5 - x, py as f32 + 0.5 - y);
            let k = b * (-(ex * ex + ey * ey) / (2.0 * sigma * sigma)).exp();
            let p = &mut img[(py as u32 * w + px as u32) as usize];
            for i in 0..3 {
                p[i] += c[i] * k;
            }
        }
    }
}

impl Backdrop {
    pub fn new() -> Backdrop {
        Backdrop { size: (0, 0), base: Vec::new(), core: Vec::new(), glow: [Vec::new(), Vec::new()], bright: Vec::new(), frame: Frame::default() }
    }

    /// Work out the still parts for a picture `w` × `h`.
    fn make(&mut self, w: u32, h: u32) {
        let n = (w * h) as usize;
        let (wf, hf) = (w as f32, h as f32);
        let mut base = vec![[0.0f32; 3]; n];
        let mut core = vec![[0.0f32; 3]; n];
        let mut glow = [vec![[0.0f32; 3]; n], vec![[0.0f32; 3]; n]];
        let (cx, cy) = (wf * 0.5, hf * 0.4);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let (u, v) = ((x as f32 - cx) / hf, (y as f32 - cy) / hf);
                let d2 = u * u + v * v;
                // The dark: deep blue-black, a little lighter low down.
                let low = (y as f32 / hf).powi(2);
                let mut c = [0.010 + low * 0.006, 0.012 + low * 0.008, 0.026 + low * 0.012];
                // A faint band of far stars across the sky.
                let band = (u * 0.42 + v + 0.18).abs();
                let haze = (-(band * band) / 0.02).exp() * (0.5 + 0.5 * crate::noise::fbm2(x as f32 / 90.0, y as f32 / 90.0, 7, 4)).max(0.0);
                for (k, t) in c.iter_mut().zip([0.020, 0.020, 0.030]) {
                    *k += haze * t;
                }
                base[i] = c;
                // The light: a small bright core in a wide soft glow, warm at
                // its heart and cooler out.
                let core_k = (-d2 / 0.0018).exp() * 0.55 + (-d2 / 0.012).exp() * 0.25;
                let glow_k = 1.0 / (1.0 + d2 / 0.04) * 0.3 * (-d2 / 0.4).exp();
                let warm = (-d2 / 0.02).exp();
                let col = [0.62 + 0.38 * warm, 0.56 + 0.30 * warm, 0.86 - 0.26 * warm];
                core[i] = col.map(|c| c * core_k);
                for (j, g) in glow.iter_mut().enumerate() {
                    let wisp = 0.5 + 0.5 * crate::noise::fbm2(x as f32 / (hf * 0.12) + j as f32 * 17.0, y as f32 / (hf * 0.12), 31 + j as u32, 4);
                    g[i] = col.map(|c| c * glow_k * (0.35 + 1.3 * wisp * wisp));
                }
            }
        }
        // Stars: most faint, a few bright; fewer where the light is.
        let count = (w * h / 260).max(40);
        let mut bright = Vec::new();
        for s in 0..count {
            let (x, y) = (hash(s, 1) * wf, hash(s, 2) * hf);
            let m = hash(s, 3).powf(7.0);
            let b = 0.06 + m * 1.6;
            let c = star_color(hash(s, 4));
            let (u, v) = ((x - cx) / hf, (y - cy) / hf);
            let near = (-(u * u + v * v) / 0.01).exp();
            let b = b * (1.0 - near * 0.8);
            if m > 0.12 {
                bright.push((x, y, c, b));
            } else {
                splat(&mut base, w, h, x, y, c, b, 0.5);
            }
        }
        // More, tinier ones in the band.
        for s in 0..count / 2 {
            let t = hash(s, 7) * 2.4 - 1.2;
            let off = (hash(s, 8) - 0.5) * 0.24;
            let (u, v) = (t, -0.18 - t * 0.42 + off);
            let (x, y) = (cx + u * hf, cy + v * hf);
            if x >= 0.0 && y >= 0.0 && x < wf && y < hf {
                splat(&mut base, w, h, x, y, star_color(hash(s, 9)), 0.05 + hash(s, 10) * 0.12, 0.45);
            }
        }
        self.size = (w, h);
        self.base = base;
        self.core = core;
        self.glow = glow;
        self.bright = bright;
    }

    /// The picture at time `t`, for a window `px` pixels big.
    pub fn frame(&mut self, px: (f32, f32), t: f32) -> &Frame {
        let k = (MAX_W / px.0.max(1.0)).min(1.0);
        let (w, h) = (((px.0 * k).round() as u32).max(8), ((px.1 * k).round() as u32).max(8));
        if self.size != (w, h) {
            self.make(w, h);
        }
        // The light breathes in slowly and out more slowly still; the wisps
        // drift from one shape to the other.
        let phase = (t / BREATH).fract();
        let breath = (std::f32::consts::PI * phase).sin().powf(1.6);
        let ic = 0.7 + 0.45 * breath;
        let ig = 0.75 + 0.35 * breath;
        let mix = 0.5 + 0.5 * (t * 0.07).sin();
        let mut img: Vec<[f32; 3]> = Vec::with_capacity(self.base.len());
        for i in 0..self.base.len() {
            let (b, c, g0, g1) = (self.base[i], self.core[i], self.glow[0][i], self.glow[1][i]);
            img.push([0, 1, 2].map(|k| b[k] + c[k] * ic + (g0[k] * (1.0 - mix) + g1[k] * mix) * ig));
        }
        for (s, &(x, y, c, b)) in self.bright.iter().enumerate() {
            // Twinkling: a quick, small flicker, each star its own.
            let r = hash(s as u32, 5);
            let tw = 0.82 + 0.18 * ((t * (1.3 + r * 2.5) + r * 50.0).sin() * (t * (0.7 + r) + r * 9.0).sin());
            splat(&mut img, w, h, x, y, c, b * tw, 0.55);
            if b > 0.6 {
                // The brightest wear a faint halo.
                splat(&mut img, w, h, x, y, c, b * 0.05 * tw, 2.2);
            }
        }
        let f = &mut self.frame;
        f.width = w;
        f.height = h;
        f.pixels.clear();
        for (i, p) in img.iter().enumerate() {
            // A soft shoulder for the bright, and a little noise against banding in the dark.
            let dither = (hash(i as u32, 77) - 0.5) / 255.0;
            let v = p.map(|c| ((1.0 - (-c * 1.15).exp()) + dither).clamp(0.0, 1.0));
            let [r, g, b] = v.map(|c| (c.powf(1.0 / 1.25) * 255.0) as u32);
            f.pixels.push(r | (g << 8) | (b << 16) | (255 << 24));
        }
        f.id = f.id.wrapping_add(1);
        &self.frame
    }

    /// Ready the screen for the picture behind it (every cell shows it);
    /// `calm` 0..1 darkens it under a panel.
    pub fn draw(&self, s: &mut Screen, _t: f32, calm: f32) {
        s.clear_decor();
        s.fill_world(s.h);
        if calm > 0.0 {
            s.deco(Deco { x: 0.0, y: 0.0, w: s.w as f32, h: s.h as f32, fill: [4, 5, 10], alpha: calm * 0.35, radius: 0.0, shadow: false, border: None });
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
        let mut b = Backdrop::new();
        for (w, h) in [(40.0, 20.0), (320.0, 200.0)] {
            let f = b.frame((w, h), 3.7);
            assert_eq!(f.pixels.len(), (f.width * f.height) as usize);
            let at = |x: u32, y: u32| crate::render::unpack(f.pixels[(y * f.width + x) as usize]).iter().map(|&c| c as u32).sum::<u32>();
            assert!(at(f.width / 2, f.height * 40 / 100) > at(0, 0) + 60, "the light in the middle");
            let mut s = Screen::new(30, 10, true);
            b.draw(&mut s, 3.7, 1.0);
        }
    }
}
