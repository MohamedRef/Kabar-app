//! Binds Kabar Cloud to the Cloudflare runtime. Kept thin: parse, check the
//! token, call the plain-Rust modules, talk to bindings and upstreams.

mod account;
mod do_sql;

use std::cell::RefCell;

use kabar_protocol::ai::{EngineCall, EngineError, EngineReply};
use kabar_protocol::cloud::{
    ApiError, ErrorCode, GoogleSignIn, PushTokenRequest, RefreshRequest, Session, TtsRequest,
};
use kabar_protocol::{DeviceId, PROTOCOL_VERSION};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use wasm_bindgen::JsValue;
use worker::{
    Cache, Context, Date, Env, Fetch, Headers, Method, Request, RequestInit, Response, Result, Stub,
    console_error, event,
};

use crate::ai::{self, Route};
use crate::config::Config;
use crate::fcm;
use crate::google::{self, Jwks};
use crate::internal::{DEVICE_HEADER, Expiry, Refresh, Register, Revoke, SetPush};
use crate::quota::{self, Service};
use crate::tokens::{self, AccessClaims, TokenKey};
use crate::tts;

pub(crate) fn now_ms() -> u64 {
    Date::now().as_millis()
}

pub(crate) fn random_hex(n: usize) -> String {
    let bytes = tokens::random_bytes().unwrap_or([0; 32]);
    bytes[..n.min(32)].iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn api_error(code: ErrorCode, message: impl Into<String>) -> Result<Response> {
    Ok(Response::from_json(&ApiError { code, message: message.into() })?.with_status(status_for(code)))
}

fn status_for(code: ErrorCode) -> u16 {
    match code {
        ErrorCode::InvalidRequest => 400,
        ErrorCode::Unauthorized | ErrorCode::DeviceRevoked => 401,
        ErrorCode::NotAllowed => 403,
        ErrorCode::NotFound => 404,
        ErrorCode::NotConfigured => 503,
        ErrorCode::QuotaExhausted => 429,
        ErrorCode::Upstream => 502,
        ErrorCode::Internal => 500,
    }
}

fn config(env: &Env) -> Config {
    let lookup = |name: &str| {
        env.secret(name).ok().map(|s| s.to_string()).or_else(|| env.var(name).ok().map(|v| v.to_string()))
    };
    match Config::from_lookup(lookup, env.ai("AI").is_ok()) {
        Ok(c) => c,
        Err(e) => {
            console_error!("config: {e}");
            // Fall back to defaults: auth reports what's missing per request.
            Config::from_lookup(|_| None, false).unwrap_or_else(|_| unreachable!("defaults parse"))
        }
    }
}

async fn body<T: DeserializeOwned>(req: &mut Request) -> std::result::Result<T, Result<Response>> {
    req.json().await.map_err(|e| api_error(ErrorCode::InvalidRequest, format!("bad JSON body: {e}")))
}

fn bearer(req: &Request) -> Option<String> {
    let h = req.headers().get("authorization").ok().flatten()?;
    h.strip_prefix("Bearer ").map(|t| t.trim().to_owned())
}

fn authenticate(req: &Request, key: &TokenKey) -> std::result::Result<AccessClaims, Result<Response>> {
    let token = bearer(req).ok_or_else(|| api_error(ErrorCode::Unauthorized, "missing bearer token"))?;
    tokens::verify_access(&key.public(), &token, now_ms())
        .map_err(|e| api_error(ErrorCode::Unauthorized, format!("access token: {e}")))
}

fn account_stub(env: &Env, account_id: &str) -> Result<Stub> {
    env.durable_object("ACCOUNT")?.id_from_name(account_id)?.get_stub()
}

/// Calls the account's Durable Object.
async fn call_do(
    env: &Env,
    account_id: &str,
    method: Method,
    path: &str,
    device: Option<&DeviceId>,
    payload: Option<&impl Serialize>,
) -> Result<Response> {
    let headers = Headers::new();
    headers.set("content-type", "application/json")?;
    if let Some(d) = device {
        headers.set(DEVICE_HEADER, &d.0)?;
    }
    let mut init = RequestInit::new();
    init.with_method(method).with_headers(headers);
    if let Some(p) = payload {
        init.with_body(Some(JsValue::from_str(&serde_json::to_string(p)?)));
    }
    let req = Request::new_with_init(&format!("https://account{path}"), &init)?;
    account_stub(env, account_id)?.fetch_with_request(req).await
}

thread_local! {
    /// Google's signing keys, cached for the isolate's life (or max-age).
    static JWKS: RefCell<Option<(Jwks, u64)>> = const { RefCell::new(None) };
}

async fn google_jwks(url: &str, force: bool) -> Result<Jwks> {
    let now = now_ms();
    if !force
        && let Some(jwks) =
            JWKS.with_borrow(|c| c.as_ref().filter(|(_, exp)| *exp > now).map(|(j, _)| j.clone()))
    {
        return Ok(jwks);
    }
    let mut resp = Fetch::Url(url.parse()?).send().await?;
    let max_age = google::jwks_max_age_ms(resp.headers().get("cache-control")?.as_deref());
    let jwks: Jwks = resp.json().await?;
    JWKS.with_borrow_mut(|c| *c = Some((jwks.clone(), now + max_age)));
    Ok(jwks)
}

/// Where Google's keys come from. `INSECURE_DEV_JWKS_URL` lets the local
/// end-to-end test sign its own ID tokens; it is ignored unless the request
/// itself is to localhost, so it can't weaken a deployed Worker.
fn jwks_url(env: &Env, req: &Request) -> String {
    let local =
        req.url().ok().and_then(|u| u.host_str().map(|h| h == "localhost" || h == "127.0.0.1")) == Some(true);
    match env.var("INSECURE_DEV_JWKS_URL").map(|v| v.to_string()) {
        Ok(url) if local && !url.is_empty() => url,
        _ => google::JWKS_URL.to_owned(),
    }
}

async fn session_response(
    key: &TokenKey,
    account_id: &str,
    device: DeviceId,
    refresh: tokens::NewRefresh,
    expiry: Expiry,
) -> Result<Response> {
    let (access_token, access_expires_at_ms) = tokens::issue_access(key, account_id, &device, now_ms());
    Response::from_json(&Session {
        account_id: account_id.to_owned(),
        device_id: device,
        email: Some(expiry.email),
        access_token,
        access_expires_at_ms,
        refresh_token: refresh.token,
        refresh_expires_at_ms: expiry.refresh_expires_ms,
    })
}

async fn sign_in_google(mut req: Request, env: &Env, cfg: &Config) -> Result<Response> {
    let key = match cfg.auth_ready() {
        Ok(k) => k.clone(),
        Err(e) => return api_error(ErrorCode::Internal, format!("server not set up: {e}")),
    };
    let sign_in: GoogleSignIn = match body(&mut req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    if !tokens::valid_device_id(&sign_in.device.device_id.0) {
        return api_error(ErrorCode::InvalidRequest, "device_id must be [A-Za-z0-9_-]{1,64}");
    }
    let now = now_ms();
    let url = jwks_url(env, &req);
    let mut jwks = google_jwks(&url, false).await?;
    let kid = google::token_kid(&sign_in.id_token).unwrap_or_default();
    if !jwks.keys.iter().any(|k| k.kid == kid) {
        jwks = google_jwks(&url, true).await?;
    }
    let identity = match google::verify_id_token(&sign_in.id_token, &jwks, &cfg.google_client_ids, now) {
        Ok(id) => id,
        Err(e) => return api_error(ErrorCode::Unauthorized, format!("Google ID token: {e}")),
    };
    if !cfg.email_allowed(&identity.email) {
        return api_error(ErrorCode::NotAllowed, "this Google account is not on Kabar's allowlist");
    }
    let account_id = tokens::account_id_for_google_sub(&identity.sub);
    let device = sign_in.device.device_id.clone();
    let refresh = tokens::new_refresh(&account_id, &device, tokens::random_bytes().map_err(js_err)?);
    let register =
        Register { info: sign_in.device, email: identity.email, refresh_hash: refresh.hash.clone() };
    let mut resp = call_do(env, &account_id, Method::Post, "/register", None, Some(&register)).await?;
    if resp.status_code() != 200 {
        return Ok(resp);
    }
    let expiry: Expiry = resp.json().await?;
    session_response(&key, &account_id, device, refresh, expiry).await
}

async fn refresh_session(mut req: Request, env: &Env, cfg: &Config) -> Result<Response> {
    let key = match cfg.auth_ready() {
        Ok(k) => k.clone(),
        Err(e) => return api_error(ErrorCode::Internal, format!("server not set up: {e}")),
    };
    let r: RefreshRequest = match body(&mut req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let Some(parsed) = tokens::parse_refresh(&r.refresh_token) else {
        return api_error(ErrorCode::Unauthorized, "malformed refresh token");
    };
    let fresh =
        tokens::new_refresh(&parsed.account_id, &parsed.device, tokens::random_bytes().map_err(js_err)?);
    let call =
        Refresh { device: parsed.device.clone(), presented_hash: parsed.hash, new_hash: fresh.hash.clone() };
    let mut resp = call_do(env, &parsed.account_id, Method::Post, "/refresh", None, Some(&call)).await?;
    if resp.status_code() != 200 {
        return Ok(resp);
    }
    let expiry: Expiry = resp.json().await?;
    if !cfg.email_allowed(&expiry.email) {
        let revoke = Revoke { device: parsed.device };
        call_do(env, &parsed.account_id, Method::Post, "/revoke", None, Some(&revoke)).await?;
        return api_error(ErrorCode::NotAllowed, "this Google account is no longer on the allowlist");
    }
    session_response(&key, &parsed.account_id, parsed.device, fresh, expiry).await
}

fn js_err(e: impl std::fmt::Display) -> worker::Error {
    worker::Error::RustError(e.to_string())
}

/// Refuses tokens of signed-out devices before spending a free-tier quota.
async fn ensure_active(env: &Env, claims: &AccessClaims) -> Result<Option<Response>> {
    let resp = call_do(env, &claims.sub, Method::Get, "/active", Some(&claims.dev), None::<&()>).await?;
    Ok((resp.status_code() != 200).then_some(resp))
}

async fn quota_used(env: &Env, service: Service, now: u64) -> Result<u64> {
    let db = env.d1("DB")?;
    let stmt = db
        .prepare(quota::SELECT_USED)
        .bind(&[JsValue::from_str(service.name()), JsValue::from_str(&service.period(now))])?;
    let used: Option<f64> = stmt.first(Some("used")).await?;
    Ok(used.unwrap_or(0.0) as u64)
}

async fn quota_add(env: &Env, service: Service, now: u64, cost: u64) -> Result<()> {
    let db = env.d1("DB")?;
    db.prepare(quota::ADD_USED)
        .bind(&[
            JsValue::from_str(service.name()),
            JsValue::from_str(&service.period(now)),
            JsValue::from_f64(cost as f64),
        ])?
        .run()
        .await?;
    Ok(())
}

/// `true` when spending `cost` stays inside the hard stop.
async fn quota_allows(env: &Env, cfg: &Config, service: Service, now: u64, cost: u64) -> Result<bool> {
    Ok(quota::allows(quota_used(env, service, now).await?, cost, service.limit(&cfg.limits)))
}

fn engine_error(e: &EngineError) -> Result<Response> {
    Ok(Response::from_json(e)?.with_status(ai::status_for(e)))
}

pub(crate) async fn post_json(
    url: &str,
    headers: Headers,
    payload: &Value,
) -> Result<(u16, Option<u64>, Value)> {
    headers.set("content-type", "application/json")?;
    post(url, headers, &payload.to_string()).await
}

/// POSTs `body` (content type already in `headers`); returns the status,
/// `retry-after` and the JSON answer (`null` if it isn't JSON).
pub(crate) async fn post(url: &str, headers: Headers, body: &str) -> Result<(u16, Option<u64>, Value)> {
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(headers).with_body(Some(JsValue::from_str(body)));
    let mut resp = Fetch::Request(Request::new_with_init(url, &init)?).send().await?;
    let status = resp.status_code();
    let retry_after = resp.headers().get("retry-after")?.and_then(|v| v.trim().parse().ok());
    let value = resp.json::<Value>().await.unwrap_or(Value::Null);
    Ok((status, retry_after, value))
}

async fn run_engine(
    env: &Env,
    route: &Route,
    call: &EngineCall,
    now: u64,
) -> Result<std::result::Result<String, EngineError>> {
    Ok(match route {
        Route::WorkersAi { model } => {
            match env.ai("AI")?.run::<Value, Value>(model, ai::workers_ai_input(call)).await {
                Ok(out) => ai::parse_workers_ai(&out),
                Err(e) => Err(ai::workers_ai_error(&e.to_string(), Service::WorkersAi.retry_after_s(now))),
            }
        }
        Route::Gemini { model, key } => {
            let headers = Headers::new();
            headers.set("x-goog-api-key", key.expose())?;
            match post_json(&ai::gemini_url(model), headers, &ai::gemini_body(call)).await? {
                (200, _, out) => ai::parse_gemini(&out),
                (status, retry, _) => Err(ai::upstream_status_error("gemini", status, retry)),
            }
        }
        Route::Claude { model, key } => {
            let headers = Headers::new();
            headers.set("x-api-key", key.expose())?;
            headers.set("anthropic-version", ai::ANTHROPIC_VERSION)?;
            match post_json(ai::ANTHROPIC_URL, headers, &ai::claude_body(call, model)).await? {
                (200, _, out) => ai::parse_claude(&out),
                (status, retry, _) => Err(ai::upstream_status_error("claude", status, retry)),
            }
        }
    })
}

async fn ai_proxy(mut req: Request, env: &Env, cfg: &Config, claims: &AccessClaims) -> Result<Response> {
    let call: EngineCall = match body(&mut req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let route = match ai::route(&call, cfg) {
        Ok(r) => r,
        Err(e) => return engine_error(&e),
    };
    let reply = |text: String| EngineReply { engine: route.engine(), text, model: route.model().to_owned() };
    let id = ai::cache_id(&call, route.model());
    let db = env.d1("DB")?;
    if ai::may_read_cache(&call) {
        let hit = db.prepare("SELECT output FROM ai_cache WHERE key = ?1").bind(&[JsValue::from_str(&id)])?;
        if let Some(text) = hit.first::<String>(Some("output")).await? {
            let headers = Headers::new();
            headers.set("x-kabar-cache", "hit")?;
            return Ok(Response::from_json(&reply(text))?.with_headers(headers));
        }
    }
    if let Some(revoked) = ensure_active(env, claims).await? {
        return Ok(revoked);
    }
    let now = now_ms();
    let service = route.service();
    if !quota_allows(env, cfg, service, now, 1).await? {
        return engine_error(&EngineError::QuotaExhausted {
            retry_after_s: Some(service.retry_after_s(now)),
        });
    }
    let result = run_engine(env, &route, &call, now).await?;
    quota_add(env, service, now, 1).await?;
    let text = match result.and_then(ai::finish) {
        Ok(t) => t,
        Err(e) => return engine_error(&e),
    };
    if ai::may_cache(&call) {
        db.prepare(
            "INSERT OR REPLACE INTO ai_cache (key, task, prompt_version, engine, model, output, created_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(&[
            JsValue::from_str(&id),
            JsValue::from_str(call.cache_key.task.as_str()),
            JsValue::from_f64(f64::from(call.cache_key.prompt_version)),
            JsValue::from_str(route.engine().as_str()),
            JsValue::from_str(route.model()),
            JsValue::from_str(&text),
            JsValue::from_f64(now as f64),
        ])?
        .run()
        .await?;
    }
    Response::from_json(&reply(text))
}

fn audio_response(bytes: Vec<u8>, cached: bool) -> Result<Response> {
    let headers = Headers::new();
    headers.set("content-type", "audio/mpeg")?;
    headers.set("cache-control", "public, max-age=31536000, immutable")?;
    headers.set("x-kabar-cache", if cached { "hit" } else { "miss" })?;
    Ok(Response::from_bytes(bytes)?.with_headers(headers))
}

async fn tts_proxy(mut req: Request, env: &Env, cfg: &Config, claims: &AccessClaims) -> Result<Response> {
    let tts_req: TtsRequest = match body(&mut req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    if let Err(e) = tts::validate(&tts_req) {
        return api_error(e.code(), e.0);
    }
    let (Some(azure_key), Some(region)) = (&cfg.azure_key, &cfg.azure_region) else {
        return api_error(ErrorCode::NotConfigured, "AZURE_SPEECH_KEY / AZURE_SPEECH_REGION are not set");
    };
    let ssml = tts::ssml(&tts_req);
    let cache_url = format!("https://kabar-cache.invalid/tts/{}.mp3", tts::cache_key(&ssml));
    let cache = Cache::default();
    if let Some(mut hit) = cache.get(cache_url.as_str(), false).await? {
        return audio_response(hit.bytes().await?, true);
    }
    if let Some(revoked) = ensure_active(env, claims).await? {
        return Ok(revoked);
    }
    let now = now_ms();
    let chars = tts::billable_chars(&tts_req);
    if !quota_allows(env, cfg, Service::Tts, now, chars).await? {
        let msg = format!("Azure TTS free characters are used up for {}", Service::Tts.period(now));
        let headers = Headers::new();
        headers.set("retry-after", &Service::Tts.retry_after_s(now).to_string())?;
        return Ok(Response::from_json(&ApiError { code: ErrorCode::QuotaExhausted, message: msg })?
            .with_status(429)
            .with_headers(headers));
    }
    let headers = Headers::new();
    headers.set("ocp-apim-subscription-key", azure_key.expose())?;
    headers.set("content-type", "application/ssml+xml")?;
    headers.set("x-microsoft-outputformat", tts::OUTPUT_FORMAT)?;
    headers.set("user-agent", "kabar-cloud")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(headers).with_body(Some(JsValue::from_str(&ssml)));
    let mut resp = Fetch::Request(Request::new_with_init(&tts::azure_url(region), &init)?).send().await?;
    if resp.status_code() != 200 {
        return api_error(ErrorCode::Upstream, format!("azure tts: HTTP {}", resp.status_code()));
    }
    let bytes = resp.bytes().await?;
    quota_add(env, Service::Tts, now, chars).await?;
    cache.put(cache_url.as_str(), audio_response(bytes.clone(), true)?).await?;
    audio_response(bytes, false)
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let cfg = config(&env);
    let path = req.path();
    let method = req.method();
    let result = route(req, &env, &cfg, method, &path).await;
    match result {
        Ok(r) => Ok(r),
        Err(e) => {
            console_error!("{path}: {e}");
            api_error(ErrorCode::Internal, "something went wrong on Kabar Cloud")
        }
    }
}

async fn route(mut req: Request, env: &Env, cfg: &Config, method: Method, path: &str) -> Result<Response> {
    match (method.clone(), path) {
        (Method::Get, "/" | "/v1/health") => {
            Response::from_json(&json!({ "name": "kabar-cloud", "protocol": PROTOCOL_VERSION }))
        }
        (Method::Post, "/v1/auth/google") => sign_in_google(req, env, cfg).await,
        (Method::Post, "/v1/auth/refresh") => refresh_session(req, env, cfg).await,
        _ => {
            let Some(key) = cfg.token_key.clone() else {
                return api_error(ErrorCode::Internal, "server not set up: TOKEN_SIGNING_KEY is not set");
            };
            let claims = match authenticate(&req, &key) {
                Ok(c) => c,
                Err(r) => return r,
            };
            match (method, path) {
                (Method::Get, "/v1/sync") => {
                    if req.headers().get("upgrade")?.as_deref() != Some("websocket") {
                        return api_error(ErrorCode::InvalidRequest, "expected a WebSocket upgrade");
                    }
                    let headers = Headers::new();
                    headers.set("upgrade", "websocket")?;
                    headers.set(DEVICE_HEADER, &claims.dev.0)?;
                    let mut init = RequestInit::new();
                    init.with_method(Method::Get).with_headers(headers);
                    let forwarded = Request::new_with_init("https://account/ws", &init)?;
                    account_stub(env, &claims.sub)?.fetch_with_request(forwarded).await
                }
                (Method::Put, "/v1/devices/me/push") => {
                    let push: PushTokenRequest = match body(&mut req).await {
                        Ok(b) => b,
                        Err(r) => return r,
                    };
                    let token = push.token.trim();
                    if !token.is_empty() && !fcm::valid_push_token(token) {
                        return api_error(ErrorCode::InvalidRequest, "bad push token");
                    }
                    let set = SetPush { token: (!token.is_empty()).then(|| token.to_owned()) };
                    call_do(env, &claims.sub, Method::Post, "/push", Some(&claims.dev), Some(&set)).await
                }
                (Method::Get, "/v1/devices") => {
                    call_do(env, &claims.sub, Method::Get, "/devices", Some(&claims.dev), None::<&()>).await
                }
                (Method::Delete, p) if p.starts_with("/v1/devices/") => {
                    let id = &p["/v1/devices/".len()..];
                    if !tokens::valid_device_id(id) {
                        return api_error(ErrorCode::InvalidRequest, "bad device id");
                    }
                    let revoke = Revoke { device: DeviceId(id.to_owned()) };
                    call_do(env, &claims.sub, Method::Post, "/revoke", None, Some(&revoke)).await
                }
                (Method::Post, "/v1/auth/signout") => {
                    let revoke = Revoke { device: claims.dev.clone() };
                    call_do(env, &claims.sub, Method::Post, "/revoke", None, Some(&revoke)).await
                }
                (Method::Get, "/v1/vault") => Response::from_json(&cfg.vault_status()),
                (Method::Post, "/v1/ai") => ai_proxy(req, env, cfg, &claims).await,
                (Method::Post, "/v1/tts") => tts_proxy(req, env, cfg, &claims).await,
                _ => api_error(ErrorCode::NotFound, "not found"),
            }
        }
    }
}
