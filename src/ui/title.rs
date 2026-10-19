//! Behind every menu until Maple Cove has loaded (and with no GPU at all):
//! the cove drawn in letters at dusk. The sun goes down over the sea, its
//! light rippling towards the shore, the lighthouse on the point blinks,
//! windows light up in the cottages by the pier, and maple leaves drift
//! down past it all.

use crate::term::{Cell, Deco, Screen};

/// The drawn cove (it keeps nothing between frames: all comes from the time).
pub struct Backdrop;

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

const SKY: [[u8; 3]; 3] = [[18, 15, 36], [74, 42, 76], [222, 126, 78]];
const SUN: [u8; 3] = [255, 214, 150];
const LAND: [u8; 3] = [30, 22, 30];
const WINDOW: [u8; 3] = [255, 196, 110];
const LEAVES: [[u8; 3]; 4] = [[214, 74, 38], [238, 150, 46], [180, 44, 40], [226, 186, 70]];

impl Backdrop {
    pub fn new() -> Backdrop {
        Backdrop
    }

    /// Draw it over the whole screen at time `t` (seconds); `calm` 0..1
    /// darkens it under a panel.
    pub fn draw(&self, s: &mut Screen, t: f32, calm: f32) {
        s.clear_decor();
        let (w, h) = (s.w as usize, s.h as usize);
        let (wf, hf) = (w as f32, h as f32);
        let shade = |c: [u8; 3]| mix(c, [8, 9, 14], calm * 0.55);
        let horizon = ((hf * 0.6) as usize).max(1);
        let shore = ((hf * 0.84) as usize).clamp(horizon + 1, h);
        let sun_x = wf * 0.3;
        let sun_r = (hf / 12.0).clamp(1.0, 5.0);
        // The sky: night above, the sunset's glow down to the sea.
        for y in 0..h.min(horizon) {
            let k = y as f32 / horizon as f32;
            let bg = if k < 0.55 { mix(SKY[0], SKY[1], k / 0.55) } else { mix(SKY[1], SKY[2], ((k - 0.55) / 0.45).powf(1.4)) };
            s.fill_row(y as u16, Cell { bg: shade(bg), ..Cell::BLANK });
            for x in 0..w {
                // The glow round the sun, and the sun itself, half set.
                let (dx, dy) = ((x as f32 + 0.5 - sun_x) / 2.0, y as f32 + 0.5 - horizon as f32);
                let d = (dx * dx + dy * dy).sqrt() / sun_r;
                if d < 6.0 {
                    let c = if d < 1.0 { mix(SUN, [255, 236, 190], 1.0 - d) } else { mix(bg, [246, 168, 96], ((6.0 - d) / 5.0).powi(2) * 0.8) };
                    s.set(x as u16, y as u16, Cell { ch: ' ', fg: c, bg: shade(c), bold: false });
                    continue;
                }
                let bg = s.get(x as u16, y as u16).map_or(bg, |c| c.bg);
                let r = hash(x as u32, y as u32 + 17);
                // The first stars, high up.
                if k < 0.4 && r > 0.988 {
                    let tw = ((t * (0.6 + r * 3.0) + r * 40.0).sin() * 0.5 + 0.5) * (1.0 - k / 0.4);
                    s.set(x as u16, y as u16, Cell { ch: '·', fg: shade(mix(bg, [240, 232, 255], tw)), bg, bold: false });
                }
            }
        }
        // Two gulls going home.
        for i in 0..2u32 {
            let gx = ((t * (1.6 + i as f32 * 0.5) + i as f32 * 37.0) % (wf + 20.0)) - 10.0;
            let gy = hf * (0.2 + i as f32 * 0.07) + (t * 1.3 + i as f32).sin() * 0.6;
            if gx >= 0.0 && gx < wf {
                let bg = s.get(gx as u16, gy as u16).map_or([0; 3], |c| c.bg);
                let ch = if (t * 3.0 + i as f32).sin() > 0.0 { 'v' } else { '-' };
                s.set(gx as u16, gy as u16, Cell { ch, fg: shade([40, 30, 50]), bg, bold: true });
            }
        }
        // The point on the right, with its lighthouse.
        let cape = |x: f32| ((0.72 * wf - x) / (0.2 * wf)).max(-1.0).min(0.0).abs().powf(0.6) * (hf * 0.1) + ridge(x, 2.0) * hf * 0.01;
        let light_x = (wf * 0.86) as usize;
        let mut light_top = horizon;
        for x in 0..w {
            let rise = cape(x as f32);
            if rise < 0.5 {
                continue;
            }
            let top = (horizon as f32 - rise).round().max(0.0) as usize;
            for y in top..horizon {
                let fg = shade(mix(LAND, [70, 44, 60], 0.6));
                let ch = if y == top && hash(x as u32, 3) > 0.6 { '^' } else { ' ' };
                s.set(x as u16, y as u16, Cell { ch, fg, bg: shade(mix(LAND, SKY[1], 0.25)), bold: false });
            }
            if x == light_x {
                light_top = top;
            }
        }
        if light_x < w && light_top >= 4 {
            for (i, ch) in ['█', '█', '█', '▄'].iter().enumerate() {
                let y = light_top - 1 - i;
                let fg = if i % 2 == 0 { [232, 226, 214] } else { [190, 60, 54] };
                let bg = s.get(light_x as u16, y as u16).map_or([0; 3], |c| c.bg);
                s.set(light_x as u16, y as u16, Cell { ch: *ch, fg: shade(fg), bg, bold: false });
            }
            // The lamp, turning: bright each time it faces this way.
            let beam = ((t * 0.9).sin() * 0.5 + 0.5).powi(6);
            let ly = light_top as f32 - 4.0;
            for (r, a) in [(3.0f32, 0.16 * beam), (1.1, 0.35 + 0.65 * beam)] {
                s.deco(Deco { x: light_x as f32 + 0.5 - r, y: ly - r * 0.5, w: r * 2.0, h: r, fill: shade([255, 240, 190]), alpha: a, radius: r * 0.5, shadow: false, border: None });
            }
        }
        // The sea: the sky again, darker, the sun's road on it.
        for y in horizon..shore {
            let k = (y - horizon) as f32 / (shore - horizon) as f32;
            let bg = shade(mix([96, 54, 74], [22, 22, 42], k.powf(0.7)));
            for x in 0..w {
                let r = hash(x as u32, y as u32 * 7 + 3);
                let wave = ((x as f32 * 0.35 - t * 1.4 + y as f32 * 1.7).sin() + r).max(0.0);
                let lit = 1.0 - ((x as f32 - sun_x).abs() / (sun_r * 2.0 + k * wf * 0.12)).min(1.0);
                let (ch, fg) = if lit > 0.0 && wave > 1.0 - lit * 0.9 {
                    (if wave > 1.2 { '~' } else { '-' }, shade(mix([190, 110, 90], SUN, 0.4 + lit * 0.6)))
                } else if wave > 1.4 {
                    ('~', shade(mix(bg, [170, 120, 150], 0.45)))
                } else if wave > 1.28 {
                    ('-', shade(mix(bg, [150, 110, 140], 0.3)))
                } else {
                    (' ', bg)
                };
                s.set(x as u16, y as u16, Cell { ch, fg, bg, bold: false });
            }
        }
        // The near shore: the landing, the pier going out, cottages and maples.
        for y in shore..h {
            let k = (y - shore) as f32 / (h - shore).max(1) as f32;
            s.fill_row(y as u16, Cell { bg: shade(mix(LAND, [12, 10, 14], k)), ..Cell::BLANK });
        }
        let pier_y = shore.saturating_sub(((hf * 0.06) as usize).max(1));
        let (p0, p1) = ((wf * 0.44) as usize, (wf * 0.6) as usize);
        for x in p0..p1.min(w) {
            let bg = s.get(x as u16, pier_y as u16).map_or([0; 3], |c| c.bg);
            s.set(x as u16, pier_y as u16, Cell { ch: '▄', fg: shade([46, 32, 34]), bg, bold: false });
            if (x - p0) % 4 == 0 {
                for y in pier_y + 1..shore {
                    let bg = s.get(x as u16, y as u16).map_or([0; 3], |c| c.bg);
                    s.set(x as u16, y as u16, Cell { ch: '│', fg: shade([40, 28, 32]), bg, bold: false });
                }
            }
        }
        // Cottages along the shore, bright roofs in the dusk, a lit window or two.
        const ROOFS: [[u8; 3]; 5] = [[176, 52, 44], [52, 84, 160], [46, 120, 84], [196, 146, 52], [150, 60, 110]];
        let mut i = 0u32;
        let mut x = (wf * 0.03) as usize;
        let tall = if hf > 30.0 { 3 } else { 2 };
        while x + 12 < w {
            if x + 12 > p0 && x < p1 + 2 {
                x = p1 + 3;
                continue;
            }
            let cw = 9 + 2 * (hash(i, 1) * 3.0) as usize;
            let rh = cw / 4;
            let eaves = shore.saturating_sub(tall);
            let roof = eaves.saturating_sub(rh);
            let wall = shade(mix(LAND, [70, 58, 60], 0.35));
            let rc = shade(mix(ROOFS[i as usize % ROOFS.len()], [30, 20, 30], 0.45));
            for ry in 0..rh {
                let y = (roof + ry) as u16;
                let inset = (rh - 1 - ry) * 2;
                for dx in inset..cw.saturating_sub(inset) {
                    let xx = (x + dx) as u16;
                    let bg = s.get(xx, y).map_or([0; 3], |c| c.bg);
                    let c = if dx == inset { Cell { ch: '◢', fg: rc, bg, bold: false } } else if dx + 1 == cw - inset { Cell { ch: '◣', fg: rc, bg, bold: false } } else { Cell { ch: ' ', fg: rc, bg: rc, bold: false } };
                    s.set(xx, y, c);
                }
            }
            for y in eaves..shore.min(h) {
                for dx in 1..cw - 1 {
                    let lit = (dx == 3 || dx == cw - 4) && y == eaves + tall / 2;
                    let on = (t * 0.05 + hash(i, dx as u32)).fract() > 0.15;
                    let c = if lit && on { Cell { ch: '▪', fg: shade(WINDOW), bg: wall, bold: false } } else { Cell { ch: ' ', fg: wall, bg: wall, bold: false } };
                    s.set((x + dx) as u16, y as u16, c);
                }
            }
            // A maple beside it, round and red.
            let tx = x + cw + 1;
            if tx + 6 < w && hash(i, 9) > 0.3 {
                let leaf = LEAVES[(hash(i, 5) * 4.0) as usize % 4];
                let rows = [" ▄██▄ ", "██████", " ▀██▀ ", "  ▐▌  "];
                for (dy, row) in rows.iter().enumerate() {
                    let y = (shore + dy).saturating_sub(rows.len());
                    for (dx, ch) in row.chars().enumerate() {
                        if ch == ' ' {
                            continue;
                        }
                        let xx = (tx + dx) as u16;
                        let bg = s.get(xx, y as u16).map_or([0; 3], |c| c.bg);
                        let fg = if dy == 3 { shade([36, 24, 22]) } else { shade(mix(leaf, [20, 10, 10], 0.4 + dy as f32 * 0.08)) };
                        s.set(xx, y as u16, Cell { ch, fg, bg, bold: false });
                    }
                }
            }
            x += cw + 8 + (hash(i, 2) * 8.0) as usize;
            i += 1;
        }
        // Leaves drifting down across it all.
        let n = (w * h / 90).clamp(6, 90);
        for j in 0..n as u32 {
            let speed = 1.2 + hash(j, 11) * 1.6;
            let fall = (hash(j, 12) * hf + t * speed) % (hf + 4.0) - 2.0;
            let sway = (t * (0.5 + hash(j, 13)) + j as f32).sin() * 2.5;
            let lx = (hash(j, 14) * wf + t * 1.1 + sway).rem_euclid(wf);
            if fall < 0.0 || fall >= hf {
                continue;
            }
            let (xx, yy) = (lx as u16, fall as u16);
            let bg = s.get(xx, yy).map_or([0; 3], |c| c.bg);
            let ch = ['*', '\'', ',', '*', '`'][((t * 2.0 + j as f32) as usize) % 5];
            s.set(xx, yy, Cell { ch, fg: shade(LEAVES[j as usize % 4]), bg, bold: true });
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
            assert!(s.cells.iter().filter(|c| c.ch != ' ').count() > 10, "drawn in letters");
        }
    }
}
