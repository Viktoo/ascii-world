//! The creations page (3 in the settings screen): everything made in this
//! world since it began, who made it, from what, and whether it is still
//! here. In real worlds most creations are meals, soon eaten, so the page
//! leads with what lasts (built things, made things, reshaped ones, beings)
//! and keeps food and the world's own changes below. Tab groups by kind, by
//! maker, or newest first.

use super::menu::{BG, HEAD, SEL_BG, ago};
use super::{ACCENT, App, DIM, TEXT};
use crate::db::CreationRow;
use glam::Vec3;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Group {
    #[default]
    Kind,
    Maker,
    Newest,
}

impl Group {
    const ALL: [Group; 3] = [Group::Kind, Group::Maker, Group::Newest];

    fn name(self) -> &'static str {
        match self {
            Group::Kind => "by kind",
            Group::Maker => "by maker",
            Group::Newest => "newest",
        }
    }

    fn next(self) -> Group {
        let i = Group::ALL.iter().position(|g| *g == self).unwrap_or(0);
        Group::ALL[(i + 1) % Group::ALL.len()]
    }
}

pub struct Book {
    rows: Vec<CreationRow>,
    sel: usize,
    group: Group,
}

/// Where a creation sits on the page by kind: what lasts first, food and
/// the world's own changes last.
fn section(r: &CreationRow) -> (u8, &'static str) {
    let tag = |t: &str| r.tags.iter().any(|x| x == t);
    match r.kind.as_str() {
        "built" => (0, "Built"),
        "reshaped" => (2, "Reshaped"),
        "being" => (3, "Beings"),
        "changed" => (5, "Changed by the world itself"),
        _ if tag("building") || tag("landmark") || tag("vehicle") => (0, "Built"),
        _ if tag("food") || tag("drink") => (4, "Food & drink"),
        _ => (1, "Things"),
    }
}

fn maker(r: &CreationRow) -> &str {
    match r.made_by.as_str() {
        "the traveler" => "you",
        "" => "not recorded",
        m => m,
    }
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n.saturating_sub(1)).collect::<String>()) }
}

/// How many of it are in the world now, and the nearest one.
struct Here {
    count: usize,
    nearest: Option<Vec3>,
}

enum Item {
    Head(String),
    Row(usize),
}

impl App {
    /// Open the creations page, read fresh from the world's record.
    pub(super) fn open_creations(&mut self) {
        let rows = self.db.with(crate::db::creations).unwrap_or_default();
        if let Some(m) = self.menu.as_mut() {
            m.creations = Some(Book { rows, sel: 0, group: Group::default() });
        }
    }

    pub(super) fn creations_key(&mut self, k: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        let order = self.creation_items().1;
        let Some(b) = self.menu.as_mut().and_then(|m| m.creations.as_mut()) else { return };
        let n = order.len().max(1);
        match k.code {
            KeyCode::Up => b.sel = (b.sel + n - 1) % n,
            KeyCode::Down => b.sel = (b.sel + 1) % n,
            KeyCode::PageUp => b.sel = b.sel.saturating_sub(8),
            KeyCode::PageDown => b.sel = (b.sel + 8).min(n - 1),
            KeyCode::Tab => {
                b.group = b.group.next();
                b.sel = 0;
            }
            _ => {}
        }
    }

