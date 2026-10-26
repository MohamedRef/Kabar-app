//! Firebase Cloud Messaging (HTTP v1) for Kak Nur's reminders.
//!
//! Kabar Cloud signs in to Google as the Firebase service account (a JWT
//! signed with RS256, exchanged for a one-hour access token at Google's
//! OAuth endpoint), then sends one data-only message per device. Plain Rust:
//! the Worker glue only does the two HTTP calls.
//!
//! - <https://firebase.google.com/docs/cloud-messaging/send/v1-api>
//! - <https://developers.google.com/identity/protocols/oauth2/service-account#httprest>

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kabar_protocol::cloud::{MAX_PUSH_TOKEN_LEN, PushNotice};
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::{SignatureEncoding, Signer};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::Sha256;

pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
/// Access tokens live an hour; use them for about 55 minutes.
pub const TOKEN_MARGIN_MS: u64 = 5 * 60 * 1000;
/// A reminder that can't be delivered within an hour is stale: drop it.
pub const MESSAGE_TTL: &str = "3600s";

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FcmError {
    #[error("FCM_SERVICE_ACCOUNT is not a Firebase service account JSON: {0}")]
    BadServiceAccount(String),
    #[error("Google token endpoint: {0}")]
    Token(String),
}

/// The fields Kabar needs from the service account JSON that Firebase
/// downloads (Project settings → Service accounts → Generate new private key).
#[derive(Clone, Deserialize)]
pub struct ServiceAccount {
    pub project_id: String,
    pub client_email: String,
    private_key: String,
}

impl fmt::Debug for ServiceAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServiceAccount({}, {}, key=***)", self.project_id, self.client_email)
    }
}

impl ServiceAccount {
    pub fn from_json(json: &str) -> Result<Self, FcmError> {
        let sa: Self = serde_json::from_str(json).map_err(|e| FcmError::BadServiceAccount(e.to_string()))?;
        if sa.project_id.is_empty() || sa.client_email.is_empty() {
            return Err(FcmError::BadServiceAccount("project_id or client_email is empty".into()));
        }
        // Fail at setup time, not at the first reminder.
        sa.key()?;
        Ok(sa)
    }

    fn key(&self) -> Result<RsaPrivateKey, FcmError> {
        RsaPrivateKey::from_pkcs8_pem(&self.private_key)
            .map_err(|e| FcmError::BadServiceAccount(format!("private_key: {e}")))
    }

    /// The signed JWT to trade for an access token (valid one hour).
    pub fn assertion(&self, now_ms: u64) -> Result<String, FcmError> {
        let iat = now_ms / 1000;
        let enc = |v: &Value| URL_SAFE_NO_PAD.encode(v.to_string());
        let input = format!(
            "{}.{}",
            enc(&json!({ "alg": "RS256", "typ": "JWT" })),
            enc(&json!({
                "iss": self.client_email,
                "scope": SCOPE,
                "aud": TOKEN_URL,
                "iat": iat,
                "exp": iat + 3600,
            }))
        );
        // PKCS#1 v1.5 signing is deterministic: no randomness needed.
        let sig = SigningKey::<Sha256>::new(self.key()?).sign(input.as_bytes());
        Ok(format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes())))
    }

    pub fn send_url(&self) -> String {
        format!("https://fcm.googleapis.com/v1/projects/{}/messages:send", self.project_id)
    }
}

/// `application/x-www-form-urlencoded` body for [`TOKEN_URL`]. The JWT is
/// base64url and dots, so it needs no escaping.
pub fn token_request_body(assertion: &str) -> String {
    format!("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={assertion}")
}

#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken {
    pub token: String,
    /// When to stop using it (a few minutes before Google's expiry).
    pub use_until_ms: u64,
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccessToken(***, until {})", self.use_until_ms)
    }
}

pub fn parse_token_response(status: u16, body: &Value, now_ms: u64) -> Result<AccessToken, FcmError> {
    if status != 200 {
        let why = body["error_description"].as_str().or(body["error"].as_str()).unwrap_or("no details");
        return Err(FcmError::Token(format!("HTTP {status}: {why}")));
    }
    let token = body["access_token"].as_str().filter(|t| !t.is_empty());
    let token = token.ok_or_else(|| FcmError::Token("no access_token".into()))?;
    let expires_in_ms = body["expires_in"].as_u64().unwrap_or(3600) * 1000;
    Ok(AccessToken {
        token: token.to_owned(),
        use_until_ms: now_ms + expires_in_ms.saturating_sub(TOKEN_MARGIN_MS),
    })
}

