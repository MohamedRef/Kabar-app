//! Kabar's pure logic. No UI, no network, no filesystem: storage, clocks and
//! HTTP are passed in, so the same code runs in the app, Kabar Cloud (wasm32)
//! and tests.

pub mod ai;
pub mod clock;
pub mod lang;
pub mod layout;
pub mod learn;
pub mod locale;
pub mod penlab;
pub mod settings;
pub mod srs;
pub mod sync;
