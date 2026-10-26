//! Types that cross a process boundary: app ↔ Kabar Cloud ↔ Kabar Home,
//! and the Rust ↔ WebView bridge. Everything here is plain serde data.

pub mod ai;
pub mod cloud;
pub mod coach;
pub mod content;
pub mod hlc;
pub mod ink;
pub mod learn;
pub mod lesson;
pub mod pen;
pub mod reader;
pub mod srs;
pub mod sync;
pub mod test;

pub use hlc::{DeviceId, Hlc};

/// Bumped whenever a wire type changes incompatibly.
pub const PROTOCOL_VERSION: u32 = 1;
