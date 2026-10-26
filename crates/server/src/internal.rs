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
