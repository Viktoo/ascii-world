//! Text for the window: the screen's cells read back as runs of words and set
//! in a rounded sans-serif (Nunito), each run starting at its cell; titles and
//! other free labels in a soft serif (Fraunces). Block and box characters are
//! not text: the grid shader draws them as shapes.

use crate::term::{Cell, Face, Field, Label, Screen};
use std::collections::HashMap;

static BODY: &[u8] = include_bytes!("../../assets/fonts/Nunito-SemiBold.ttf");
static BOLD: &[u8] = include_bytes!("../../assets/fonts/Nunito-ExtraBold.ttf");
static TITLE: &[u8] = include_bytes!("../../assets/fonts/Fraunces-SoftSemiBold.ttf");

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
    cache: HashMap<(char, u8, u32), Slot>,
    /// Shelf packing: the current row's top, its height, the next free x.
    shelf: (u32, u32, u32),
    /// Glyph pixels waiting to go up to the atlas: (x, y, w, h, coverage).
    pub uploads: Vec<(u32, u32, u32, u32, Vec<u8>)>,
    /// The atlas filled up and was cleared; everything must be uploaded again.
    pub reset: bool,
    /// Glyphs before this one lie under the screen's `over_decor`; the rest over it.
    pub split: usize,
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

/// How wide `text` is set in `face`, `size` rows tall, in cells (a cell is
/// half a row wide): what a free label or a pill around words needs.
pub fn measure(text: &str, face: Face, size: f32) -> f32 {
    static FONTS: std::sync::OnceLock<Vec<fontdue::Font>> = std::sync::OnceLock::new();
    let fonts = FONTS.get_or_init(|| [BODY, BOLD, TITLE].iter().map(|b| fontdue::Font::from_bytes(*b, fontdue::FontSettings::default()).expect("bundled font")).collect());
    let f = match face {
        Face::Body => 0,
        Face::Bold => 1,
        Face::Title => 2,
    };
    let adv: f32 = text
        .chars()
        .map(|ch| {
            let main = if fonts[f].lookup_glyph_index(ch) != 0 { &fonts[f] } else { &fonts[f.min(1)] };
            let arrow = ('\u{2190}'..='\u{21FF}').contains(&ch);
            match () {
                // A symbol from a fallback font (big, read only to draw):
                // about this wide; arrows half again as big (see `glyph`).
                _ if arrow => 120.0,
                _ if ch != ' ' && main.lookup_glyph_index(ch) == 0 => 85.0,
                _ => main.metrics(ch, 100.0).advance_width,
            }
        })
        .sum();
    // Advances scale with the size: measured at 100 px, a label is
    // size × cell_h / 1.32 px and a cell cell_h / 2 wide.
    adv / 100.0 * size / 1.32 * 2.0
}

impl Text {
    pub fn new() -> Text {
        let opts = fontdue::FontSettings::default;
        let fonts = [BODY, BOLD, TITLE].iter().map(|b| fontdue::Font::from_bytes(*b, opts()).expect("bundled font")).collect();
        let fallbacks = FALLBACKS.iter().filter_map(|p| std::fs::read(p).ok()).filter_map(|b| fontdue::Font::from_bytes(b, opts()).ok()).collect();
        let mut t = Text { fonts, fallbacks, cache: HashMap::new(), shelf: (0, 0, 0), uploads: Vec::new(), reset: false, split: 0 };
        t.solid();
        t
    }

    /// A solid block at the atlas's corner, for plain shapes drawn as glyphs (the caret).
    fn solid(&mut self) {
        self.uploads.push((0, 0, 2, 2, vec![255; 4]));
        self.shelf = (0, 2, 3);
    }

    /// Font size in pixels for a cell this tall.
    pub fn px_for(cell_h: f32) -> f32 {
        (cell_h / 1.32).round()
    }

