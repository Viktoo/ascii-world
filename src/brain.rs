//! Everything that may be slow: LLM calls, validation and commits. Runs on a
//! tokio runtime plus one committer thread; talks to the main thread only
//! through channels, so walking never waits on AI.

use crate::db::Db;
use crate::decider::{Decider, DecisionCtx};
use crate::lang::probe::probe;
use crate::lang::{Diag, Stage, compile, format_diags};
use crate::llm::{Llm, Msg, Req, Role, extract_code, extract_json};
use crate::model::{CommitOk, CommitRequest, NewType, Placement, RegionCommit, TypeRef, WorldModel, region_info_from_plan};
use crate::pace::{self, Pacer, Task};
use crate::prompts;
use crate::terrain::{Terrain, WATER_LEVEL};
use crate::world::characters::Decision;
use crate::world::describe::View;
use crate::world::{Look, Persona, REGION, WorldSnapshot};
use glam::Vec3;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Semaphore, mpsc, oneshot};

pub enum Cmd {
    Genesis,
    Region((i32, i32)),
    /// `id` identifies the request (answered by `Event::Created`); `by` is a
    /// character making something (None: the player).
    Create { id: Option<u64>, by: Option<(i64, String)>, text: String, view: View, target: Vec3, yaw: f32 },
    Undo,
    History,
    Talk { cid: i64, text: String, context: String, history: Vec<(bool, String)> },
    Decide { cid: i64, event: String, context: String },
    Witness { cid: i64, text: String, importance: f32 },
    /// What does this action do? Answered by `Event::Interpreted`.
    Interpret { id: u64, actor: String, text: String, context: String },
    /// Two characters talk (the player can hear). Answered by `Event::ChatLines`.
    Chat { a: i64, b: i64, context: String },
    /// Write a new object type. Answered by `Event::TypeBuilt`.
    BuildType { id: u64, name: String, description: String, size: [f32; 3], props: Vec<(String, f32)>, fits: Option<String> },
    /// Write a gesture's pose keyframes. Answered by `Event::GestureBuilt`.
    BuildGesture { id: u64, name: String, body: String },
    /// Rewrite one thing's shape code. Answered by `Event::TypeBuilt`.
    EditType { id: u64, name: String, source: String, change: String, spot: String, cuts: Vec<[f32; 4]>, with: Option<(String, String, f32)> },
    /// Write a new species from a brief (`fixed` laid over it). Answered by `Event::SpeciesMade`.
    NewSpecies { id: u64, brief: String, fixed: Value },
    /// A species (JSON) on a generic body gets its own, written from
    /// `template`. Answered by `Event::TypeBuilt`.
    SpeciesBody { id: u64, species: String, template: String },
    /// One being's body reshaped: `source` (the body `from`) changed as
    /// `change` says, as the body `name`. Answered by `Event::TypeBuilt`.
    ReshapeBody { id: u64, name: String, from: String, source: String, change: String },
}

pub enum Event {
    Flip { snap: Arc<WorldSnapshot>, region: Option<((i32, i32), String)> },
    Log(String),
    Building(i32),
    RegionStarted((i32, i32)),
    RegionFinished((i32, i32), bool),
    Token { cid: i64, text: String },
    ReplyDone { cid: i64, ok: bool },
    /// A character's decision (None: nothing to do; every Decide is answered).
    Decision { cid: i64, decision: Option<Decision> },
    History(Vec<String>),
    GenesisDone,
    Interpreted { id: u64, result: Result<Value, String> },
    Created { id: Option<u64>, instances: Vec<i64> },
    ChatLines { a: i64, b: i64, lines: Vec<(i64, String)> },
    TypeBuilt { id: u64, type_id: Option<u32> },
    /// The universe's own properties and rules changed (genesis).
    RulesChanged,
    GestureBuilt { id: u64, result: Result<Value, String> },
    /// How far along genesis or a region is (0..1), for the loading screen.
    Progress { task: Task, frac: f32 },
    /// Something named as the world is made: ("shaping", "lighthouse").
    Made { task: Task, verb: &'static str, name: String },
    /// A species was written (None: it failed).
    SpeciesMade { id: u64, name: Option<String> },
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
    /// The region the traveler begins in.
    start: (i32, i32),
}

/// What every request is told the universe is: the land once genesis has
/// written it, the player's own prompt before that (and in older worlds).
pub type Universe = Arc<std::sync::RwLock<String>>;

#[derive(Clone)]
struct Ctx {
    llm: Arc<Llm>,
    db: Arc<Db>,
    /// The player's own prompt, for genesis.
    prompt: String,
    universe: Universe,
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
        let prompt = model.bible.clone();
        let universe: Universe = Arc::new(std::sync::RwLock::new(if model.look.land.is_empty() { prompt.clone() } else { model.look.land.clone() }));
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
        let decider = crate::decider::from_env(llm.clone(), universe.clone());
        let ev2 = events.clone();
        let commit2 = ctx_tx.clone();
        let mem_db = db.clone();
        rt.spawn(async move {
            let ctx = llm.map(|llm| Ctx { llm, db, prompt, universe, commit: commit2.clone(), events: ev2.clone(), decider, regions: Arc::new(Semaphore::new(2)) });
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
                    // Memories need no model.
                    Cmd::Witness { cid, text, importance } => {
                        let _ = mem_db.add_memory(cid, crate::db::now(), &text, importance);
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
                    // No LLM: every request still gets an (empty) answer.
                    let reply = match cmd {
                        Cmd::Talk { cid, .. } => Some(Event::ReplyDone { cid, ok: false }),
                        Cmd::Decide { cid, .. } => Some(Event::Decision { cid, decision: None }),
                        Cmd::Interpret { id, .. } => Some(Event::Interpreted { id, result: Err("no LLM".into()) }),
                        Cmd::Create { id, .. } => Some(Event::Created { id, instances: vec![] }),
                        Cmd::Chat { a, b, .. } => Some(Event::ChatLines { a, b, lines: vec![] }),
                        Cmd::BuildType { id, .. } | Cmd::EditType { id, .. } | Cmd::SpeciesBody { id, .. } | Cmd::ReshapeBody { id, .. } => Some(Event::TypeBuilt { id, type_id: None }),
                        Cmd::BuildGesture { id, .. } => Some(Event::GestureBuilt { id, result: Err("no LLM".into()) }),
                        Cmd::NewSpecies { id, .. } => Some(Event::SpeciesMade { id, name: None }),
                        _ => None,
                    };
                    if let Some(r) = reply {
                        let _ = ev2.send(r);
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
                let _ = reply.send(Info { types: model.type_names(), terrain: model.terrain(), neighbours: model.region_names_near(r), look: model.look.clone(), start: crate::world::region_of(model.spawn.x, model.spawn.z) });
            }
        }
    }
}

impl Ctx {
    fn universe(&self) -> String {
        self.universe.read().map(|u| u.clone()).unwrap_or_default()
    }

