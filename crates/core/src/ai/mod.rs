//! The AI router (spec §19, PLAN §9).
//!
//! Deterministic code handles what it can before this is reached. For one
//! task the router: picks the engines from the route table that are switched
//! on, free (the $0 hard stop) and allowed to see the data (personal tasks
//! never go to an engine that trains on inputs); returns a cached answer if
//! any of them has one; otherwise calls them best first, validating each
//! reply with serde plus the task's own checks and retrying once with the
//! error. Nothing degrades silently: the answer says which engine produced
//! it and whether its route is still provisional, and failures list every
//! attempt.

pub mod json;
pub mod prompt;
pub mod routes;
pub mod tasks;

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::pin::Pin;

use kabar_protocol::ai::{CacheKey, EngineCall, EngineError, EngineId, EngineReply, Task};
use serde_json::Value;

pub use prompt::{LearnerProfile, PromptSet, PromptTemplate};
pub use routes::{Route, RouteStatus, RouteTable};
pub use tasks::AiOutput;

pub type EngineFuture<'a> = Pin<Box<dyn Future<Output = Result<EngineReply, EngineError>> + 'a>>;

/// One AI backend. Implemented in the app (HTTP to Kabar Cloud / Kabar Home)
/// and in Kabar Cloud (Workers AI, Gemini, Anthropic). Not `Send`: Workers
/// and the WebView run single-threaded.
pub trait Engine {
    fn id(&self) -> EngineId;
    /// Calling it costs nothing (free tier, or Claude via plan credits).
    fn is_free(&self) -> bool;
    fn call<'a>(&'a self, call: &'a EngineCall) -> EngineFuture<'a>;
}

/// Validated answers keyed by (task, input hash, engine, prompt version).
/// The app backs it with SQLite, Kabar Cloud with D1.
pub trait Cache {
    fn get(&self, key: &CacheKey) -> Option<String>;
    fn put(&self, key: &CacheKey, json: &str);
    fn remove(&self, key: &CacheKey);
}

#[derive(Default)]
pub struct MemoryCache(RefCell<HashMap<CacheKey, String>>);

