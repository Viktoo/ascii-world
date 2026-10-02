//! LLM layer: Anthropic Messages API or any OpenAI-compatible endpoint
//! (Ollama, LM Studio, …), with SSE streaming, per-role models, cost
//! accounting and an optional budget cap. The only network calls Pocket makes.

use crate::db::Db;
use anyhow::{Result, anyhow, bail};
use futures_util::StreamExt;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Builder,
    Character,
    Decider,
    Summarizer,
}

impl Role {
    fn env(self) -> &'static str {
        match self {
            Role::Builder => "POCKET_MODEL_BUILDER",
            Role::Character => "POCKET_MODEL_CHARACTER",
            Role::Decider => "POCKET_MODEL_DECIDER",
            Role::Summarizer => "POCKET_MODEL_SUMMARIZER",
        }
    }
}

/// Test double: (system, messages) → reply.
#[cfg(test)]
pub type Script = Arc<dyn Fn(&str, &[Msg]) -> String + Send + Sync>;

#[derive(Clone)]
enum Provider {
    /// `workspace`: required for user-scoped keys (sk-ant-usr-…).
    Anthropic { key: String, base: String, workspace: Option<String> },
    OpenAi { key: Option<String>, base: String },
    #[cfg(test)]
    Script(Script),
}

pub struct Msg {
    pub user: bool,
    pub text: String,
}

pub struct Req {
    pub role: Role,
    /// Stable prefix (cached on Anthropic).
    pub system: String,
    /// Per-request system text appended after the cached prefix.
    pub system_tail: String,
    pub messages: Vec<Msg>,
    pub max_tokens: u32,
    pub effort: Option<&'static str>,
}

impl Req {
    pub fn new(role: Role, system: impl Into<String>, user: impl Into<String>) -> Req {
        Req { role, system: system.into(), system_tail: String::new(), messages: vec![Msg { user: true, text: user.into() }], max_tokens: 16000, effort: None }
    }
}

#[derive(Debug)]
pub struct BudgetReached;
impl std::fmt::Display for BudgetReached {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "budget reached")
    }
}
impl std::error::Error for BudgetReached {}

pub struct Llm {
    http: reqwest::Client,
    provider: Provider,
    models: [String; 4],
    db: Arc<Db>,
    pub spent: Mutex<f64>,
    /// Spending cap for this session (POCKET_BUDGET_USD, or the settings screen).
    pub budget: Mutex<Option<f64>>,
}

/// (input, output, cache read) USD per million tokens.
fn price(model: &str) -> (f64, f64, f64) {
    if let (Ok(i), Ok(o)) = (std::env::var("POCKET_PRICE_IN"), std::env::var("POCKET_PRICE_OUT")) {
        let i: f64 = i.parse().unwrap_or(0.0);
        return (i, o.parse().unwrap_or(0.0), i * 0.1);
    }
    match model {
        m if m.starts_with("claude-opus-5-5") => (4.0, 20.0, 0.20),
        m if m.starts_with("claude-opus") => (5.0, 25.0, 0.50),
        m if m.starts_with("claude-fable") => (10.0, 50.0, 0.25),
        m if m.starts_with("claude-sonnet-4") => (3.0, 15.0, 0.30),
        m if m.starts_with("claude-sonnet") => (2.0, 10.0, 0.20),
        m if m.starts_with("claude-haiku") => (1.0, 5.0, 0.10),
        _ => (0.0, 0.0, 0.0),
    }
}

/// Models that accept `fallbacks: "default"` on the Claude API.
fn supports_fallback(model: &str) -> bool {
    ["claude-opus-5-5", "claude-opus-5", "claude-fable-5-1", "claude-sonnet-5-5"].contains(&model)
}

#[derive(Default, Debug, Clone, Copy)]
struct Usage {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
}

