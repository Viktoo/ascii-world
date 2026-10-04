//! Saving and loading the live world. Live state is saved continuously (not
//! versioned): things, changed scatter cells, relationships, characters'
//! needs and what they hold, the universe's own properties and rules, the
//! interpreter's cache, and recent events.

use super::env::Cell;
use super::interp::InterpEffect;
use super::props::*;
use super::rules::RuleSpec;
use super::things::{Origin, Thing};
use super::{ActorId, Sim, SimEvent};
use crate::db::Db;
use glam::Vec3;
use rusqlite::{OptionalExtension, params};
use std::collections::HashSet;

fn actor_str(a: Option<ActorId>) -> Option<String> {
    a.map(|a| a.key())
}

fn parse_actor(s: Option<String>) -> Option<ActorId> {
    s.and_then(|s| ActorId::parse(&s))
}

pub fn universe_props(db: &Db) -> Vec<(String, f32, String)> {
    db.with(|c| {
        let mut st = c.prepare("SELECT name, default_value, meaning FROM vocab ORDER BY rowid")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)? as f32, r.get::<_, String>(2)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    })
    .unwrap_or_default()
}

/// What the engine knows about a universe's own properties (when they are
/// news, whether they spread, whether they harm), by name.
pub fn universe_meta(db: &Db) -> Vec<(String, crate::sim::props::PropMeta)> {
    db.kv_get("vocab.meta").and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
}

pub fn set_universe_meta(db: &Db, meta: &[(String, crate::sim::props::PropMeta)]) -> anyhow::Result<()> {
    db.kv_set("vocab.meta", &serde_json::to_string(meta)?)
}

/// Near-miss property names this world has ruled on, by tidied name: the
/// property each means, or "" for none (so nobody asks again).
pub fn prop_aliases(db: &Db) -> std::collections::BTreeMap<String, String> {
    db.kv_get("vocab.aliases").and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
}

pub fn set_prop_aliases(db: &Db, add: &[(String, String)]) -> anyhow::Result<()> {
    let mut all = prop_aliases(db);
    for (from, to) in add {
        all.insert(norm_name(from), to.clone());
    }
    db.kv_set("vocab.aliases", &serde_json::to_string(&all)?)
}

/// This world's whole vocabulary: built-ins, its own properties with what
/// the engine knows about them, and the aliases it has learned.
pub fn world_vocab(db: &Db) -> Vocab {
    let mut vocab = Vocab::builtin();
    for (name, default, meaning) in universe_props(db) {
        if let Err(e) = vocab.add(&name, default, &meaning) {
            crate::log::error(format!("universe property {name}: {e}"));
        }
    }
    for (name, meta) in universe_meta(db) {
        vocab.set_meta(&name, meta);
    }
    for (from, to) in prop_aliases(db) {
        vocab.add_alias(&from, &to);
    }
    vocab
}

pub fn universe_rules(db: &Db) -> Vec<RuleSpec> {
    db.with(|c| {
        let mut st = c.prepare("SELECT json FROM rules ORDER BY id")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).filter_map(|j| serde_json::from_str::<RuleSpec>(&j).ok()).collect())
    })
    .unwrap_or_default()
}

/// Store a universe's own properties and rules (from genesis).
pub fn set_universe_rules(db: &Db, props: &[(String, f32, String)], rules: &[RuleSpec]) -> anyhow::Result<()> {
    db.tx(|tx| {
        for (n, d, m) in props {
            tx.execute("INSERT INTO vocab(name, meaning, default_value) VALUES (?1, ?2, ?3) ON CONFLICT(name) DO UPDATE SET meaning = excluded.meaning, default_value = excluded.default_value", params![n, m, *d as f64])?;
        }
        tx.execute("DELETE FROM rules", [])?;
        for r in rules {
            tx.execute("INSERT INTO rules(json) VALUES (?1)", [serde_json::to_string(r)?])?;
        }
        Ok(())
    })
}

