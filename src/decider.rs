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
                "new_building" => "watch",
                "player_near" => "watch",
                _ => "ignore",
            };
            Some(Decision { action: action.into(), line: None })
        })
    }
}

pub struct LlmDecider {
    pub llm: Arc<Llm>,
    pub bible: String,
}

impl Decider for LlmDecider {
    fn decide(&self, ctx: DecisionCtx) -> BoxFuture<'_, Option<Decision>> {
        Box::pin(async move {
            if ctx.event == "night_fell" {
                return Some(Decision { action: "go_home".into(), line: None });
            }
            let system = format!("{}\n\nUniverse:\n{}", crate::prompts::DECIDER_TASK, self.bible);
            let user = serde_json::to_string_pretty(&ctx).ok()?;
            let mut req = Req::new(Role::Decider, system, user);
            req.max_tokens = 300;
            req.effort = Some("low");
            let reply = self.llm.complete(&req, "decide").await.ok()?;
            let v = extract_json(&reply).ok()?;
            let action = v.get("action")?.as_str()?.to_string();
            let line = v.get("line").and_then(|l| l.as_str()).map(str::to_string).filter(|s| !s.trim().is_empty());
            Some(Decision { action, line })
        })
    }
}

/// POSTs the context as JSON and expects `{"action": …, "line": …}` back.
pub struct HttpDecider {
    pub url: String,
    pub http: reqwest::Client,
}

impl Decider for HttpDecider {
    fn decide(&self, ctx: DecisionCtx) -> BoxFuture<'_, Option<Decision>> {
        Box::pin(async move {
            let r = self.http.post(&self.url).json(&ctx).send().await.ok()?;
            r.json::<Decision>().await.ok()
        })
    }
}

pub fn from_env(llm: Option<Arc<Llm>>, bible: &str) -> Arc<dyn Decider> {
    if let Ok(url) = std::env::var("POCKET_DECIDER_URL") {
        if !url.is_empty() {
            return Arc::new(HttpDecider { url, http: reqwest::Client::new() });
        }
    }
    match llm {
        Some(llm) => Arc::new(LlmDecider { llm, bible: bible.to_string() }),
        None => Arc::new(RuleDecider),
    }
}
