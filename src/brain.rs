//! Everything that may be slow: LLM calls, validation and commits. Runs on a
//! tokio runtime plus one committer thread; talks to the main thread only
//! through channels, so walking never waits on AI.

use crate::db::Db;
use crate::decider::{Decider, DecisionCtx};
use crate::lang::probe::probe;
use crate::lang::{Diag, Stage, compile, format_diags};
use crate::llm::{Llm, Msg, Req, Role, extract_code, extract_json};
use crate::model::{CommitOk, CommitRequest, NewType, Placement, RegionCommit, TypeRef, WorldModel, region_info_from_plan};
use crate::prompts;
use crate::terrain::{Terrain, WATER_LEVEL};
use crate::world::characters::Decision;
use crate::world::describe::View;
use crate::world::{FigureLook, Look, Persona, REGION, WorldSnapshot};
use glam::Vec3;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Semaphore, mpsc, oneshot};

pub enum Cmd {
    Genesis,
    Region((i32, i32)),
    Create { text: String, view: View, target: Vec3, yaw: f32 },
    Undo,
    History,
    Talk { cid: i64, text: String, context: String, history: Vec<(bool, String)> },
    Decide { cid: i64, event: String, context: String },
    Witness { cid: i64, text: String, importance: f32 },
}

pub enum Event {
    Flip { snap: Arc<WorldSnapshot>, region: Option<((i32, i32), String)> },
    Log(String),
    Building(i32),
    RegionStarted((i32, i32)),
    RegionFinished((i32, i32), bool),
    Token { cid: i64, text: String },
    ReplyDone { cid: i64, ok: bool },
    Decision { cid: i64, decision: Decision },
    History(Vec<String>),
    GenesisDone,
}

enum CMsg {
    Commit(Box<CommitRequest>, oneshot::Sender<Result<CommitOk, Vec<Diag>>>),
    Undo(oneshot::Sender<Result<(Arc<WorldSnapshot>, String), String>>),
    History(oneshot::Sender<Vec<String>>),
    Info((i32, i32), oneshot::Sender<Info>),
}

struct Info {
    types: Vec<(u32, String, Vec<String>, [f32; 3])>,
    terrain: Arc<Terrain>,
    neighbours: Vec<String>,
    look: Arc<Look>,
}

#[derive(Clone)]
struct Ctx {
    llm: Arc<Llm>,
    db: Arc<Db>,
    bible: String,
    commit: crossbeam_channel::Sender<CMsg>,
    events: crossbeam_channel::Sender<Event>,
    decider: Arc<dyn Decider>,
    regions: Arc<Semaphore>,
}

pub struct Brain {
    tx: mpsc::UnboundedSender<Cmd>,
    _rt: tokio::runtime::Runtime,
    pub has_llm: bool,
}

impl Brain {
    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }

    /// Start the committer thread and the async runtime. `model` moves to the committer.
    /// `full_pending`: the main thread started from a partial snapshot; the
    /// committer builds the full world first and flips to it.
    pub fn start(model: WorldModel, llm: Option<Arc<Llm>>, events: crossbeam_channel::Sender<Event>, full_pending: bool) -> Brain {
        let db = model.db.clone();
        let bible = model.bible.clone();
        let (ctx_tx, ctx_rx) = crossbeam_channel::unbounded::<CMsg>();
        let ev_c = events.clone();
        std::thread::Builder::new()
            .name("committer".into())
            .stack_size(16 << 20)
            .spawn(move || {
                let mut model = model;
                if full_pending {
                    match model.snapshot() {
                        Ok(snap) => {
                            let _ = ev_c.send(Event::Flip { snap, region: None });
                        }
                        Err(d) => crate::log::error(format!("full world shader failed: {}", format_diags(&d))),
                    }
                }
                committer(model, ctx_rx)
            })
            .expect("spawn committer");
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).max_blocking_threads(4).thread_stack_size(8 << 20).enable_all().build().expect("tokio runtime");
        let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
        let has_llm = llm.is_some();
        let decider = crate::decider::from_env(llm.clone(), &bible);
        let ev2 = events.clone();
        let commit2 = ctx_tx.clone();
        rt.spawn(async move {
            let ctx = llm.map(|llm| Ctx { llm, db, bible, commit: commit2.clone(), events: ev2.clone(), decider, regions: Arc::new(Semaphore::new(2)) });
            while let Some(cmd) = rx.recv().await {
                // Commands that need no model.
                match cmd {
                    Cmd::Undo => {
                        let (otx, orx) = oneshot::channel();
                        let _ = commit2.send(CMsg::Undo(otx));
                        let ev = ev2.clone();
                        tokio::spawn(async move {
                            match orx.await {
                                Ok(Ok((snap, what))) => {
                                    let _ = ev.send(Event::Flip { snap, region: None });
                                    let _ = ev.send(Event::Log(format!("Undone: {what}.")));
                                }
                                Ok(Err(e)) => {
                                    let _ = ev.send(Event::Log(format!("Undo: {e}.")));
                                }
                                Err(_) => {}
                            }
                        });
                        continue;
                    }
                    Cmd::History => {
                        let (otx, orx) = oneshot::channel();
                        let _ = commit2.send(CMsg::History(otx));
                        let ev = ev2.clone();
                        tokio::spawn(async move {
                            if let Ok(h) = orx.await {
                                let _ = ev.send(Event::History(h));
                            }
                        });
                        continue;
                    }
                    _ => {}
                }
                let Some(ctx) = ctx.clone() else {
                    if let Cmd::Talk { cid, .. } = cmd {
                        let _ = ev2.send(Event::ReplyDone { cid, ok: false });
                    }
                    continue;
                };
                tokio::spawn(run(ctx, cmd));
            }
        });
        Brain { tx, _rt: rt, has_llm }
    }
}

