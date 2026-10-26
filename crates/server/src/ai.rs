//! AI proxy for the router in `kabar-core`: request checks, the shared cache
//! key, and each server-side engine's request/response shapes. The router
//! renders prompts and validates answers; the server adds the key, enforces
//! privacy and quotas, and caches. HTTP calls live in `wasm`.

use kabar_protocol::ai::{EngineCall, EngineError, EngineId, Task};
use serde_json::{Value, json};

use crate::config::{Config, SecretString};
use crate::quota::Service;

pub const MAX_PROMPT_CHARS: usize = 48_000;
pub const DEFAULT_MAX_TOKENS: u32 = 1_024;
pub const MAX_MAX_TOKENS: u32 = 8_192;

pub const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
pub const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
pub const ANTHROPIC_VERSION: &str = "2026-10-21";

/// HTTP status for an engine error.
pub fn status_for(e: &EngineError) -> u16 {
    match e {
        EngineError::PrivacyBlocked => 403,
        EngineError::QuotaExhausted { .. } | EngineError::RateLimited { .. } => 429,
        EngineError::Unavailable { .. } => 503,
        EngineError::Other { .. } => 502,
    }
}

fn unavailable(reason: impl Into<String>) -> EngineError {
    EngineError::Unavailable { reason: reason.into() }
}

fn other(message: impl Into<String>) -> EngineError {
    EngineError::Other { message: message.into() }
}

/// Which upstream an accepted call goes to, with what it needs.
#[derive(Debug, PartialEq)]
pub enum Route {
    WorkersAi { model: String },
    Gemini { model: String, key: SecretString },
    Claude { model: String, key: SecretString },
}

impl Route {
    pub fn engine(&self) -> EngineId {
        match self {
            Route::WorkersAi { .. } => EngineId::WorkersAi,
            Route::Gemini { .. } => EngineId::GeminiFree,
            Route::Claude { .. } => EngineId::Claude,
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Route::WorkersAi { model } | Route::Gemini { model, .. } | Route::Claude { model, .. } => model,
        }
    }

    pub fn service(&self) -> Service {
        match self {
            Route::WorkersAi { .. } => Service::WorkersAi,
            Route::Gemini { .. } => Service::Gemini,
            Route::Claude { .. } => Service::Claude,
        }
    }
}

/// Validates a call and picks the upstream. Privacy is enforced here too,
/// not only in the router: personal tasks never reach an engine that trains
/// on inputs.
pub fn route(call: &EngineCall, config: &Config) -> Result<Route, EngineError> {
    let key = &call.cache_key;
    if call.is_personal() && key.engine.trains_on_inputs() {
        return Err(EngineError::PrivacyBlocked);
    }
    if call.user.trim().is_empty()
        || call.system.chars().count() + call.user.chars().count() > MAX_PROMPT_CHARS
    {
        return Err(other(format!("prompt must be 1..={MAX_PROMPT_CHARS} characters")));
    }
    if call.max_tokens.is_some_and(|m| m == 0 || m > MAX_MAX_TOKENS) {
        return Err(other(format!("max_tokens must be 1..={MAX_MAX_TOKENS}")));
    }
    // structure_screenshot also works from the on-device OCR text alone, so
    // it passes as long as no image is attached.
    let text_only = key.task == Task::StructureScreenshot && call.image_sha256.is_none();
    if (key.task.needs_vision() && !text_only) || call.image_sha256.is_some() {
        return Err(unavailable("vision tasks need the blob store, which Kabar Cloud doesn't have yet"));
    }
    match key.engine {
        EngineId::WorkersAi if config.workers_ai_bound => {
            Ok(Route::WorkersAi { model: config.workers_ai_model.clone() })
        }
        EngineId::WorkersAi => Err(unavailable("the AI binding is missing")),
        EngineId::GeminiFree => match &config.gemini_key {
            Some(k) => Ok(Route::Gemini { model: config.gemini_model.clone(), key: k.clone() }),
            None => Err(unavailable("GEMINI_API_KEY is not in the vault")),
        },
        EngineId::Claude if !config.claude_enabled => {
            Err(unavailable("claude is switched off (CLAUDE_ENABLED)"))
        }
        EngineId::Claude => match &config.claude_key {
            Some(k) => Ok(Route::Claude { model: config.claude_model.clone(), key: k.clone() }),
            None => Err(unavailable("ANTHROPIC_API_KEY is not in the vault")),
        },
        EngineId::Deterministic | EngineId::Local => {
            Err(unavailable("this engine runs on the device or Kabar Home, not in Kabar Cloud"))
        }
    }
}

