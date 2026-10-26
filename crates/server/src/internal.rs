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

#[derive(Debug, Serialize, Deserialize)]
pub struct Expiry {
    pub refresh_expires_ms: u64,
    pub email: String,
}
