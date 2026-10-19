//! What stands behind a screen that has nothing of its own behind it (the
//! title until Maple Cove has come, opening a world, notices, and with no
//! GPU at all): the cosmos (see `cosmos`), seen through every cell.

use super::cosmos::Cosmos;
use crate::render::Frame;
use crate::term::{Cell, Deco, Screen};

pub struct Backdrop {
    cosmos: Cosmos,
}

impl Backdrop {
    pub fn new() -> Backdrop {
        Backdrop { cosmos: Cosmos::new() }
    }

    /// The picture at time `t`, for a window `px` pixels big.
    pub fn frame(&mut self, px: (f32, f32), t: f32) -> &Frame {
        self.cosmos.frame(px, t)
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
