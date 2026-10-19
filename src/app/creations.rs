//! The creations page (in the pause menu): everything made in this
//! world since it began, who made it, from what, and whether it is still
//! here. In real worlds most creations are meals, soon eaten, so the page
//! leads with what lasts (built things, made things, reshaped ones, beings)
//! and keeps food and the world's own changes below. Tab groups by kind, by
//! maker, or newest first.

use super::menu::{HEAD, ago};
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
    /// How far the list is scrolled (rows).
    scroll: f32,
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

/// Who made it, as the one looking is told: "you" for their own. `guest`:
/// playing in someone's world (our name there, the host's name).
fn maker<'a>(r: &'a CreationRow, guest: Option<&'a (String, String)>) -> &'a str {
    match (r.made_by.as_str(), guest) {
        ("the traveler", Some((_, host))) => host,
        ("the traveler", None) => "you",
        ("", _) => "not recorded",
        (m, Some((me, _))) if m == me => "you",
        (m, _) => m,
    }
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
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
            m.creations = Some(Book { rows, sel: 0, group: Group::default(), scroll: 0.0 });
        }
    }

    /// Tab: the next way of grouping.
    pub(super) fn creations_key(&mut self, k: crossterm::event::KeyEvent) {
        let Some(b) = self.menu.as_mut().and_then(|m| m.creations.as_mut()) else { return };
        if k.code == crossterm::event::KeyCode::Tab {
            b.group = b.group.next();
            b.sel = 0;
            b.scroll = 0.0;
        }
    }

    /// The page's lines (headings and rows) and its rows in that order.
    fn creation_items(&self) -> (Vec<Item>, Vec<usize>) {
        let gn = self.guest_names();
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
                    *counts.entry(maker(r, gn.as_ref())).or_default() += 1;
                }
                // Busiest makers first; the unrecorded last.
                idx.sort_by_key(|i| {
                    let m = maker(&b.rows[*i], gn.as_ref());
                    (m == "not recorded", std::cmp::Reverse(counts[m]), m.to_string(), section(&b.rows[*i]).0)
                });
                let mut last = "";
                for i in &idx {
                    let m = maker(&b.rows[*i], gn.as_ref());
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

    /// The page: what was made, grouped, as a list; the chosen one in full
    /// beside it (below, in a narrow window).
    pub(super) fn creations_page(&mut self, ui: &mut crate::ui::Ui, s: &mut crate::term::Screen, w: u16, vh: u16) {
        use crate::term::{Deco, Face, Label};
        use crate::ui::{FAINT, Kind, R, left};
        let gn = self.guest_names();
        let (items, order) = self.creation_items();
        let here = self.creations_here();
        let area = Self::page_panel(s, w, vh, 110.0, 200.0, "Creations");
        let back = R::new(area.x + area.w - 12.0, area.y + 1.0, 10.0, 1.8);
        if ui.button(s, back, "Back", Kind::Quiet) || ui.back() {
            return self.menu_page(super::menu::Page::Pause);
        }
        let Some(b) = self.menu.as_ref().and_then(|m| m.creations.as_ref()) else { return };
        let makers = b.rows.iter().map(|r| maker(r, gn.as_ref())).filter(|m| *m != "not recorded").collect::<std::collections::HashSet<_>>().len();
        let still = b.rows.iter().filter(|r| here.contains_key(&r.key)).count();
        let summary = format!("{} made · {makers} maker{} · {still} still here", b.rows.len(), if makers == 1 { "" } else { "s" });
        s.labels.push(left(area.x + 16.5, area.y + 2.05, &summary, 0.9, DIM, Face::Body));
        let names: Vec<String> = Group::ALL.iter().map(|g| cap(g.name())).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let cur = Group::ALL.iter().position(|g| *g == b.group).unwrap_or(0);
        if let Some(t) = ui.tabs(s, area.x + 2.0, area.y + 3.6, &names, cur) {
            if let Some(b) = self.menu.as_mut().and_then(|m| m.creations.as_mut()) {
                b.group = Group::ALL[t];
                b.sel = 0;
                b.scroll = 0.0;
            }
            return;
        }
        // The list on the left, the chosen one on the right (or below).
        let wide = area.w >= 90.0;
        let top = area.y + 6.4;
        let bottom = area.y + area.h - if wide { 1.0 } else { 9.0 };
        let lw = if wide { area.w * 0.58 } else { area.w - 4.0 };
        let lx = area.x + 2.0;
        let Some(b) = self.menu.as_mut().and_then(|m| m.creations.as_mut()) else { return };
        let mut sel = b.sel.min(order.len().saturating_sub(1));
        let (_, focused) = ui.list(R::new(lx, top, lw, bottom - top), order.len(), &mut sel);
        let rh = 1.9;
        let hh = 1.9;
        // Each line's place.
        let mut ys = Vec::with_capacity(items.len());
        let mut y = 0.0f32;
        for it in &items {
            ys.push(y);
            y += match it {
                Item::Head(_) => hh,
                Item::Row(_) => rh,
            };
        }
        let len = y;
        let view = bottom - top;
        b.scroll = (b.scroll - ui.wheel * 3.0).clamp(0.0, (len - view).max(0.0));
        let sel_row = order.get(sel).copied();
        // Keep the chosen row in view when the keys move it (not over the wheel).
        if sel != b.sel {
            if let Some(k) = items.iter().position(|it| matches!(it, Item::Row(i) if Some(*i) == sel_row)) {
                // Keep its heading in view too when it is the first under one.
                let sy = if k > 0 && matches!(items[k - 1], Item::Head(_)) { ys[k - 1] } else { ys[k] };
                if sy < b.scroll {
                    b.scroll = sy;
                } else if ys[k] + rh > b.scroll + view {
                    b.scroll = ys[k] + rh - view;
                }
            }
        }
        b.sel = sel;
        let scroll = b.scroll;
        let mut clicked = None;
        for (k, it) in items.iter().enumerate() {
            let yy = ys[k] - scroll;
            let h = if matches!(it, Item::Head(_)) { hh } else { rh };
            if yy < -0.01 || yy + h > view + 0.01 {
                continue;
            }
            let ry = top + yy;
            match it {
                Item::Head(t) => s.labels.push(left(lx + 0.3, ry + 1.05, t, 1.1, HEAD, Face::Title)),
                Item::Row(i) => {
                    let Some(b) = self.menu.as_ref().and_then(|m| m.creations.as_ref()) else { return };
                    let r = &b.rows[*i];
                    let rr = R::new(lx, ry + 0.1, lw, rh - 0.2);
                    let is_sel = Some(*i) == sel_row;
                    let c = ui.card(s, rr, is_sel, focused);
                    if c.click {
                        clicked = Some(order.iter().position(|o| o == i).unwrap_or(0));
                    }
                    let h = here.get(&r.key);
                    // A dot: still in the world (green) or gone.
                    s.deco(Deco::pill(rr.x + 0.7, rr.mid_y() - 0.22, 0.88, 0.44, if h.is_some() { [130, 210, 130] } else { [70, 74, 92] }));
                    let by_maker = b.group == Group::Maker;
                    let who = if r.made_by.is_empty() { "" } else { maker(r, gn.as_ref()) };
                    let name_w = if by_maker { lw - 8.0 } else { lw * 0.62 - 3.0 };
                    let mut name = r.name.clone();
                    if r.count > 1 {
                        name = format!("{name} ×{}", r.count);
                    }
                    s.labels.push(left(rr.x + 2.0, rr.mid_y(), &crate::ui::fit(&name, name_w, 0.95), 0.95, if h.is_some() { TEXT } else { DIM }, if is_sel { Face::Bold } else { Face::Body }));
                    if !by_maker && !who.is_empty() {
                        s.labels.push(left(rr.x + lw * 0.64, rr.mid_y(), &crate::ui::fit(who, lw * 0.34, 0.85), 0.85, FAINT, Face::Body));
                    }
                }
            }
        }
        if let (Some(c), Some(b)) = (clicked, self.menu.as_mut().and_then(|m| m.creations.as_mut())) {
            b.sel = c;
        }
        if items.is_empty() {
            s.labels.push(Label { x: lx + lw * 0.5, y: top + 3.0, text: "Nothing has been made here yet.".into(), size: 1.0, color: DIM, face: Face::Body, center: true });
            s.labels.push(Label { x: lx + lw * 0.5, y: top + 4.4, text: "Press / in the world and say what to make; its people make things too.".into(), size: 0.85, color: FAINT, face: Face::Body, center: true });
        }
        // The chosen one, in full.
        let Some(b) = self.menu.as_ref().and_then(|m| m.creations.as_ref()) else { return };
        let Some(r) = order.get(b.sel.min(order.len().saturating_sub(1))).map(|i| &b.rows[*i]) else { return };
        let d = if wide { R::new(lx + lw + 1.5, top, area.x + area.w - 2.0 - (lx + lw + 1.5), bottom - top) } else { R::new(lx, bottom + 0.6, lw, area.y + area.h - 1.0 - bottom - 0.6) };
        s.deco(Deco::panel(d.x, d.y, d.w, d.h, [18, 20, 28]).border(crate::ui::PANEL_EDGE));
        let tw = ((d.w - 3.0) / 0.75) as usize;
        let mut y = d.y + 1.0;
        for l in crate::term::wrap(&r.name, ((d.w - 3.0) / 1.0) as usize).iter().take(2) {
            s.labels.push(left(d.x + 1.5, y + 0.6, l, 1.3, ACCENT, Face::Title));
            y += 1.5;
        }
        y += 0.5;
        let from = if r.from.is_empty() { String::new() } else { format!(" from {}", r.from) };
        let who = match maker(r, gn.as_ref()) {
            "not recorded" => " (maker not recorded)".to_string(),
            m => format!(" by {m}"),
        };
        let how = match r.kind.as_str() {
            "built" => format!("Built{who}"),
            "reshaped" => format!("Reshaped{who}{from}"),
            "changed" => format!("Became this{from}, by itself"),
            "being" => format!("Brought into the world{who}"),
            _ => format!("Made{who}{from}"),
        };
        let times = if r.count > 1 { format!(", {} times", r.count) } else { String::new() };
        let when = match r.t {
            Some(t) => format!("Day {}, {}", (t / crate::render::sky::DAY_SECONDS) as i64 + 1, crate::render::sky::time_label(t)),
            None => cap(&ago(crate::db::now() - r.wall)),
        };
        let place = r.at.map(|[x, z]| format!(" · at {}", self.where_from_you(Vec3::new(x, 0.0, z), true))).unwrap_or_default();
        let now = match here.get(&r.key) {
            Some(h) if r.kind == "being" => format!("Alive · {}", self.where_from_you(h.nearest.unwrap_or_default(), true)),
            Some(h) => format!("{} in the world · nearest {}", h.count, self.where_from_you(h.nearest.unwrap_or_default(), true)),
            None if r.kind == "being" => "No longer living".into(),
            None => "None left in the world".into(),
        };
        let lines: Vec<(String, [u8; 3])> = vec![(format!("{how}{times}."), TEXT), (format!("{when}{place}"), DIM), (now, if here.contains_key(&r.key) { [150, 220, 150] } else { DIM }), (r.tags.join(" · "), FAINT)];
        for (t, c) in lines {
            for l in crate::term::wrap(&t, tw) {
                if y + 1.2 > d.y + d.h {
                    break;
                }
                s.labels.push(left(d.x + 1.5, y + 0.5, &l, 0.9, c, Face::Body));
                y += 1.1;
            }
            y += 0.35;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn your_own_makings_are_yours_wherever_you_play() {
        let row = |by: &str| CreationRow { key: String::new(), kind: "made".into(), name: "kite".into(), made_by: by.into(), from: String::new(), t: None, at: None, wall: 0.0, count: 1, tags: Vec::new() };
        let guest = ("Bea".to_string(), "Hal".to_string());
        assert_eq!(maker(&row("the traveler"), None), "you");
        assert_eq!(maker(&row("the traveler"), Some(&guest)), "Hal", "the host's, seen by a guest");
        assert_eq!(maker(&row("Bea"), Some(&guest)), "you");
        assert_eq!(maker(&row("Bea"), None), "Bea", "a guest's, seen by the host");
    }
}
