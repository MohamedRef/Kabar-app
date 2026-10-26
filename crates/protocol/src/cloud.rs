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
    /// `EngineCall::image_sha256` names an image the server doesn't have
    /// (never uploaded, or older than a week): upload it and retry.
    BlobMissing,
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

// --- push (Kak Nur's reminders) ------------------------------------------------

/// Longest FCM registration token accepted.
pub const MAX_PUSH_TOKEN_LEN: usize = 4096;

/// `PUT /v1/devices/me/push`: this device's FCM registration token. Send it on
/// every launch and whenever Firebase rotates it; an empty token turns push
/// off for this device. Signing out (or a remote sign-out) forgets it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushTokenRequest {
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushTokenReply {
    /// The token is stored (false after an empty token cleared it).
    pub registered: bool,
    /// Kabar Cloud has its Firebase key, so pushes can actually go out.
    pub push_configured: bool,
}

/// What a push is about. Unknown kinds from a newer server are `Other`, so an
/// old app ignores them instead of failing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PushKind {
    Study,
    WelcomeBack,
    WeeklyReport,
    #[serde(other)]
    Other,
}

/// The data of a Kabar push (an FCM data-only message; every value is a
/// string on the wire, see [`PushNotice::to_data`]). It carries no personal
/// text: the device renders Kak Nur's line itself, in the UI language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushNotice {
    pub kind: PushKind,
    /// Kak Nur's line, as the snake_case name of `kabar_core::coach::voice::Line`
    /// (e.g. `remind_study`).
    pub line: String,
    /// The learner's local day number (days since 1970-01-01): the `seed`
    /// for `voice::say`, so every device picks the same wording.
    pub seed: i32,
}

impl PushNotice {
    /// The FCM `data` map: string keys, string values.
    pub fn to_data(&self) -> Vec<(&'static str, String)> {
        let kind = match self.kind {
            PushKind::Study => "study",
            PushKind::WelcomeBack => "welcome_back",
            PushKind::WeeklyReport => "weekly_report",
            PushKind::Other => "other",
        };
        vec![("kind", kind.to_owned()), ("line", self.line.clone()), ("seed", self.seed.to_string())]
    }

    /// Reads the FCM `data` map back (on the device).
    pub fn from_data<'a>(mut get: impl FnMut(&str) -> Option<&'a str>) -> Option<Self> {
        let kind = serde_json::from_value(serde_json::Value::String(get("kind")?.to_owned())).ok()?;
        Some(Self { kind, line: get("line")?.to_owned(), seed: get("seed")?.parse().ok()? })
    }
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
    /// Pitch in percent relative to the voice, -20..=20. Azure has two
    /// voices per language, so story characters share them and differ in
    /// pitch. Older servers ignore it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pitch_percent: i8,
}

fn is_zero(v: &i8) -> bool {
    *v == 0
}

/// The proxy's MP3 bitrate (`audio-24khz-48kbitrate-mono-mp3`, constant), so a
/// file's size tells its length.
pub const TTS_MP3_KBPS: u32 = 48;

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
    /// Firebase Cloud Messaging is set up, so reminders can be pushed.
    #[serde(default)]
    pub push_configured: bool,
}

// --- usage (the cost dashboard) ----------------------------------------------

/// A metered service behind a hard stop. Unknown services from a newer
/// server are `Other`, so an old app skips them instead of failing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageService {
    WorkersAi,
    GeminiFree,
    Claude,
    AzureTts,
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageUnit {
    Requests,
    Characters,
    Neurons,
    RowsRead,
    RowsWritten,
    #[serde(other)]
    Other,
}

/// How often a counter starts again (at 00:00 UTC).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsagePeriod {
    Day,
    Month,
    #[serde(other)]
    Other,
}

/// One service's counter for its current period.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceUsage {
    pub service: UsageService,
    pub unit: UsageUnit,
    pub used: u64,
    /// Kabar's hard stop, kept under the provider's free limit. A call that
    /// would cross it is refused before it is made.
    pub limit: u64,
    pub period: UsagePeriod,
    /// When the counter starts again (Unix ms, UTC).
    pub resets_at_ms: u64,
    /// Switched on on the server (Claude is off unless the learner opts in).
    pub enabled: bool,
    /// Its key or binding is present.
    pub configured: bool,
}

/// A Cloudflare platform limit, for information only: on the Free plan
/// Cloudflare has no card on file and cannot bill; it stops serving instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformLimit {
    /// e.g. "Workers", "Durable Objects", "D1".
    pub name: String,
    pub unit: UsageUnit,
    pub limit: u64,
    pub period: UsagePeriod,
}

/// `GET /v1/usage` — what Kabar Cloud has used this period, and what it spent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageReport {
    pub services: Vec<ServiceUsage>,
    #[serde(default)]
    pub platform: Vec<PlatformLimit>,
    /// Money spent this month, in US cents. Always 0 while hard stops are on.
    pub spend_usd_cents: u64,
    /// Every metered service refuses calls past its free limit.
    #[serde(default = "always")]
    pub hard_stops: bool,
    pub generated_at_ms: u64,
}

fn always() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_report_tolerates_newer_servers() {
        let json = r#"{
            "services": [
                {"service":"gemini_free","unit":"requests","used":12,"limit":1000,"period":"day",
                 "resets_at_ms":1793059200000,"enabled":true,"configured":true},
                {"service":"mistral_free","unit":"tokens","used":1,"limit":2,"period":"hour",
                 "resets_at_ms":0,"enabled":true,"configured":true,"extra":1}
            ],
            "spend_usd_cents": 0,
            "generated_at_ms": 1793011856000
        }"#;
        let r: UsageReport = serde_json::from_str(json).unwrap();
        assert_eq!(r.services[0].service, UsageService::GeminiFree);
        assert_eq!(r.services[1].service, UsageService::Other);
        assert_eq!(r.services[1].unit, UsageUnit::Other);
        assert_eq!(r.services[1].period, UsagePeriod::Other);
        assert!(r.hard_stops);
        assert!(r.platform.is_empty());
    }

    #[test]
    fn unknown_platform_is_other() {
        let p: Platform = serde_json::from_str(r#""ios""#).unwrap();
        assert_eq!(p, Platform::Other);
    }

    #[test]
    fn push_notice_round_trips_through_fcm_data() {
        let n = PushNotice { kind: PushKind::WelcomeBack, line: "remind_welcome_back".into(), seed: 20_752 };
        let data = n.to_data();
        assert_eq!(data[0], ("kind", "welcome_back".to_owned()));
        assert_eq!(data[2], ("seed", "20752".to_owned()));
        let get = |k: &str| data.iter().find(|(key, _)| *key == k).map(|(_, v)| v.as_str());
        assert_eq!(PushNotice::from_data(get), Some(n));
        let newer = |k: &str| match k {
            "kind" => Some("streak_party"),
            "line" => Some("x"),
            _ => Some("1"),
        };
        assert_eq!(PushNotice::from_data(newer).unwrap().kind, PushKind::Other);
    }

    #[test]
    fn old_vault_status_still_parses() {
        let v: VaultStatus = serde_json::from_str(r#"{"engines":[],"tts_configured":true}"#).unwrap();
        assert!(!v.push_configured);
    }
}
