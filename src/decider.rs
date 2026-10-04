//! Character intents on events. Pluggable: an LLM with structured output by
//! default, an external decision service (e.g. Jev) via POCKET_DECIDER_URL, or
//! simple rules when no model is configured.

use crate::llm::{Llm, Req, Role, extract_json};
use crate::world::characters::Decision;
use futures_util::future::BoxFuture;
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize)]
pub struct DecisionCtx {
    pub character: String,
    pub persona: String,
    /// "player_near", "night_fell", "new_building", …
    pub event: String,
    pub context: String,
}

pub trait Decider: Send + Sync {
    fn decide(&self, ctx: DecisionCtx) -> BoxFuture<'_, Option<Decision>>;
}

pub struct RuleDecider;

impl Decider for RuleDecider {
    fn decide(&self, ctx: DecisionCtx) -> BoxFuture<'_, Option<Decision>> {
        Box::pin(async move {
            let action = match ctx.event.as_str() {
                "night_fell" => "go_home",
                "new_building" | "something_made" | "changed" | "new_being" => "watch",
                "player_near" => "watch",
                _ => "ignore",
            };
            Some(Decision { action: action.into(), ..Default::default() })
        })
    }
}

pub struct LlmDecider {
    pub llm: Arc<Llm>,
    pub universe: crate::brain::Universe,
}

impl Decider for LlmDecider {
    fn decide(&self, ctx: DecisionCtx) -> BoxFuture<'_, Option<Decision>> {
        Box::pin(async move {
            if ctx.event == "night_fell" {
                return Some(Decision { action: "go_home".into(), ..Default::default() });
            }
            let universe = self.universe.read().map(|u| u.clone()).unwrap_or_default();
            let system = format!("{}\n\nUniverse:\n{}", crate::prompts::DECIDER_TASK, universe);
            let user = serde_json::to_string_pretty(&ctx).ok()?;
            let mut req = Req::new(Role::Decider, system, user);
            req.max_tokens = 700;
            req.effort = Some("low");
            let reply = self.llm.complete(&req, "decide").await.ok()?;
            let v = extract_json(&reply).ok()?;
            parse_decision(&v)
        })
    }
}

/// Accept both the plan format ({goal, say, steps}) and the older single
/// action format ({action, line}).
pub fn parse_decision(v: &serde_json::Value) -> Option<Decision> {
    let mut d: Decision = serde_json::from_value(v.clone()).ok()?;
    d.line = d.line.filter(|s| !s.trim().is_empty());
    d.say = d.say.filter(|s| !s.trim().is_empty() && s != "null");
    if d.action.is_empty() && d.steps.is_empty() && d.say.is_none() && d.promise.is_none() && d.aim.is_none() && d.kept.is_none() {
        return None;
    }
    Some(d)
}

/// POSTs the context as JSON and expects `{"action": …, "line": …}` (or a plan) back.
pub struct HttpDecider {
    pub url: String,
    pub http: reqwest::Client,
}

impl Decider for HttpDecider {
    fn decide(&self, ctx: DecisionCtx) -> BoxFuture<'_, Option<Decision>> {
        Box::pin(async move {
            let r = self.http.post(&self.url).json(&ctx).send().await.ok()?;
            let v = r.json::<serde_json::Value>().await.ok()?;
            parse_decision(&v)
        })
    }
}

pub fn from_env(llm: Option<Arc<Llm>>, universe: crate::brain::Universe) -> Arc<dyn Decider> {
    if let Ok(url) = std::env::var("POCKET_DECIDER_URL") {
        if !url.is_empty() {
            return Arc::new(HttpDecider { url, http: reqwest::Client::new() });
        }
    }
    match llm {
        Some(llm) => Arc::new(LlmDecider { llm, universe }),
        None => Arc::new(RuleDecider),
    }
}
