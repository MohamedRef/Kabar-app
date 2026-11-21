//! Messages between the Worker and an account's Durable Object. Both sides
//! ship in the same build, so these don't need versioning; devices never see
//! them. The DO is reachable only through the Worker's binding, so the
//! `x-kabar-*` headers set by the Worker are trusted there.

use kabar_protocol::DeviceId;
use kabar_protocol::cloud::DeviceInfo;
use serde::{Deserialize, Serialize};

pub const DEVICE_HEADER: &str = "x-kabar-device";

#[derive(Debug, Serialize, Deserialize)]
pub struct Register {
    pub info: DeviceInfo,
    pub email: String,
    pub refresh_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Refresh {
    pub device: DeviceId,
    pub presented_hash: String,
    pub new_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Revoke {
    pub device: DeviceId,
}

/// Sets (or with `None`, clears) the calling device's FCM token.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetPush {
    pub token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Expiry {
    pub refresh_expires_ms: u64,
    pub email: String,
}

/// Stores a validated image for the vision tasks (see `crate::blobs`).
#[derive(Debug, Serialize, Deserialize)]
pub struct BlobPut {
    pub sha256: String,
    pub mime: String,
    pub data_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlobGet {
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlobData {
    pub mime: String,
    pub data_b64: String,
}

/// Account object `GET /login` and `POST /login`: the caller's email,
/// current username, and any staged/retired usernames still to clear.
#[derive(Debug, Serialize, Deserialize)]
pub struct AccountLoginState {
    pub email: String,
    pub username: Option<String>,
    #[serde(default)]
    pub retire: Vec<String>,
}

/// Account object `POST /login`: stages a rename, commits the username, or
/// marks a retired username cleared.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LoginName {
    pub username: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub retired: Option<String>,
}

/// Login object `POST /set`.
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginSet {
    pub account_id: String,
    pub email: String,
    pub salt: String,
    pub verifier: String,
}

/// Login object `POST /clear`.
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginClear {
    pub account_id: String,
}

/// Login object `POST /check`: the app's stretched key, base64url.
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginCheck {
    #[serde(default)]
    pub username: String,
    pub key: String,
}
