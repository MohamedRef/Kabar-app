//! Types that cross a process boundary: app ↔ Kabar Cloud ↔ Kabar Home,
//! and the Rust ↔ WebView bridge. Everything here is plain serde data.

pub mod ai;
pub mod hlc;
pub mod lesson;
pub mod pen;
pub mod srs;

pub use hlc::{DeviceId, Hlc};

/// Bumped whenever a wire type changes incompatibly.
pub const PROTOCOL_VERSION: u32 = 1;