    fn glyph(&mut self, ch: char, face: Face, px: f32) -> Slot {
        let f = match face {
            Face::Body => 0,
            Face::Bold => 1,
            Face::Title => 2,
        };
        let key = (ch, f as u8, px as u32);
        if let Some(s) = self.cache.get(&key) {
            return *s;
        }
        // Titles fall back to the body face for what they lack.
        let main = if self.fonts[f].lookup_glyph_index(ch) != 0 { &self.fonts[f] } else { &self.fonts[f.min(1)] };
        // Nunito's arrows are tiny: a symbol font's read better.
        let arrow = ('\u{2190}'..='\u{21FF}').contains(&ch);
        let main = match self.fallbacks.iter().find(|f| arrow && f.lookup_glyph_index(ch) != 0) {
            Some(f) => f,
            None => main,
        };
        let font = if main.lookup_glyph_index(ch) != 0 || ch == ' ' {
            main
        } else {
            self.fallbacks.iter().find(|f| f.lookup_glyph_index(ch) != 0).unwrap_or(main)
        };
        // Arrows are drawn small in text faces: half again as big, still on the line.
        let (m, bitmap) = font.rasterize(ch, if arrow { px * 1.5 } else { px });
        let lift = if arrow { -(px * 0.12) } else { 0.0 };
        let (w, h) = (m.width as u32, m.height as u32);
        if self.shelf.2 + w + 1 > ATLAS {
            self.shelf = (self.shelf.0 + self.shelf.1 + 1, 0, 0);
        }
        if self.shelf.0 + h + 1 > ATLAS {
            // Full: start over (new sizes after a resize mostly).
            self.cache.clear();
            self.uploads.clear();
            self.shelf = (0, 0, 0);
            self.solid();
            self.reset = true;
        }
        let (x, y) = (self.shelf.2, self.shelf.0);
        self.shelf.2 += w + 1;
        self.shelf.1 = self.shelf.1.max(h);
        if w > 0 && h > 0 {
            self.uploads.push((x, y, w, h, bitmap));
        }
        let s = Slot { x, y, w, h, xmin: m.xmin as f32, ymin: m.ymin as f32 + lift, advance: m.advance_width };
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
            // The caret, if on this row: where the words put its cell.
            let caret = screen.caret.filter(|c| c.1 as usize == y);
            let mut caret_x = caret.map(|c| c.0 as f32 * cell_w);
            let (mut placed, mut last) = (false, None);
            let in_grid = |x: usize| screen.grids.iter().any(|g| (g[0] as usize..(g[0] + g[2]) as usize).contains(&x) && (g[1] as usize..(g[1] + g[3]) as usize).contains(&y));
            let mut x = 0;
            while x < w {
                let c = row[x];
                if !is_text(c) {
                    x += 1;
                    continue;
                }
                // On a grid: the glyph alone, in the middle of its cell.
                if in_grid(x) {
                    let adv = self.glyph(c.ch, face(c), px).advance;
                    self.run(&row[x..x + 1], x as f32 * cell_w + (cell_w - adv) * 0.5, y as f32 * cell_h + base, cell_w.max(adv), px, None, out);
                    last = None;
                    x += 1;
                    continue;
                }
                // A run: same colour and weight, no gap of two spaces, no shapes.
                let start = x;
                let mut end = x + 1;
                while end < w {
                    let n = row[end];
                    let gap = n.ch == ' ' && row.get(end + 1).is_none_or(|m| m.ch == ' ');
                    if gap || in_grid(end) || (n.ch != ' ' && (!is_text(n) || n.fg != c.fg || n.bold != c.bold)) {
                        break;
                    }
                    end += 1;
                }
                let at = caret.map(|c| c.0 as usize).filter(|c| (start..=end).contains(c)).map(|c| c - start);
                let (p, pen) = self.run(&row[start..end], start as f32 * cell_w, y as f32 * cell_h + base, (end - start) as f32 * cell_w, px, at, out);
                if p.is_some() {
                    caret_x = p;
                    placed = true;
                }
                last = Some((end, pen));
                x = end;
            }
            // Past the words by spaces alone (just typed one): after them, not
            // at its cell, which is further right than the words reach.
            if let (false, Some(c), Some((end, pen))) = (placed, caret, last) {
                let col = c.0 as usize;
                if col > end && row[end..col.min(w)].iter().all(|c| c.ch == ' ') {
                    caret_x = Some(pen + (col - end) as f32 * self.glyph(' ', Face::Body, px).advance);
                }
            }
            if let (Some((_, _, col)), Some(cx)) = (caret, caret_x) {
                let c = col.map(|v| v as f32 / 255.0);
                let (w, h) = ((cell_h * 0.08).max(1.5).round(), (cell_h * 0.78).round());
                out.push(Quad { rect: [cx.round(), (y as f32 * cell_h + (cell_h - h) * 0.5).round(), w, h], uv: [0.5, 0.5, 1.5, 1.5], color: [c[0], c[1], c[2], 1.0] });
            }
        }
        for l in &screen.labels {
            self.label(l, cell_w, cell_h, out);
        }
        if let Some(f) = &screen.field {
            self.field(f, cell_w, cell_h, base, px, out);
        }
        self.split = out.len();
        for l in &screen.over_labels {
            self.label(l, cell_w, cell_h, out);
        }
    }

    /// The line being typed: as much of its end as fits, then the caret.
    #[allow(clippy::too_many_arguments)]
    fn field(&mut self, f: &Field, cell_w: f32, cell_h: f32, base: f32, px: f32, out: &mut Vec<Quad>) {
        let slots: Vec<(char, Slot)> = f.text.chars().map(|c| (c, self.glyph(c, Face::Body, px))).collect();
        let room = f.w as f32 * cell_w - (cell_h * 0.2).max(2.0);
        // Show the end, unless the caret is further left: then from the caret on.
        let at = f.at.min(slots.len());
        let mut width: f32 = slots.iter().map(|s| s.1.advance).sum();
        let mut first = 0;
        while width > room && first < at {
            width -= slots[first].1.advance;
            first += 1;
        }
        let col = f.color.map(|v| v as f32 / 255.0);
        let (x0, baseline) = (f.x as f32 * cell_w, f.y as f32 * cell_h + base);
        let (mut pen, mut caret_x) = (x0, x0);
        for (i, (ch, s)) in slots.iter().enumerate().skip(first) {
            if i == at {
                caret_x = pen;
            }
            if pen + s.advance - x0 > room {
                break;
            }
            if s.w > 0 && *ch != ' ' {
                out.push(Quad {
                    rect: [(pen + s.xmin).round(), (baseline - s.ymin - s.h as f32).round(), s.w as f32, s.h as f32],
                    uv: [s.x as f32, s.y as f32, (s.x + s.w) as f32, (s.y + s.h) as f32],
                    color: [col[0], col[1], col[2], 1.0],
                });
            }
            pen += s.advance;
        }
        if at == slots.len() {
            caret_x = pen;
        }
        if let Some(c) = f.caret {
            let c = c.map(|v| v as f32 / 255.0);
            let (w, h) = ((cell_h * 0.08).max(1.5).round(), (cell_h * 0.78).round());
            out.push(Quad { rect: [caret_x.round(), (f.y as f32 * cell_h + (cell_h - h) * 0.5).round(), w, h], uv: [0.5, 0.5, 1.5, 1.5], color: [c[0], c[1], c[2], 1.0] });
        }
    }

    /// Free text: `size` rows tall, at its place in cells.
    fn label(&mut self, l: &Label, cell_w: f32, cell_h: f32, out: &mut Vec<Quad>) {
        let px = (l.size * cell_h / 1.32).round().max(6.0);
        let lm = self.fonts[if l.face == Face::Title { 2 } else { 0 }].horizontal_line_metrics(px).expect("line metrics");
        let slots: Vec<(char, Slot)> = l.text.chars().map(|c| (c, self.glyph(c, l.face, px))).collect();
        let width: f32 = slots.iter().map(|s| s.1.advance).sum();
        let x0 = l.x * cell_w - if l.center { width * 0.5 } else { 0.0 };
        // Centred: on the line's middle; otherwise from its top.
        let baseline = if l.center { l.y * cell_h + (lm.ascent + lm.descent) * 0.5 } else { l.y * cell_h + lm.ascent };
        let col = l.color.map(|v| v as f32 / 255.0);
        let mut pen = x0;
        for (ch, s) in slots {
            if s.w > 0 && ch != ' ' {
                out.push(Quad {
                    rect: [(pen + s.xmin).round(), (baseline - s.ymin - s.h as f32).round(), s.w as f32, s.h as f32],
                    uv: [s.x as f32, s.y as f32, (s.x + s.w) as f32, (s.y + s.h) as f32],
                    color: [col[0], col[1], col[2], 1.0],
                });
            }
            pen += s.advance;
        }
    }

    /// Set a run of cells from `x0`; returns where letter `caret` begins, if
    /// asked, and where the run ends.
    #[allow(clippy::too_many_arguments)]
    fn run(&mut self, cells: &[Cell], x0: f32, baseline: f32, room: f32, px: f32, caret: Option<usize>, out: &mut Vec<Quad>) -> (Option<f32>, f32) {
        let slots: Vec<Slot> = cells.iter().map(|c| self.glyph(c.ch, face(*c), px)).collect();
        let total: f32 = slots.iter().map(|s| s.advance).sum();
        // Wider than its cells (rare: "mmm"): squeeze the spacing to fit.
        let k = if total > room { room / total } else { 1.0 };
        let mut pen = x0;
        let mut at = None;
        for (i, (c, s)) in cells.iter().zip(&slots).enumerate() {
            if caret == Some(i) {
                at = Some(pen);
            }
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
        if caret == Some(cells.len()) {
            at = Some(pen);
        }
        (at, pen)
    }
}