    /// This universe's vocabulary (built-in, its own, and learned aliases).
    fn vocab(&self) -> crate::sim::props::Vocab {
        crate::sim::persist::world_vocab(&self.db)
    }

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
        Cmd::Create { id, by, text, view, target, yaw } => {
            let _ = ctx.events.send(Event::Building(1));
            match create(&ctx, &text, &view, target, yaw, by.as_ref()).await {
                Ok(instances) => {
                    let _ = ctx.events.send(Event::Created { id, instances });
                }
                Err(e) => {
                    crate::log::error(format!("create '{text}' failed: {e:#}"));
                    if by.is_none() {
                        ctx.log(budget_msg(&e).unwrap_or_else(|| format!("Couldn't build that: {}", short(&e.to_string()))));
                    }
                    let _ = ctx.events.send(Event::Created { id, instances: vec![] });
                }
            }
            let _ = ctx.events.send(Event::Building(-1));
        }
        Cmd::Interpret { id, actor, text, context } => {
            let result = interpret(&ctx, &actor, &text, &context).await.map_err(|e| {
                crate::log::error(format!("interpret '{text}' failed: {e:#}"));
                short(&e.to_string())
            });
            let _ = ctx.events.send(Event::Interpreted { id, result });
        }
        Cmd::Chat { a, b, context } => {
            let lines = match chat(&ctx, a, b, &context).await {
                Ok(l) => l,
                Err(e) => {
                    crate::log::error(format!("chat failed: {e:#}"));
                    vec![]
                }
            };
            let _ = ctx.events.send(Event::ChatLines { a, b, lines });
        }
        Cmd::BuildGesture { id, name, body } => {
            let (base, task) = match name.split_once('@') {
                Some((g, sp)) => (g.to_string(), format!("The gesture: \"{g}\", as a {sp} does it.\nThe body: {body}")),
                None => (name.clone(), format!("The gesture: \"{name}\"")),
            };
            let _ = base;
            let mut req = Req::new(Role::Decider, prompts::GESTURE_TASK, task);
            req.max_tokens = 600;
            req.effort = Some("low");
            let result = match ctx.llm.complete(&req, "gesture").await {
                Ok(reply) => extract_json(&reply).map_err(|e| e.to_string()),
                Err(e) => Err(short(&e.to_string())),
            };
            let _ = ctx.events.send(Event::GestureBuilt { id, result });
        }
        Cmd::BuildType { id, name, description, size, props, fits } => {
            let _ = ctx.events.send(Event::Building(1));
            let type_id = match build_item_type(&ctx, &name, &description, size, &props, fits.as_deref()).await {
                Ok(t) => Some(t),
                Err(e) => {
                    crate::log::error(format!("building type '{name}' failed: {e:#}"));
                    None
                }
            };
            let _ = ctx.events.send(Event::TypeBuilt { id, type_id });
            let _ = ctx.events.send(Event::Building(-1));
        }
        Cmd::EditType { id, name, source, change, spot, cuts, with } => {
            let _ = ctx.events.send(Event::Building(1));
            let type_id = match edit_item_type(&ctx, &name, &source, &change, &spot, &cuts, with.as_ref()).await {
                Ok(t) => Some(t),
                Err(e) => {
                    crate::log::error(format!("reshaping into '{name}' failed: {e:#}"));
                    None
                }
            };
            let _ = ctx.events.send(Event::TypeBuilt { id, type_id });
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
            let mems = ctx.db.memories(cid).unwrap_or_default();
            let recent: Vec<String> = mems.iter().rev().take(6).map(|m| m.text.clone()).collect();
            let dctx = DecisionCtx {
                character: name,
                persona,
                event,
                context: format!("{context}\nWhat they remember of the traveler: {summary}\nTheir latest memories: {}", if recent.is_empty() { "none".into() } else { recent.join(" | ") }),
            };
            let d = ctx.decider.decide(dctx).await;
            let _ = ctx.events.send(Event::Decision { cid, decision: d });
        }
        Cmd::NewSpecies { id, brief, fixed } => {
            let _ = ctx.events.send(Event::Building(1));
            let name = match new_species(&ctx, &brief, &fixed).await {
                Ok(n) => Some(n),
                Err(e) => {
                    crate::log::error(format!("new species failed: {e:#}"));
                    None
                }
            };
            let _ = ctx.events.send(Event::SpeciesMade { id, name });
            let _ = ctx.events.send(Event::Building(-1));
        }
        Cmd::SpeciesBody { id, species, template } => {
            let _ = ctx.events.send(Event::Building(1));
            let type_id = match species_body(&ctx, &species, &template).await {
                Ok(t) => Some(t),
                Err(e) => {
                    crate::log::error(format!("a species' own body failed: {e:#}"));
                    None
                }
            };
            let _ = ctx.events.send(Event::TypeBuilt { id, type_id });
            let _ = ctx.events.send(Event::Building(-1));
        }
        Cmd::ReshapeBody { id, name, from, source, change } => {
            let _ = ctx.events.send(Event::Building(1));
            let type_id = match reshape_body(&ctx, &name, &from, &source, &change).await {
                Ok(t) => Some(t),
                Err(e) => {
                    crate::log::error(format!("reshaping the body '{name}' failed: {e:#}"));
                    None
                }
            };
            let _ = ctx.events.send(Event::TypeBuilt { id, type_id });
            let _ = ctx.events.send(Event::Building(-1));
        }
        Cmd::Undo | Cmd::History | Cmd::Witness { .. } => {}
    }
}

/// One species from a brief: written, its body built if new, stored, and
/// the snapshot flipped so the body can be drawn.
async fn new_species(ctx: &Ctx, brief: &str, fixed: &Value) -> anyhow::Result<String> {
    let system = prompts::builder_system(&ctx.universe(), &ctx.vocab());
    let task = prompts::species_task(brief, &species_list(&ctx.db));
    let mut req = Req::new(Role::Builder, system.clone(), task);
    req.max_tokens = 2000;
    let reply = ctx.llm.complete(&req, "species").await?;
    let mut v = extract_json(&reply)?;
    if let Some(s) = v.get("species").cloned() {
        v = s;
    }
    if let Some(a) = v.as_array().and_then(|a| a.first()).cloned() {
        v = a;
    }
    if let (Some(o), Some(f)) = (v.as_object_mut(), fixed.as_object()) {
        for (k, x) in f {
            o.insert(k.clone(), x.clone());
        }
    }
    let (types, names) = species_from(ctx, &system, &[v], 1).await;
    let name = names.into_iter().next().ok_or_else(|| anyhow::anyhow!("the species was unreadable"))?;
    if !types.is_empty() {
        let ok = ctx
            .commit(CommitRequest { kind: "species", summary: format!("new species: {name}"), new_types: types, placements: vec![], nudge: true, region: None, look: None })
            .await
            .map_err(|d| anyhow::anyhow!(format_diags(&d)))?;
        let _ = ctx.events.send(Event::Flip { snap: ok.snapshot, region: None });
    }
    Ok(name)
}

fn budget_msg(e: &anyhow::Error) -> Option<String> {
    e.downcast_ref::<crate::llm::BudgetReached>().map(|_| "Budget reached: generation is paused (walking still works).".to_string())
}

fn short(s: &str) -> String {
    let s = s.lines().next().unwrap_or("");
    if s.chars().count() > 90 { format!("{}…", s.chars().take(90).collect::<String>()) } else { s.to_string() }
}

/// Every property a type names must be one this universe knows and lets
/// things have (never an act property like `force`).
pub fn check_type_props(ct: &crate::lang::CompiledType, vocab: &crate::sim::props::Vocab) -> Result<(), String> {
    let mut bad: Vec<String> = Vec::new();
    for n in ct.meta.props.iter().map(|(n, _)| n).chain(ct.prop_names.iter()) {
        match vocab.lookup(n) {
            Some(i) if vocab.is_act(i) => bad.push(format!("{n} (what an action is doing right now; no thing has it)")),
            Some(_) => {}
            None => bad.push(n.clone()),
        }
    }
    bad.dedup();
    if bad.is_empty() { Ok(()) } else { Err(format!("unknown properties: {}; this universe knows: {}", bad.join(", "), vocab.writable_names().join(", "))) }
}

/// Steps 1–4 on the worker pool, plus: every property it names must be one
/// this universe knows.
async fn validate(code: String, vocab: crate::sim::props::Vocab) -> Result<NewType, Vec<Diag>> {
    tokio::task::spawn_blocking(move || {
        let ct = compile(&code)?;
        check_type_props(&ct, &vocab).map_err(|e| vec![Diag::new(Stage::Allowlist, 0, e)])?;
        let report = probe(&ct)?;
        Ok(NewType { ct: Arc::new(ct), report: Arc::new(report) })
    })
    .await
    .unwrap_or_else(|e| Err(vec![Diag::new(Stage::Translate, 0, format!("validator crashed: {e}"))]))
}

/// Ask for a type, validate it, and feed errors back up to two times.
/// `roles`: roles a body must still answer to (a reshaped body keeps them).
async fn build_type(ctx: &Ctx, system: &str, task: String, name: &str, extra_tags: &[&str], roles: &[String]) -> anyhow::Result<NewType> {
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
                match validate(code, ctx.vocab()).await {
                    Ok(t) if extra_tags.contains(&"body") && t.ct.meta.body.is_none() => vec![Diag::new(Stage::Allowlist, 0, "a body needs meta.body (height, eye, radius, reach, grip, roles, gait…); see the body rules above".into())],
                    Ok(t) if t.ct.meta.body.as_ref().is_some_and(|b| roles.iter().any(|r| !b.roles.contains(r))) => {
                        let missing: Vec<&String> = roles.iter().filter(|r| !t.ct.meta.body.as_ref().is_some_and(|b| b.roles.contains(r))).collect();
                        vec![Diag::new(Stage::Allowlist, 0, format!("the body must still answer to every role it had; missing: {}", missing.iter().map(|r| r.as_str()).collect::<Vec<_>>().join(", ")))]
                    }
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

/// Names of the body types this universe has (built-in and written).
fn body_names(db: &Db) -> std::collections::HashSet<String> {
    let mut out: std::collections::HashSet<String> = ["figure".to_string(), "quadruped".to_string()].into_iter().collect();
    for r in db.types().unwrap_or_default() {
        if (r.status == "ok" || r.status == "builtin") && r.code.contains("body:") {
            if let Ok(ct) = compile(&r.code) {
                if ct.meta.body.is_some() {
                    out.insert(ct.meta.name.clone());
                }
            }
        }
    }
    out
}

/// One line per species the universe has, for plans.
fn species_list(db: &Db) -> String {
    let book = crate::world::species::SpeciesBook::load(&db.with(|c| crate::db::species_rows(c)).unwrap_or_default(), None);
    book.list.iter().map(|s| format!("- {} ({}; body {}, {:?} mind, speech {:?})", s.name, if s.description.is_empty() { "people" } else { &s.description }, s.body, s.mind, s.speech).to_lowercase()).collect::<Vec<_>>().join("\n")
}

/// Generic bodies a species starts from: a species named with one gets its
/// own body written from it (a cat's from the quadruped).
pub const TEMPLATE_BODIES: [&str; 2] = ["figure", "quadruped"];

/// A body's code, from the stored types (built-in or written).
fn body_source(db: &Db, name: &str) -> String {
    db.types().unwrap_or_default().into_iter().rev().find(|t| t.name == name && (t.status == "builtin" || t.status == "ok")).map(|t| t.code).unwrap_or_default()
}

/// A written body as the one asked for: its name, and the body it was
/// written from in `meta.body.from` (in the source too, so it survives a
/// reload), or none.
fn own_body(mut t: NewType, name: &str, from: Option<&str>) -> NewType {
    let mut ct = (*t.ct).clone();
    if ct.meta.name != name {
        ct.source = rename_meta(&ct.source, &ct.meta.name, name);
        ct.meta.name = name.to_string();
    }
    if let (Some(from), Some(b)) = (from, ct.meta.body.as_mut()) {
        let old = b.from.as_deref().map(|o| [format!("from: \"{o}\""), format!("from: '{o}'")]);
        let src = match old.and_then(|o| o.into_iter().find(|o| ct.source.contains(o.as_str()))) {
            Some(o) => Some(ct.source.replacen(&o, &format!("from: \"{}\"", from.replace(['\\', '"'], "")), 1)),
            None if b.from.is_none() => insert_body_from(&ct.source, from),
            None => None,
        };
        if let Some(src) = src {
            ct.source = src;
            b.from = Some(from.to_string());
        }
    } else if let (None, Some(b)) = (from, ct.meta.body.as_mut()) {
        // Not of any body's line: a `from` it was given goes.
        if let Some(o) = b.from.take() {
            for q in [format!("from: \"{o}\","), format!("from: '{o}',"), format!("from: \"{o}\""), format!("from: '{o}'")] {
                if ct.source.contains(&q) {
                    ct.source = ct.source.replacen(&q, "", 1);
                    break;
                }
            }
        }
    }
    t.ct = Arc::new(ct);
    t
}

/// Add `from: "<name>"` at the start of a module's `body: { … }` meta.
fn insert_body_from(src: &str, from: &str) -> Option<String> {
    let esc = from.replace(['\\', '"'], "");
    for (i, _) in src.match_indices("body:") {
        let rest = &src[i + 5..];
        let gap = rest.len() - rest.trim_start().len();
        if rest.trim_start().starts_with('{') {
            let j = i + 5 + gap;
            return Some(format!("{} from: \"{esc}\",{}", &src[..=j], &src[j + 1..]));
        }
    }
    None
}

/// A body's standing height, from its stored code.
/// How big the people who build here are: doorways and rooms are sized for
/// the tallest of them (and the traveler, when they fit).
pub fn folk_heights(db: &Db) -> (f32, f32) {
    let book = crate::world::species::SpeciesBook::load(&db.with(|c| crate::db::species_rows(c)).unwrap_or_default(), db.kv_get("species.world").as_deref());
    let traveler = book.traveler_height.filter(|h| h.is_finite()).unwrap_or(1.75).clamp(0.3, 12.0);
    let mut tallest: f32 = 0.0;
    for sp in &book.list {
        if sp.mind != crate::world::species::Mind::Sapient {
            continue;
        }
        let world = book.sizes.get(&sp.name).copied().unwrap_or(1.0);
        let h = match sp.look.get("height") {
            Some(r) => r[1],
            None => body_height(db, &sp.body).unwrap_or(1.75) * sp.size,
        } * world;
        tallest = tallest.max(h);
    }
    if tallest <= 0.0 {
        tallest = 1.75 * book.sizes.get("human").copied().unwrap_or(1.0);
    }
    (tallest, traveler)
}

/// The clearance line for region and building tasks.
fn folk_note(db: &Db) -> String {
    let (folk, traveler) = folk_heights(db);
    let door = (folk * 1.15).max(0.5);
    let fits = if traveler * 0.62 > door { " (too low for the traveler: their houses are closed to them unless they are built bigger)" } else if traveler > door { " (the traveler gets through only by crouching)" } else { "" };
    format!("Folk height: the tallest people who build here are about {folk:.2} m; the traveler is {traveler:.2} m. Doorways for them are at least {door:.2} m tall and {:.2} m wide{fits}.", (door * 0.5).max(0.8))
}

fn body_height(db: &Db, name: &str) -> Option<f32> {
    compile(&body_source(db, name)).ok()?.meta.body.map(|b| b.height)
}

/// A species' size against its own body, written at its natural size, that
/// keeps the height it had on the generic body (a cat at 0.34 of the
/// one-metre quadruped stays 0.34 m tall on its own 0.32 m body).
fn size_on_own_body(size: f32, template_height: Option<f32>, own_height: f32) -> f32 {
    match template_height {
        Some(th) if size.is_finite() && own_height > 0.01 => (size * th / own_height).clamp(0.2, 12.0),
        _ => size,
    }
}

/// Write one species' body: its own, written from a generic body
/// (`derived`: it is of that body's line), or a new one that only takes
/// `template` as an example of the contract (a dragon is no quadruped).
async fn write_body(ctx: &Ctx, system: &str, name: &str, desc: &str, species: &str, template: &str, derived: bool) -> anyhow::Result<NewType> {
    let src = body_source(&ctx.db, template);
    let task = prompts::body_task(name, desc, species, (!src.is_empty()).then_some((template, src.as_str())), derived);
    let t = build_type(ctx, system, task, name, &["body"], &[]).await?;
    Ok(own_body(t, name, Some(template).filter(|_| derived)))
}

/// Species the LLM described (at genesis or in a region plan): any body
/// nobody has yet is written first (all at once), and a species on a
/// generic body gets its own, written from it. Returns the new body types
/// (to commit) and the species' names.
async fn species_from(ctx: &Ctx, system: &str, list: &[Value], max: usize) -> (Vec<NewType>, Vec<String>) {
    let mut bodies = body_names(&ctx.db);
    // (species value, its body, what to write it from: (template, description),
    // the generic body its own one comes from).
    let mut plans: Vec<(Value, String, Option<(String, String)>, Option<String>)> = Vec::new();
    for v in list.iter().take(max) {
        let mut v = v.clone();
        let Some(o) = v.as_object_mut() else { continue };
        let name = o.get("name").and_then(|x| x.as_str()).unwrap_or("").trim().to_lowercase();
        if name.is_empty() || name.len() > 32 {
            continue;
        }
        o.insert("name".into(), Value::String(name.clone()));
        let sapient = o.get("mind").and_then(|x| x.as_str()).is_none_or(|m| m == "sapient");
        let fallback = if sapient { "figure" } else { "quadruped" };
        let mut body = o.get("body").and_then(|x| x.as_str()).unwrap_or("").trim().to_lowercase();
        if body.is_empty() || body == "human" || body == "person" {
            body = fallback.into();
        }
        let desc = o.get("body_description").or_else(|| o.get("description")).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let mut template_of = None;
        let write = if name == "human" {
            None
        } else if TEMPLATE_BODIES.contains(&body.as_str()) {
            // Its own body, from the generic one (named after the species).
            let template = std::mem::replace(&mut body, name.clone());
            template_of = Some(template.clone());
            (!bodies.contains(&body)).then(|| (template, desc))
        } else if !bodies.contains(&body) {
            Some((fallback.to_string(), desc))
        } else {
            None
        };
        if write.is_some() {
            // Two species naming one new body share one writing.
            bodies.insert(body.clone());
        }
        plans.push((v, body, write, template_of));
    }
    let futs = plans.iter().map(|(v, body, write, template_of)| async move {
        let (template, desc) = write.as_ref()?;
        match write_body(ctx, system, body, desc, &v.to_string(), template, template_of.is_some()).await {
            Ok(t) => Some((body.clone(), Ok(t))),
            Err(e) => Some((body.clone(), Err(e))),
        }
    });
    let mut types = Vec::new();
    let mut failed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (body, res) in futures_util::future::join_all(futs).await.into_iter().flatten() {
        match res {
            Ok(t) => types.push(t),
            Err(e) => {
                crate::log::info(format!("body {body} failed: {e:#}"));
                failed.insert(body);
            }
        }
    }
    let mut names = Vec::new();
    for (mut v, mut body, write, template_of) in plans {
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if failed.contains(&body) {
            // Its body couldn't be written: the generic one it started from.
            let sapient = v.get("mind").and_then(|x| x.as_str()).is_none_or(|m| m == "sapient");
            body = write.map(|w| w.0).unwrap_or_else(|| if sapient { "figure".into() } else { "quadruped".into() });
        }
        if let Some(o) = v.as_object_mut() {
            // A body of its own from a generic one: its sliders are its own
            // (the generic ranges don't apply), and it keeps its height.
            if let Some(template) = template_of.filter(|_| !TEMPLATE_BODIES.contains(&body.as_str())) {
                o.remove("look");
                let own = types.iter().find(|t| t.ct.meta.name == body).and_then(|t| t.ct.meta.body.as_ref()).map(|b| b.height).or_else(|| body_height(&ctx.db, &body));
                let size = o.get("size").and_then(|x| x.as_f64()).unwrap_or(1.0) as f32;
                if let Some(h) = own {
                    o.insert("size".into(), serde_json::json!(size_on_own_body(size, body_height(&ctx.db, &template), h)));
                }
            }
            o.insert("body".into(), Value::String(body));
            o.remove("body_description");
        }
        match serde_json::from_value::<crate::world::species::Species>(v) {
            Ok(mut sp) => {
                sp.sanitize();
                if let Ok(j) = serde_json::to_string(&sp) {
                    let _ = ctx.db.with(|c| crate::db::put_species(c, &sp.name, &j));
                    crate::log::info(format!("species: {}", sp.name));
                    names.push(sp.name.clone());
                }
            }
            Err(e) => crate::log::info(format!("species {name}: unreadable ({e})")),
        }
    }
    (types, names)
}

/// A species still on a generic body gets its own, written from it: stored
/// with the species and committed. Returns the new body's type id.
async fn species_body(ctx: &Ctx, species: &str, template: &str) -> anyhow::Result<u32> {
    let mut sp: crate::world::species::Species = serde_json::from_str(species)?;
    let system = prompts::builder_system(&ctx.universe(), &ctx.vocab());
    let desc = if sp.description.is_empty() { sp.name.clone() } else { format!("{}: {}", sp.name, sp.description) };
    let t = write_body(ctx, &system, &sp.name, &desc, species, template, true).await?;
    let old = serde_json::to_string(&sp)?;
    if let Some(h) = t.ct.meta.body.as_ref().map(|b| b.height) {
        sp.size = size_on_own_body(sp.size, body_height(&ctx.db, template), h);
    }
    sp.body = sp.name.clone();
    sp.look.clear();
    let new = serde_json::to_string(&sp)?;
    // Stored first: the commit's snapshot carries the species with it.
    ctx.db.with(|c| crate::db::put_species(c, &sp.name, &new))?;
    let ok = match ctx.commit(CommitRequest { kind: "species", summary: format!("{}'s own body", sp.name), new_types: vec![t], placements: vec![], nudge: true, region: None, look: None }).await {
        Ok(ok) => ok,
        Err(d) => {
            let _ = ctx.db.with(|c| crate::db::put_species(c, &sp.name, &old));
            anyhow::bail!(format_diags(&d));
        }
    };
    let id = ok.snapshot.body_named(&sp.name).map(|e| e.id).ok_or_else(|| anyhow::anyhow!("body vanished"))?;
    let _ = ctx.events.send(Event::Flip { snap: ok.snapshot, region: None });
    Ok(id)
}

/// One being's body reshaped (a poofy tail): written from its body now,
/// keeping every role. Returns the new body's type id.
async fn reshape_body(ctx: &Ctx, name: &str, from: &str, source: &str, change: &str) -> anyhow::Result<u32> {
    let system = prompts::builder_system(&ctx.universe(), &ctx.vocab());
    let roles = crate::lang::compile(source).ok().and_then(|ct| ct.meta.body.map(|b| b.roles)).unwrap_or_default();
    let task = prompts::body_reshape_task(name, from, source, change, &roles);
    let t = build_type(ctx, &system, task, name, &["body"], &roles).await?;
    let t = own_body(t, name, Some(from));
    let ok = ctx
        // Not a "reshaped: " commit: those one-off types are dropped once no
        // placed thing uses them, and nothing is placed in a body.
        .commit(CommitRequest { kind: "interp", summary: format!("own body: {name}"), new_types: vec![t], placements: vec![], nudge: true, region: None, look: None })
        .await
        .map_err(|d| anyhow::anyhow!(format_diags(&d)))?;
    let id = ok.snapshot.body_named(name).map(|e| e.id).ok_or_else(|| anyhow::anyhow!("body vanished"))?;
    let _ = ctx.events.send(Event::Flip { snap: ok.snapshot, region: None });
    Ok(id)
}

/// Varieties a plan describes, added to (or replacing ones of) their species.
fn varieties_from(db: &Db, list: &[Value]) {
    if list.is_empty() {
        return;
    }
    let book = crate::world::species::SpeciesBook::load(&db.with(|c| crate::db::species_rows(c)).unwrap_or_default(), None);
    for v in list.iter().take(4) {
        let sp = s(v, "species").trim().to_lowercase();
        let sp = if sp.is_empty() { "human".to_string() } else { sp };
        let Some(base) = book.get(&sp) else { continue };
        let Ok(mut var) = serde_json::from_value::<crate::world::species::Variety>(v.clone()) else { continue };
        var.name = var.name.trim().to_lowercase();
        if var.name.is_empty() {
            continue;
        }
        var.layers.truncate(3);
        let mut species = (**base).clone();
        species.varieties.retain(|x| x.name != var.name);
        species.varieties.push(var);
        species.sanitize();
        if let Ok(j) = serde_json::to_string(&species) {
            let _ = db.with(|c| crate::db::put_species(c, &species.name, &j));
        }
    }
}

/// World-wide species settings from the genesis plan (attitudes, sizes, the
/// traveler's height).
fn species_world_from(db: &Db, v: &Value) {
    let mut w: crate::world::species::SpeciesWorld = db.kv_get("species.world").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut changed = false;
    if let Some(a) = v.get("attitudes").and_then(|x| serde_json::from_value::<Vec<crate::world::species::Attitude>>(x.clone()).ok()) {
        w.attitudes = a
            .into_iter()
            .take(12)
            .map(|mut x| {
                x.a = x.a.trim().to_lowercase();
                x.b = x.b.trim().to_lowercase();
                x.affection = x.affection.clamp(-0.8, 0.8);
                x.trust = x.trust.clamp(-0.8, 0.8);
                x.rivalry = x.rivalry.clamp(0.0, 0.8);
                x
            })
            .collect();
        changed = true;
    }
    if let Some(s) = v.get("sizes").and_then(|x| serde_json::from_value::<std::collections::BTreeMap<String, f32>>(x.clone()).ok()) {
        w.sizes = s.into_iter().filter(|(_, v)| v.is_finite()).map(|(k, v)| (k.trim().to_lowercase(), v.clamp(0.2, 6.0))).collect();
        changed = true;
    }
    if let Some(h) = v.get("traveler_height").or_else(|| v.get("traveller_height")).and_then(|x| x.as_f64()).filter(|h| h.is_finite()) {
        w.traveler_height = Some((h as f32).clamp(0.3, 12.0));
        changed = true;
    }
    if changed {
        let _ = db.kv_set("species.world", &serde_json::to_string(&w).unwrap_or_default());
    }
}

async fn genesis(ctx: &Ctx) -> anyhow::Result<()> {
    let system = prompts::builder_system(&ctx.prompt, &ctx.vocab());
    let mut req = Req::new(Role::Builder, system.clone(), prompts::GENESIS_TASK);
    req.max_tokens = 32000;
    req.effort = Some("medium");
    let mut pacer = Pacer::new(ctx.events.clone(), Task::Genesis, pace::GENESIS_CHARS, 0.0, 0.7);
    let reply = ctx.llm.stream(&req, "genesis", |t| pacer.text(t)).await?;
    pacer.finish();
    let v = extract_json(&reply)?;
    // The universe's own properties and rules come first: base types may use them.
    let (uprops, urules) = universe_rules_from(&v);
    if !uprops.is_empty() || !urules.is_empty() {
        let (props, rules) = check_universe_rules(&uprops, &urules);
        crate::sim::persist::set_universe_rules(&ctx.db, &props, &rules)?;
        let meta: Vec<_> = universe_meta_from(&v).into_iter().filter(|(n, _)| props.iter().any(|p| &p.0 == n)).collect();
        crate::sim::persist::set_universe_meta(&ctx.db, &meta)?;
        crate::log::info(format!("universe rules: {} properties, {} rules", props.len(), rules.len()));
        let _ = ctx.events.send(Event::RulesChanged);
    }
    let mut look: Look = serde_json::from_value(v.clone()).unwrap_or_default();
    look.land = look.land.trim().to_string();
    look.start = look.start.trim().to_string();
    look.climate.sanitize();
    let land = look.land.clone();
    if look.biomes.is_empty() {
        look.biomes = crate::terrain::default_biomes();
    }
    for b in &mut look.biomes {
        b.scatter.entry("grass".into()).or_insert(1.0);
    }
    crate::terrain::add_litter(&mut look.biomes);
    // Validate each base type; repair failures one at a time.
    let done = std::sync::atomic::AtomicUsize::new(0);
    let done = &done;
    let mut types = Vec::new();
    let mut futs = Vec::new();
    let system_ref = &system;
    let blocks: Vec<String> = all_code_blocks(&reply).into_iter().take(8).collect();
    let n_blocks = blocks.len().max(1);
    for code in blocks {
        let code = add_tags(&code, &["base"]);
        let system = system_ref;
        futs.push(async move {
            let t = match validate(code.clone(), ctx.vocab()).await {
                Ok(t) => Some(t),
                Err(d) if d.iter().all(|x| x.stage.repairable()) => {
                    let name = code.split("name:").nth(1).and_then(|s| s.split('"').nth(1)).unwrap_or("object").to_string();
                    let task = format!("This base type failed validation:\n```js\n{code}\n```\nErrors:\n{}\n\n{}", format_diags(&d), prompts::TYPE_TASK);
                    build_type(ctx, system, task, &name, &[], &[]).await.ok().map(|mut t| {
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
            };
            let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            pace::step(&ctx.events, Task::Genesis, 0.7 + 0.2 * k as f32 / n_blocks as f32);
            t
        });
    }
    for t in futures_util::future::join_all(futs).await.into_iter().flatten() {
        types.push(t);
    }
    // The universe's peoples and beasts.
    species_world_from(&ctx.db, &v);
    let specs: Vec<Value> = v.get("species").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let (bodies, _) = species_from(ctx, &system, &specs, 6).await;
    types.extend(bodies);
    pace::step(&ctx.events, Task::Genesis, 0.96);
    let name = look.name.clone();
    let n = types.len();
    let ok = ctx
        .commit(CommitRequest { kind: "genesis", summary: format!("genesis: {} with {n} base types", if name.is_empty() { "the land" } else { &name }), new_types: types, placements: vec![], nudge: true, region: None, look: Some(look) })
        .await
        .map_err(|d| anyhow::anyhow!(format_diags(&d)))?;
    ctx.db.kv_set("genesis", "done")?;
    if !land.is_empty() {
        crate::log::info(format!("land: {land}"));
        if let Ok(mut u) = ctx.universe.write() {
            *u = land;
        }
    }
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
    let system = prompts::builder_system(&ctx.universe(), &ctx.vocab());
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
    let task = format!("{task}\nSpecies in this universe:\n{}\n{}", species_list(&ctx.db), folk_note(&ctx.db));
    let task = if r == info.start && !info.look.start.is_empty() {
        format!("{task}\n\nThe traveler begins in this region: {}\nPlan the region around it.", info.look.start)
    } else if !info.look.land.is_empty() {
        format!("{task}\n\nThe traveler did not begin here: make this region its own place in the land, different from its neighbours.")
    } else {
        task
    };
    let mut req = Req::new(Role::Builder, system.clone(), task);
    req.max_tokens = 16000;
    req.effort = Some("medium");
    let mut pacer = Pacer::new(ctx.events.clone(), Task::Region(r), pace::REGION_CHARS, 0.0, 0.6);
    let reply = ctx.llm.stream(&req, "region", |t| pacer.text(t)).await?;
    pacer.finish();
    let plan = extract_json(&reply)?;
    let rname = s(&plan, "name");

    // New types, in parallel.
    let mut new_types: Vec<NewType> = Vec::new();
    let mut new_names: Vec<(String, usize)> = Vec::new();
    let specs: Vec<Value> = plan.get("new_types").and_then(|x| x.as_array()).cloned().unwrap_or_default().into_iter().take(8).collect();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let done = &done;
    let n_specs = specs.len().max(1);
    let futs = specs.iter().map(|spec| {
        let name = s(spec, "name");
        // Clothing and gear are written for a body, against its code.
        let is_layer = spec.get("tags").and_then(|t| t.as_array()).is_some_and(|t| t.iter().any(|x| x.as_str() == Some("layer")));
        let layer_note = if is_layer {
            let fits = Some(s(spec, "fits").trim().to_lowercase()).filter(|f| !f.is_empty()).unwrap_or_else(|| "figure".into());
            let src = ctx.db.types().unwrap_or_default().into_iter().rev().find(|t| t.name == fits && (t.status == "builtin" || t.status == "ok")).map(|t| t.code).unwrap_or_default();
            prompts::layer_note(&fits, &src)
        } else {
            String::new()
        };
        let folk = folk_note(&ctx.db);
        let task = format!(
            "Object type to write: \"{name}\"\nDescription: {}\nApproximate size (w × h × d, metres): {}\nTags: {}\nProperties (meta.props): {}\nIt appears in the region \"{rname}\" ({}).\n{folk}\n\n{}",
            s(spec, "description"),
            spec.get("size_m").map(|v| v.to_string()).unwrap_or_else(|| "unspecified".into()),
            spec.get("tags").map(|v| v.to_string()).unwrap_or_else(|| "[]".into()),
            spec.get("props").map(|v| v.to_string()).unwrap_or_else(|| "choose fitting ones".into()),
            s(&plan, "mood"),
            prompts::TYPE_TASK
        );
        let task = if layer_note.is_empty() { task } else { format!("{task}\n\n{layer_note}") };
        let system = system.clone();
        let tags: &[&str] = if is_layer { &["layer"] } else { &[] };
        async move {
            let res = build_type(ctx, &system, task, &name, tags, &[]).await;
            let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            pace::step(&ctx.events, Task::Region(r), 0.6 + 0.3 * k as f32 / n_specs as f32);
            (name.clone(), res)
        }
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
    // What the new buildings hold (a sword on its rack, a pot on the shelf),
    // written too when the world has nothing of that name.
    let known = |n: &str| new_names.iter().any(|(k, _)| k == n) || info.types.iter().any(|(_, tn, _, _)| tn.to_lowercase() == n);
    for (name, t) in write_contents(ctx, &system, &new_types, &known, &format!("the region \"{rname}\" ({})", s(&plan, "mood"))).await {
        let i = new_types.len();
        new_names.push((name, i));
        new_names.push((t.ct.meta.name.to_lowercase(), i));
        new_types.push(t);
    }
    // Varieties of species (a warrior village's people, hill folk).
    varieties_from(&ctx.db, plan.get("varieties").and_then(|x| x.as_array()).map(|a| a.as_slice()).unwrap_or(&[]));
    let settlement_variety = plan.get("settlement").map(|st| s(st, "variety").trim().to_lowercase()).unwrap_or_default();
    // A new species for this region (its body is written first if needed).
    let specs: Vec<Value> = plan.get("new_species").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let (bodies, _) = species_from(ctx, &system, &specs, 1).await;
    new_types.extend(bodies);
    pace::step(&ctx.events, Task::Region(r), 0.95);
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
        p.look.clear();
        if let Some(o) = lk.as_object() {
            for (k, v) in o.iter().take(8) {
                if let Some(v) = v.as_f64().filter(|v| v.is_finite()) {
                    p.look.insert(k.trim().to_lowercase(), v as f32);
                }
            }
        }
        p.variety = p.variety.trim().to_lowercase();
        if p.variety.is_empty() {
            p.variety = settlement_variety.clone();
        }
        p.layers.truncate(3);
        if p.species.trim().is_empty() || p.species.eq_ignore_ascii_case("human") {
            p.species.clear();
            let hue = |x: Option<f32>| x.map(|v| v.rem_euclid(1.0));
            let get = |n: &str| crate::world::species::look_value(&p.look, n);
            let fixed = [
                ("height", get("height").map(|v| v.clamp(1.3, 2.0))),
                ("build", get("build").map(|v| v.clamp(0.7, 1.4))),
                ("skin", get("skin").map(|v| v.clamp(0.0, 1.0))),
                ("shirt", hue(get("shirt"))),
                ("trousers", hue(get("trousers"))),
            ];
            // What the plan leaves out is chosen from the species (or
            // variety) range for each person.
            p.look.clear();
            for (k, v) in fixed {
                if let Some(v) = v {
                    p.look.insert(k.into(), v);
                }
            }
        } else {
            p.species = p.species.trim().to_lowercase();
        }
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
    // Small things lying about: each trade's tools and materials by its
    // person's home, and the settlement's plaything in its middle.
    let middle = plan.get("settlement").filter(|v| v.is_object()).map(|st| (ox + clampl(f(st, "x", 128.0)), oz + clampl(f(st, "z", 128.0))));
    let mut by_spot: std::collections::HashMap<String, usize> = Default::default();
    for (i, th) in plan.get("things").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().take(12).enumerate() {
        let Some(ty) = resolve(&s(th, "type")) else { continue };
        let near = s(th, "near").trim().to_lowercase();
        let spot = chars.iter().find(|(p, _)| !near.is_empty() && p.name.trim().to_lowercase() == near).map(|(_, h)| (h.x, h.z)).or(middle).or_else(|| chars.first().map(|(_, h)| (h.x, h.z)));
        let Some((sx, sz)) = spot else { continue };
        // Spread round the spot so a person's things don't pile up.
        let k = by_spot.entry(near).or_default();
        let a = *k as f32 * 2.399 + i as f32 * 0.7;
        let rad = 1.6 + 0.5 * *k as f32;
        *k += 1;
        let (x, z) = (sx + a.cos() * rad, sz + a.sin() * rad);
        if info.terrain.height(x, z) <= WATER_LEVEL + 0.2 {
            continue;
        }
        placements.push(Placement { ty, x, z, y: None, rot_y: a * 3.0, scale: 1.0, params: [100.0 + i as f32, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5] });
    }
    // Creatures: herds, packs, pets, beasts. No voice or trade, just a
    // species, a home, maybe a name and a person they belong to.
    let mut n_creatures = 0;
    for c in plan.get("creatures").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().take(6) {
        let sp = s(c, "species").trim().to_lowercase();
        if sp.is_empty() || sp == "human" {
            continue;
        }
        let count = (f(c, "count", 1.0) as usize).clamp(1, 8);
        let names: Vec<String> = c.get("names").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|n| n.as_str().map(|s| s.trim().to_string())).collect()).unwrap_or_default();
        let owner = s(c, "owner");
        let cx = ox + clampl(f(c, "x", 128.0));
        let cz = oz + clampl(f(c, "z", 128.0));
        for i in 0..count {
            if n_creatures >= 24 {
                break;
            }
            let name = names.get(i).cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("the {}", sp.replace('-', " ")));
            let a = i as f32 * 2.399;
            let mut home = Vec3::new(cx + a.cos() * (1.5 + i as f32), 0.0, cz + a.sin() * (1.5 + i as f32));
            for k in 0..30 {
                if info.terrain.height(home.x, home.z) > WATER_LEVEL + 0.5 {
                    break;
                }
                let b = k as f32 * 2.399;
                home += Vec3::new(b.cos(), 0.0, b.sin()) * (3.0 + k as f32);
            }
            let p = Persona {
                name,
                species: sp.clone(),
                appearance: s(c, "description"),
                relationships: if owner.is_empty() { vec![] } else { vec![format!("{owner}: owner")] },
                home: s(c, "home"),
                ..Default::default()
            };
            chars.push((p, home));
            n_creatures += 1;
        }
    }
    // How the settlement's ways look: roads between its doors, and what lights them.
    let settlement = plan.get("settlement").filter(|v| v.is_object()).map(|st| {
        let road = match st.get("road") {
            Some(Value::Null) | Some(Value::Bool(false)) => None,
            Some(v) => {
                let c = v.get("color").and_then(|c| c.as_array()).filter(|a| a.len() == 3).map(|a| {
                    let ch = |i: usize| (a[i].as_f64().unwrap_or(128.0) as f32 / 255.0).clamp(0.0, 1.0);
                    [ch(0), ch(1), ch(2)]
                });
                let half = v.get("width").and_then(|w| w.as_f64()).map(|w| (w as f32 * 0.5).clamp(0.4, 4.0)).unwrap_or(1.1);
                Some((c, half))
            }
            None => Some((None, 1.1)),
        };
        let lamps = match st.get("lamps") {
            Some(Value::String(n)) if !n.trim().is_empty() => resolve(n).or_else(|| resolve("lamppost")),
            Some(Value::Bool(true)) => resolve("lamppost"),
            _ => None,
        };
        crate::model::Settlement { x: ox + clampl(f(st, "x", 128.0)), z: oz + clampl(f(st, "z", 128.0)), road, lamps }
    });
    let n_chars = chars.len();
    let ok = ctx
        .commit(CommitRequest {
            kind: "region",
            summary: format!("region {} ({}, {})", if rname.is_empty() { "unnamed" } else { &rname }, r.0, r.1),
            new_types,
            placements,
            nudge: true,
            region: Some(RegionCommit { r, plan_json: plan.to_string(), info: region_info_from_plan(&plan.to_string()), characters: chars, settlement }),
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

/// The things new shapes' slots and parts name (a sword on its rack, a
/// ladder up to a tree-house) that the world has no type for yet, written
/// in parallel (at most six). Returns (the name asked for, its type).
async fn write_contents(ctx: &Ctx, system: &str, types: &[NewType], known: &(dyn Fn(&str) -> bool + Sync), place: &str) -> Vec<(String, NewType)> {
    let mut wanted: Vec<(String, String, bool)> = Vec::new();
    for t in types {
        for a in &t.ct.meta.anchors {
            if let Some(h) = &a.holds {
                let n = h.trim().to_lowercase();
                if n != "door" && !known(&n) && !wanted.iter().any(|w| w.0 == n) && wanted.len() < 6 {
                    wanted.push((n, t.ct.meta.name.clone(), a.kind == "part"));
                }
            }
        }
    }
    let futs = wanted.iter().map(|(n, b, part)| {
        let what = if *part {
            format!("a {n}, a part fixed to the {b} in {place} (a sign, a bell, a ladder, a lantern on its hook); size it to fit the {b}")
        } else {
            format!("a {n}, as it is found in the {b} in {place}. If it is worked by hand (a blade, an axe, a spade, a hammer, a bucket), give it meta.tool")
        };
        let task = format!(
            "Object type to write: \"{n}\"\nDescription: {what}.\nApproximate size (w × h × d, metres): {}\nTags: {}\nProperties (meta.props): choose fitting ones\n\n{}",
            if *part { "to fit" } else { "small, under 1 m" },
            if *part { "[\"part\"]" } else { "[\"item\"]" },
            prompts::TYPE_TASK
        );
        async move { (n.clone(), build_type(ctx, system, task, n, &[], &[]).await) }
    });
    futures_util::future::join_all(futs).await.into_iter().filter_map(|(n, r)| r.ok().map(|t| (n, t))).collect()
}

async fn create(ctx: &Ctx, text: &str, view: &View, target: Vec3, yaw: f32, by: Option<&(i64, String)>) -> anyhow::Result<Vec<i64>> {
    let info = ctx.info(crate::world::region_of(target.x, target.z)).await.ok_or_else(|| anyhow::anyhow!("committer gone"))?;
    let system = prompts::builder_system(&ctx.universe(), &ctx.vocab());
    let type_list: Vec<String> = info.types.iter().map(|(_, n, tags, b)| format!("- {n} [{}] ~{:.0}×{:.0}×{:.0} m", tags.join(", "), b[0] * 2.0, b[1] * 2.0, b[2] * 2.0)).collect();
    let (who, sees) = match by {
        Some((_, name)) => (format!("Made by: {name}, a character who lives here (not the player)."), format!("What {name} sees")),
        None => ("Made by: the player.".to_string(), "What the player sees".to_string()),
    };
    let task = format!(
        "{}\n{who}\n{sees}:\n```json\n{}\n```\nExisting object types:\n{}\n\nThe request: \"{}\"",
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
                    new_types.push(validate(code, ctx.vocab()).await?);
                    // What it holds (a sword on its rack), written too.
                    let known = |n: &str| info.types.iter().any(|(_, tn, _, _)| tn.to_lowercase() == n);
                    for (_, t) in write_contents(ctx, &system, &new_types, &known, "the world").await {
                        new_types.push(t);
                    }
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
            let kind = if by.is_some() { "npc_create" } else { "create" };
            ctx.commit(CommitRequest { kind, summary, new_types, placements, nudge: by.is_some(), region: None, look: None }).await
        }
        .await;
        match outcome {
            Ok(ok) => {
                let v = ok.snapshot.version;
                // What was asked for comes first; its door and contents after.
                let name = ok.snapshot.instances.iter().find(|p| p.version == v).and_then(|p| ok.snapshot.type_of(p.type_id)).map(|t| t.name().to_string()).unwrap_or_else(|| "it".into());
                let ids: Vec<i64> = ok.snapshot.instances.iter().filter(|p| p.version == v).map(|p| p.id).collect();
                let _ = ctx.events.send(Event::Flip { snap: ok.snapshot, region: None });
                if by.is_none() {
                    ctx.log(format!("Built the {name}. (/undo to remove)"));
                }
                return Ok(ids);
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


/// Universe properties and rules from a genesis reply.
fn universe_rules_from(v: &Value) -> (Vec<(String, f32, String)>, Vec<crate::sim::rules::RuleSpec>) {
    let mut props = Vec::new();
    for p in v.get("properties").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().take(8) {
        let name = s(p, "name").trim().to_lowercase();
        if name.is_empty() {
            continue;
        }
        props.push((name, f(p, "default", 0.0), s(p, "meaning")));
    }
    let rules = v.get("rules").and_then(|x| x.as_array()).cloned().unwrap_or_default().into_iter().filter_map(|r| serde_json::from_value(r).ok()).take(crate::sim::rules::MAX_UNIVERSE_RULES).collect();
    (props, rules)
}

/// What genesis says about its own properties beyond their number: when a
/// change is news ("rises", "falls"), whether it spreads as one story
/// ("spreads": the words to tell it), how harmful it is, its range.
pub fn universe_meta_from(v: &Value) -> Vec<(String, crate::sim::props::PropMeta)> {
    use crate::sim::props::{Crossing, PropMeta, Words};
    let mut out = Vec::new();
    for p in v.get("properties").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().take(8) {
        let name = s(p, "name").trim().to_lowercase();
        if name.is_empty() {
            continue;
        }
        let text = |k: &str| p.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
        let mut m = PropMeta { keep: true, ..Default::default() };
        if let Some(t) = text("rises") {
            m.rises.push(Crossing { kind: format!("{name}_rose"), text: t, when: None, saved: false });
        }
        if let Some(t) = text("falls") {
            m.falls.push(Crossing { kind: format!("{name}_fell"), text: t, when: None, saved: true });
        }
        if let Some(w) = p.get("spreads").filter(|w| w.is_object()) {
            let noun = s(w, "noun").trim().to_string();
            if !noun.is_empty() {
                m.incident = Some(Words { noun, big: s(w, "big"), active: s(w, "active"), spent: s(w, "spent"), ended: s(w, "ended"), stopped: s(w, "stopped") });
            }
        }
        m.hazard = f(p, "hazard", 0.0).clamp(0.0, 2.0);
        m.scale = s(p, "scale").trim().to_string();
        if let Some([lo, hi]) = p.get("range").and_then(|r| serde_json::from_value::<[f32; 2]>(r.clone()).ok()).filter(|[lo, hi]| lo.is_finite() && hi.is_finite() && lo < hi) {
            m.range = Some([lo, hi]);
        }
        out.push((name, m));
    }
    out
}

/// Keep the properties with good names and the rules that parse and survive a
/// test scene (one by one, so one bad rule doesn't sink the rest).
pub fn check_universe_rules(props: &[(String, f32, String)], rules: &[crate::sim::rules::RuleSpec]) -> (Vec<(String, f32, String)>, Vec<crate::sim::rules::RuleSpec>) {
    let mut vocab = crate::sim::props::Vocab::builtin();
    let mut kept_props = Vec::new();
    for (n, d, m) in props {
        match vocab.add(n, *d, m) {
            Ok(_) => kept_props.push((n.clone(), *d, m.clone())),
            Err(e) => crate::log::info(format!("universe property dropped: {e}")),
        }
    }
    let mut kept = Vec::new();
    for r in rules {
        let mut trial = kept.clone();
        trial.push(r.clone());
        match crate::sim::env::probe_rules(&trial, &vocab) {
            Ok(_) => kept.push(r.clone()),
            Err(e) => crate::log::info(format!("universe rule '{}' dropped: {e}", r.name)),
        }
    }
    (kept_props, kept)
}

/// The interpreter: what does this action do, in primitives. Property
/// names in the answer are checked like a type's: near misses are mapped by
/// the world's aliases, and what is still unknown is sent back to be fixed.
async fn interpret(ctx: &Ctx, actor: &str, text: &str, context: &str) -> anyhow::Result<Value> {
    let system = format!("{}\n\nThe universe:\n{}\n\nProperties this universe knows:\n{}", prompts::INTERPRET_TASK, ctx.universe(), ctx.vocab().describe());
    let user = format!("{actor} does this: \"{text}\"\n\nThe situation (JSON):\n{context}");
    let mut req = Req::new(Role::Character, system, user);
    req.max_tokens = 1200;
    req.effort = Some("low");
    let reply = ctx.llm.complete(&req, "interpret").await?;
    let mut v = extract_json(&reply)?;
    let unknown = known_deed_props(ctx, &mut v).await;
    if unknown.is_empty() {
        return Ok(v);
    }
    let names = ctx.vocab().writable_names().join(", ");
    req.messages.push(Msg { user: false, text: reply });
    req.messages.push(Msg { user: true, text: prompts::repair(&format!("unknown properties: {}; this universe knows: {names}. Use only those names, or leave the change out.", unknown.join(", "))) });
    let fixed = ctx.llm.complete(&req, "interpret").await.ok().and_then(|r| extract_json(&r).ok());
    let mut v = fixed.unwrap_or(v);
    let still = known_deed_props(ctx, &mut v).await;
    if !still.is_empty() {
        crate::log::info(format!("deed '{text}': left out unknown properties {}", still.join(", ")));
        strip_deed_props(&mut v, &still);
    }
    Ok(v)
}

/// The property maps in a deed answer (what it changes, makes, or dresses someone in).
fn deed_prop_maps(v: &mut Value) -> Vec<&mut serde_json::Map<String, Value>> {
    let mut out = Vec::new();
    let Some(o) = v.as_object_mut() else { return out };
    for (k, list) in o.iter_mut() {
        let items: Vec<&mut Value> = match k.as_str() {
            "changes" | "create" => list.as_array_mut().map(|a| a.iter_mut().collect()).unwrap_or_default(),
            "being" => list.get_mut("wear").and_then(|w| w.as_array_mut()).map(|a| a.iter_mut().collect()).unwrap_or_default(),
            _ => Vec::new(),
        };
        out.extend(items.into_iter().filter_map(|i| i.get_mut("props")).filter_map(|p| p.as_object_mut()));
    }
    out
}

fn strip_deed_props(v: &mut Value, names: &[String]) {
    for m in deed_prop_maps(v) {
        m.retain(|k, _| !names.contains(k));
    }
}

/// Map a deed answer's property names onto this world's: exact, then known
/// aliases, then one LLM ruling per new name (kept with the world). Act
/// properties (`force`) are left out: no deed sets them. Returns the names
/// that are still not properties.
async fn known_deed_props(ctx: &Ctx, v: &mut Value) -> Vec<String> {
    let vocab = ctx.vocab();
    let aliases = crate::sim::persist::prop_aliases(&ctx.db);
    let mut names: Vec<String> = deed_prop_maps(v).into_iter().flat_map(|m| m.keys().cloned().collect::<Vec<_>>()).collect();
    names.sort();
    names.dedup();
    let mut rename: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut unknown = Vec::new();
    let mut ask = Vec::new();
    let mut acts = Vec::new();
    for n in names {
        match vocab.lookup(&n) {
            Some(i) if vocab.writable(i) => {
                if vocab.names[i] != n {
                    rename.insert(n, vocab.names[i].clone());
                }
            }
            Some(_) => {
                crate::log::info(format!("deed answer set '{n}', which only actions have; left out"));
                acts.push(n);
            }
            None if aliases.get(&crate::sim::props::norm_name(&n)).is_some_and(|a| a.is_empty()) => unknown.push(n),
            None => ask.push(n),
        }
    }
    if !ask.is_empty() {
        let ruled = rule_aliases(ctx, &ask, &vocab).await;
        for n in ask {
            match ruled.as_ref().and_then(|r| r.get(&n)) {
                Some(Some(p)) => {
                    rename.insert(n, p.clone());
                }
                _ => unknown.push(n),
            }
        }
        if let Some(r) = ruled {
            let add: Vec<(String, String)> = r.into_iter().map(|(k, v)| (k, v.unwrap_or_default())).collect();
            if let Err(e) = crate::sim::persist::set_prop_aliases(&ctx.db, &add) {
                crate::log::error(format!("couldn't keep property aliases: {e}"));
            }
        }
    }
    strip_deed_props(v, &acts);
    for m in deed_prop_maps(v) {
        for (from, to) in &rename {
            if let Some(x) = m.remove(from) {
                m.entry(to.clone()).or_insert(x);
            }
        }
    }
    unknown
}

/// One LLM ruling on what each unknown name means (a writable property, or
/// none). None when the ruling couldn't be had.
async fn rule_aliases(ctx: &Ctx, names: &[String], vocab: &crate::sim::props::Vocab) -> Option<std::collections::BTreeMap<String, Option<String>>> {
    let system = format!("{}\n\nThe world's properties:\n{}", prompts::PROP_ALIAS_TASK, vocab.describe());
    let mut req = Req::new(Role::Character, system, format!("Unknown names: {}", names.join(", ")));
    req.max_tokens = 300;
    req.effort = Some("low");
    let reply = ctx.llm.complete(&req, "alias").await.map_err(|e| crate::log::info(format!("alias ruling failed: {e}"))).ok()?;
    let v = extract_json(&reply).ok()?;
    let mut out = std::collections::BTreeMap::new();
    for n in names {
        let to = v.get(n).and_then(|x| x.as_str()).and_then(|p| vocab.id(p)).filter(|i| vocab.writable(*i)).map(|i| vocab.names[i].clone());
        out.insert(n.clone(), to);
    }
    Some(out)
}

/// Two characters talk; returns (speaker id, line) pairs.
async fn chat(ctx: &Ctx, a: i64, b: i64, context: &str) -> anyhow::Result<Vec<(i64, String)>> {
    let chars = ctx.db.characters()?;
    let persona = |id: i64| chars.iter().find(|c| c.id == id).and_then(|c| serde_json::from_str::<Persona>(&c.persona_json).ok()).unwrap_or_default();
    let (pa, pb) = (persona(a), persona(b));
    let mem = |id: i64| ctx.db.memories(id).unwrap_or_default().iter().rev().take(5).map(|m| m.text.clone()).collect::<Vec<_>>().join(" | ");
    let system = format!("{}\n\nThe universe:\n{}", prompts::CHAT_TASK, ctx.universe());
    let user = format!(
        "{} ({}; voice: {}) meets {} ({}; voice: {}).\n{}'s recent memories: {}\n{}'s recent memories: {}\n\n{}",
        pa.name, pa.personality, pa.voice, pb.name, pb.personality, pb.voice, pa.name, mem(a), pb.name, mem(b), context
    );
    let mut req = Req::new(Role::Character, system, user);
    req.max_tokens = 600;
    req.effort = Some("low");
    let reply = ctx.llm.complete(&req, "chat").await?;
    let v = extract_json(&reply)?;
    let mut out = Vec::new();
    for l in v.get("lines").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().take(6) {
        let who = s(l, "who");
        let text = s(l, "text");
        if text.trim().is_empty() {
            continue;
        }
        let id = if who.eq_ignore_ascii_case(&pb.name) || (!pb.name.is_empty() && who.to_lowercase().contains(&pb.name.to_lowercase())) { b } else { a };
        out.push((id, text.trim().to_string()));
        let mem_text = format!("I talked with {}: \"{}\"", if id == a { &pb.name } else { &pa.name }, text.trim());
        let _ = ctx.db.add_memory(id, crate::db::now(), &mem_text, 0.3);
    }
    Ok(out)
}

/// Write a small new object type on demand (for interpretations and spawn()).
async fn build_item_type(ctx: &Ctx, name: &str, description: &str, size: [f32; 3], props: &[(String, f32)], fits: Option<&str>) -> anyhow::Result<u32> {
    let system = prompts::builder_system(&ctx.universe(), &ctx.vocab());
    let props_s = if props.is_empty() { "choose fitting ones".to_string() } else { props.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", ") };
    let task = format!(
        "Object type to write: \"{name}\"\nDescription: {description}\nApproximate size (w × h × d, metres): {:?}\nProperties (meta.props): {props_s}\nIt is a thing people can pick up and use, unless it is clearly too big.\n\n{}",
        size,
        prompts::TYPE_TASK
    );
    let (task, tags): (String, &[&str]) = match fits {
        Some(body) => {
            let src = ctx.db.types().unwrap_or_default().into_iter().rev().find(|t| t.name == body && (t.status == "builtin" || t.status == "ok")).map(|t| t.code).unwrap_or_default();
            (format!("{task}\n\n{}", prompts::layer_note(body, &src)), &["layer"])
        }
        None => (task, &[]),
    };
    let t = build_type(ctx, &system, task, name, tags, &[]).await?;
    let tname = t.ct.meta.name.clone();
    let ok = ctx
        .commit(CommitRequest { kind: "interp", summary: format!("new kind of thing: {tname}"), new_types: vec![t], placements: vec![], nudge: true, region: None, look: None })
        .await
        .map_err(|d| anyhow::anyhow!(format_diags(&d)))?;
    let id = ok.snapshot.scene.types.values().filter(|e| e.name() == tname).map(|e| e.id).max().ok_or_else(|| anyhow::anyhow!("type vanished"))?;
    let _ = ctx.events.send(Event::Flip { snap: ok.snapshot, region: None });
    Ok(id)
}

/// Rewrite one thing's shape: the builder edits its current code (working
/// in another thing's, if given).
async fn edit_item_type(ctx: &Ctx, name: &str, source: &str, change: &str, spot: &str, cuts: &[[f32; 4]], with: Option<&(String, String, f32)>) -> anyhow::Result<u32> {
    let system = prompts::builder_system(&ctx.universe(), &ctx.vocab());
    let task = prompts::edit_task(name, source, change, spot, cuts, with);
    let mut t = build_type(ctx, &system, task, name, &[], &[]).await?;
    // The thing is found again by name: insist on the one asked for.
    if t.ct.meta.name != name {
        let mut ct = (*t.ct).clone();
        ct.meta.name = name.to_string();
        ct.source = rename_meta(&ct.source, &t.ct.meta.name, name);
        t.ct = Arc::new(ct);
    }
    let ok = ctx
        .commit(CommitRequest { kind: "interp", summary: format!("reshaped: {name}"), new_types: vec![t], placements: vec![], nudge: true, region: None, look: None })
        .await
        .map_err(|d| anyhow::anyhow!(format_diags(&d)))?;
    let id = ok.snapshot.scene.types.values().filter(|e| e.name() == name).map(|e| e.id).max().ok_or_else(|| anyhow::anyhow!("type vanished"))?;
    let _ = ctx.events.send(Event::Flip { snap: ok.snapshot, region: None });
    Ok(id)
}

/// Replace the name string in a module's meta (so the stored source matches).
fn rename_meta(src: &str, old: &str, new: &str) -> String {
    let esc = new.replace('\\', "").replace('"', "'");
    for q in ['"', '\''] {
        let from = format!("name: {q}{old}{q}");
        if src.contains(&from) {
            return src.replacen(&from, &format!("name: \"{esc}\""), 1);
        }
    }
    src.to_string()
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
    let summary = ctx.db.summary(cid).map(|s| s.0).unwrap_or_else(|| "I have not met the traveler before.".into());
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
        ctx.universe(),
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
        "Your memory of the traveler so far: {summary}\n\nSpecific memories:\n{}\n\nRight now: {context}",
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
    let mem = format!("The traveler said: \"{}\" I replied: \"{}\"", text.trim(), reply.trim());
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

#[cfg(test)]
mod body_tests {
    use super::*;

    #[test]
    fn written_bodies_keep_their_line_in_the_source() {
        let src = "export const meta = { name: \"x\", bounds: [1, 1, 1], tags: [\"body\"],\n  body: { height: 1, roles: [] } };";
        let out = insert_body_from(src, "quadruped").unwrap();
        assert!(out.contains("body: { from: \"quadruped\", height: 1"), "{out}");
        // A comment that says "body:" first is skipped.
        let src2 = format!("// body: a test\n{src}");
        assert!(insert_body_from(&src2, "cat").unwrap().contains("body: { from: \"cat\","));
        assert!((size_on_own_body(0.34, Some(1.0), 0.32) - 1.0625).abs() < 1e-3);
        assert_eq!(size_on_own_body(2.0, None, 5.0), 2.0);
    }
}
