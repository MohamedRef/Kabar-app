//! Kabar Cloud: auth, the device registry, the per-account sync hub, the key
//! vault and the AI/TTS proxy.
//!
//! Everything outside `wasm` is plain Rust, tested natively against real
//! SQLite. `wasm` binds it to the Cloudflare runtime (Worker + Durable Object).

pub mod ai;
pub mod config;
pub mod devices;
pub mod google;
pub mod hub;
pub mod internal;
pub mod quota;
pub mod sql;
pub mod time;
pub mod tokens;
pub mod tts;

#[cfg(target_arch = "wasm32")]
mod wasm;
