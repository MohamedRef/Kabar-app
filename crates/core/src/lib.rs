//! Kabar's pure logic. No UI, no network, no filesystem: storage, clocks and
//! HTTP are passed in, so the same code runs in the app, Kabar Cloud (wasm32)
//! and tests.

pub mod ai;
pub mod aksara;
pub mod bundle_check;
pub mod cast;
pub mod chat;
pub mod clock;
pub mod coach;
pub mod content_ids;
pub mod games;
pub mod grammar;
pub mod ink;
pub mod journey;
pub mod lang;
pub mod layout;
pub mod learn;
pub mod lesson;
pub mod locale;
pub mod packs;
pub mod pagi;
pub mod penlab;
pub mod pron;
pub mod reader;
pub mod review;
pub mod settings;
pub mod srs;
pub mod stats;
pub mod story;
pub mod sync;
pub mod sync_client;
pub mod theme;
pub mod update;