fn committer(mut model: WorldModel, rx: crossbeam_channel::Receiver<CMsg>) {
    while let Ok(m) = rx.recv() {
        match m {
            CMsg::Commit(req, reply) => {
                let r = model.commit(*req);
                let _ = reply.send(r);
            }
            CMsg::Undo(reply) => {
                let _ = reply.send(model.undo());
            }
            CMsg::History(reply) => {
                let _ = reply.send(model.history());
            }
            CMsg::Info(r, reply) => {
                let _ = reply.send(Info { types: model.type_names(), terrain: model.terrain(), neighbours: model.region_names_near(r), look: model.look.clone() });
            }
        }
    }
}

impl Ctx {
    fn log(&self, s: impl Into<String>) {
        let _ = self.events.send(Event::Log(s.into()));
    }

    async fn info(&self, r: (i32, i32)) -> Option<Info> {
        let (tx, rx) = oneshot::channel();
        self.commit.send(CMsg::Info(r, tx)).ok()?;
        rx.await.ok()
    }

    async fn commit(&self, req: CommitRequest) -> Result<CommitOk, Vec<Diag>> {
        let (tx, rx) = oneshot::channel();
        self.commit.send(CMsg::Commit(Box::new(req), tx)).map_err(|_| vec![Diag::new(Stage::Gpu, 0, "committer stopped".into())])?;
        rx.await.map_err(|_| vec![Diag::new(Stage::Gpu, 0, "committer stopped".into())])?
    }

    fn record_failure(&self, name: &str, code: &str, diags: &[Diag]) {
        let text = format_diags(diags);
        crate::log::error(format!("type '{name}' failed:\n{text}\n--- code ---\n{code}"));
        let _ = self.db.with(|c| crate::db::add_type(c, None, name, code, "{}", "failed", &text));
    }
}

async fn run(ctx: Ctx, cmd: Cmd) {
    match cmd {
        Cmd::Genesis => {
            let _ = ctx.events.send(Event::Building(1));
            if let Err(e) = genesis(&ctx).await {
                crate::log::error(format!("genesis failed: {e}"));
                ctx.log(budget_msg(&e).unwrap_or_else(|| "The world's true shape stays hidden for now (generation failed; see ~/.pocket/pocket.log).".into()));
            }
            let _ = ctx.events.send(Event::Building(-1));
            let _ = ctx.events.send(Event::GenesisDone);
        }
        Cmd::Region(r) => {
            let _ = ctx.events.send(Event::RegionStarted(r));
            let permit = ctx.regions.clone().acquire_owned().await;
            let _ = ctx.events.send(Event::Building(1));
            let ok = match region(&ctx, r).await {
                Ok(()) => true,
                Err(e) => {
                    crate::log::error(format!("region {r:?} failed: {e:#}"));
                    if let Some(m) = budget_msg(&e) {
                        ctx.log(m);
                    }
                    false
                }
            };
            drop(permit);
            let _ = ctx.events.send(Event::Building(-1));
            let _ = ctx.events.send(Event::RegionFinished(r, ok));
        }
        Cmd::Create { text, view, target, yaw } => {
            let _ = ctx.events.send(Event::Building(1));
            if let Err(e) = create(&ctx, &text, &view, target, yaw).await {
                crate::log::error(format!("create '{text}' failed: {e:#}"));
                ctx.log(budget_msg(&e).unwrap_or_else(|| format!("Couldn't build that: {}", short(&e.to_string()))));
            }
            let _ = ctx.events.send(Event::Building(-1));
        }
        Cmd::Talk { cid, text, context, history } => {
            let ok = match talk(&ctx, cid, &text, &context, &history).await {
                Ok(()) => true,
                Err(e) => {
                    crate::log::error(format!("dialogue failed: {e:#}"));
                    ctx.log(budget_msg(&e).unwrap_or_else(|| format!("(no reply: {})", short(&e.to_string()))));
                    false
                }
            };
            let _ = ctx.events.send(Event::ReplyDone { cid, ok });
        }
        Cmd::Decide { cid, event, context } => {
            let persona = ctx.db.characters().ok().and_then(|cs| cs.into_iter().find(|c| c.id == cid)).map(|c| c.persona_json).unwrap_or_default();
            let name = serde_json::from_str::<Persona>(&persona).map(|p| p.name).unwrap_or_default();
            let summary = ctx.db.summary(cid).map(|s| s.0).unwrap_or_default();
            let dctx = DecisionCtx { character: name, persona, event, context: format!("{context}\nWhat they remember of the traveller: {summary}") };
            if let Some(d) = ctx.decider.decide(dctx).await {
                let _ = ctx.events.send(Event::Decision { cid, decision: d });
            }
        }
        Cmd::Witness { cid, text, importance } => {
            let _ = ctx.db.add_memory(cid, crate::db::now(), &text, importance);
        }
        Cmd::Undo | Cmd::History => {}
    }
}

