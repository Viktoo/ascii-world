//! The raw truth about anything: what F2 shows and `pocket inspect` prints.

use super::props::*;
use super::{ActorId, Sim, Target};
use serde_json::{Value, json};

fn r1(x: f32) -> f64 {
    (x as f64 * 10.0).round() / 10.0
}

fn r2(x: f32) -> f64 {
    (x as f64 * 100.0).round() / 100.0
}

impl Sim {
    pub fn inspect(&mut self, t: &Target) -> Value {
        let t = match t {
            Target::Name(n) => {
                let p = self.player.pos;
                match self.find_named(n, p, ActorId::Player) {
                    Some(x) => x,
                    None => return json!({ "error": format!("nothing called '{n}' nearby") }),
                }
            }
            Target::Instance(i) => match self.things.by_instance.get(i) {
                Some(id) => Target::Thing(*id),
                None => t.clone(),
            },
            Target::Cell(c) => match self.things.taken.get(&(c[0], c[1])) {
                Some(id) => Target::Thing(*id),
                None => t.clone(),
            },
            other => other.clone(),
        };
        match &t {
            Target::Thing(id) => self.inspect_thing(*id),
            Target::Actor(a) => self.inspect_actor(*a),
            Target::Instance(i) => self.inspect_instance(*i),
            Target::Cell(c) => self.inspect_cell((c[0], c[1])),
            Target::Point(p) => {
                let pos = glam::Vec3::from(*p);
                let b = self.snap.terrain.biome_at(pos.x, pos.z);
                json!({
                    "kind": "ground",
                    "at": [r1(pos.x), r1(pos.y), r1(pos.z)],
                    "biome": self.snap.terrain.biomes[b].name,
                    "region": self.snap.region_name(crate::world::region_of(pos.x, pos.z)),
                    "tier": self.tier(pos),
                })
            }
            Target::Name(_) => json!({ "error": "unresolved" }),
        }
    }