/// D1 key: the router's key plus the model actually used, so changing a
/// model never serves the old model's answers.
pub fn cache_id(call: &EngineCall, model: &str) -> String {
    format!("{}:{model}", call.cache_key.id())
}

/// Personal answers stay out of the shared cache; the app keeps them locally.
pub fn may_cache(call: &EngineCall) -> bool {
    !call.is_personal()
}

/// A repair retry exists because the last answer was bad: don't serve it again.
pub fn may_read_cache(call: &EngineCall) -> bool {
    may_cache(call) && !call.is_repair
}

fn max_tokens(call: &EngineCall) -> u32 {
    call.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS)
}

/// Rejects empty answers; everything else goes back raw for the router to validate.
pub fn finish(text: String) -> Result<String, EngineError> {
    if text.trim().is_empty() { Err(other("empty answer")) } else { Ok(text) }
}

// --- Workers AI ---------------------------------------------------------------

pub fn workers_ai_input(call: &EngineCall) -> Value {
    let mut messages = Vec::new();
    if !call.system.trim().is_empty() {
        messages.push(json!({ "role": "system", "content": call.system }));
    }
    messages.push(json!({ "role": "user", "content": call.user }));
    // No `response_format` here: JSON mode isn't listed for SEA-LION on
    // Workers AI, and the router asks for JSON in the prompt and validates.
    json!({ "messages": messages, "max_tokens": max_tokens(call) })
}

/// Text models answer `{ "response": "..." }`; some put an object there when
/// they produce JSON.
pub fn parse_workers_ai(out: &Value) -> Result<String, EngineError> {
    match out.get("response") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(v @ (Value::Object(_) | Value::Array(_))) => Ok(v.to_string()),
        _ => Err(other("workers ai: no response field")),
    }
}

/// Workers AI errors are thrown as text; the daily allocation one mentions neurons.
pub fn workers_ai_error(message: &str, retry_after_s: u64) -> EngineError {
    let m = message.to_lowercase();
    if m.contains("neuron") || m.contains("allocation") || m.contains("quota") {
        EngineError::QuotaExhausted { retry_after_s: Some(retry_after_s) }
    } else if m.contains("rate limit") || m.contains("too many") {
        EngineError::RateLimited { retry_after_s: None }
    } else {
        other(format!("workers ai: {message}"))
    }
}

// --- Gemini -------------------------------------------------------------------

pub fn gemini_url(model: &str) -> String {
    format!("{GEMINI_BASE}/{model}:generateContent")
}

pub fn gemini_body(call: &EngineCall) -> Value {
    let mut body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": call.user }] }],
        "generationConfig": { "maxOutputTokens": max_tokens(call) },
    });
    if call.json {
        body["generationConfig"]["responseMimeType"] = json!("application/json");
    }
    if !call.system.trim().is_empty() {
        body["systemInstruction"] = json!({ "parts": [{ "text": call.system }] });
    }
    body
}

pub fn parse_gemini(out: &Value) -> Result<String, EngineError> {
    let parts = out.pointer("/candidates/0/content/parts").and_then(Value::as_array).ok_or_else(|| {
        let reason =
            out.pointer("/promptFeedback/blockReason").and_then(Value::as_str).unwrap_or("no candidates");
        other(format!("gemini: {reason}"))
    })?;
    Ok(parts.iter().filter_map(|p| p.get("text").and_then(Value::as_str)).collect())
}

// --- Claude -------------------------------------------------------------------