fn budget_msg(e: &anyhow::Error) -> Option<String> {
    e.downcast_ref::<crate::llm::BudgetReached>().map(|_| "Budget reached: generation is paused (walking still works).".to_string())
}

fn short(s: &str) -> String {
    let s = s.lines().next().unwrap_or("");
    if s.chars().count() > 90 { format!("{}…", s.chars().take(90).collect::<String>()) } else { s.to_string() }
}

/// Steps 1–4 on the worker pool.
async fn validate(code: String) -> Result<NewType, Vec<Diag>> {
    tokio::task::spawn_blocking(move || {
        let ct = compile(&code)?;
        let report = probe(&ct)?;
        Ok(NewType { ct: Arc::new(ct), report: Arc::new(report) })
    })
    .await
    .unwrap_or_else(|e| Err(vec![Diag::new(Stage::Translate, 0, format!("validator crashed: {e}"))]))
}

/// Ask for a type, validate it, and feed errors back up to two times.
async fn build_type(ctx: &Ctx, system: &str, task: String, name: &str, extra_tags: &[&str]) -> anyhow::Result<NewType> {
    let mut msgs = vec![Msg { user: true, text: task }];
    let mut last = String::new();
    let mut last_code = String::new();
    for _attempt in 0..3 {
        let mut req = Req { role: Role::Builder, system: system.to_string(), system_tail: String::new(), messages: std::mem::take(&mut msgs), max_tokens: 16000, effort: Some("medium") };
        let reply = ctx.llm.complete(&req, "type").await?;
        msgs = std::mem::take(&mut req.messages);
        let diags = match extract_code(&reply) {
            None => vec![Diag::new(Stage::Parse, 0, "no ```js code block found".into())],
            Some(code) => {
                let code = add_tags(&code, extra_tags);
                last_code = code.clone();
                match validate(code).await {
                    Ok(t) => return Ok(t),
                    Err(d) => d,
                }
            }
        };
        last = format_diags(&diags);
        if !diags.iter().all(|d| d.stage.repairable()) {
            break;
        }
        msgs.push(Msg { user: false, text: reply });
        msgs.push(Msg { user: true, text: prompts::repair(&last) });
    }
    ctx.record_failure(name, &last_code, &[Diag::new(Stage::Probe, 0, last.clone())]);
    anyhow::bail!("couldn't build the {name}: {last}")
}

/// Append tags to `meta.tags` textually (used for "base" types).
fn add_tags(code: &str, tags: &[&str]) -> String {
    if tags.is_empty() {
        return code.to_string();
    }
    if let Some(i) = code.find("tags:") {
        if let Some(j) = code[i..].find('[') {
            let at = i + j + 1;
            let ins: String = tags.iter().map(|t| format!("\"{t}\", ")).collect();
            return format!("{}{}{}", &code[..at], ins, &code[at..]);
        }
    }
    code.to_string()
}

fn all_code_blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("```") {
        let after = &rest[i + 3..];
        let nl = after.find('\n').unwrap_or(0);
        let lang = after[..nl].trim().to_lowercase();
        let body = &after[nl..];
        let Some(j) = body.find("```") else { break };
        if lang == "js" || lang == "javascript" {
            out.push(body[..j].trim().to_string());
        }
        rest = &body[j + 3..];
    }
    out
}

