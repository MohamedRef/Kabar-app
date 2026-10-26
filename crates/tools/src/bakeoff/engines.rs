//! The engines a bake-off calls, from the Mac. Request bodies and reply
//! parsing come from Kabar Cloud's own builders (`kabar_server::ai`), so the
//! bake-off measures exactly what production sends. Workers AI is reached
//! through Cloudflare's REST API (same models, same free daily Neurons) so
//! the bake-off can run before Kabar Cloud is deployed; local models through
//! Ollama.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use kabar_protocol::ai::{EngineCall, EngineId};
use kabar_server::ai as cloud;
use kabar_server::blobs::Image;
use kabar_server::config::{
    DEFAULT_CLAUDE_MODEL, DEFAULT_GEMINI_MODEL, DEFAULT_WORKERS_AI_MODEL, DEFAULT_WORKERS_AI_VISION_MODEL,
};
use serde_json::{Value, json};

pub const OLLAMA_URL: &str = "http://localhost:11434";
pub const CLOUDFLARE_API: &str = "https://api.cloudflare.com/client/v4";

/// One HTTP POST with a JSON body. `ureq` in the CLI, scripted in tests.
pub trait Http {
    /// (status, parsed body). A body that isn't JSON comes back as a string.
    fn post_json(&self, url: &str, headers: &[(&str, String)], body: &Value) -> Result<(u16, Value)>;
}

pub struct Ureq(ureq::Agent);

impl Ureq {
    pub fn new() -> Self {
        Self(
            ureq::Agent::config_builder()
                .http_status_as_error(false)
                // Local 4B models on an 8 GB Mac can take minutes on a long answer.
                .timeout_global(Some(Duration::from_secs(300)))
                .build()
                .into(),
        )
    }
}

impl Http for Ureq {
    fn post_json(&self, url: &str, headers: &[(&str, String)], body: &Value) -> Result<(u16, Value)> {
        let mut req = self.0.post(url).header("content-type", "application/json");
        for (k, v) in headers {
            req = req.header(*k, v);
        }
        let mut res = req.send(serde_json::to_vec(body)?).with_context(|| format!("POST {url}"))?;
        let status = res.status().as_u16();
        let text = res.body_mut().read_to_string()?;
        Ok((status, serde_json::from_str(&text).unwrap_or(Value::String(text))))
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    WorkersAi { account: String, token: String, vision_model: String },
    Gemini { key: String },
    Claude { key: String },
    Ollama { base: String },
}

/// One candidate: an engine id plus the model it runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Engine {
    pub id: EngineId,
    pub model: String,
    kind: Kind,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reply {
    pub text: String,
    /// The model that answered (Workers AI switches to its vision model for images).
    pub model: String,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
}

/// Why a call produced no reply.
#[derive(Debug, PartialEq)]
pub enum CallError {
    /// Out of free quota or rate-limited: stop calling this engine for now
    /// and record nothing, so a later run picks the item up again.
    Exhausted(String),
    /// Anything else; recorded against the item.
    Failed(String),
}

impl Engine {
    /// `workers_ai`, `gemini_free`, `claude`, or `local=<ollama model>`.
    /// Credentials come from the environment (`.env` through `just`).
    pub fn from_spec(spec: &str, env: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let var = |k: &str| env(k).filter(|v| !v.trim().is_empty());
        let need = |k: &str| var(k).with_context(|| format!("`{spec}` needs {k} in .env (docs/BAKEOFF.md)"));
        let (name, model) = spec.split_once('=').map_or((spec, None), |(n, m)| (n, Some(m.to_string())));
        Ok(match name {
            "workers_ai" => Self {
                id: EngineId::WorkersAi,
                model: model
                    .or_else(|| var("WORKERS_AI_MODEL"))
                    .unwrap_or_else(|| DEFAULT_WORKERS_AI_MODEL.into()),
                kind: Kind::WorkersAi {
                    account: need("CLOUDFLARE_ACCOUNT_ID")?,
                    token: need("CLOUDFLARE_API_TOKEN")?,
                    vision_model: var("WORKERS_AI_VISION_MODEL")
                        .unwrap_or_else(|| DEFAULT_WORKERS_AI_VISION_MODEL.into()),
                },
            },
            "gemini_free" => Self {
                id: EngineId::GeminiFree,
                model: model.or_else(|| var("GEMINI_MODEL")).unwrap_or_else(|| DEFAULT_GEMINI_MODEL.into()),
                kind: Kind::Gemini { key: need("GEMINI_API_KEY")? },
            },
            "claude" => Self {
                id: EngineId::Claude,
                model: model.or_else(|| var("CLAUDE_MODEL")).unwrap_or_else(|| DEFAULT_CLAUDE_MODEL.into()),
                kind: Kind::Claude { key: need("ANTHROPIC_API_KEY")? },
            },
            "local" => Self {
                id: EngineId::Local,
                model: model.context("`local` needs a model: local=<ollama model name>")?,
                kind: Kind::Ollama { base: var("OLLAMA_URL").unwrap_or_else(|| OLLAMA_URL.into()) },
            },
            _ => bail!("unknown engine `{spec}`: use workers_ai, gemini_free, claude or local=<model>"),
        })
    }

    /// File-safe name, unique per model: `workers_ai`, `local-qwen-sea-lion-v4-4b`.
    pub fn label(&self) -> String {
        match self.kind {
            Kind::Ollama { .. } => {
                let m: String =
                    self.model.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
                format!("local-{}", m.trim_matches('-'))
            }
            _ => self.id.as_str().to_string(),
        }
    }