impl Llm {
    /// None when no provider is configured (the base layer still works).
    pub fn from_env(db: Arc<Db>) -> Option<Arc<Llm>> {
        let base_override = std::env::var("POCKET_LLM_BASE_URL").ok().filter(|s| !s.is_empty());
        let provider = match base_override {
            Some(base) if !base.contains("anthropic.com") => Provider::OpenAi {
                key: std::env::var("POCKET_LLM_API_KEY").or_else(|_| std::env::var("OPENAI_API_KEY")).ok().filter(|s| !s.is_empty()),
                base: base.trim_end_matches('/').to_string(),
            },
            other => {
                let key = std::env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty())?;
                let workspace = std::env::var("ANTHROPIC_WORKSPACE_ID").or_else(|_| std::env::var("POCKET_ANTHROPIC_WORKSPACE")).ok().filter(|s| !s.is_empty());
                Provider::Anthropic { key, base: other.unwrap_or_else(|| "https://api.anthropic.com".into()).trim_end_matches('/').to_string(), workspace }
            }
        };
        let default_model = |r: Role| -> String {
            if let Ok(m) = std::env::var(r.env()) {
                return m;
            }
            if let Ok(m) = std::env::var("POCKET_MODEL") {
                return m;
            }
            match (&provider, r) {
                (Provider::Anthropic { .. }, Role::Builder | Role::Character) => "claude-opus-5-5".into(),
                (Provider::Anthropic { .. }, _) => "claude-haiku-4-5".into(),
                _ => "llama3.1".into(),
            }
        };
        let models = [default_model(Role::Builder), default_model(Role::Character), default_model(Role::Decider), default_model(Role::Summarizer)];
        let http = reqwest::Client::builder().connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(600)).build().ok()?;
        let budget = crate::settings::Settings::load().budget();
        Some(Arc::new(Llm { http, provider, models, db, spent: Mutex::new(0.0), budget: Mutex::new(budget) }))
    }

    #[cfg(test)]
    pub fn scripted(db: Arc<Db>, f: Script) -> Arc<Llm> {
        Arc::new(Llm { http: reqwest::Client::new(), provider: Provider::Script(f), models: std::array::from_fn(|_| "script".to_string()), db, spent: Mutex::new(0.0), budget: Mutex::new(None) })
    }

    pub fn model(&self, r: Role) -> &str {
        &self.models[r as usize]
    }

    pub fn describe(&self) -> String {
        match &self.provider {
            Provider::Anthropic { .. } => format!("Anthropic ({})", self.models[0]),
            Provider::OpenAi { base, .. } => format!("{base} ({})", self.models[0]),
            #[cfg(test)]
            Provider::Script(_) => "script".into(),
        }
    }

    pub fn over_budget(&self) -> bool {
        self.budget.lock().is_some_and(|b| *self.spent.lock() >= b)
    }

    fn account(&self, purpose: &str, model: &str, u: Usage) {
        let (pi, po, pc) = price(model);
        let cost = (u.input as f64 * pi + u.cache_write as f64 * pi * 1.25 + u.cache_read as f64 * pc + u.output as f64 * po) / 1e6;
        *self.spent.lock() += cost;
        let _ = self.db.add_usage(purpose, u.input + u.cache_read + u.cache_write, u.output, cost);
    }

    /// Complete a request, returning the full text.
    pub async fn complete(&self, req: &Req, purpose: &str) -> Result<String> {
        self.stream(req, purpose, |_| {}).await
    }

    /// Stream a request; `on_text` receives each text delta.
    pub async fn stream(&self, req: &Req, purpose: &str, mut on_text: impl FnMut(&str)) -> Result<String> {
        if self.over_budget() {
            return Err(BudgetReached.into());
        }
        let model = self.model(req.role).to_string();
        let mut attempt = 0;
        loop {
            attempt += 1;
            let r = match &self.provider {
                Provider::Anthropic { key, base, workspace } => self.anthropic(key, base, workspace.as_deref(), &model, req, &mut on_text).await,
                Provider::OpenAi { key, base } => self.openai(key.as_deref(), base, &model, req, &mut on_text).await,
                #[cfg(test)]
                Provider::Script(f) => {
                    let system = format!("{}\n\n{}", req.system, req.system_tail);
                    let text = f(&system, &req.messages);
                    for chunk in text.as_bytes().chunks(16) {
                        on_text(&String::from_utf8_lossy(chunk));
                    }
                    Ok((text, Usage { input: 1000, output: 100, ..Default::default() }))
                }
            };
            match r {
                Ok((text, usage)) => {
                    self.account(purpose, &model, usage);
                    return Ok(text);
                }
                Err(e) if attempt < 3 && is_retryable(&e) => {
                    tokio::time::sleep(Duration::from_millis(800 * attempt)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn anthropic(&self, key: &str, base: &str, workspace: Option<&str>, model: &str, req: &Req, on_text: &mut impl FnMut(&str)) -> Result<(String, Usage)> {
        let mut system = vec![json!({ "type": "text", "text": req.system, "cache_control": { "type": "ephemeral" } })];
        if !req.system_tail.is_empty() {
            system.push(json!({ "type": "text", "text": req.system_tail }));
        }
        let messages: Vec<Value> = req.messages.iter().map(|m| json!({ "role": if m.user { "user" } else { "assistant" }, "content": m.text })).collect();
        let mut body = json!({
            "model": model,
            "max_tokens": req.max_tokens,
            "stream": true,
            "system": system,
            "messages": messages,
        });
        let is_claude5 = !model.starts_with("claude-haiku") && !model.starts_with("claude-3");
        if let (Some(e), true) = (req.effort, is_claude5) {
            body["output_config"] = json!({ "effort": e });
        }
        let mut rb = self.http.post(format!("{base}/v1/messages")).header("x-api-key", key).header("anthropic-version", "2023-06-01").header("content-type", "application/json");
        if let Some(ws) = workspace {
            rb = rb.header("anthropic-workspace-id", ws);
        }
        if supports_fallback(model) {
            body["fallbacks"] = json!("default");
            rb = rb.header("anthropic-beta", "server-side-fallback-2026-07-01");
        }
        let resp = rb.json(&body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let t = resp.text().await.unwrap_or_default();
            bail!(HttpError { status: status.as_u16(), body: t.chars().take(600).collect() });
        }
        let mut usage = Usage::default();
        let mut text = String::new();
        let mut stop = String::new();
        let mut sse = Sse::default();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            for data in sse.feed(&chunk) {
                let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };
                match v["type"].as_str().unwrap_or("") {
                    "message_start" => {
                        let u = &v["message"]["usage"];
                        usage.input = u["input_tokens"].as_u64().unwrap_or(0);
                        usage.cache_read = u["cache_read_input_tokens"].as_u64().unwrap_or(0);
                        usage.cache_write = u["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                    }
                    "content_block_delta" => {
                        if v["delta"]["type"] == "text_delta" {
                            if let Some(t) = v["delta"]["text"].as_str() {
                                text.push_str(t);
                                on_text(t);
                            }
                        }
                    }
                    "message_delta" => {
                        if let Some(o) = v["usage"]["output_tokens"].as_u64() {
                            usage.output = o;
                        }
                        if let Some(s) = v["delta"]["stop_reason"].as_str() {
                            stop = s.to_string();
                        }
                    }
                    "error" => bail!("API error: {}", v["error"]["message"].as_str().unwrap_or("unknown")),
                    _ => {}
                }
            }
        }
        if stop == "refusal" {
            self.account("refused", model, usage);
            bail!("the model declined this request");
        }
        Ok((text, usage))
    }

    async fn openai(&self, key: Option<&str>, base: &str, model: &str, req: &Req, on_text: &mut impl FnMut(&str)) -> Result<(String, Usage)> {
        let mut messages = vec![json!({ "role": "system", "content": format!("{}\n\n{}", req.system, req.system_tail) })];
        for m in &req.messages {
            messages.push(json!({ "role": if m.user { "user" } else { "assistant" }, "content": m.text }));
        }
        let body = json!({
            "model": model,
            "max_tokens": req.max_tokens.min(8192),
            "stream": true,
            "stream_options": { "include_usage": true },
            "messages": messages,
        });
        let mut rb = self.http.post(format!("{base}/chat/completions")).header("content-type", "application/json");
        if let Some(k) = key {
            rb = rb.bearer_auth(k);
        }
        let resp = rb.json(&body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let t = resp.text().await.unwrap_or_default();
            bail!(HttpError { status: status.as_u16(), body: t.chars().take(600).collect() });
        }
        let mut usage = Usage::default();
        let mut text = String::new();
        let mut sse = Sse::default();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            for data in sse.feed(&chunk) {
                if data.trim() == "[DONE]" {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };
                if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                    text.push_str(t);
                    on_text(t);
                }
                if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
                    usage.input = u["prompt_tokens"].as_u64().unwrap_or(0);
                    usage.output = u["completion_tokens"].as_u64().unwrap_or(0);
                }
            }
        }
        Ok((text, usage))
    }
}

#[derive(Debug)]
struct HttpError {
    status: u16,
    body: String,
}
impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}: {}", self.status, self.body)
    }
}
impl std::error::Error for HttpError {}