async fn genesis(ctx: &Ctx) -> anyhow::Result<()> {
    let system = prompts::builder_system(&ctx.bible);
    let mut req = Req::new(Role::Builder, system.clone(), prompts::GENESIS_TASK);
    req.max_tokens = 32000;
    req.effort = Some("medium");
    let reply = ctx.llm.complete(&req, "genesis").await?;
    let v = extract_json(&reply)?;
    let mut look: Look = serde_json::from_value(v.clone()).unwrap_or_default();
    if look.biomes.is_empty() {
        look.biomes = crate::terrain::default_biomes();
    }
    for b in &mut look.biomes {
        b.scatter.entry("grass".into()).or_insert(1.0);
    }
    // Validate each base type; repair failures one at a time.
    let mut types = Vec::new();
    let mut futs = Vec::new();
    let system_ref = &system;
    for code in all_code_blocks(&reply).into_iter().take(8) {
        let code = add_tags(&code, &["base"]);
        let system = system_ref;
        futs.push(async move {
            match validate(code.clone()).await {
                Ok(t) => Some(t),
                Err(d) if d.iter().all(|x| x.stage.repairable()) => {
                    let name = code.split("name:").nth(1).and_then(|s| s.split('"').nth(1)).unwrap_or("object").to_string();
                    let task = format!("This base type failed validation:\n```js\n{code}\n```\nErrors:\n{}\n\n{}", format_diags(&d), prompts::TYPE_TASK);
                    build_type(ctx, system, task, &name, &[]).await.ok().map(|mut t| {
                        if !t.ct.meta.tags.iter().any(|x| x == "base") {
                            let mut ct = (*t.ct).clone();
                            ct.meta.tags.push("base".into());
                            t.ct = Arc::new(ct);
                        }
                        t
                    })
                }
                Err(d) => {
                    ctx.record_failure("base type", &code, &d);
                    None
                }
            }
        });
    }
    for t in futures_util::future::join_all(futs).await.into_iter().flatten() {
        types.push(t);
    }
    let name = look.name.clone();
    let n = types.len();
    let ok = ctx
        .commit(CommitRequest { kind: "genesis", summary: format!("genesis: {} with {n} base types", if name.is_empty() { "the land" } else { &name }), new_types: types, placements: vec![], nudge: true, region: None, look: Some(look) })
        .await
        .map_err(|d| anyhow::anyhow!(format_diags(&d)))?;
    ctx.db.kv_set("genesis", "done")?;
    let _ = ctx.events.send(Event::Flip { snap: ok.snapshot, region: None });
    ctx.log(if name.is_empty() { "The world settles into its true shape.".to_string() } else { format!("The world settles into its true shape: {name}.") });
    Ok(())
}

/// Terrain notes for a region plan: heights, water, biomes, flat spots, a coarse map.
fn region_notes(t: &Terrain, r: (i32, i32)) -> String {
    let ox = r.0 as f32 * REGION;
    let oz = r.1 as f32 * REGION;
    let n = 17;
    let step = REGION / (n - 1) as f32;
    let mut h = vec![0.0f32; n * n];
    let mut biome_count = std::collections::BTreeMap::<String, usize>::new();
    for j in 0..n {
        for i in 0..n {
            let (x, z) = (ox + i as f32 * step, oz + j as f32 * step);
            h[j * n + i] = t.height(x, z);
            *biome_count.entry(t.biomes[t.biome_at(x, z)].name.clone()).or_default() += 1;
        }
    }
    let water = h.iter().filter(|v| **v < WATER_LEVEL).count() as f32 / h.len() as f32;
    let (mut hi, mut lo) = (0, 0);
    for k in 0..h.len() {
        if h[k] > h[hi] {
            hi = k;
        }
        if h[k] < h[lo] {
            lo = k;
        }
    }
    let at = |k: usize| ((k % n) as f32 * step, (k / n) as f32 * step);
    // Flat dry spots.
    let mut flat: Vec<(f32, usize)> = Vec::new();
    for j in 1..n - 1 {
        for i in 1..n - 1 {
            let k = j * n + i;
            if h[k] < WATER_LEVEL + 1.0 {
                continue;
            }
            let s = (h[k + 1] - h[k - 1]).abs() + (h[k + n] - h[k - n]).abs();
            flat.push((s, k));
        }
    }
    flat.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut spots: Vec<usize> = Vec::new();
    for (_, k) in flat {
        let (x, z) = at(k);
        if spots.iter().all(|s| {
            let (sx, sz) = at(*s);
            ((sx - x).powi(2) + (sz - z).powi(2)).sqrt() > 60.0
        }) {
            spots.push(k);
        }
        if spots.len() >= 3 {
            break;
        }
    }
    let mut map = String::new();
    for j in (0..n).step_by(2) {
        for i in (0..n).step_by(2) {
            let v = h[j * n + i];
            map.push(match v {
                v if v < WATER_LEVEL => '~',
                v if v < 3.0 => '.',
                v if v < 12.0 => '-',
                v if v < 25.0 => '+',
                _ => '^',
            });
        }
        map.push('\n');
    }
    let biomes: Vec<String> = biome_count.iter().rev().map(|(k, v)| format!("{k} {}%", v * 100 / (n * n))).collect();
    let (hx, hz) = at(hi);
    let (lx, lz) = at(lo);
    let spots: Vec<String> = spots.iter().map(|k| {
        let (x, z) = at(*k);
        format!("({x:.0}, {z:.0}) at {:.0} m", h[*k])
    }).collect();
    format!(
        "Biomes: {}. Water covers {:.0}% of the region (sea level 0 m).\nHighest point ({hx:.0}, {hz:.0}) at {:.0} m; lowest ({lx:.0}, {lz:.0}) at {:.0} m.\nFlat dry spots: {}.\nMap (x → right, z ↓ down, 32 m per cell; ~ water, . shore/low, - plain, + hills, ^ high):\n{map}",
        biomes.join(", "),
        water * 100.0,
        h[hi],
        h[lo],
        if spots.is_empty() { "none".into() } else { spots.join(", ") }
    )
}