fn face(c: Cell) -> Face {
    if c.bold { Face::Bold } else { Face::Body }
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

    #[test]
    fn caret_after_a_space_follows_the_words() {
        let mut s = Screen::new(40, 1, true);
        s.text(0, 0, "mmm mmm ", [255, 255, 255], [0, 0, 0], false);
        s.caret = Some((8, 0, [255, 0, 0]));
        let (mut t, mut q) = (Text::new(), Vec::new());
        t.layout(&s, 20.0, 20.0, &mut q);
        // Just after the space, not at the ninth cell (160 px).
        let caret = q.last().unwrap().rect[0];
        let words = q[q.len() - 2].rect;
        assert!(caret > words[0] + words[2] && caret < 140.0, "caret at {caret}, words end {words:?}");
    }

    #[test]
    fn a_long_field_shows_its_end() {
        let mut s = Screen::new(20, 1, true);
        let text = "a long line of words that will not fit in twenty cells at the end";
        s.field = Some(Field { x: 1, y: 0, w: 18, text: text.into(), at: text.chars().count(), color: [255, 255, 255], caret: Some([255, 0, 0]) });
        let (mut t, mut q) = (Text::new(), Vec::new());
        t.layout(&s, 10.0, 20.0, &mut q);
        let caret = q.last().unwrap().rect[0];
        assert!(caret <= 190.0 && caret > 150.0, "caret at {caret}");
        assert!(q.len() - 1 < text.chars().filter(|c| *c != ' ').count(), "the start scrolled off");
    }
}