    /// The page's lines (headings and rows) and its rows in that order.
    fn creation_items(&self) -> (Vec<Item>, Vec<usize>) {
        let Some(b) = self.menu.as_ref().and_then(|m| m.creations.as_ref()) else { return (Vec::new(), Vec::new()) };
        let mut idx: Vec<usize> = (0..b.rows.len()).collect();
        let mut items = Vec::new();
        match b.group {
            Group::Kind => {
                idx.sort_by_key(|i| (section(&b.rows[*i]).0, std::cmp::Reverse(b.rows[*i].wall as i64)));
                let mut last = "";
                for i in &idx {
                    let (_, name) = section(&b.rows[*i]);
                    if name != last {
                        last = name;
                        let n = idx.iter().filter(|j| section(&b.rows[**j]).1 == name).count();
                        items.push(Item::Head(format!("{name} · {n}")));
                    }
                    items.push(Item::Row(*i));
                }
            }
            Group::Maker => {
                let mut counts: HashMap<&str, usize> = HashMap::new();
                for r in &b.rows {
                    *counts.entry(maker(r)).or_default() += 1;
                }
                // Busiest makers first; the unrecorded last.
                idx.sort_by_key(|i| {
                    let m = maker(&b.rows[*i]);
                    (m == "not recorded", std::cmp::Reverse(counts[m]), m.to_string(), section(&b.rows[*i]).0)
                });
                let mut last = "";
                for i in &idx {
                    let m = maker(&b.rows[*i]);
                    if m != last {
                        last = m;
                        let head = if m == "not recorded" { format!("Maker not recorded · {} (made before makers were kept)", counts[m]) } else { format!("{} · {}", cap(m), counts[m]) };
                        items.push(Item::Head(head));
                    }
                    items.push(Item::Row(*i));
                }
            }
            Group::Newest => {
                idx.sort_by(|a, b2| b.rows[*b2].wall.total_cmp(&b.rows[*a].wall));
                items.extend(idx.iter().map(|i| Item::Row(*i)));
            }
        }
        (items, idx)
    }

    /// What of each creation is in the world now.
    fn creations_here(&self) -> HashMap<String, Here> {
        let me = self.pos();
        let mut out: HashMap<String, Here> = HashMap::new();
        let mut add = |key: String, p: Vec3| {
            let h = out.entry(key).or_insert(Here { count: 0, nearest: None });
            h.count += 1;
            if h.nearest.is_none_or(|n| (n - me).length() > (p - me).length()) {
                h.nearest = Some(p);
            }
        };
        for t in self.sim.things.live() {
            add(format!("type:{}", t.type_id), t.pos);
        }
        for p in &self.snap.instances {
            add(format!("type:{}", p.type_id), p.pos);
        }
        for n in self.sim.cast.npcs.iter().filter(|n| n.here()) {
            add(format!("being:{}", n.def.id), n.a.pos);
        }
        out
    }

    /// How far and which way from you, with the place's name if it has one.
    fn where_from_you(&self, p: Vec3, named: bool) -> String {
        let me = self.pos();
        let d = Vec3::new(p.x - me.x, 0.0, p.z - me.z);
        let dist = d.length();
        let far = if dist < 15.0 { "right here".to_string() } else if dist >= 1000.0 { format!("{:.1} km {}", dist / 1000.0, crate::sim::compass(d)) } else { format!("{} m {}", (dist / 10.0).round() as i32 * 10, crate::sim::compass(d)) };
        match self.snap.region_name(crate::world::region_of(p.x, p.z)).filter(|n| named && !n.is_empty()) {
            Some(n) => format!("{n}, {far}"),
            None => far,
        }
    }

