//! Kabar Cloud HTTP API types (everything under `/v1`, except the WebSocket
//! messages, which live in [`crate::sync`]).
//!
//! Authenticated routes take `Authorization: Bearer <access_token>`.

use serde::{Deserialize, Serialize};

use crate::DeviceId;
use crate::ai::EngineId;

// --- errors ----------------------------------------------------------------

/// Machine-readable error. The app turns `code` into a friendly, localized
/// sentence; `message` is for logs only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    /// Missing, expired or bad token. Refresh, or sign in again.
    Unauthorized,
    /// Signed in with a Google account that is not on the allowlist.
    NotAllowed,
    /// This device was signed out remotely. Sign in again.
    DeviceRevoked,
    NotFound,
    /// The TTS key is not in the vault.
    NotConfigured,
    /// A free-tier limit would be exceeded. Wait for the reset.
    QuotaExhausted,
    /// The upstream TTS service failed.
    Upstream,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

// --- auth ------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Android,
    Macos,
    Web,
    #[serde(other)]
    Other,
}

/// What a device says about itself when it signs in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Stable per installation: `[A-Za-z0-9_-]{1,64}`.
    pub device_id: DeviceId,
    /// Shown on the Devices screen, e.g. "Galaxy S24 Ultra".
    pub name: String,
    #[serde(default)]
    pub model: Option<String>,
    pub platform: Platform,
}

/// `POST /v1/auth/google`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoogleSignIn {
    /// Google ID token from Credential Manager (Android) or OAuth + PKCE (Mac).
    pub id_token: String,
    pub device: DeviceInfo,
}

/// `POST /v1/auth/refresh`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Returned by sign-in and refresh. Store the refresh token in the Android
/// Keystore / macOS Keychain; it rotates on every refresh.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub account_id: String,
    pub device_id: DeviceId,
    pub email: Option<String>,
    pub access_token: String,
    pub access_expires_at_ms: u64,
    pub refresh_token: String,
    pub refresh_expires_at_ms: u64,
}

// --- devices ---------------------------------------------------------------

/// One row on the Devices screen. `GET /v1/devices`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceSummary {
    pub device_id: DeviceId,
    pub name: String,
    pub model: Option<String>,
    pub platform: Platform,
    pub signed_in_at_ms: u64,
    pub last_seen_ms: u64,
    /// Last time the device pushed or pulled ops; `None` before its first sync.
    pub last_sync_ms: Option<u64>,
    /// The device making the request.
    pub current: bool,
    pub revoked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceList {
    pub devices: Vec<DeviceSummary>,
}

// --- AI proxy --------------------------------------------------------------

// `POST /v1/ai` takes a [`crate::ai::EngineCall`] (the engine is
// `call.cache_key.engine`) and answers 200 with [`crate::ai::EngineReply`], or
// 403/429/502/503 with [`crate::ai::EngineError`]. Auth failures use [`ApiError`].

// --- TTS proxy -------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Voice {
    /// id-ID-GadisNeural
    IdGadis,
    /// id-ID-ArdiNeural
    IdArdi,
    /// jv-ID-SitiNeural
    JvSiti,
    /// jv-ID-DimasNeural
    JvDimas,
}

impl Voice {
    pub fn azure_name(self) -> &'static str {
        match self {
            Voice::IdGadis => "id-ID-GadisNeural",
            Voice::IdArdi => "id-ID-ArdiNeural",
            Voice::JvSiti => "jv-ID-SitiNeural",
            Voice::JvDimas => "jv-ID-DimasNeural",
        }
    }

    pub fn locale(self) -> &'static str {
        match self {
            Voice::IdGadis | Voice::IdArdi => "id-ID",
            Voice::JvSiti | Voice::JvDimas => "jv-ID",
        }
    }
}

/// One stretch of speech. Several segments make a dialogue (Kabar Radio).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TtsSegment {
    pub voice: Voice,
    pub text: String,
}

/// `POST /v1/tts` → `audio/mpeg` (24 kHz mono MP3). The server builds the
/// SSML, so only plain text is accepted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TtsRequest {
    pub segments: Vec<TtsSegment>,
    /// Speaking rate in percent relative to normal, -50..=50 (e.g. -20 for slow).
    #[serde(default)]
    pub rate_percent: i8,
}

// --- vault -----------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineStatus {
    pub engine: EngineId,
    /// Its key (or binding) is present.
    pub configured: bool,
    pub enabled: bool,
    pub model: Option<String>,
}

/// `GET /v1/vault` — which services the server can reach. Never contains keys.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultStatus {
    pub engines: Vec<EngineStatus>,
    pub tts_configured: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_platform_is_other() {
        let p: Platform = serde_json::from_str(r#""ios""#).unwrap();
        assert_eq!(p, Platform::Other);
    }
}
