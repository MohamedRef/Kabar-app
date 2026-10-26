//! Pen Lab wire types: samples sent from the WebView ink module to Rust,
//! and the report Rust builds from them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PointerKind {
    Pen,
    Touch,
    Mouse,
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PenEventKind {
    Down,
    Move,
    Up,
    /// A move while the pointer is in range but not touching (pen hover).
    Hover,
    Enter,
    Leave,
    Cancel,
    /// A touch that arrived while a pen was in range (palm candidate).
    Palm,
}

/// One pointer event as seen by the WebView. Times are `performance.now()`
/// milliseconds on the same clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PenSample {
    pub kind: PenEventKind,
    pub pointer: PointerKind,
    /// `event.timeStamp`: when the platform says the input happened.
    pub t_event: f64,
    /// When our handler ran.
    pub t_handled: f64,
    /// Start of the next animation frame after handling (ink on screen soon after).
    pub t_frame: Option<f64>,
    pub pressure: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub twist: f32,
    /// `PointerEvent.buttons` bitmask (2 = barrel button, 32 = eraser).
    pub buttons: u16,
    /// `getCoalescedEvents().length`, `None` when the API is missing.
    pub coalesced: Option<u16>,
    /// `getPredictedEvents().length`, `None` when the API is missing.
    pub predicted: Option<u16>,
}

/// What the WebView reports about its own capabilities.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WebViewInfo {
    pub user_agent: String,
    pub device_pixel_ratio: f64,
    pub viewport_w: f64,
    pub viewport_h: f64,
    pub desynchronized_canvas: bool,
    pub coalesced_api: bool,
    pub predicted_api: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
    pub n: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PenLabReport {
    pub device_model: Option<String>,
    pub webview: WebViewInfo,
    /// "pen:move" → count, etc.
    pub counts: BTreeMap<String, u64>,
    pub pressure_min: Option<f32>,
    pub pressure_max: Option<f32>,
    pub pressure_levels_seen: usize,
    pub tilt_seen: bool,
    pub twist_seen: bool,
    pub hover_seen: bool,
    pub barrel_button_seen: bool,
    pub eraser_seen: bool,
    pub palm_touches_while_pen_in_range: u64,
    pub coalesced_avg: Option<f64>,
    pub predicted_avg: Option<f64>,
    /// Pen moves per second while touching.
    pub pen_move_rate_hz: Option<f64>,
    /// Platform timestamp → our handler (ms).
    pub input_delay_ms: Percentiles,
    /// Our handler → next animation frame (ms).
    pub frame_delay_ms: Percentiles,
}
