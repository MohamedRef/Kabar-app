//! Where things go on the map: one pin per level, its units as a small loop of
//! stops around it, and the route between levels (flight arcs for the BIPA
//! islands, a train line with a branch for Javanese).

use super::geo::{self, ViewBox};
use super::{Curriculum, PlaceKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteStyle {
    /// BIPA: island hopping by plane.
    Islands,
    /// Javanese: a train line across Java.
    Rail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentKind {
    Flight,
    Rail,
    Branch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub kind: SegmentKind,
    pub from: (f64, f64),
    pub to: (f64, f64),
    /// Quadratic control point for flight arcs.
    pub control: Option<(f64, f64)>,
}

impl Segment {
    /// The SVG path `d` attribute.
    pub fn path_d(&self) -> String {
        let (a, b) = (self.from, self.to);
        match self.control {
            Some(c) => format!("M{:.2} {:.2}Q{:.2} {:.2} {:.2} {:.2}", a.0, a.1, c.0, c.1, b.0, b.1),
            None => format!("M{:.2} {:.2}L{:.2} {:.2}", a.0, a.1, b.0, b.1),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LevelPin {
    pub level_id: String,
    pub x: f64,
    pub y: f64,
    /// One point per unit, in order: the stops around the place.
    pub units: Vec<(f64, f64)>,
    /// Where the pin is drawn at the default zoom, relative to (x, y), when it
    /// sits too close to a neighbour (Bali and Lombok): see
    /// [`MapLayout::spread_pins`]. (0, 0) for a pin with room around it.
    pub nudge: (f64, f64),
    /// The zoom at which the nudge has faded to nothing (1 without a nudge).
    /// At zoom `s` the pin is drawn at
    /// `(x, y) + nudge * max(0, (fade_zoom / s - 1) / (fade_zoom - 1))`, so a
    /// nudged pair stays exactly `min_gap / s` apart until the real places
    /// are far enough apart on their own.
    pub fade_zoom: f64,
}

impl LevelPin {
    /// The units' little route as an SVG path (empty with fewer than two units).
    pub fn units_d(&self) -> String {
        let mut d = String::new();
        for (i, (x, y)) in self.units.iter().enumerate() {
            d.push_str(&format!("{}{x:.2} {y:.2}", if i == 0 { 'M' } else { 'L' }));
        }
        if self.units.len() < 2 { String::new() } else { d }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapLayout {
    pub style: RouteStyle,
    pub view: ViewBox,
    pub pins: Vec<LevelPin>,
    pub segments: Vec<Segment>,
    /// Map units for the unit loops; pins and labels are sized from it.
    pub radius: f64,
}

impl MapLayout {
    pub fn new(curriculum: &Curriculum) -> Self {
        let rail = !curriculum.levels.iter().any(|l| l.kind() == PlaceKind::Island)
            && curriculum
                .levels
                .iter()
                .any(|l| matches!(l.kind(), PlaceKind::TrainLine | PlaceKind::BranchLine));
        let (style, view, radius) = if rail {
            (RouteStyle::Rail, geo::java_view(), 0.9)
        } else {
            (RouteStyle::Islands, geo::indonesia_view(), 5.0)
        };

        let mut pins: Vec<LevelPin> = Vec::new();
        let mut segments = Vec::new();
        // The last main-line stop: branches leave from it, and the main line
        // carries on from it after a branch.
        let mut trunk: Option<(f64, f64)> = None;
        for level in &curriculum.levels {
            let (x, y) = match geo::place_for(&level.id, level.place()) {
                Some(p) => p.point(),
                // Unknown place: beside the previous pin, so it stays tappable.
                None => pins
                    .last()
                    .map_or((view.x + view.w / 2.0, view.y + view.h / 2.0), |p| (p.x + radius * 4.0, p.y)),
            };
            let kind = match (style, level.kind()) {
                (RouteStyle::Rail, PlaceKind::BranchLine) => SegmentKind::Branch,
                (RouteStyle::Rail, _) => SegmentKind::Rail,
                (RouteStyle::Islands, _) => SegmentKind::Flight,
            };
            let from = match kind {
                SegmentKind::Flight => pins.last().map(|p| (p.x, p.y)),
                _ => trunk,
            };
            if let Some(from) = from {
                let control = (kind == SegmentKind::Flight).then(|| arc_control(from, (x, y)));
                segments.push(Segment { kind, from, to: (x, y), control });
            }
            if kind != SegmentKind::Branch {
                trunk = Some((x, y));
            }
            let units = ring((x, y), level.units.len(), radius);
            pins.push(LevelPin {
                level_id: level.id.clone(),
                x,
                y,
                units,
                nudge: (0.0, 0.0),
                fade_zoom: 1.0,
            });
        }
        Self { style, view, pins, segments, radius }
    }

    pub fn pin(&self, level_id: &str) -> Option<&LevelPin> {
        self.pins.iter().find(|p| p.level_id == level_id)
    }

    /// Spreads pins that sit closer than `min_gap` map units at the default
    /// zoom (Bali and Lombok are 1° apart), so their discs never cover each
    /// other. Only the drawn position moves ([`LevelPin::nudge`]): the route,
    /// the unit stops and the real place stay where they are, and the nudge
    /// fades as the learner zooms in. Nudged pins stay inside the view.
    pub fn spread_pins(&mut self, min_gap: f64) {
        let real: Vec<(f64, f64)> = self.pins.iter().map(|p| (p.x, p.y)).collect();
        let mut at = real.clone();
        // Pairwise relaxation: a pair too close moves apart along the line
        // between them, half each way. A handful of rounds settles small
        // clusters; the map only ever has a few close neighbours.
        for _ in 0..64 {
            let mut moved = false;
            for i in 0..at.len() {
                for j in i + 1..at.len() {
                    let (dx, dy) = (at[j].0 - at[i].0, at[j].1 - at[i].1);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d >= min_gap - 1e-9 {
                        continue;
                    }
                    // Same spot: split east–west.
                    let (ux, uy) = if d < 1e-9 { (1.0, 0.0) } else { (dx / d, dy / d) };
                    let push = (min_gap - d) / 2.0 + 1e-6;
                    at[i] = (at[i].0 - ux * push, at[i].1 - uy * push);
                    at[j] = (at[j].0 + ux * push, at[j].1 + uy * push);
                    moved = true;
                }
            }
            for (p, r) in at.iter_mut().zip(&real) {
                if p != r {
                    *p = self.view.clamp(*p, min_gap / 2.0);
                }
            }
            if !moved {
                break;
            }
        }
        for (i, pin) in self.pins.iter_mut().enumerate() {
            let nudge = (at[i].0 - real[i].0, at[i].1 - real[i].1);
            if nudge.0.hypot(nudge.1) < 1e-6 {
                (pin.nudge, pin.fade_zoom) = ((0.0, 0.0), 1.0);
                continue;
            }
            // The closest real neighbour sets how far in the nudge is needed.
            let nearest = real
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, q)| (q.0 - real[i].0).hypot(q.1 - real[i].1))
                .fold(f64::INFINITY, f64::min);
            pin.nudge = nudge;
            pin.fade_zoom = (min_gap / nearest.max(min_gap / 64.0)).max(1.0 + 1e-6);
        }
    }
}

impl LevelPin {
    /// Where the pin is drawn at zoom `s` (1 = the whole view).
    pub fn drawn_at(&self, s: f64) -> (f64, f64) {
        let f = if self.fade_zoom > 1.0 {
            ((self.fade_zoom / s.max(1.0) - 1.0) / (self.fade_zoom - 1.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (self.x + self.nudge.0 * f, self.y + self.nudge.1 * f)
    }
}

/// Bows a flight arc northwards by a fifth of its length.
fn arc_control(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(f64::EPSILON);
    let (mut px, mut py) = (dy / len, -dx / len);
    if py > 0.0 {
        (px, py) = (-px, -py);
    }
    let k = len * 0.2;
    ((a.0 + b.0) / 2.0 + px * k, (a.1 + b.1) / 2.0 + py * k)
}

/// `n` points on a circle, clockwise from the top.
fn ring(center: (f64, f64), n: usize, radius: f64) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / n.max(1) as f64;
            (center.0 + radius * a.cos(), center.1 + radius * a.sin())
        })
        .collect()
}
