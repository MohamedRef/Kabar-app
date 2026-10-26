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
            pins.push(LevelPin { level_id: level.id.clone(), x, y, units });
        }
        Self { style, view, pins, segments, radius }
    }

    pub fn pin(&self, level_id: &str) -> Option<&LevelPin> {
        self.pins.iter().find(|p| p.level_id == level_id)
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