    /// Calls a minute the free tier allows; the runner paces itself to it.
    pub fn per_minute(&self) -> Option<u32> {
        match self.kind {
            // Gemini Flash free tier: 10 requests a minute.
            Kind::Gemini { .. } => Some(10),
            _ => None,
        }
    }

    /// (url, headers, body) for one call.
    pub fn request(
        &self,
        call: &EngineCall,
        image: Option<&Image>,
    ) -> (String, Vec<(&'static str, String)>, Value) {
        match &self.kind {
            Kind::WorkersAi { account, token, vision_model } => {
                let model = if image.is_some() { vision_model } else { &self.model };
                (
                    format!("{CLOUDFLARE_API}/accounts/{account}/ai/run/{model}"),
                    vec![("authorization", format!("Bearer {token}"))],
                    cloud::workers_ai_input(call, image),
                )
            }
            Kind::Gemini { key } => (
                cloud::gemini_url(&self.model),
                vec![("x-goog-api-key", key.clone())],
                cloud::gemini_body(call, image),
            ),
            Kind::Claude { key } => (
                cloud::ANTHROPIC_URL.to_string(),
                vec![("x-api-key", key.clone()), ("anthropic-version", cloud::ANTHROPIC_VERSION.to_string())],
                cloud::claude_body(call, &self.model, image),
            ),
            Kind::Ollama { base } => {
                let mut user = json!({ "role": "user", "content": call.user });
                if let Some(img) = image {
                    user["images"] = json!([img.base64]);
                }
                let mut messages = Vec::new();
                if !call.system.trim().is_empty() {
                    messages.push(json!({ "role": "system", "content": call.system }));
                }
                messages.push(user);
                let mut body = json!({
                    "model": self.model,
                    "messages": messages,
                    "stream": false,
                    "options": { "num_predict": call.max_tokens.unwrap_or(cloud::DEFAULT_MAX_TOKENS) },
                });
                if call.json {
                    body["format"] = json!("json");
                }
                (format!("{}/api/chat", base.trim_end_matches('/')), Vec::new(), body)
            }
        }
    }

    /// Reads a provider's answer, or says why there is none.
    pub fn parse(&self, status: u16, body: &Value, image: bool) -> Result<Reply, CallError> {
        let model = match &self.kind {
            Kind::WorkersAi { vision_model, .. } if image => vision_model.clone(),
            _ => self.model.clone(),
        };
        let message = || {
            body.pointer("/errors/0/message")
                .or_else(|| body.pointer("/error/message"))
                .or_else(|| body.get("error"))
                .and_then(Value::as_str)
                .map_or_else(|| body.to_string(), str::to_string)
        };
        if status == 429 {
            return Err(CallError::Exhausted(format!(
                "{}: rate limited or out of quota: {}",
                self.label(),
                message()
            )));
        }
        let refused = |m: String| {
            let lower = m.to_lowercase();
            if ["neuron", "quota", "exhausted", "allocation"].iter().any(|w| lower.contains(w)) {
                CallError::Exhausted(format!("{}: {m}", self.label()))
            } else {
                CallError::Failed(format!("{}: HTTP {status}: {m}", self.label()))
            }
        };
        if !(200..300).contains(&status) {
            return Err(refused(message()));
        }
        let usage = |a: &str, b: &str| {
            (body.pointer(a).and_then(Value::as_u64), body.pointer(b).and_then(Value::as_u64))
        };
        let failed =
            |e: kabar_protocol::ai::EngineError| CallError::Failed(format!("{}: {e:?}", self.label()));
        let (text, (tokens_in, tokens_out)) = match &self.kind {
            Kind::WorkersAi { .. } => {
                if body.get("success").and_then(Value::as_bool) == Some(false) {
                    return Err(refused(message()));
                }
                let result = body.get("result").unwrap_or(body);
                (
                    cloud::parse_workers_ai(result).map_err(failed)?,
                    usage("/result/usage/prompt_tokens", "/result/usage/completion_tokens"),
                )
            }
            Kind::Gemini { .. } => (
                cloud::parse_gemini(body).map_err(failed)?,
                usage("/usageMetadata/promptTokenCount", "/usageMetadata/candidatesTokenCount"),
            ),
            Kind::Claude { .. } => (
                cloud::parse_claude(body).map_err(failed)?,
                usage("/usage/input_tokens", "/usage/output_tokens"),
            ),
            Kind::Ollama { .. } => (
                body.pointer("/message/content").and_then(Value::as_str).map(str::to_string).ok_or_else(
                    || CallError::Failed(format!("{}: no message.content: {}", self.label(), message())),
                )?,
                usage("/prompt_eval_count", "/eval_count"),
            ),
        };
        if text.trim().is_empty() {
            return Err(CallError::Failed(format!("{}: empty answer", self.label())));
        }
        Ok(Reply { text, model, tokens_in, tokens_out })
    }

    pub fn call(
        &self,
        http: &dyn Http,
        call: &EngineCall,
        image: Option<&Image>,
    ) -> Result<Reply, CallError> {
        let (url, headers, body) = self.request(call, image);
        // Unreachable (Ollama not running, no network) says nothing about the
        // item: stop this engine and leave the item for the next run.
        let (status, out) = http
            .post_json(&url, &headers, &body)
            .map_err(|e| CallError::Exhausted(format!("{} unreachable: {e:#}", self.label())))?;
        self.parse(status, &out, image.is_some())
    }
}
