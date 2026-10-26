//! Pen Lab: turns raw pointer samples from the WebView into a report of
//! which pen features actually arrive and how fast.

use std::collections::{BTreeMap, BTreeSet};

use kabar_protocol::pen::{PenEventKind, PenLabReport, PenSample, Percentiles, PointerKind, WebViewInfo};

const BUTTON_BARREL: u16 = 2;
const BUTTON_ERASER: u16 = 32;

#[derive(Clone, Debug, Default)]
pub struct PenLab {
    counts: BTreeMap<String, u64>,
    pressure_min: Option<f32>,
    pressure_max: Option<f32>,
    /// Pressures quantized to 1/1000, to count distinct levels.
    pressure_levels: BTreeSet<u32>,
    tilt_seen: bool,
    twist_seen: bool,
    hover_seen: bool,
    barrel_seen: bool,
    eraser_seen: bool,
    palm: u64,
    coalesced: Vec<f64>,
    predicted: Vec<f64>,
    input_delay: Vec<f64>,
    frame_delay: Vec<f64>,
    pen_moves_down: u64,
    pen_down_since: Option<f64>,
    pen_down_ms: f64,
}

impl PenLab {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn total_events(&self) -> u64 {
        self.counts.values().sum()
    }

    pub fn ingest(&mut self, samples: &[PenSample]) {
        for s in samples {
            self.ingest_one(s);
        }
    }

    fn ingest_one(&mut self, s: &PenSample) {
        *self.counts.entry(format!("{}:{}", pointer_name(s.pointer), kind_name(s.kind))).or_default() += 1;

        let delay = s.t_handled - s.t_event;
        if delay.is_finite() && delay >= 0.0 {
            self.input_delay.push(delay);
        }
        if let Some(frame) = s.t_frame {
            let d = frame - s.t_handled;
            if d.is_finite() && d >= 0.0 {
                self.frame_delay.push(d);
            }
        }

        if s.kind == PenEventKind::Palm {
            self.palm += 1;
        }
        if s.pointer != PointerKind::Pen {
            return;
        }

        match s.kind {
            PenEventKind::Hover => self.hover_seen = true,
            PenEventKind::Down => self.pen_down_since = Some(s.t_event),
            PenEventKind::Up | PenEventKind::Cancel => {
                if let Some(start) = self.pen_down_since.take() {
                    self.pen_down_ms += (s.t_event - start).max(0.0);
                }
            }
            PenEventKind::Move => self.pen_moves_down += 1,
            _ => {}
        }

        if matches!(s.kind, PenEventKind::Down | PenEventKind::Move) && s.pressure > 0.0 {
            self.pressure_min = Some(self.pressure_min.map_or(s.pressure, |m| m.min(s.pressure)));
            self.pressure_max = Some(self.pressure_max.map_or(s.pressure, |m| m.max(s.pressure)));
            self.pressure_levels.insert((s.pressure * 1000.0).round() as u32);
        }
        if s.tilt_x != 0.0 || s.tilt_y != 0.0 {
            self.tilt_seen = true;
        }
        if s.twist != 0.0 {
            self.twist_seen = true;
        }
        if s.buttons & BUTTON_BARREL != 0 {
            self.barrel_seen = true;
        }
        if s.buttons & BUTTON_ERASER != 0 {
            self.eraser_seen = true;
        }
        if s.kind == PenEventKind::Move {
            if let Some(c) = s.coalesced {
                self.coalesced.push(f64::from(c));
            }
            if let Some(p) = s.predicted {
                self.predicted.push(f64::from(p));
            }
        }
    }

    pub fn report(&self, webview: WebViewInfo, device_model: Option<String>) -> PenLabReport {
        PenLabReport {
            device_model,
            webview,
            counts: self.counts.clone(),
            pressure_min: self.pressure_min,
            pressure_max: self.pressure_max,
            pressure_levels_seen: self.pressure_levels.len(),
            tilt_seen: self.tilt_seen,
            twist_seen: self.twist_seen,
            hover_seen: self.hover_seen,
            barrel_button_seen: self.barrel_seen,
            eraser_seen: self.eraser_seen,
            palm_touches_while_pen_in_range: self.palm,
            coalesced_avg: mean(&self.coalesced),
            predicted_avg: mean(&self.predicted),
            pen_move_rate_hz: (self.pen_down_ms > 0.0)
                .then(|| self.pen_moves_down as f64 / (self.pen_down_ms / 1000.0)),
            input_delay_ms: percentiles(&self.input_delay),
            frame_delay_ms: percentiles(&self.frame_delay),
        }
    }
}