impl Cache for MemoryCache {
    fn get(&self, key: &CacheKey) -> Option<String> {
        self.0.borrow().get(key).cloned()
    }
    fn put(&self, key: &CacheKey, json: &str) {
        self.0.borrow_mut().insert(key.clone(), json.to_string());
    }
    fn remove(&self, key: &CacheKey) {
        self.0.borrow_mut().remove(key);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouterSettings {
    /// Engines the learner switched off.
    pub disabled: BTreeSet<EngineId>,
    /// Lift the $0 hard stop. Off by default; the UI asks first.
    pub allow_paid: bool,
}

impl Default for RouterSettings {
    fn default() -> Self {
        Self { disabled: BTreeSet::from([EngineId::Claude]), allow_paid: false }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AiRequest {
    pub input: Value,
    /// The input itself is personal (journal text, chat history), whatever the task.
    pub personal: bool,
    pub image_sha256: Option<String>,
}

impl AiRequest {
    pub fn new(input: Value) -> Self {
        Self { input, ..Self::default() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Answer<T> {
    pub value: T,
    pub engine: EngineId,
    pub prompt_version: u32,
    pub from_cache: bool,
    /// The route has not passed the bake-off yet; show a badge.
    pub provisional: bool,
    /// Show as unverified until the learner approves it.
    pub needs_review: bool,
    pub cache_key: CacheKey,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttemptError {
    Engine(EngineError),
    /// The reply failed parsing or validation twice; the last error.
    Invalid(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attempt {
    pub engine: EngineId,
    pub error: AttemptError,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AiError {
    /// No bundled prompt for the task yet: a gap to report, not hide.
    NoPrompt(Task),
    /// Every routed engine is off, paid, or not allowed to see this data.
    NoEngine { task: Task, skipped: Vec<(EngineId, Skip)> },
    /// Every allowed engine was tried and failed.
    AllFailed { task: Task, attempts: Vec<Attempt> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    NotConnected,
    SwitchedOff,
    Paid,
    Privacy,
}

/// What the UI says, warmly, when there is no answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendlyReason {
    /// Free quotas are used up for today.
    ResetsLater,
    /// Offline, or Kabar Home is asleep.
    Unreachable,
    /// Nothing is set up for this yet.
    NotAvailable,
    TryAgain,
}

impl AiError {
    pub fn friendly(&self) -> FriendlyReason {
        match self {
            Self::NoPrompt(_) | Self::NoEngine { .. } => FriendlyReason::NotAvailable,
            Self::AllFailed { attempts, .. } => {
                let all = |f: fn(&AttemptError) -> bool| attempts.iter().all(|a| f(&a.error));
                if all(|e| {
                    matches!(
                        e,
                        AttemptError::Engine(
                            EngineError::QuotaExhausted { .. } | EngineError::RateLimited { .. }
                        )
                    )
                }) {
                    FriendlyReason::ResetsLater
                } else if all(|e| matches!(e, AttemptError::Engine(EngineError::Unavailable { .. }))) {
                    FriendlyReason::Unreachable
                } else {
                    FriendlyReason::TryAgain
                }
            }
        }
    }
}

pub struct Router {
    routes: RouteTable,
    prompts: PromptSet,
    engines: Vec<Box<dyn Engine>>,
    cache: Box<dyn Cache>,
    pub settings: RouterSettings,
    pub profile: LearnerProfile,
}

impl Router {
    pub fn new(routes: RouteTable, prompts: PromptSet, cache: Box<dyn Cache>) -> Self {
        Self {
            routes,
            prompts,
            engines: Vec::new(),
            cache,
            settings: RouterSettings::default(),
            profile: LearnerProfile::default(),
        }
    }

    pub fn with_engine(mut self, engine: Box<dyn Engine>) -> Self {
        self.engines.retain(|e| e.id() != engine.id());
        self.engines.push(engine);
        self
    }

    fn engine(&self, id: EngineId) -> Option<&dyn Engine> {
        self.engines.iter().find(|e| e.id() == id).map(|e| e.as_ref())
    }

    /// The engines this request may use, best first.
    pub fn plan(&self, task: Task, personal: bool) -> Result<Vec<EngineId>, AiError> {
        let personal = personal || task.is_personal();
        let mut plan = Vec::new();
        let mut skipped = Vec::new();
        for &id in &self.routes.get(task).engines {
            let skip = match self.engine(id) {
                None => Some(Skip::NotConnected),
                Some(_) if self.settings.disabled.contains(&id) => Some(Skip::SwitchedOff),
                Some(_) if personal && id.trains_on_inputs() => Some(Skip::Privacy),
                Some(e) if !e.is_free() && !self.settings.allow_paid => Some(Skip::Paid),
                Some(_) => None,
            };
            match skip {
                Some(s) => skipped.push((id, s)),
                None => plan.push(id),
            }
        }
        if plan.is_empty() { Err(AiError::NoEngine { task, skipped }) } else { Ok(plan) }
    }

    pub async fn run<T: AiOutput>(&self, req: &AiRequest) -> Result<Answer<T>, AiError> {
        let plan = self.plan(T::TASK, req.personal)?;
        self.run_plan(req, &plan).await
    }

    /// 👎 on an answer: drop it from the cache and ask the next engine up.
    pub async fn regenerate<T: AiOutput>(
        &self,
        req: &AiRequest,
        rejected: &Answer<T>,
    ) -> Result<Answer<T>, AiError> {
        self.cache.remove(&rejected.cache_key);
        let plan = self.plan(T::TASK, req.personal)?;
        let after: Vec<EngineId> =
            plan.iter().copied().skip_while(|&e| e != rejected.engine).skip(1).collect();
        if after.is_empty() {
            return Err(AiError::AllFailed { task: T::TASK, attempts: Vec::new() });
        }
        self.run_plan(req, &after).await
    }

    async fn run_plan<T: AiOutput>(&self, req: &AiRequest, plan: &[EngineId]) -> Result<Answer<T>, AiError> {
        let task = T::TASK;
        let route = self.routes.get(task);
        let template = self.prompts.get(task, route.prompt_version).ok_or(AiError::NoPrompt(task))?;
        let input_hash = json::input_hash(&req.input);
        let key_for = |engine| CacheKey {
            task,
            input_hash: input_hash.clone(),
            engine,
            prompt_version: template.version,
        };
        let answer = |value: T, key: CacheKey, from_cache| Answer {
            needs_review: value.needs_review() || T::input_needs_review(&req.input),
            value,
            engine: key.engine,
            prompt_version: key.prompt_version,
            from_cache,
            provisional: route.status == RouteStatus::Provisional,
            cache_key: key,
        };

        for &engine in plan {
            let key = key_for(engine);
            if let Some(hit) = self.cache.get(&key) {
                match decode::<T>(&hit, &req.input) {
                    Ok(v) => return Ok(answer(v, key, true)),
                    // Stale shape from an older build: forget it.
                    Err(_) => self.cache.remove(&key),
                }
            }
        }

        let mut attempts = Vec::new();
        for &engine in plan {
            let Some(e) = self.engine(engine) else { continue };
            let key = key_for(engine);
            let (system, user) = template.render(&self.profile, &req.input, engine);
            let mut call = EngineCall {
                cache_key: key.clone(),
                system,
                user,
                image_sha256: req.image_sha256.clone(),
                is_repair: false,
                json: true,
                max_tokens: route.max_tokens,
                personal: req.personal,
            };
            match self.call_validated::<T>(e, &mut call, &req.input).await {
                Ok((value, json)) => {
                    self.cache.put(&key, &json);
                    return Ok(answer(value, key, false));
                }
                Err(error) => attempts.push(Attempt { engine, error }),
            }
        }
        Err(AiError::AllFailed { task, attempts })
    }

    /// One call, plus one repair call if the reply doesn't validate.
    async fn call_validated<T: AiOutput>(
        &self,
        engine: &dyn Engine,
        call: &mut EngineCall,
        input: &Value,
    ) -> Result<(T, String), AttemptError> {
        let reply = engine.call(call).await.map_err(AttemptError::Engine)?;
        let error = match decode::<T>(&reply.text, input) {
            Ok(v) => return Ok((v, json::extract_json(&reply.text).to_string())),
            Err(e) => e,
        };
        call.is_repair = true;
        call.user = format!(
            "{}\n\nYour previous reply could not be used: {error}\nPrevious reply:\n{}\n\nReply again with only the corrected JSON object.",
            call.user,
            truncate(&reply.text, 4000),
        );
        let reply = engine.call(call).await.map_err(AttemptError::Engine)?;
        decode::<T>(&reply.text, input)
            .map(|v| (v, json::extract_json(&reply.text).to_string()))
            .map_err(AttemptError::Invalid)
    }
}

fn decode<T: AiOutput>(text: &str, input: &Value) -> Result<T, String> {
    let v: T = json::parse(text)?;
    v.validate()?;
    v.check(input)?;
    Ok(v)
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[cfg(test)]
mod tests;