fn is_retryable(e: &anyhow::Error) -> bool {
    if let Some(h) = e.downcast_ref::<HttpError>() {
        return h.status == 429 || h.status == 408 || h.status >= 500;
    }
    e.downcast_ref::<reqwest::Error>().is_some_and(|r| r.is_connect() || r.is_timeout())
}

/// Minimal server-sent-events line splitter.
#[derive(Default)]
struct Sse {
    buf: String,
    data: String,
}

impl Sse {
    fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.push_str(&String::from_utf8_lossy(bytes));
        let mut out = Vec::new();
        while let Some(i) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=i).collect();
            let line = line.trim_end_matches(['\n', '\r']);
            if line.is_empty() {
                if !self.data.is_empty() {
                    out.push(std::mem::take(&mut self.data));
                }
            } else if let Some(d) = line.strip_prefix("data:") {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(d.trim_start());
            }
        }
        out
    }
}

/// Pull the first JSON object out of a model reply (tolerates prose and fences).
pub fn extract_json(text: &str) -> Result<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
        return Ok(v);
    }
    let fenced = text.find("```json").map(|i| &text[i + 7..]).and_then(|t| t.find("```").map(|j| &t[..j]));
    if let Some(f) = fenced {
        if let Ok(v) = serde_json::from_str::<Value>(f.trim()) {
            return Ok(v);
        }
    }
    let start = text.find('{').ok_or_else(|| anyhow!("no JSON object in reply"))?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, c) in text[start..].char_indices() {
        if in_str {
            match (esc, c) {
                (true, _) => esc = false,
                (false, '\\') => esc = true,
                (false, '"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(serde_json::from_str(&text[start..start + i + 1])?);
                }
            }
            _ => {}
        }
    }
    bail!("unterminated JSON object in reply")
}