pub fn claude_body(call: &EngineCall, model: &str) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens(call),
        "messages": [{ "role": "user", "content": call.user }],
    });
    if !call.system.trim().is_empty() {
        body["system"] = json!(call.system);
    }
    body
}

pub fn parse_claude(out: &Value) -> Result<String, EngineError> {
    if out.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        return Err(other("claude declined this request"));
    }
    let content = out.get("content").and_then(Value::as_array).ok_or_else(|| other("claude: no content"))?;
    Ok(content
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect())
}

/// Maps an upstream HTTP status, keeping rate limits distinct.
pub fn upstream_status_error(service: &str, status: u16, retry_after_s: Option<u64>) -> EngineError {
    match status {
        429 => EngineError::RateLimited { retry_after_s },
        401 | 403 => unavailable(format!("{service}: key rejected (HTTP {status})")),
        _ => other(format!("{service}: HTTP {status}")),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use kabar_protocol::ai::{CacheKey, Task};

    use super::*;

    fn config(pairs: &[(&str, &str)]) -> Config {
        let map: HashMap<String, String> =
            pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Config::from_lookup(|k| map.get(k).cloned(), true).unwrap()
    }

    fn call(task: Task, engine: EngineId) -> EngineCall {
        EngineCall {
            cache_key: CacheKey { task, input_hash: "ab12".into(), engine, prompt_version: 1 },
            system: "Return JSON.".into(),
            // docs/SPEC.md §24
            user: "Sek tak lapor pusat".into(),
            image_sha256: None,
            is_repair: false,
            json: true,
            max_tokens: None,
            personal: false,
        }
    }

    fn fixture(name: &str) -> Value {
        let path = format!("{}/../../fixtures/server/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn personal_tasks_never_reach_gemini_free() {
        let c = config(&[("GEMINI_API_KEY", "k")]);
        assert!(route(&call(Task::AnalyzeText, EngineId::GeminiFree), &c).is_ok());
        let screenshot = EngineCall { personal: true, ..call(Task::LearnEntry, EngineId::GeminiFree) };
        assert_eq!(route(&screenshot, &c), Err(EngineError::PrivacyBlocked));
        assert!(!may_cache(&screenshot));
        for task in Task::ALL.into_iter().filter(|t| t.is_personal()) {
            assert_eq!(route(&call(task, EngineId::GeminiFree), &c), Err(EngineError::PrivacyBlocked));
            assert!(route(&call(task, EngineId::WorkersAi), &c).is_ok());
            assert!(!may_cache(&call(task, EngineId::WorkersAi)));
        }
    }

    #[test]
    fn engines_need_keys_and_claude_needs_switching_on() {
        let none = config(&[]);
        let unavailable = |r: Result<Route, EngineError>| matches!(r, Err(EngineError::Unavailable { .. }));
        assert!(unavailable(route(&call(Task::AnalyzeText, EngineId::GeminiFree), &none)));
        assert!(unavailable(route(&call(Task::AnalyzeText, EngineId::Claude), &none)));
        let on = config(&[("CLAUDE_ENABLED", "true")]);
        assert!(unavailable(route(&call(Task::AnalyzeText, EngineId::Claude), &on)));
        let ready = config(&[("CLAUDE_ENABLED", "true"), ("ANTHROPIC_API_KEY", "k")]);
        let r = route(&call(Task::AnalyzeText, EngineId::Claude), &ready).unwrap();
        assert_eq!((r.engine(), r.model()), (EngineId::Claude, "claude-opus-5-5"));
        assert!(unavailable(route(&call(Task::AnalyzeText, EngineId::Local), &ready)));
        assert!(unavailable(route(&call(Task::ReadHandwriting, EngineId::WorkersAi), &ready)));
        // A screenshot's OCR text is fine; its image waits for the blob store.
        let ocr_text = call(Task::StructureScreenshot, EngineId::WorkersAi);
        assert!(route(&ocr_text, &none).is_ok());
        let with_image = EngineCall { image_sha256: Some("ab".repeat(32)), ..ocr_text };
        assert!(unavailable(route(&with_image, &none)));
        assert_eq!(
            route(&call(Task::AnalyzeText, EngineId::WorkersAi), &none).unwrap().service(),
            Service::WorkersAi
        );
    }

    #[test]
    fn prompts_are_bounded() {
        let mut c = call(Task::AnalyzeText, EngineId::WorkersAi);
        c.user = "a".repeat(MAX_PROMPT_CHARS);
        assert!(matches!(route(&c, &config(&[])), Err(EngineError::Other { .. })));
        c.user = "ok".into();
        c.max_tokens = Some(MAX_MAX_TOKENS + 1);
        assert!(matches!(route(&c, &config(&[])), Err(EngineError::Other { .. })));
        c.max_tokens = None;
        c.user = "  ".into();
        assert!(matches!(route(&c, &config(&[])), Err(EngineError::Other { .. })));
    }

    #[test]
    fn cache_rules() {
        let c = call(Task::AnalyzeText, EngineId::WorkersAi);
        assert_eq!(cache_id(&c, "m1"), format!("{}:m1", c.cache_key.id()));
        assert_ne!(cache_id(&c, "m1"), cache_id(&c, "m2"));
        assert!(may_read_cache(&c));
        let repair = EngineCall { is_repair: true, ..c };
        assert!(may_cache(&repair) && !may_read_cache(&repair));
    }

    #[test]
    fn workers_ai_shapes() {
        let input = workers_ai_input(&call(Task::AnalyzeText, EngineId::WorkersAi));
        assert_eq!(input["messages"][0]["role"], "system");
        assert_eq!(input["messages"][1]["content"], "Sek tak lapor pusat");
        assert_eq!(input["max_tokens"], DEFAULT_MAX_TOKENS);
        let text = parse_workers_ai(&fixture("workers_ai_analyze.json")).unwrap();
        assert!(finish(text).unwrap().contains("lapor"));
        assert!(finish(" ".into()).is_err());
        assert!(matches!(
            workers_ai_error("4006: you have used up your daily free allocation of 10,000 neurons", 60),
            EngineError::QuotaExhausted { retry_after_s: Some(60) }
        ));
    }

    #[test]
    fn gemini_shapes() {
        let body = gemini_body(&call(Task::AnalyzeText, EngineId::GeminiFree));
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "Return JSON.");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "Sek tak lapor pusat");
        assert_eq!(body["generationConfig"]["responseMimeType"], "application/json");
        let plain = EngineCall {
            json: false,
            max_tokens: Some(4096),
            ..call(Task::RoleplayTurn, EngineId::GeminiFree)
        };
        let body = gemini_body(&plain);
        assert!(body["generationConfig"].get("responseMimeType").is_none());
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 4096);
        assert_eq!(gemini_url("gemini-2.5-flash"), format!("{GEMINI_BASE}/gemini-2.5-flash:generateContent"));
        assert!(parse_gemini(&fixture("gemini_analyze.json")).unwrap().contains("pusat"));
        let blocked = json!({ "promptFeedback": { "blockReason": "SAFETY" } });
        assert_eq!(parse_gemini(&blocked), Err(EngineError::Other { message: "gemini: SAFETY".into() }));
    }

    #[test]
    fn claude_shapes() {
        let body = claude_body(&call(Task::AnalyzeText, EngineId::Claude), "claude-opus-5-5");
        assert_eq!(body["system"], "Return JSON.");
        assert!(body.get("temperature").is_none());
        assert!(parse_claude(&fixture("claude_analyze.json")).unwrap().contains("lapor"));
        let refusal = json!({ "stop_reason": "refusal", "content": [] });
        assert!(parse_claude(&refusal).is_err());
    }

    #[test]
    fn upstream_errors() {
        assert_eq!(
            upstream_status_error("gemini", 429, Some(30)),
            EngineError::RateLimited { retry_after_s: Some(30) }
        );
        assert_eq!(status_for(&upstream_status_error("gemini", 500, None)), 502);
        assert_eq!(status_for(&upstream_status_error("gemini", 403, None)), 503);
        assert_eq!(status_for(&EngineError::PrivacyBlocked), 403);
    }
}
