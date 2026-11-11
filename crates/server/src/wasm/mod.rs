//! Binds Kabar Cloud to the Cloudflare runtime. Kept thin: parse, check the
//! token, call the plain-Rust modules, talk to bindings and upstreams.

mod account;
mod do_sql;
mod login;

use std::cell::{Cell, RefCell};

use kabar_protocol::ai::{AiReject, EngineCall, EngineError, EngineReply};
use kabar_protocol::cloud::{
    ApiError, ErrorCode, GoogleSignIn, PasswordLogin, PasswordSignIn, PushTokenRequest, RefreshRequest,
    Session, SetPassword, TtsRequest, normalize_username, valid_username,
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
use crate::blobs::{self, Image};
use crate::config::Config;
use crate::fcm;
use crate::google::{self, Jwks};
use crate::internal::{
    AccountLoginState, BlobData, BlobGet, BlobPut, DEVICE_HEADER, Expiry, LoginCheck, LoginClear, LoginName,
    LoginSet, Refresh, Register, Revoke, SetPush,
};
use crate::login::{self as pw, Checked};
use crate::quota::{self, Service};
use crate::tokens::{self, AccessClaims, TokenKey};
use crate::tts;

pub(crate) fn now_ms() -> u64 {
    Date::now().as_millis()
}

/// `n` (≤ 32) random bytes as hex. Never falls back to a fixed value.
pub(crate) fn random_hex(n: usize) -> std::result::Result<String, getrandom::Error> {
    let bytes = tokens::random_bytes()?;
    Ok(bytes[..n.min(32)].iter().map(|b| format!("{b:02x}")).collect())
}

pub(crate) fn api_error(code: ErrorCode, message: impl Into<String>) -> Result<Response> {
    error_response(ApiError { code, message: message.into(), retry_after_ms: None })
}

fn error_response(e: ApiError) -> Result<Response> {
    let status = status_for(e.code);
    Ok(Response::from_json(&e)?.with_status(status))
}

fn status_for(code: ErrorCode) -> u16 {
    match code {
        ErrorCode::InvalidRequest => 400,
        ErrorCode::Unauthorized | ErrorCode::DeviceRevoked => 401,
        ErrorCode::NotAllowed => 403,
        ErrorCode::NotFound | ErrorCode::BlobMissing => 404,
        ErrorCode::NotConfigured => 503,
        ErrorCode::QuotaExhausted => 429,
        ErrorCode::Upstream => 502,
        ErrorCode::Internal => 500,
        ErrorCode::WrongPassword => 401,
        ErrorCode::UsernameTaken => 409,
        ErrorCode::TooManyTries => 429,
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
    if let Some(h) = req.headers().get("authorization").ok().flatten() {
        return h.strip_prefix("Bearer ").map(|t| t.trim().to_owned());
    }
    // A browser's WebSocket can't carry Authorization (crate::web).
    if req.path() == "/v1/sync" {
        let protocols = req.headers().get("sec-websocket-protocol").ok().flatten()?;
        return crate::web::ws_bearer(&protocols);
    }
    None
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
    /// Last forced refetch after encountering an unknown signing key.
    static JWKS_FORCED: Cell<Option<u64>> = const { Cell::new(None) };
}

async fn google_jwks(url: &str, force: bool) -> Result<Jwks> {
    let now = now_ms();
    // Never let attacker-controlled `kid` values bypass the cached keys
    // more than once per five minutes in a Worker isolate.
    let force = force && google::may_refetch(JWKS_FORCED.get(), now);
    if force {
        JWKS_FORCED.set(Some(now));
    }
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

/// A username's own Durable Object (`crate::login`).
async fn call_login(env: &Env, username: &str, path: &str, payload: &impl Serialize) -> Result<Response> {
    let headers = Headers::new();
    headers.set("content-type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&serde_json::to_string(payload)?)));
    let req = Request::new_with_init(&format!("https://login{path}"), &init)?;
    env.durable_object("LOGIN")?
        .id_from_name(&format!("u:{username}"))?
        .get_stub()?
        .fetch_with_request(req)
        .await
}

/// `POST /v1/auth/password`: the same account as Google, on a device without it.
async fn sign_in_password(mut req: Request, env: &Env, cfg: &Config) -> Result<Response> {
    let key = match cfg.auth_ready() {
        Ok(k) => k.clone(),
        Err(e) => return api_error(ErrorCode::Internal, format!("server not set up: {e}")),
    };
    let sign_in: PasswordSignIn = match body(&mut req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    if !tokens::valid_device_id(&sign_in.device.device_id.0) {
        return api_error(ErrorCode::InvalidRequest, "device_id must be [A-Za-z0-9_-]{1,64}");
    }
    let username = normalize_username(&sign_in.username);
    if !valid_username(&username) || pw::parse_key(&sign_in.password_key).is_none() {
        return api_error(ErrorCode::WrongPassword, "username or password is wrong");
    }
    let check = LoginCheck { key: sign_in.password_key };
    let mut resp = call_login(env, &username, "/check", &check).await?;
    if resp.status_code() != 200 {
        return Ok(resp);
    }
    let (account_id, email) = match resp.json::<Checked>().await? {
        Checked::Ok { account_id, email } => (account_id, email),
        Checked::Wrong => return api_error(ErrorCode::WrongPassword, "username or password is wrong"),
        Checked::Locked { retry_after_ms } => {
            return error_response(ApiError {
                code: ErrorCode::TooManyTries,
                message: "too many wrong passwords for this username".into(),
                retry_after_ms: Some(retry_after_ms),
            });
        }
    };
    if !cfg.email_allowed(&email) {
        return api_error(ErrorCode::NotAllowed, "this account is no longer on Kabar's allowlist");
    }
    let device = sign_in.device.device_id.clone();
    let refresh = tokens::new_refresh(&account_id, &device, tokens::random_bytes().map_err(js_err)?);
    let register = Register { info: sign_in.device, email, refresh_hash: refresh.hash.clone() };
    let mut resp = call_do(env, &account_id, Method::Post, "/register", None, Some(&register)).await?;
    if resp.status_code() != 200 {
        return Ok(resp);
    }
    let expiry: Expiry = resp.json().await?;
    session_response(&key, &account_id, device, refresh, expiry).await
}

/// The account's username now, and the caller's email; or the error answer.
async fn login_state(
    env: &Env,
    claims: &AccessClaims,
) -> Result<std::result::Result<AccountLoginState, Response>> {
    let mut resp = call_do(env, &claims.sub, Method::Get, "/login", Some(&claims.dev), None::<&()>).await?;
    if resp.status_code() != 200 {
        return Ok(Err(resp));
    }
    Ok(Ok(resp.json().await?))
}

/// `GET`, `POST` and `DELETE /v1/login`: the username and password of the
/// signed-in account.
async fn manage_login(
    mut req: Request,
    env: &Env,
    claims: &AccessClaims,
    method: Method,
) -> Result<Response> {
    let state = match login_state(env, claims).await? {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };
    let username = match method {
        Method::Get => state.username,
        Method::Post => {
            let set: SetPassword = match body(&mut req).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            let username = normalize_username(&set.username);
            if !valid_username(&username) {
                return api_error(ErrorCode::InvalidRequest, "username must be 3-32 of a-z 0-9 . _ -");
            }
            let Some(key) = pw::parse_key(&set.password_key) else {
                return api_error(ErrorCode::InvalidRequest, "password_key must be 32 bytes, base64url");
            };
            let salt = random_hex(16).map_err(js_err)?;
            let put = LoginSet {
                account_id: claims.sub.clone(),
                email: state.email,
                verifier: pw::verifier(&salt, &key),
                salt,
            };
            let resp = call_login(env, &username, "/set", &put).await?;
            if resp.status_code() != 200 {
                return Ok(resp);
            }
            if let Some(old) = state.username.filter(|old| *old != username) {
                call_login(env, &old, "/clear", &LoginClear { account_id: claims.sub.clone() }).await?;
            }
            Some(username)
        }
        _ => {
            if let Some(old) = &state.username {
                call_login(env, old, "/clear", &LoginClear { account_id: claims.sub.clone() }).await?;
            }
            None
        }
    };
    if method != Method::Get {
        let name = LoginName { username: username.clone() };
        let resp = call_do(env, &claims.sub, Method::Post, "/login", Some(&claims.dev), Some(&name)).await?;
        if resp.status_code() != 200 {
            return Ok(resp);
        }
    }
    Response::from_json(&PasswordLogin { username })
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

/// Reserves `cost` units of `service` for this period in one D1 statement
/// (`quota::RESERVE`); `Some(period)` when it fits the hard stop, to pass
/// to [`quota_refund`] if the call then never happens.
async fn quota_reserve(
    env: &Env,
    cfg: &Config,
    service: Service,
    now: u64,
    cost: u64,
) -> Result<Option<String>> {
    let period = service.period(now);
    let limit = service.limit(&cfg.limits);
    if !quota::allows(0, cost, limit) {
        return Ok(None);
    }
    let db = env.d1("DB")?;
    let stmt = db.prepare(quota::RESERVE).bind(&[
        JsValue::from_str(service.name()),
        JsValue::from_str(&period),
        JsValue::from_f64(cost as f64),
        JsValue::from_f64(limit as f64),
    ])?;
    let used: Option<f64> = stmt.first(Some("used")).await?;
    Ok(used.map(|_| period))
}

/// Gives a reservation back (`quota::REFUND`).
async fn quota_refund(env: &Env, service: Service, period: &str, cost: u64) -> Result<()> {
    env.d1("DB")?
        .prepare(quota::REFUND)
        .bind(&[
            JsValue::from_str(service.name()),
            JsValue::from_str(period),
            JsValue::from_f64(cost as f64),
        ])?
        .run()
        .await?;
    Ok(())
}

/// `GET /v1/usage`: reads each counter, then `usage::report` builds the answer.
async fn usage_report(env: &Env, cfg: &Config) -> Result<Response> {
    let now = now_ms();
    let mut used = Vec::with_capacity(crate::usage::SERVICES.len());
    for service in crate::usage::SERVICES {
        used.push((service, quota_used(env, service, now).await?));
    }
    let read = |s: Service| used.iter().find(|(x, _)| *x == s).map_or(0, |(_, n)| *n);
    Response::from_json(&crate::usage::report(cfg, now, read))
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
    image: Option<&Image>,
    now: u64,
) -> Result<std::result::Result<String, EngineError>> {
    Ok(match route {
        Route::WorkersAi { model } => {
            match env.ai("AI")?.run::<Value, Value>(model, ai::workers_ai_input(call, image)).await {
                Ok(out) => ai::parse_workers_ai(&out),
                Err(e) => Err(ai::workers_ai_error(&e.to_string(), Service::WorkersAi.retry_after_s(now))),
            }
        }
        Route::Gemini { model, key } => {
            let headers = Headers::new();
            headers.set("x-goog-api-key", key.expose())?;
            match post_json(&ai::gemini_url(model), headers, &ai::gemini_body(call, image)).await? {
                (200, _, out) => ai::parse_gemini(&out),
                (status, retry, _) => Err(ai::upstream_status_error("gemini", status, retry)),
            }
        }
        Route::Claude { model, key } => {
            let headers = Headers::new();
            headers.set("x-api-key", key.expose())?;
            headers.set("anthropic-version", ai::ANTHROPIC_VERSION)?;
            match post_json(ai::ANTHROPIC_URL, headers, &ai::claude_body(call, model, image)).await? {
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
    // A missing image is the device's to fix (upload it again), so it is
    // reported before anything about the engines.
    let image = match &call.image_sha256 {
        None => None,
        Some(sha) => {
            let get = BlobGet { sha256: sha.clone() };
            let mut resp = call_do(env, &claims.sub, Method::Post, "/blob/get", None, Some(&get)).await?;
            if resp.status_code() != 200 {
                return Ok(resp);
            }
            let data: BlobData = resp.json().await?;
            Some(Image { mime: data.mime, base64: data.data_b64 })
        }
    };
    let route = match ai::route(&call, cfg) {
        Ok(r) => r,
        Err(e) => return engine_error(&e),
    };
    // A revoked device reads nothing, cached answers included.
    if let Some(revoked) = ensure_active(env, claims).await? {
        return Ok(revoked);
    }
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
    let now = now_ms();
    let service = route.service();
    let Some(period) = quota_reserve(env, cfg, service, now, 1).await? else {
        return engine_error(&EngineError::QuotaExhausted {
            retry_after_s: Some(service.retry_after_s(now)),
        });
    };
    // Reserved before the call; given back only if the call never ran.
    let result = match run_engine(env, &route, &call, image.as_ref(), now).await {
        Ok(r) => r,
        Err(e) => {
            quota_refund(env, service, &period, 1).await?;
            return Err(e);
        }
    };
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

/// `POST /v1/ai/reject`: the learner gave 👎, so every model's copy of that
/// answer leaves the shared cache and the regenerate runs fresh.
async fn ai_reject(mut req: Request, env: &Env, claims: &AccessClaims) -> Result<Response> {
    let reject: AiReject = match body(&mut req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    if let Some(revoked) = ensure_active(env, claims).await? {
        return Ok(revoked);
    }
    let prefix = ai::cache_prefix(&reject.cache_key);
    // `substr` rather than LIKE: task names hold `_`, a LIKE wildcard.
    env.d1("DB")?
        .prepare("DELETE FROM ai_cache WHERE substr(key, 1, length(?1)) = ?1")
        .bind(&[JsValue::from_str(&prefix)])?
        .run()
        .await?;
    Ok(Response::empty()?.with_status(204))
}

/// `PUT /v1/blobs/{sha256}`: an image for the next vision calls.
async fn put_blob(mut req: Request, env: &Env, claims: &AccessClaims, sha: &str) -> Result<Response> {
    if let Some(revoked) = ensure_active(env, claims).await? {
        return Ok(revoked);
    }
    let bytes = req.bytes().await?;
    let mime = match blobs::validate(sha, &bytes) {
        Ok(m) => m,
        Err(e) => return api_error(ErrorCode::InvalidRequest, e.to_string()),
    };
    let put = BlobPut { sha256: sha.to_owned(), mime: mime.to_owned(), data_b64: blobs::encode(&bytes) };
    let resp = call_do(env, &claims.sub, Method::Post, "/blob/put", None, Some(&put)).await?;
    if resp.status_code() != 200 {
        return Ok(resp);
    }
    Ok(Response::empty()?.with_status(204))
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
    let Some(period) = quota_reserve(env, cfg, Service::Tts, now, chars).await? else {
        let msg = format!("Azure TTS free characters are used up for {}", Service::Tts.period(now));
        let headers = Headers::new();
        headers.set("retry-after", &Service::Tts.retry_after_s(now).to_string())?;
        return Ok(Response::from_json(&ApiError {
            code: ErrorCode::QuotaExhausted,
            message: msg,
            retry_after_ms: None,
        })?
        .with_status(429)
        .with_headers(headers));
    };
    // Reserved before the call; Azure bills only what it speaks, so a call
    // that fails gives its characters back.
    let bytes = match speak_azure(azure_key.expose(), region, &ssml).await {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(status)) => {
            quota_refund(env, Service::Tts, &period, chars).await?;
            return api_error(ErrorCode::Upstream, format!("azure tts: HTTP {status}"));
        }
        Err(e) => {
            quota_refund(env, Service::Tts, &period, chars).await?;
            return Err(e);
        }
    };
    cache.put(cache_url.as_str(), audio_response(bytes.clone(), true)?).await?;
    audio_response(bytes, false)
}

/// One Azure TTS call: the audio, or the HTTP status it failed with.
async fn speak_azure(key: &str, region: &str, ssml: &str) -> Result<std::result::Result<Vec<u8>, u16>> {
    let headers = Headers::new();
    headers.set("ocp-apim-subscription-key", key)?;
    headers.set("content-type", "application/ssml+xml")?;
    headers.set("x-microsoft-outputformat", tts::OUTPUT_FORMAT)?;
    headers.set("user-agent", "kabar-cloud")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(headers).with_body(Some(JsValue::from_str(ssml)));
    let mut resp = Fetch::Request(Request::new_with_init(&tts::azure_url(region), &init)?).send().await?;
    if resp.status_code() != 200 {
        return Ok(Err(resp.status_code()));
    }
    Ok(Ok(resp.bytes().await?))
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let cfg = config(&env);
    let path = req.path();
    let method = req.method();
    // The web app's origin (docs/WEB.md) gets CORS; others get none.
    let origin = req.headers().get("origin")?.filter(|o| crate::web::allowed_origin(o, &cfg.web_origins));
    if method == Method::Options && origin.is_some() {
        return with_cors(Response::empty()?.with_status(204), origin.as_deref());
    }
    let result = route(req, &env, &cfg, method, &path).await;
    let response = match result {
        Ok(r) => Ok(r),
        Err(e) => {
            console_error!("{path}: {e}");
            api_error(ErrorCode::Internal, "something went wrong on Kabar Cloud")
        }
    }?;
    with_cors(response, origin.as_deref())
}

/// Adds the CORS headers for an allowed web app origin. A WebSocket upgrade
/// (status 101) passes through untouched: browsers don't apply CORS to it.
fn with_cors(mut response: Response, origin: Option<&str>) -> Result<Response> {
    let Some(origin) = origin else { return Ok(response) };
    if response.status_code() == 101 {
        return Ok(response);
    }
    let headers = Headers::new();
    for (k, v) in response.headers().entries() {
        headers.set(&k, &v)?;
    }
    headers.set("access-control-allow-origin", origin)?;
    headers.set("vary", "Origin")?;
    headers.set("access-control-allow-headers", crate::web::ALLOW_HEADERS)?;
    headers.set("access-control-allow-methods", crate::web::ALLOW_METHODS)?;
    headers.set("access-control-max-age", crate::web::MAX_AGE)?;
    response = response.with_headers(headers);
    Ok(response)
}

async fn route(mut req: Request, env: &Env, cfg: &Config, method: Method, path: &str) -> Result<Response> {
    match (method.clone(), path) {
        (Method::Get, "/" | "/v1/health") => {
            Response::from_json(&json!({ "name": "kabar-cloud", "protocol": PROTOCOL_VERSION }))
        }
        (Method::Post, "/v1/auth/google") => sign_in_google(req, env, cfg).await,
        (Method::Post, "/v1/auth/refresh") => refresh_session(req, env, cfg).await,
        (Method::Post, "/v1/auth/password") => sign_in_password(req, env, cfg).await,
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
                    // The browser offered its token as a subprotocol: the
                    // socket must answer with the one it picks.
                    let offered = req.headers().get("sec-websocket-protocol")?;
                    if offered.as_deref().and_then(crate::web::ws_bearer).is_some() {
                        headers.set(crate::web::PROTOCOL_HEADER, crate::web::WS_PROTOCOL)?;
                    }
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
                (m @ (Method::Get | Method::Post | Method::Delete), "/v1/login") => {
                    manage_login(req, env, &claims, m).await
                }
                (Method::Post, "/v1/auth/signout") => {
                    let revoke = Revoke { device: claims.dev.clone() };
                    call_do(env, &claims.sub, Method::Post, "/revoke", None, Some(&revoke)).await
                }
                (Method::Get, "/v1/vault") => Response::from_json(&cfg.vault_status()),
                (Method::Get, "/v1/usage") => usage_report(env, cfg).await,
                (Method::Post, "/v1/ai") => ai_proxy(req, env, cfg, &claims).await,
                (Method::Post, "/v1/ai/reject") => ai_reject(req, env, &claims).await,
                (Method::Post, "/v1/tts") => tts_proxy(req, env, cfg, &claims).await,
                (Method::Put, p) if p.starts_with("/v1/blobs/") => {
                    let sha = p["/v1/blobs/".len()..].to_owned();
                    put_blob(req, env, &claims, &sha).await
                }
                _ => api_error(ErrorCode::NotFound, "not found"),
            }
        }
    }
}