/// Pull a ```js code block out of a reply.
pub fn extract_code(text: &str) -> Option<String> {
    for fence in ["```javascript", "```js", "```"] {
        if let Some(i) = text.find(fence) {
            let rest = &text[i + fence.len()..];
            let rest = rest.strip_prefix('\n').unwrap_or(rest);
            if let Some(j) = rest.find("```") {
                return Some(rest[..j].trim().to_string());
            }
        }
    }
    text.contains("export function sdf").then(|| text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_splits_events() {
        let mut s = Sse::default();
        let mut out = s.feed(b"event: a\ndata: {\"x\":1}\n\ndata: {\"y\"");
        out.extend(s.feed(b":2}\n\n"));
        assert_eq!(out, vec!["{\"x\":1}", "{\"y\":2}"]);
    }

    #[test]
    fn json_and_code_extraction() {
        let v = extract_json("Sure! ```json\n{\"a\": {\"b\": \"}\"}}\n``` done").unwrap();
        assert_eq!(v["a"]["b"], "}");
        let v = extract_json("prefix {\"k\": [1, 2]} suffix").unwrap();
        assert_eq!(v["k"][1], 2);
        let c = extract_code("here:\n```js\nexport const meta = {};\n```\n").unwrap();
        assert_eq!(c, "export const meta = {};");
    }
}