/// The `messages:send` body: data only, so the app renders Kak Nur's line in
/// the learner's UI language (and nothing personal crosses Google).
pub fn message(device_token: &str, notice: &PushNotice) -> Value {
    let data: Map<String, Value> =
        notice.to_data().into_iter().map(|(k, v)| (k.to_owned(), Value::String(v))).collect();
    json!({
        "message": {
            "token": device_token,
            "data": data,
            "android": { "priority": "high", "ttl": MESSAGE_TTL },
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SendOutcome {
    Sent,
    /// The token is dead (app uninstalled, data cleared): forget it.
    Unregistered,
    Failed(String),
}

pub fn classify_send(status: u16, body: &Value) -> SendOutcome {
    if status == 200 {
        return SendOutcome::Sent;
    }
    let unregistered = body["error"]["details"]
        .as_array()
        .is_some_and(|d| d.iter().any(|x| x["errorCode"] == "UNREGISTERED"));
    if status == 404 || unregistered {
        return SendOutcome::Unregistered;
    }
    let status_text = body["error"]["status"].as_str().unwrap_or("no details");
    SendOutcome::Failed(format!("HTTP {status}: {status_text}"))
}

/// FCM tokens are long, printable and contain no spaces.
pub fn valid_push_token(token: &str) -> bool {
    (1..=MAX_PUSH_TOKEN_LEN).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_graphic())
}

#[cfg(test)]
mod tests {
    use kabar_protocol::cloud::PushKind;
    use rsa::pkcs1v15::{Signature, VerifyingKey};
    use rsa::pkcs8::{EncodePrivateKey, LineEnding};
    use rsa::signature::Verifier;

    use super::*;
    use crate::google::testing::key;

    const NOW: u64 = 1_793_011_856_000;

    fn fixture(name: &str) -> Value {
        let path = format!("{}/../../fixtures/server/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn service_account_json() -> String {
        let pem = key().to_pkcs8_pem(LineEnding::LF).unwrap();
        json!({
            "type": "service_account",
            "project_id": "kabar-test",
            "private_key_id": "abc",
            "private_key": pem.as_str(),
            "client_email": "firebase-adminsdk-x@kabar-test.iam.gserviceaccount.com",
            "token_uri": "https://oauth2.googleapis.com/token",
        })
        .to_string()
    }

    #[test]
    fn signs_a_verifiable_assertion() {
        let sa = ServiceAccount::from_json(&service_account_json()).unwrap();
        assert!(!format!("{sa:?}").contains("PRIVATE"));
        let jwt = sa.assertion(NOW).unwrap();
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["iss"], "firebase-adminsdk-x@kabar-test.iam.gserviceaccount.com");
        assert_eq!(claims["aud"], TOKEN_URL);
        assert_eq!(claims["scope"], SCOPE);
        assert_eq!(claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap(), 3600);
        let sig = Signature::try_from(URL_SAFE_NO_PAD.decode(parts[2]).unwrap().as_slice()).unwrap();
        let input = &jwt[..parts[0].len() + 1 + parts[1].len()];
        VerifyingKey::<Sha256>::new(key().to_public_key()).verify(input.as_bytes(), &sig).unwrap();
        assert_eq!(sa.send_url(), "https://fcm.googleapis.com/v1/projects/kabar-test/messages:send");
        assert!(token_request_body(&jwt).ends_with(&format!("&assertion={jwt}")));
    }

    #[test]
    fn bad_service_accounts_are_reported() {
        assert!(matches!(ServiceAccount::from_json("{}"), Err(FcmError::BadServiceAccount(_))));
        let bad_key = json!({ "project_id": "p", "client_email": "e", "private_key": "nope" }).to_string();
        assert!(matches!(ServiceAccount::from_json(&bad_key), Err(FcmError::BadServiceAccount(_))));
    }

    #[test]
    fn token_response_is_cached_for_55_minutes() {
        let t = parse_token_response(200, &fixture("fcm_token.json"), NOW).unwrap();
        assert!(t.token.starts_with("ya29."));
        assert_eq!(t.use_until_ms, NOW + 3599 * 1000 - TOKEN_MARGIN_MS);
        assert!(!format!("{t:?}").contains("ya29"));
        let denied = json!({ "error": "invalid_grant", "error_description": "Invalid JWT Signature." });
        assert_eq!(
            parse_token_response(400, &denied, NOW),
            Err(FcmError::Token("HTTP 400: Invalid JWT Signature.".into()))
        );
    }

    #[test]
    fn message_is_data_only() {
        let notice = PushNotice { kind: PushKind::Study, line: "remind_study".into(), seed: 20_752 };
        let m = message("device-token", &notice);
        assert_eq!(m["message"]["token"], "device-token");
        assert_eq!(m["message"]["data"], json!({ "kind": "study", "line": "remind_study", "seed": "20752" }));
        assert!(m["message"].get("notification").is_none());
        assert_eq!(m["message"]["android"]["ttl"], MESSAGE_TTL);
    }

    #[test]
    fn send_results() {
        assert_eq!(classify_send(200, &fixture("fcm_send_ok.json")), SendOutcome::Sent);
        assert_eq!(classify_send(404, &fixture("fcm_unregistered.json")), SendOutcome::Unregistered);
        // Same error code under another HTTP status still means a dead token.
        assert_eq!(classify_send(400, &fixture("fcm_unregistered.json")), SendOutcome::Unregistered);
        let quota = json!({ "error": { "code": 429, "status": "RESOURCE_EXHAUSTED" } });
        assert_eq!(classify_send(429, &quota), SendOutcome::Failed("HTTP 429: RESOURCE_EXHAUSTED".into()));
    }

    #[test]
    fn push_tokens_are_checked() {
        assert!(valid_push_token("dQw4w9WgXcQ:APA91bH-x_y"));
        assert!(!valid_push_token(""));
        assert!(!valid_push_token("has space"));
        assert!(!valid_push_token(&"a".repeat(MAX_PUSH_TOKEN_LEN + 1)));
    }
}
