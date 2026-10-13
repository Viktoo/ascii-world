//! Text for the window: the screen's cells read back as runs of words and set
//! in a proportional sans-serif (Inter), each run starting at its cell. Block
//! and box characters are not text: the grid shader draws them as shapes.

use crate::term::{Cell, Screen};
use std::collections::HashMap;

static REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Medium.ttf");
static BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.ttf");

/// Symbol fonts tried, in order, for what Inter lacks (when present here).
const FALLBACKS: &[&str] = &[
    "/System/Library/Fonts/Apple Symbols.ttf",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "C:\\Windows\\Fonts\\seguisym.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

pub const ATLAS: u32 = 2048;

/// One glyph to draw: where (window pixels), which part of the atlas, what colour.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Quad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}

#[derive(Clone, Copy)]
struct Slot {
    /// Atlas position and size.
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    xmin: f32,
    ymin: f32,
    advance: f32,
}

pub struct Text {
    fonts: Vec<fontdue::Font>,
    fallbacks: Vec<fontdue::Font>,
    cache: HashMap<(char, bool, u32), Slot>,
    /// Shelf packing: the current row's top, its height, the next free x.
    shelf: (u32, u32, u32),
    /// Glyph pixels waiting to go up to the atlas: (x, y, w, h, coverage).
    pub uploads: Vec<(u32, u32, u32, u32, Vec<u8>)>,
    /// The atlas filled up and was cleared; everything must be uploaded again.
    pub reset: bool,
}

/// The grid shader's shape for a block or box character, if it is one.
pub fn shape(ch: char) -> u8 {
    const QUADS: [char; 16] = [' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█'];
    if let Some(i) = QUADS.iter().position(|q| *q == ch) {
        return i as u8;
    }
    match ch {
        '░' => 16,
        '▒' => 17,
        '▓' => 18,
        '─' => 19,
        '│' => 20,
        '┌' | '╭' => 21,
        '┐' | '╮' => 22,
        '└' | '╰' => 23,
        '┘' | '╯' => 24,
        '├' => 25,
        '┤' => 26,
        '┬' => 27,
        '┴' => 28,
        '┼' => 29,
        '━' => 30,
        '═' => 31,
        '║' => 32,
        '╔' => 33,
        '╗' => 34,
        '╚' => 35,
        '╝' => 36,
        '▁'..='▇' => 40 + (ch as u32 - '▁' as u32) as u8,
        '▉'..='▏' => 47 + (ch as u32 - '▉' as u32) as u8,
        _ => 0,
    }
}

impl Text {
    pub fn new() -> Text {
        let opts = fontdue::FontSettings::default;
        let fonts = vec![fontdue::Font::from_bytes(REGULAR, opts()).expect("Inter"), fontdue::Font::from_bytes(BOLD, opts()).expect("Inter Bold")];
        let fallbacks = FALLBACKS.iter().filter_map(|p| std::fs::read(p).ok()).filter_map(|b| fontdue::Font::from_bytes(b, opts()).ok()).collect();
        Text { fonts, fallbacks, cache: HashMap::new(), shelf: (0, 0, 0), uploads: Vec::new(), reset: false }
    }

    /// Font size in pixels for a cell this tall.
    pub fn px_for(cell_h: f32) -> f32 {
        (cell_h / 1.32).round()
    }

    fn glyph(&mut self, ch: char, bold: bool, px: f32) -> Slot {
        let key = (ch, bold, px as u32);
        if let Some(s) = self.cache.get(&key) {
            return *s;
        }
        let main = &self.fonts[bold as usize];
        let font = if main.lookup_glyph_index(ch) != 0 || ch == ' ' {
            main
        } else {
            self.fallbacks.iter().find(|f| f.lookup_glyph_index(ch) != 0).unwrap_or(main)
        };
        let (m, bitmap) = font.rasterize(ch, px);
        let (w, h) = (m.width as u32, m.height as u32);
        if self.shelf.2 + w + 1 > ATLAS {
            self.shelf = (self.shelf.0 + self.shelf.1 + 1, 0, 0);
        }
        if self.shelf.0 + h + 1 > ATLAS {
            // Full: start over (new sizes after a resize mostly).
            self.cache.clear();
            self.uploads.clear();
            self.shelf = (0, 0, 0);
            self.reset = true;
        }
        let (x, y) = (self.shelf.2, self.shelf.0);
        self.shelf.2 += w + 1;
        self.shelf.1 = self.shelf.1.max(h);
        if w > 0 && h > 0 {
            self.uploads.push((x, y, w, h, bitmap));
        }
        let s = Slot { x, y, w, h, xmin: m.xmin as f32, ymin: m.ymin as f32, advance: m.advance_width };
        self.cache.insert(key, s);
        s
    }

    /// The glyphs for the screen's text, laid out run by run.
    pub fn layout(&mut self, screen: &Screen, cell_w: f32, cell_h: f32, out: &mut Vec<Quad>) {
        out.clear();
        let px = Self::px_for(cell_h);
        let lm = self.fonts[0].horizontal_line_metrics(px).expect("line metrics");
        let base = ((cell_h - (lm.ascent - lm.descent)) * 0.5 + lm.ascent).round();
        let w = screen.w as usize;
        for y in 0..screen.h as usize {
            let row = &screen.cells[y * w..(y + 1) * w];
            let mut x = 0;
            while x < w {
                let c = row[x];
                if !is_text(c) {
                    x += 1;
                    continue;
                }
                // A run: same colour and weight, no gap of two spaces, no shapes.
                let start = x;
                let mut end = x + 1;
                while end < w {
                    let n = row[end];
                    let gap = n.ch == ' ' && row.get(end + 1).is_none_or(|m| m.ch == ' ');
                    if gap || (n.ch != ' ' && (!is_text(n) || n.fg != c.fg || n.bold != c.bold)) {
                        break;
                    }
                    end += 1;
                }
                self.run(&row[start..end], start as f32 * cell_w, y as f32 * cell_h + base, (end - start) as f32 * cell_w, px, out);
                x = end;
            }
        }
    }

    fn run(&mut self, cells: &[Cell], x0: f32, baseline: f32, room: f32, px: f32, out: &mut Vec<Quad>) {
        let bold = cells[0].bold;
        let slots: Vec<Slot> = cells.iter().map(|c| self.glyph(c.ch, bold, px)).collect();
        let total: f32 = slots.iter().map(|s| s.advance).sum();
        // Wider than its cells (rare: "mmm"): squeeze the spacing to fit.
        let k = if total > room { room / total } else { 1.0 };
        let mut pen = x0;
        for (c, s) in cells.iter().zip(&slots) {
            if s.w > 0 && c.ch != ' ' {
                let gx = (pen + s.xmin).round();
                let gy = (baseline - s.ymin - s.h as f32).round();
                let col = c.fg.map(|v| v as f32 / 255.0);
                out.push(Quad {
                    rect: [gx, gy, s.w as f32, s.h as f32],
                    uv: [s.x as f32, s.y as f32, (s.x + s.w) as f32, (s.y + s.h) as f32],
                    color: [col[0], col[1], col[2], 1.0],
                });
            }
            pen += s.advance * k;
        }
    }
}

fn is_text(c: Cell) -> bool {
    c.ch != ' ' && shape(c.ch) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_are_shapes_and_letters_are_text() {
        assert_eq!(shape('█'), 15);
        assert_eq!(shape('▀'), 3);
        assert_eq!(shape('▁'), 40);
        assert_eq!(shape('▏'), 53);
        assert_eq!(shape('a'), 0);
    }

    #[test]
    fn runs_break_at_wide_gaps() {
        let mut s = Screen::new(40, 1, true);
        s.text(0, 0, "Shadows      on", [255, 255, 255], [0, 0, 0], false);
        let mut t = Text::new();
        let mut q = Vec::new();
        t.layout(&s, 10.0, 20.0, &mut q);
        // "on" starts at its own cell, not right after "Shadows".
        let on = q.iter().rev().nth(1).unwrap();
        assert!(on.rect[0] >= 13.0 * 10.0 - 2.0, "{:?}", on.rect);
        assert_eq!(q.len(), 9);
    }
}