fn pointer_name(p: PointerKind) -> &'static str {
    match p {
        PointerKind::Pen => "pen",
        PointerKind::Touch => "touch",
        PointerKind::Mouse => "mouse",
        PointerKind::Other => "other",
    }
}

fn kind_name(k: PenEventKind) -> &'static str {
    match k {
        PenEventKind::Down => "down",
        PenEventKind::Move => "move",
        PenEventKind::Up => "up",
        PenEventKind::Hover => "hover",
        PenEventKind::Enter => "enter",
        PenEventKind::Leave => "leave",
        PenEventKind::Cancel => "cancel",
        PenEventKind::Palm => "palm",
    }
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// Nearest-rank percentiles.
pub fn percentiles(values: &[f64]) -> Percentiles {
    if values.is_empty() {
        return Percentiles::default();
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let rank = |p: f64| v[((p * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1];
    Percentiles { p50: rank(0.50), p95: rank(0.95), max: v[v.len() - 1], n: v.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(kind: PenEventKind, pointer: PointerKind, t: f64) -> PenSample {
        PenSample {
            kind,
            pointer,
            t_event: t,
            t_handled: t + 4.0,
            t_frame: Some(t + 12.0),
            pressure: 0.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            twist: 0.0,
            buttons: 0,
            coalesced: None,
            predicted: None,
        }
    }

    #[test]
    fn a_stroke_with_hover_pressure_and_tilt() {
        use PenEventKind::*;
        let mut lab = PenLab::new();
        let mut events = vec![sample(Hover, PointerKind::Pen, 0.0), sample(Down, PointerKind::Pen, 10.0)];
        for i in 0..240 {
            let mut s = sample(Move, PointerKind::Pen, 10.0 + f64::from(i) * 4.0);
            s.pressure = 0.2 + f64::from(i % 10) as f32 * 0.05;
            s.tilt_x = 30.0;
            s.coalesced = Some(2);
            s.predicted = Some(1);
            events.push(s);
        }
        events.push(sample(Up, PointerKind::Pen, 1010.0));
        lab.ingest(&events);

        let r = lab.report(WebViewInfo::default(), Some("SM-S928B".into()));
        assert!(r.hover_seen && r.tilt_seen);
        assert!(!r.twist_seen && !r.eraser_seen && !r.barrel_button_seen);
        assert_eq!(r.pressure_levels_seen, 10);
        assert_eq!(r.counts["pen:move"], 240);
        assert_eq!(r.coalesced_avg, Some(2.0));
        assert!((r.pen_move_rate_hz.unwrap() - 240.0).abs() < 1e-9);
        assert_eq!(r.input_delay_ms.p50, 4.0);
        assert_eq!(r.frame_delay_ms.p95, 8.0);
    }

    #[test]
    fn buttons_and_palm_are_detected() {
        let mut lab = PenLab::new();
        let mut barrel = sample(PenEventKind::Move, PointerKind::Pen, 0.0);
        barrel.buttons = 1 | 2;
        let mut eraser = sample(PenEventKind::Down, PointerKind::Pen, 1.0);
        eraser.buttons = 32;
        let palm = sample(PenEventKind::Palm, PointerKind::Touch, 2.0);
        lab.ingest(&[barrel, eraser, palm]);
        let r = lab.report(WebViewInfo::default(), None);
        assert!(r.barrel_button_seen && r.eraser_seen);
        assert_eq!(r.palm_touches_while_pen_in_range, 1);
        assert_eq!(lab.total_events(), 3);
    }

    #[test]
    fn percentiles_nearest_rank() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        let p = percentiles(&v);
        assert_eq!((p.p50, p.p95, p.max, p.n), (50.0, 95.0, 100.0, 100));
        assert_eq!(percentiles(&[]).n, 0);
    }
}