    pub(super) fn draw_creations(&mut self, x0: u16, pw: usize, vh: u16, tabs: &str) {
        let (items, order) = self.creation_items();
        let here = self.creations_here();
        let Some(b) = self.menu.as_ref().and_then(|m| m.creations.as_ref()) else { return };
        let inner = pw.saturating_sub(4);
        let by_maker = b.group == Group::Maker;
        let (wn, wm) = if by_maker { (inner.saturating_sub(24), 0) } else { (inner.saturating_sub(40).max(16), 16) };
        let sel_row = order.get(b.sel.min(order.len().saturating_sub(1))).copied();
        let mut list: Vec<(String, [u8; 3], bool)> = Vec::new();
        let mut sel_line = 0;
        for it in &items {
            match it {
                Item::Head(h) => list.push((h.clone(), HEAD, false)),
                Item::Row(i) => {
                    let r = &b.rows[*i];
                    let h = here.get(&r.key);
                    let mut status = match h {
                        Some(h) if h.count > 0 => format!("here · {}", self.where_from_you(h.nearest.unwrap_or_default(), false)),
                        _ => "gone".to_string(),
                    };
                    if r.count > 1 {
                        status = format!("×{} · {status}", r.count);
                    }
                    let sel = Some(*i) == sel_row;
                    if sel {
                        sel_line = list.len();
                    }
                    // An unrecorded maker leaves the cell empty rather than saying so on every row.
                    let who = if r.made_by.is_empty() { "" } else { maker(r) };
                    let text = if by_maker { format!("{:<wn$} {status}", clip(&r.name, wn)) } else { format!("{:<wn$} {:<wm$} {status}", clip(&r.name, wn), clip(who, wm)) };
                    list.push((text, if h.is_some() { TEXT } else { DIM }, sel));
                }
            }
        }
        // The chosen one, in full.
        let mut detail: Vec<(String, [u8; 3], bool)> = vec![(String::new(), TEXT, false)];
        if let Some(r) = sel_row.map(|i| &b.rows[i]) {
            detail.push((r.name.clone(), ACCENT, false));
            let from = if r.from.is_empty() { String::new() } else { format!(" from {}", r.from) };
            let who = match maker(r) {
                "not recorded" => " (maker not recorded)".to_string(),
                m => format!(" by {m}"),
            };
            let how = match r.kind.as_str() {
                "built" => format!("built{who}"),
                "reshaped" => format!("reshaped{who}{from}"),
                "changed" => format!("became this{from}, by itself"),
                "being" => format!("brought into the world{who}"),
                _ => format!("made{who}{from}"),
            };
            let times = if r.count > 1 { format!(" · made {} times", r.count) } else { String::new() };
            detail.push((format!("{how}{times}"), TEXT, false));
            let when = match r.t {
                Some(t) => format!("day {}, {}", (t / crate::render::sky::DAY_SECONDS) as i64 + 1, crate::render::sky::time_label(t)),
                None => ago(crate::db::now() - r.wall),
            };
            let place = r.at.map(|[x, z]| format!(" · first made at {}", self.where_from_you(Vec3::new(x, 0.0, z), true))).unwrap_or_default();
            detail.push((format!("{when}{place}"), DIM, false));
            let now = match here.get(&r.key) {
                Some(h) if r.kind == "being" => format!("alive · {}", self.where_from_you(h.nearest.unwrap_or_default(), true)),
                Some(h) => format!("{} in the world · nearest {}", h.count, self.where_from_you(h.nearest.unwrap_or_default(), true)),
                None if r.kind == "being" => "no longer living".into(),
                None => "none left in the world".into(),
            };
            detail.push((now, DIM, false));
            if !r.tags.is_empty() {
                detail.push((r.tags.join(" · "), DIM, false));
            }
        } else {
            for l in super::menu::hang("Nothing has been made in this world yet. Try / to make something, or wait: its people make things too.", pw - 3, 0) {
                detail.push((l, DIM, false));
            }
        }
        detail.push((String::new(), TEXT, false));
        for l in super::menu::hang("↑↓ choose · Tab group · 1 settings · 2 achievements · q quit · Esc close", pw - 3, 0) {
            detail.push((l, DIM, false));
        }
        // Heading: how much, by how many, how much is still here.
        let makers = b.rows.iter().map(maker).filter(|m| *m != "not recorded").collect::<std::collections::HashSet<_>>().len();
        let still = b.rows.iter().filter(|r| here.contains_key(&r.key)).count();
        let groups: String = Group::ALL.iter().map(|g| if *g == b.group { format!("[{}]", g.name()) } else { format!(" {} ", g.name()) }).collect::<Vec<_>>().join(" ");
        let summary = format!("{} creations · {makers} maker{} · {still} still here", b.rows.len(), if makers == 1 { "" } else { "s" });
        let fit = (vh as usize).saturating_sub(detail.len() + 7).max(3);
        let top = sel_line.saturating_sub(fit / 2).min(list.len().saturating_sub(fit));
        let mut lines = vec![(tabs.to_string(), ACCENT, false), (summary, DIM, false), (groups, TEXT, false), (String::new(), TEXT, false)];
        lines.extend(list.into_iter().skip(top).take(fit));
        lines.extend(detail);
        let blank = (String::new(), TEXT, false);
        let y0 = (vh as usize).saturating_sub(lines.len() + 2) as u16 / 2;
        let rows = std::iter::once(blank.clone()).chain(lines).chain(std::iter::once(blank));
        for (i, (text, fg, sel)) in rows.enumerate() {
            let y = y0 + i as u16;
            if y >= vh {
                break;
            }
            let t: String = text.chars().take(pw - 3).collect();
            let s = format!("{}{t:<width$} ", if sel { "› " } else { "  " }, width = pw - 3);
            self.screen.text(x0, y, &s, fg, if sel { SEL_BG } else { BG }, sel);
        }
    }
}