    /// near / medium / far, as the simulation sees a point.
    pub fn tier(&self, p: glam::Vec3) -> &'static str {
        let d = self.dist_to_player(p);
        if d <= self.cfg.near {
            "near"
        } else if d <= self.cfg.medium {
            "medium"
        } else {
            "far"
        }
    }

    fn inspect_thing(&mut self, id: i64) -> Value {
        let Some(t) = self.things.get(id).cloned() else { return json!({ "error": format!("no thing {id}") }) };
        let Some((ty, base)) = self.type_info(t.type_id) else { return json!({ "error": "type missing" }) };
        let base = scaled((*base).clone(), t.scale);
        let mut events = self.log.about(&format!("thing:{id}"), None, 8);
        if events.is_empty() {
            events = super::persist::events_about(&self.db, &format!("thing:{id}"), 8);
        }
        json!({
            "kind": "thing",
            "id": id,
            "type": ty.name(),
            "type_id": ty.id,
            "tags": ty.ct.meta.tags,
            "at": [r1(t.pos.x), r1(t.pos.y), r1(t.pos.z)],
            "tier": self.tier(t.pos),
            "scale": r2(t.scale),
            "held_by": t.holder.map(|h| self.actor_name(h)),
            "co_held_by": t.co_holder.map(|h| self.actor_name(h)),
            "moving": !t.asleep,
            "anchored": t.anchored,
            "props": named(&self.vocab, &t.props),
            "changed": diff(&self.vocab, &t.props, &base),
            "state": t.state.iter().map(|v| r2(*v)).collect::<Vec<_>>(),
            "origin": t.origin,
            "age_s": r1((self.t - t.born) as f32),
            "rules_fired": t.fired.iter().rev().map(|(n, at)| json!({ "rule": n, "ago_s": r1((self.t - at) as f32) })).collect::<Vec<_>>(),
            "behavior": if ty.ct.has_behavior() { Some(ty.ct.source.clone()) } else { None },
            "events": events,
        })
    }

    fn inspect_instance(&mut self, i: i64) -> Value {
        let Some(p) = self.snap.instances.iter().find(|p| p.id == i).cloned() else { return json!({ "error": format!("no instance {i}") }) };
        let Some((ty, base)) = self.type_info(p.type_id) else { return json!({ "error": "type missing" }) };
        json!({
            "kind": "placed object (static until touched)",
            "instance": i,
            "type": ty.name(),
            "type_id": ty.id,
            "tags": ty.ct.meta.tags,
            "at": [r1(p.pos.x), r1(p.pos.y), r1(p.pos.z)],
            "tier": self.tier(p.pos),
            "props": named(&self.vocab, &scaled((*base).clone(), p.scale)),
            "made_by": super::persist::made_by(&self.db, i),
            "behavior": if ty.ct.has_behavior() { Some(ty.ct.source.clone()) } else { None },
        })
    }

    fn inspect_cell(&mut self, c: (i32, i32)) -> Value {
        let snap = self.snap.clone();
        let Some(it) = self.cache.item_at(&snap, c) else { return json!({ "error": "nothing grows there" }) };
        let Some((ty, base)) = self.type_info(it.inst.info[0]) else { return json!({ "error": "type missing" }) };
        let cell = self.field.cells.get(&c);
        let props = cell.map(|x| x.props.clone()).unwrap_or_else(|| scaled((*base).clone(), it.inst.pos_scale[3]));
        json!({
            "kind": "scatter item",
            "cell": [c.0, c.1],
            "type": ty.name(),
            "tags": ty.ct.meta.tags,
            "at": [r1(it.inst.pos_scale[0]), r1(it.inst.pos_scale[1]), r1(it.inst.pos_scale[2])],
            "active": cell.is_some_and(|x| x.active),
            "props": named(&self.vocab, &props),
            "rules_fired": cell.map(|x| x.fired.iter().rev().map(|(n, at)| json!({ "rule": n, "ago_s": r1((self.t - at) as f32) })).collect::<Vec<_>>()).unwrap_or_default(),
        })
    }

    fn inspect_actor(&mut self, a: ActorId) -> Value {
        let Some(x) = self.actor(a).cloned() else { return json!({ "error": "no such person" }) };
        let held = x.held.map(|h| json!({ "id": h, "name": self.thing_name(h) }));
        let joint = self.social.joint_of(a).map(|j| json!({ "doing": j.label(), "with": self.actor_name(j.other(a)), "count": j.count }));
        let proposals: Vec<Value> = self.social.proposals.iter().filter(|p| p.to == a || p.from == a).map(|p| json!({ "from": self.actor_name(p.from), "to": self.actor_name(p.to), "activity": p.activity.label() })).collect();
        let rels: Vec<Value> = self
            .actor_ids()
            .into_iter()
            .filter(|o| *o != a)
            .filter_map(|o| self.social.rel(a, o).map(|r| (o, r.clone())))
            .filter(|(_, r)| r.familiarity > 0.0 || r.affection != 0.0)
            .map(|(o, r)| json!({ "with": self.actor_name(o), "affection": r2(r.affection), "trust": r2(r.trust), "rivalry": r2(r.rivalry), "family": r.family, "partner": r.partner, "seen_as": r.describe() }))
            .collect();
        let mut v = json!({
            "kind": if a == ActorId::Player { "player" } else { "character" },
            "id": a,
            "name": self.actor_name(a),
            "at": [r1(x.pos.x), r1(x.pos.y), r1(x.pos.z)],
            "yaw_deg": r1(x.yaw.to_degrees().rem_euclid(360.0)),
            "tier": self.tier(x.pos),
            "holding": held,
            "task": x.task.as_ref().map(|t| format!("{t:?}")),
            "gesture": x.gesture.as_ref().map(|g| json!({ "kind": g.kind.name(), "with": g.with.map(|w| self.actor_name(w)) })),
            "together": joint,
            "proposals": proposals,
            "relationships": rels,
            "asleep": x.asleep,
        });
        if let ActorId::Npc(c) = a {
            if let Some(n) = self.cast.get(c) {
                v["persona"] = json!({ "age": n.def.persona.age, "personality": n.def.persona.personality, "goals": n.def.persona.goals, "home": n.def.persona.home });
                v["needs"] = json!({ "hunger": r2(n.needs.hunger), "tiredness": r2(n.needs.fatigue), "loneliness": r2(n.needs.social), "boredom": r2(n.needs.fun), "curiosity": r2(n.needs.curiosity) });
                v["traits"] = serde_json::to_value(n.traits).unwrap_or_default();
                v["doing"] = json!(n.doing);
                v["goal"] = json!(n.goal);
                v["plan"] = json!(n.plan.iter().map(|s| serde_json::to_value(s).unwrap_or_default()).collect::<Vec<_>>());
                v["decisions"] = json!(n.decisions.iter().map(|(t, d)| json!({ "ago_s": r1((self.t - t) as f32), "what": d })).collect::<Vec<_>>());
            }
            let mems = self.db.memories(c).unwrap_or_default();
            v["memories"] = json!(mems.iter().rev().take(8).map(|m| m.text.clone()).collect::<Vec<_>>());
            v["summary"] = json!(self.db.summary(c).map(|s| s.0));
        }
        let key = a.key();
        let mut events = self.log.about(&key, Some(a), 8);
        if events.is_empty() {
            events = super::persist::events_about(&self.db, &key, 8);
        }
        v["events"] = json!(events);
        v
    }

    /// A few lines for the F2 overlay.
    pub fn inspect_lines(&mut self, t: &Target) -> Vec<String> {
        let v = self.inspect(t);
        let mut out = Vec::new();
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        let kind = s("kind").unwrap_or_default();
        let title = s("type").or_else(|| s("name")).unwrap_or_else(|| kind.clone());
        out.push(format!("{title}  ({kind}{})", v.get("id").map(|i| format!(" #{i}")).unwrap_or_default()));
        if let Some(at) = v.get("at") {
            out.push(format!("at {at}  tier {}", s("tier").unwrap_or_default()));
        }
        if let Some(h) = s("held_by") {
            out.push(format!("held by {h}"));
        }
        if let Some(d) = s("doing") {
            out.push(format!("doing: {d}"));
        }
        if let Some(g) = s("goal").filter(|g| !g.is_empty()) {
            out.push(format!("goal: {g}"));
        }
        if let Some(n) = v.get("needs") {
            out.push(format!("needs {n}"));
        }
        if let Some(Value::Object(c)) = v.get("changed").or(v.get("props")) {
            let mut parts: Vec<String> = c.iter().filter(|(k, _)| !matches!(k.as_str(), "mass" | "solid" | "friction" | "bounce") || v.get("changed").is_some()).map(|(k, x)| format!("{k} {x}")).collect();
            parts.truncate(10);
            if !parts.is_empty() {
                out.push(format!("props: {}", parts.join(", ")));
            }
        }
        if let Some(Value::Array(st)) = v.get("state") {
            if st.iter().any(|x| x.as_f64().unwrap_or(0.0) != 0.0) {
                out.push(format!("state: {}", Value::Array(st.clone())));
            }
        }
        if let Some(Value::Object(o)) = v.get("origin") {
            if !o.is_empty() {
                out.push(format!("origin: {}", Value::Object(o.clone())));
            }
        }
        if let Some(Value::Array(r)) = v.get("rules_fired") {
            let names: Vec<String> = r.iter().take(4).filter_map(|x| x.get("rule").and_then(|n| n.as_str()).map(str::to_string)).collect();
            if !names.is_empty() {
                out.push(format!("rules: {}", names.join(", ")));
            }
        }
        if let Some(Value::Array(r)) = v.get("relationships") {
            let parts: Vec<String> = r.iter().take(4).map(|x| format!("{} ({})", x["with"].as_str().unwrap_or("?"), x["seen_as"].as_str().unwrap_or(""))).collect();
            if !parts.is_empty() {
                out.push(format!("knows: {}", parts.join("; ")));
            }
        }
        if let Some(Value::Array(d)) = v.get("decisions") {
            if let Some(last) = d.last() {
                out.push(format!("last decision: {}", last["what"].as_str().unwrap_or("")));
            }
        }
        if let Some(Value::Array(e)) = v.get("events") {
            for x in e.iter().rev().take(3) {
                out.push(format!("· {}", x["text"].as_str().unwrap_or("")));
            }
        }
        if v.get("behavior").is_some_and(|b| !b.is_null()) {
            out.push("has behaviour code (pocket inspect shows it)".into());
        }
        out
    }
}