fn f(v: &Value, k: &str, d: f32) -> f32 {
    v.get(k).and_then(|x| x.as_f64()).map(|x| x as f32).filter(|x| x.is_finite()).unwrap_or(d)
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

async fn region(ctx: &Ctx, r: (i32, i32)) -> anyhow::Result<()> {
    let info = ctx.info(r).await.ok_or_else(|| anyhow::anyhow!("committer gone"))?;
    let system = prompts::builder_system(&ctx.bible);
    let type_list: Vec<String> = info.types.iter().map(|(_, n, tags, b)| format!("- {n} [{}] ~{:.0}×{:.0}×{:.0} m", tags.join(", "), b[0] * 2.0, b[1] * 2.0, b[2] * 2.0)).collect();
    let task = format!(
        "{}\n\nRegion ({}, {}) in the land of {}.\nTerrain notes:\n{}\nNeighbouring regions: {}\nExisting object types:\n{}",
        prompts::REGION_TASK,
        r.0,
        r.1,
        if info.look.name.is_empty() { "this universe" } else { &info.look.name },
        region_notes(&info.terrain, r),
        if info.neighbours.is_empty() { "none yet".into() } else { info.neighbours.join("; ") },
        type_list.join("\n")
    );
    let mut req = Req::new(Role::Builder, system.clone(), task);
    req.max_tokens = 16000;
    req.effort = Some("medium");
    let reply = ctx.llm.complete(&req, "region").await?;
    let plan = extract_json(&reply)?;
    let rname = s(&plan, "name");

    // New types, in parallel.
    let mut new_types: Vec<NewType> = Vec::new();
    let mut new_names: Vec<(String, usize)> = Vec::new();
    let specs: Vec<Value> = plan.get("new_types").and_then(|x| x.as_array()).cloned().unwrap_or_default().into_iter().take(3).collect();
    let futs = specs.iter().map(|spec| {
        let name = s(spec, "name");
        let task = format!(
            "Object type to write: \"{name}\"\nDescription: {}\nApproximate size (w × h × d, metres): {}\nTags: {}\nIt appears in the region \"{rname}\" ({}).\n\n{}",
            s(spec, "description"),
            spec.get("size_m").map(|v| v.to_string()).unwrap_or_else(|| "unspecified".into()),
            spec.get("tags").map(|v| v.to_string()).unwrap_or_else(|| "[]".into()),
            s(&plan, "mood"),
            prompts::TYPE_TASK
        );
        let system = system.clone();
        async move { (name.clone(), build_type(ctx, &system, task, &name, &[]).await) }
    });
    for (name, res) in futures_util::future::join_all(futs).await {
        match res {
            Ok(t) => {
                let i = new_types.len();
                new_names.push((name.to_lowercase(), i));
                new_names.push((t.ct.meta.name.to_lowercase(), i));
                new_types.push(t);
            }
            Err(e) => {
                if let Some(m) = budget_msg(&e) {
                    ctx.log(m);
                } else {
                    ctx.log(format!("Couldn't build the {name}."));
                }
            }
        }
    }
    let resolve = |name: &str| -> Option<TypeRef> {
        let n = name.trim().to_lowercase();
        if let Some((_, i)) = new_names.iter().find(|(k, _)| *k == n) {
            return Some(TypeRef::New(*i));
        }
        info.types.iter().rev().find(|(_, tn, _, _)| tn.to_lowercase() == n).map(|(id, ..)| TypeRef::Existing(*id))
    };
    let ox = r.0 as f32 * REGION;
    let oz = r.1 as f32 * REGION;
    let clampl = |v: f32| v.clamp(4.0, REGION - 4.0);
    let mut placements = Vec::new();
    for lm in plan.get("landmarks").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().take(3) {
        let Some(ty) = resolve(&s(lm, "type")) else { continue };
        placements.push(Placement {
            ty,
            x: ox + clampl(f(lm, "x", 128.0)),
            z: oz + clampl(f(lm, "z", 128.0)),
            y: None,
            rot_y: f(lm, "rot", 0.0).to_radians(),
            scale: f(lm, "scale", 1.0).clamp(0.3, 4.0),
            params: [0.0, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
        });
    }
    if let Some(st) = plan.get("settlement").filter(|v| v.is_object()) {
        let cx = ox + clampl(f(st, "x", 128.0));
        let cz = oz + clampl(f(st, "z", 128.0));
        for (i, b) in st.get("buildings").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().take(6).enumerate() {
            let Some(ty) = resolve(&s(b, "type")) else { continue };
            placements.push(Placement {
                ty,
                x: cx + f(b, "dx", 0.0).clamp(-60.0, 60.0),
                z: cz + f(b, "dz", 0.0).clamp(-60.0, 60.0),
                y: None,
                rot_y: f(b, "rot", 0.0).to_radians(),
                scale: 1.0,
                params: [i as f32 + 1.0, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
            });
        }
    }
    let mut chars = Vec::new();
    for c in plan.get("characters").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().take(6) {
        let mut p: Persona = serde_json::from_value(c.clone()).unwrap_or_default();
        if p.name.trim().is_empty() {
            continue;
        }
        let lk = c.get("look").cloned().unwrap_or_default();
        p.look = FigureLook {
            height: f(&lk, "height", 1.75).clamp(1.3, 2.0),
            build: f(&lk, "build", 1.0).clamp(0.7, 1.4),
            skin: f(&lk, "skin", 0.3).clamp(0.0, 1.0),
            shirt_hue: f(&lk, "shirt_hue", 0.6).rem_euclid(1.0),
            trousers_hue: f(&lk, "trousers_hue", 0.1).rem_euclid(1.0),
        };
        let mut home = Vec3::new(ox + clampl(f(c, "home_x", 128.0)), 0.0, oz + clampl(f(c, "home_z", 128.0)));
        // Homes must be on dry land: spiral out until we find some.
        for k in 0..40 {
            if info.terrain.height(home.x, home.z) > WATER_LEVEL + 0.5 {
                break;
            }
            let a = k as f32 * 2.399;
            home += Vec3::new(a.cos(), 0.0, a.sin()) * (3.0 + k as f32);
        }
        chars.push((p, home));
    }
    let n_chars = chars.len();
    let ok = ctx
        .commit(CommitRequest {
            kind: "region",
            summary: format!("region {} ({}, {})", if rname.is_empty() { "unnamed" } else { &rname }, r.0, r.1),
            new_types,
            placements,
            nudge: true,
            region: Some(RegionCommit { r, plan_json: plan.to_string(), info: region_info_from_plan(&plan.to_string()), characters: chars }),
            look: None,
        })
        .await
        .map_err(|d| anyhow::anyhow!(format_diags(&d)))?;
    for d in &ok.dropped {
        crate::log::info(format!("region {r:?}: dropped placement: {d}"));
    }
    crate::log::info(format!("region {r:?} '{rname}': {} placed, {} dropped, {n_chars} characters", ok.placed, ok.dropped.len()));
    let _ = ctx.events.send(Event::Flip { snap: ok.snapshot, region: Some((r, rname)) });
    Ok(())
}

async fn create(ctx: &Ctx, text: &str, view: &View, target: Vec3, yaw: f32) -> anyhow::Result<()> {
    let info = ctx.info(crate::world::region_of(target.x, target.z)).await.ok_or_else(|| anyhow::anyhow!("committer gone"))?;
    let system = prompts::builder_system(&ctx.bible);
    let type_list: Vec<String> = info.types.iter().map(|(_, n, tags, b)| format!("- {n} [{}] ~{:.0}×{:.0}×{:.0} m", tags.join(", "), b[0] * 2.0, b[1] * 2.0, b[2] * 2.0)).collect();
    let task = format!(
        "{}\nWhat the player sees:\n```json\n{}\n```\nExisting object types:\n{}\n\nThe player typed: \"{}\"",
        prompts::CREATE_TASK,
        serde_json::to_string_pretty(view)?,
        type_list.join("\n"),
        text
    );
    let mut msgs = vec![Msg { user: true, text: task }];
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let fwd = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    let mut last_err = String::new();
    for attempt in 0..3 {
        let mut req = Req { role: Role::Builder, system: system.clone(), system_tail: String::new(), messages: std::mem::take(&mut msgs), max_tokens: 16000, effort: Some("medium") };
        let reply = ctx.llm.complete(&req, "create").await?;
        msgs = std::mem::take(&mut req.messages);
        let outcome: Result<CommitOk, Vec<Diag>> = async {
            let v = extract_json(&reply).map_err(|e| vec![Diag::new(Stage::Parse, 0, format!("reply JSON: {e}"))])?;
            let reuse = v.get("reuse").and_then(|x| x.as_str()).map(str::to_string).filter(|x| !x.is_empty() && x != "null");
            let mut new_types = Vec::new();
            let ty = match reuse {
                Some(name) => match info.types.iter().rev().find(|(_, n, ..)| n.eq_ignore_ascii_case(name.trim())) {
                    Some((id, ..)) => TypeRef::Existing(*id),
                    None => return Err(vec![Diag::new(Stage::Place, 0, format!("there is no existing type named \"{name}\"; set reuse to null and write the type"))]),
                },
                None => {
                    let code = all_code_blocks(&reply).into_iter().next().or_else(|| extract_code(&reply)).ok_or_else(|| vec![Diag::new(Stage::Parse, 0, "no ```js block with the new type".into())])?;
                    new_types.push(validate(code).await?);
                    TypeRef::New(0)
                }
            };
            let mut placements = Vec::new();
            for p in v.get("placements").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().take(24) {
                let pos = target + right * f(p, "right", 0.0).clamp(-80.0, 80.0) + fwd * f(p, "forward", 0.0).clamp(-80.0, 80.0);
                let scale = f(p, "scale", 1.0).clamp(0.2, 5.0);
                let lift = p.get("lift").and_then(|x| x.as_f64()).map(|x| x as f32);
                let y = lift.map(|l| {
                    let bottom = match &ty {
                        TypeRef::New(i) => new_types[*i].report.bottom,
                        TypeRef::Existing(_) => 0.0,
                    };
                    info.terrain.height(pos.x, pos.z) + l - bottom * scale
                });
                placements.push(Placement { ty: ty.clone(), x: pos.x, z: pos.z, y, rot_y: yaw + f(p, "rot", 0.0).to_radians(), scale, params: [placements.len() as f32, scale, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5] });
            }
            if placements.is_empty() {
                placements.push(Placement { ty, x: target.x, z: target.z, y: None, rot_y: yaw, scale: 1.0, params: [0.0, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5] });
            }
            let summary = {
                let s0 = s(&v, "summary");
                if s0.is_empty() { text.to_string() } else { s0 }
            };
            ctx.commit(CommitRequest { kind: "create", summary, new_types, placements, nudge: false, region: None, look: None }).await
        }
        .await;
        match outcome {
            Ok(ok) => {
                let name = ok.snapshot.instances.last().and_then(|p| ok.snapshot.type_of(p.type_id)).map(|t| t.name().to_string()).unwrap_or_else(|| "it".into());
                let _ = ctx.events.send(Event::Flip { snap: ok.snapshot, region: None });
                ctx.log(format!("Built the {name}. (/undo to remove)"));
                return Ok(());
            }
            Err(d) => {
                last_err = format_diags(&d);
                crate::log::info(format!("create attempt {attempt} failed:\n{last_err}"));
                if !d.iter().all(|x| x.stage.repairable()) {
                    break;
                }
                msgs.push(Msg { user: false, text: reply });
                msgs.push(Msg { user: true, text: prompts::repair(&last_err) });
            }
        }
    }
    anyhow::bail!("{}", last_err.lines().next().unwrap_or("validation failed").trim_start_matches(|c: char| c == '[' || c.is_alphabetic() || c == ']').trim())
}

fn words(s: &str) -> std::collections::HashSet<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 3).map(str::to_string).collect()
}

fn importance(text: &str) -> f32 {
    let mut i: f32 = 0.3;
    let t = text.to_lowercase();
    for k in ["promise", "secret", "name", "help", "love", "hate", "kill", "find", "lost", "remember", "gift", "owe", "vanish", "danger"] {
        if t.contains(k) {
            i += 0.15;
        }
    }
    i.min(1.0)
}

async fn talk(ctx: &Ctx, cid: i64, text: &str, context: &str, history: &[(bool, String)]) -> anyhow::Result<()> {
    let row = ctx.db.characters()?.into_iter().find(|c| c.id == cid).ok_or_else(|| anyhow::anyhow!("unknown character"))?;
    let persona: Persona = serde_json::from_str(&row.persona_json).unwrap_or_default();
    let region = ctx.db.regions()?.into_iter().find(|r| format!("{},{}", r.rx, r.rz) == row.region);
    let rinfo = region.map(|r| region_info_from_plan(&r.plan_json)).unwrap_or_default();
    let summary = ctx.db.summary(cid).map(|s| s.0).unwrap_or_else(|| "I have not met the traveller before.".into());
    let mems = ctx.db.memories(cid)?;
    // Most relevant and most recent memories.
    let q = words(text);
    let mut scored: Vec<(f32, &crate::db::MemoryRow)> = mems
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let overlap = words(&m.text).intersection(&q).count() as f32;
            let recency = (i + 1) as f32 / mems.len().max(1) as f32;
            (overlap * 1.0 + m.importance + recency * 0.5, m)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut chosen: Vec<&crate::db::MemoryRow> = scored.iter().take(6).map(|x| x.1).collect();
    for m in mems.iter().rev().take(6) {
        if !chosen.iter().any(|c| c.id == m.id) {
            chosen.push(m);
        }
    }
    chosen.sort_by_key(|m| m.id);
    let memories: Vec<String> = chosen.iter().map(|m| format!("- {}", m.text)).collect();
    let system = format!(
        "{}\n\nThe universe:\n{}\n\nYou are {}{}.\nAppearance: {}\nPersonality: {}\nGoals: {}\nVoice: {}\nHome: {}\nRelationships: {}\n\nWhat people in {} know:\n{}",
        prompts::DIALOGUE_RULES,
        ctx.bible,
        persona.name,
        if persona.age > 0 { format!(", aged {}", persona.age) } else { String::new() },
        persona.appearance,
        persona.personality,
        persona.goals,
        persona.voice,
        persona.home,
        persona.relationships.join("; "),
        if rinfo.name.is_empty() { "this place".to_string() } else { rinfo.name.clone() },
        rinfo.facts.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")
    );
    let tail = format!(
        "Your memory of the traveller so far: {summary}\n\nSpecific memories:\n{}\n\nRight now: {context}",
        if memories.is_empty() { "(none yet)".into() } else { memories.join("\n") }
    );
    let mut messages: Vec<Msg> = history.iter().rev().take(8).rev().map(|(u, t)| Msg { user: *u, text: t.clone() }).collect();
    // The API needs alternating turns starting with the user.
    while messages.first().is_some_and(|m| !m.user) {
        messages.remove(0);
    }
    messages.push(Msg { user: true, text: text.to_string() });
    let req = Req { role: Role::Character, system, system_tail: tail, messages, max_tokens: 1024, effort: Some("low") };
    let ev = ctx.events.clone();
    let reply = ctx.llm.stream(&req, "dialogue", |t| {
        let _ = ev.send(Event::Token { cid, text: t.to_string() });
    })
    .await?;
    let now = crate::db::now();
    let mem = format!("The traveller said: \"{}\" I replied: \"{}\"", text.trim(), reply.trim());
    ctx.db.add_memory(cid, now, &mem, importance(&mem))?;
    // Refresh the rolling summary every few memories.
    let n = ctx.db.memories(cid)?.len();
    if n % 5 == 1 {
        let ctx2 = ctx.clone();
        let name = persona.name.clone();
        tokio::spawn(async move {
            let mems = ctx2.db.memories(cid).unwrap_or_default();
            let prev = ctx2.db.summary(cid).map(|s| s.0).unwrap_or_default();
            let recent: Vec<String> = mems.iter().rev().take(12).rev().map(|m| format!("- {}", m.text)).collect();
            let user = format!("Character: {name}\nPrevious summary: {prev}\nRecent memories:\n{}", recent.join("\n"));
            let mut req = Req::new(Role::Summarizer, prompts::SUMMARY_TASK, user);
            req.max_tokens = 600;
            if let Ok(s) = ctx2.llm.complete(&req, "summary").await {
                let _ = ctx2.db.set_summary(cid, s.trim());
            }
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_blocks_and_tags() {
        let t = "intro\n```json\n{}\n```\n```js\nA\n```\ntext\n```javascript\nB\n```";
        assert_eq!(all_code_blocks(t), vec!["A", "B"]);
        assert_eq!(add_tags("tags: [\"tree\"]", &["base"]), "tags: [\"base\", \"tree\"]");
    }

    #[test]
    fn region_notes_mention_water_and_map() {
        let t = Terrain::new(3, crate::terrain::default_biomes());
        let n = region_notes(&t, (0, 0));
        assert!(n.contains("Water covers"));
        assert_eq!(n.lines().filter(|l| l.len() == 9 && l.chars().all(|c| "~.-+^".contains(c))).count(), 9);
    }
}