/// Gestures this universe has learned (written by the LLM as pose keyframes).
pub fn load_gestures(db: &Db) {
    let rows: Vec<String> = db
        .with(|c| {
            let mut st = c.prepare("SELECT json FROM gestures ORDER BY name")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    for j in rows {
        if let Ok(g) = serde_json::from_str::<super::actor::CustomGesture>(&j) {
            let _ = super::actor::register_gesture(g);
        }
    }
}

pub fn save_gesture(db: &Db, g: &super::actor::CustomGesture) {
    let _ = db.with(|c| {
        c.execute("INSERT INTO gestures(name, json) VALUES (?1, ?2) ON CONFLICT(name) DO UPDATE SET json = excluded.json", params![g.name, serde_json::to_string(g)?])?;
        Ok(())
    });
}

pub fn cached_interp(db: &Db, key: &str) -> Option<InterpEffect> {
    db.with(|c| Ok(c.query_row("SELECT effect_json FROM interp_cache WHERE key = ?1", [key], |r| r.get::<_, String>(0)).optional()?)).ok().flatten().and_then(|j| serde_json::from_str(&j).ok())
}

pub fn cache_interp(db: &Db, key: &str, fx: &InterpEffect) {
    let _ = db.with(|c| {
        c.execute("INSERT INTO interp_cache(key, effect_json, t) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET effect_json = excluded.effect_json, t = excluded.t", params![key, serde_json::to_string(fx)?, crate::db::now()])?;
        Ok(())
    });
}

pub fn made_by(db: &Db, instance: i64) -> Option<String> {
    db.with(|c| Ok(c.query_row("SELECT made_by FROM origins WHERE instance_id = ?1", [instance], |r| r.get::<_, String>(0)).optional()?)).ok().flatten()
}

pub fn set_made_by(db: &Db, instance: i64, who: &str) {
    let _ = db.with(|c| {
        c.execute("INSERT INTO origins(instance_id, made_by) VALUES (?1, ?2) ON CONFLICT(instance_id) DO UPDATE SET made_by = excluded.made_by", params![instance, who])?;
        Ok(())
    });
}

/// Recent events about a subject, from the database (for `pocket inspect`).
pub fn events_about(db: &Db, subject: &str, n: usize) -> Vec<SimEvent> {
    db.with(|c| {
        let mut st = c.prepare("SELECT json FROM events WHERE subject = ?1 OR actor = ?1 ORDER BY id DESC LIMIT ?2")?;
        let rows = st.query_map(params![subject, n as i64], |r| r.get::<_, String>(0))?;
        let mut v: Vec<SimEvent> = rows.filter_map(|r| r.ok()).filter_map(|j| serde_json::from_str(&j).ok()).collect();
        v.reverse();
        Ok(v)
    })
    .unwrap_or_default()
}

pub fn load(sim: &mut Sim) {
    let db = sim.db.clone();
    let rows: Vec<(i64, u32, [f32; 6], String, String, String, Option<String>, Option<String>, bool, bool, String, f64)> = db
        .with(|c| {
            let mut st = c.prepare("SELECT id, type_id, x, y, z, yaw, scale, params_json, state_json, props_json, holder, co_holder, asleep, anchored, origin_json, born FROM things ORDER BY id")?;
            let rows = st.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)? as u32,
                    [r.get::<_, f64>(2)? as f32, r.get::<_, f64>(3)? as f32, r.get::<_, f64>(4)? as f32, r.get::<_, f64>(5)? as f32, r.get::<_, f64>(6)? as f32, 0.0],
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, Option<String>>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, i64>(12)? != 0,
                    r.get::<_, i64>(13)? != 0,
                    r.get::<_, String>(14)?,
                    r.get::<_, f64>(15)?,
                ))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .unwrap_or_default();
    // Parts, cuts and edit histories.
    let shapes: std::collections::HashMap<i64, super::things::Shape> = db
        .with(|c| {
            let mut st = c.prepare("SELECT id, json FROM thing_shapes")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            Ok(rows.filter_map(|r| r.ok()).filter_map(|(id, j)| serde_json::from_str(&j).ok().map(|s| (id, s))).collect())
        })
        .unwrap_or_default();
    let alive: HashSet<i64> = sim.snap.instances.iter().map(|p| p.id).collect();
    let mut gone = Vec::new();
    for (id, type_id, v, params_json, state_json, props_json, holder, co_holder, asleep, anchored, origin_json, born) in rows {
        let Some((ty, base)) = sim.type_info(type_id) else {
            gone.push(id);
            continue;
        };
        let origin: Origin = serde_json::from_str(&origin_json).unwrap_or_default();
        if origin.instance.is_some_and(|i| !alive.contains(&i)) {
            gone.push(id);
            continue;
        }
        let mut params = [0.5f32; 8];
        for (i, x) in serde_json::from_str::<Vec<f32>>(&params_json).unwrap_or_default().iter().take(8).enumerate() {
            params[i] = *x;
        }
        let mut state = [0.0f32; 8];
        for (i, x) in serde_json::from_str::<Vec<f32>>(&state_json).unwrap_or_default().iter().take(8).enumerate() {
            state[i] = *x;
        }
        let props = apply_diff(&sim.vocab, &scaled((*base).clone(), v[4]), &props_json);
        let mut t = Thing::new(id, &ty, Vec3::new(v[0], v[1], v[2]), v[3], v[4], params, props, born);
        t.state = state;
        // Everything wakes at rest; physics resumes when touched.
        let _ = asleep;
        t.asleep = true;
        t.anchored = anchored;
        t.origin = origin;
        // A worn layer is saved as holder "worn:<who>".
        match holder.as_deref().and_then(|h| h.strip_prefix("worn:")) {
            Some(w) => t.worn = ActorId::parse(w),
            None => t.holder = parse_actor(holder),
        }
        t.co_holder = parse_actor(co_holder);
        t.shape = shapes.get(&id).cloned().unwrap_or_default();
        t.dirty = false;
        sim.things.insert(t);
    }
    for id in gone {
        sim.things.removed.push(id);
    }
    // Who holds what.
    let held: Vec<(i64, Option<ActorId>, Option<ActorId>)> = sim.things.live().map(|t| (t.id, t.holder, t.co_holder)).collect();
    for (id, h, c) in held {
        for a in [h, c].into_iter().flatten() {
            match sim.actor_mut(a) {
                Some(x) if x.held.is_none() => x.held = Some(id),
                _ => {
                    if let Some(t) = sim.things.get_mut(id) {
                        if t.holder == Some(a) {
                            t.holder = None;
                        }
                        if t.co_holder == Some(a) {
                            t.co_holder = None;
                        }
                    }
                }
            }
        }
    }
    // Changed scatter cells.
    let cells: Vec<(i32, i32, String, bool)> = db
        .with(|c| {
            let mut st = c.prepare("SELECT gx, gz, props_json, active FROM cells")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)? as i32, r.get::<_, i64>(1)? as i32, r.get::<_, String>(2)?, r.get::<_, i64>(3)? != 0)))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .unwrap_or_default();
    let snap = sim.snap.clone();
    for (gx, gz, pj, active) in cells {
        let Some(it) = sim.cache.item_at(&snap, (gx, gz)) else { continue };
        let Some((ty, base)) = sim.type_info(it.inst.info[0]) else { continue };
        let scale = it.inst.pos_scale[3];
        let props = apply_diff(&sim.vocab, &scaled((*base).clone(), scale), &pj);
        let size = ty.radius() * scale;
        let pos = it.inst.pos();
        let spent_at = (props[P_FUEL] <= 0.0).then_some(sim.t);
        sim.field.cells.insert((gx, gz), Cell { props, type_id: ty.id, pos, size, small: size < 0.9, water: snap.terrain.height(pos.x, pos.z) < crate::terrain::WATER_LEVEL, active, fired: Vec::new(), spent_at });
    }
    // Relationships.
    let rels: Vec<(i64, i64, String)> = db
        .with(|c| {
            let mut st = c.prepare("SELECT a, b, json FROM relationships")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .unwrap_or_default();
    for (a, b, j) in rels {
        if let Ok(r) = serde_json::from_str(&j) {
            sim.social.rels.insert((a, b), r);
        }
    }
    sim.social.dirty = false;
    if let Some(st) = db.kv_get("sim.night").and_then(|j| serde_json::from_str(&j).ok()) {
        sim.night = st;
    }
    if let Some(st) = db.kv_get("sim.incidents").and_then(|j| serde_json::from_str(&j).ok()) {
        sim.incidents = st;
    }
    let goals: Vec<String> = db
        .with(|c| {
            let mut st = c.prepare("SELECT json FROM goals ORDER BY id")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    sim.goals.list = goals.iter().filter_map(|j| serde_json::from_str(j).ok()).collect();
    sim.goals.next = sim.goals.list.iter().map(|g| g.id + 1).max().unwrap_or(1);
    sim.goals.seen = sim.log.total;
    if let Some(j) = db.kv_get("sim.region_seen") {
        if let Ok(v) = serde_json::from_str::<Vec<((i32, i32), f64)>>(&j) {
            sim.region_seen = v.into_iter().collect();
        }
    }
    sim.field.saved = sim.field.cells.iter().map(|(k, c)| (*k, c.save_hash())).collect();
    // Used-up scatter (eaten, burnt away) stays gone until it regrows.
    let spent: Vec<(i32, i32, f64)> = db
        .with(|c| {
            let mut st = c.prepare("SELECT gx, gz, t FROM spent_cells")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)? as i32, r.get::<_, i64>(1)? as i32, r.get::<_, f64>(2)?)))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .unwrap_or_default();
    for (gx, gz, t) in spent {
        sim.things.spent_cells.insert((gx, gz), t);
        sim.things.taken.entry((gx, gz)).or_insert(-1);
    }
}

/// Write everything that changed since the last save.
pub fn save(sim: &mut Sim) {
    let db = sim.db.clone();
    let vocab = sim.vocab.clone();
    let mut thing_rows = Vec::new();
    let ids: Vec<i64> = sim.things.map.keys().copied().collect();
    for id in ids {
        let Some(t) = sim.things.map.get(&id) else { continue };
        if t.removed || !t.dirty {
            continue;
        }
        let Some(ty) = sim.snap.type_of(t.type_id).cloned() else { continue };
        let base = scaled((*sim.type_props.get(&vocab, &ty)).clone(), t.scale);
        let t = &sim.things.map[&id];
        thing_rows.push((
            t.id,
            t.type_id,
            t.pos,
            t.yaw,
            t.scale,
            serde_json::to_string(&t.params.to_vec()).unwrap_or_default(),
            serde_json::to_string(&t.state.to_vec()).unwrap_or_default(),
            serde_json::Value::Object(diff(&vocab, &t.props, &base)).to_string(),
            t.worn.map(|w| format!("worn:{}", w.key())).or_else(|| actor_str(t.holder)),
            actor_str(t.co_holder),
            t.asleep,
            t.anchored,
            serde_json::to_string(&t.origin).unwrap_or_default(),
            t.born,
            (!t.shape.is_empty()).then(|| serde_json::to_string(&t.shape).unwrap_or_default()),
        ));
    }
    let removed = std::mem::take(&mut sim.things.removed);
    for t in sim.things.map.values_mut() {
        t.dirty = false;
    }
    sim.things.map.retain(|_, t| !t.removed);
    // Cells: store the difference from the item's type, for the ones that
    // changed since the last save (a burnt steppe has tens of thousands).
    let snap = sim.snap.clone();
    let mut cell_rows = Vec::new();
    let mut now_cells = std::collections::HashMap::with_capacity(sim.field.cells.len());
    for ((gx, gz), c) in &sim.field.cells {
        let h = c.save_hash();
        now_cells.insert((*gx, *gz), h);
        if sim.field.saved.get(&(*gx, *gz)) == Some(&h) {
            continue;
        }
        let Some(ty) = snap.type_of(c.type_id) else { continue };
        let base = sim.type_props.get(&vocab, ty);
        cell_rows.push((*gx, *gz, serde_json::Value::Object(diff(&vocab, &c.props, &base)).to_string(), c.active));
    }
    let gone_cells: Vec<(i32, i32)> = sim.field.saved.keys().filter(|k| !now_cells.contains_key(*k)).copied().collect();
    sim.field.saved = now_cells;
    let rels: Vec<((i64, i64), String)> = if sim.social.dirty { sim.social.rels.iter().map(|(k, r)| (*k, serde_json::to_string(r).unwrap_or_default())).collect() } else { Vec::new() };
    sim.social.dirty = false;
    // The parts of an incident are counted on it; only its first is kept.
    let events: Vec<_> = std::mem::take(&mut sim.log.unsaved).into_iter().filter(|e| e.data.get("part").and_then(|v| v.as_bool()) != Some(true)).collect();
    let incidents = serde_json::to_string(&sim.incidents).unwrap_or_default();
    let goals: Vec<(u64, i64, String, String)> = sim.goals.list.iter().map(|g| (g.id, g.owner, format!("{:?}", g.status).to_lowercase(), serde_json::to_string(g).unwrap_or_default())).collect();
    let npc_states: Vec<(i64, String)> = sim
        .cast
        .npcs
        .iter()
        .map(|n| {
            let mut st = n.saved(sim.t);
            st.props = sim.body_diff(n);
            (n.def.id, serde_json::to_string(&st).unwrap_or_default())
        })
        .collect();
    let seen: Vec<((i32, i32), f64)> = sim.region_seen.iter().map(|(k, v)| (*k, *v)).collect();
    let night = serde_json::to_string(&sim.night).unwrap_or_default();
    let spent: Vec<((i32, i32), f64)> = sim.things.spent_cells.iter().map(|(k, v)| (*k, *v)).collect();
    let r = db.tx(|tx| {
        for r in &thing_rows {
            tx.execute(
                "INSERT INTO things(id, type_id, x, y, z, yaw, scale, params_json, state_json, props_json, holder, co_holder, asleep, anchored, origin_json, born) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                 ON CONFLICT(id) DO UPDATE SET type_id = excluded.type_id, x = excluded.x, y = excluded.y, z = excluded.z, yaw = excluded.yaw, scale = excluded.scale, params_json = excluded.params_json, state_json = excluded.state_json, props_json = excluded.props_json, holder = excluded.holder, co_holder = excluded.co_holder, asleep = excluded.asleep, anchored = excluded.anchored, origin_json = excluded.origin_json",
                params![r.0, r.1 as i64, r.2.x as f64, r.2.y as f64, r.2.z as f64, r.3 as f64, r.4 as f64, r.5, r.6, r.7, r.8, r.9, r.10 as i64, r.11 as i64, r.12, r.13],
            )?;
            match &r.14 {
                Some(j) => tx.execute("INSERT INTO thing_shapes(id, json) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET json = excluded.json", params![r.0, j])?,
                None => tx.execute("DELETE FROM thing_shapes WHERE id = ?1", [r.0])?,
            };
        }
        for id in &removed {
            tx.execute("DELETE FROM things WHERE id = ?1", [id])?;
            tx.execute("DELETE FROM thing_shapes WHERE id = ?1", [id])?;
        }
        for (gx, gz, pj, active) in &cell_rows {
            tx.execute("INSERT INTO cells(gx, gz, props_json, active) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(gx, gz) DO UPDATE SET props_json = excluded.props_json, active = excluded.active", params![gx, gz, pj, *active as i64])?;
        }
        for (gx, gz) in &gone_cells {
            tx.execute("DELETE FROM cells WHERE gx = ?1 AND gz = ?2", params![gx, gz])?;
        }
        for ((a, b), j) in &rels {
            tx.execute("INSERT INTO relationships(a, b, json) VALUES (?1, ?2, ?3) ON CONFLICT(a, b) DO UPDATE SET json = excluded.json", params![a, b, j])?;
        }
        for e in &events {
            tx.execute(
                "INSERT INTO events(t, kind, actor, subject, json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![e.t, e.kind, e.actor.map(|a| a.key()), e.subject, serde_json::to_string(e)?],
            )?;
        }
        if !events.is_empty() {
            tx.execute("DELETE FROM events WHERE id <= (SELECT MAX(id) FROM events) - 5000", [])?;
        }
        for (id, s) in &npc_states {
            tx.execute("UPDATE characters SET state_json = ?1 WHERE id = ?2", params![s, id])?;
        }
        crate::db::kv_set(tx, "sim.region_seen", &serde_json::to_string(&seen)?)?;
        crate::db::kv_set(tx, "sim.night", &night)?;
        crate::db::kv_set(tx, "sim.incidents", &incidents)?;
        tx.execute("DELETE FROM goals", [])?;
        for (id, owner, status, j) in &goals {
            tx.execute("INSERT INTO goals(id, owner, status, json) VALUES (?1, ?2, ?3, ?4)", params![*id as i64, owner, status, j])?;
        }
        tx.execute("DELETE FROM spent_cells", [])?;
        for ((gx, gz), t) in &spent {
            tx.execute("INSERT INTO spent_cells(gx, gz, t) VALUES (?1, ?2, ?3)", params![gx, gz, t])?;
        }
        Ok(())
    });
    if let Err(e) = r {
        crate::log::error(format!("saving the live world failed: {e:#}"));
    }
}
