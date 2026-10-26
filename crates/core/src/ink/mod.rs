//! The Tablet Studio's ink engine (`docs/SPEC.md` §4, PLAN §17): stroke
//! geometry, grouping strokes into lines and words for recognition, notebook
//! search, handwriting feedback, spelling suggestions, Gambar! rounds and
//! mind maps. Pure logic: the WebView draws, the app stores.

pub mod answer;
pub mod gambar;
pub mod group;
pub mod mindmap;
pub mod recognize;
pub mod search;
pub mod spell;

use kabar_protocol::ink::{COORD_SCALE, InkStroke, MAX_STROKE_POINTS};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Axis-aligned box, page pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect { x, y, w: self.right().max(o.right()) - x, h: self.bottom().max(o.bottom()) - y }
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        self.x <= o.right() && o.x <= self.right() && self.y <= o.bottom() && o.y <= self.bottom()
    }

    /// Grown by `d` on every side.
    pub fn inflate(&self, d: f32) -> Rect {
        Rect { x: self.x - d, y: self.y - d, w: self.w + 2.0 * d, h: self.h + 2.0 * d }
    }

    /// Gap between two boxes along x and y (0 when they overlap on that axis).
    pub fn gap(&self, o: &Rect) -> (f32, f32) {
        let gx = (o.x - self.right()).max(self.x - o.right()).max(0.0);
        let gy = (o.y - self.bottom()).max(self.y - o.bottom()).max(0.0);
        (gx, gy)
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.w, self.h]
    }
}

/// The stroke's points in page pixels.
pub fn points(s: &InkStroke) -> Vec<Point> {
    s.xs.iter()
        .zip(&s.ys)
        .map(|(&x, &y)| Point { x: x as f32 / COORD_SCALE, y: y as f32 / COORD_SCALE })
        .collect()
}

pub fn bbox(s: &InkStroke) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (&x, &y) in s.xs.iter().zip(&s.ys) {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    if x0 > x1 {
        return Rect::default();
    }
    let k = COORD_SCALE;
    Rect { x: x0 as f32 / k, y: y0 as f32 / k, w: (x1 - x0) as f32 / k, h: (y1 - y0) as f32 / k }
}

pub fn bbox_all<'a>(strokes: impl IntoIterator<Item = &'a InkStroke>) -> Option<Rect> {
    strokes.into_iter().map(bbox).reduce(|a, b| a.union(&b))
}

fn dist_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0) };
    let (cx, cy) = (a.x + t * dx, a.y + t * dy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// The eraser at `p` with `radius` touches this stroke (half its width counts).
pub fn hits(s: &InkStroke, p: Point, radius: f32) -> bool {
    let reach = radius + s.width / 2.0;
    if !bbox(s).inflate(reach).intersects(&Rect { x: p.x, y: p.y, w: 0.0, h: 0.0 }) {
        return false;
    }
    let pts = points(s);
    match pts.as_slice() {
        [] => false,
        [only] => dist_to_segment(p, *only, *only) <= reach,
        _ => pts.windows(2).any(|w| dist_to_segment(p, w[0], w[1]) <= reach),
    }
}

/// Indices of the points Ramer–Douglas–Peucker keeps at `tolerance` px.
pub fn simplify(pts: &[Point], tolerance: f32) -> Vec<usize> {
    if pts.len() <= 2 {
        return (0..pts.len()).collect();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (mut best, mut at) = (0.0f32, 0);
        for i in a + 1..b {
            let d = dist_to_segment(pts[i], pts[a], pts[b]);
            if d > best {
                best = d;
                at = i;
            }
        }
        if best > tolerance {
            keep[at] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
    (0..pts.len()).filter(|&i| keep[i]).collect()
}

/// Drops points that don't change the shape by more than `tolerance` px.
/// Keeps pressure and timing of the points that stay.
pub fn simplify_stroke(s: &InkStroke, tolerance: f32) -> InkStroke {
    let keep = simplify(&points(s), tolerance);
    let pick = |v: &[i32]| keep.iter().map(|&i| v[i]).collect::<Vec<_>>();
    InkStroke {
        xs: pick(&s.xs),
        ys: pick(&s.ys),
        ps: if s.ps.is_empty() { Vec::new() } else { keep.iter().map(|&i| s.ps[i]).collect() },
        ts: keep.iter().map(|&i| s.ts[i]).collect(),
        ..s.clone()
    }
}

/// Splits a stroke longer than [`MAX_STROKE_POINTS`] into overlapping pieces
/// (each piece starts at the previous one's last point, so ink stays joined).
/// `next_id` names the extra pieces.
pub fn split_stroke(s: InkStroke, mut next_id: impl FnMut() -> String) -> Vec<InkStroke> {
    if s.len() <= MAX_STROKE_POINTS {
        return vec![s];
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start + 1 < s.len() {
        let end = (start + MAX_STROKE_POINTS).min(s.len());
        let t0 = s.ts[start];
        let piece = InkStroke {
            id: if start == 0 { s.id.clone() } else { next_id() },
            xs: s.xs[start..end].to_vec(),
            ys: s.ys[start..end].to_vec(),
            ps: if s.ps.is_empty() { Vec::new() } else { s.ps[start..end].to_vec() },
            ts: s.ts[start..end].iter().map(|t| t - t0).collect(),
            started_ms: s.started_ms + u64::from(t0),
            ..s.clone()
        };
        out.push(piece);
        start = end - 1;
    }
    out
}

#[cfg(test)]
pub(crate) mod testing {
    use kabar_protocol::ink::{InkStroke, InkTool};
    use kabar_protocol::pen::PointerKind;

    /// A pen stroke through `pts` (page pixels), one point every 8 ms.
    pub fn stroke(id: &str, started_ms: u64, pts: &[(f32, f32)]) -> InkStroke {
        InkStroke {
            id: id.into(),
            doc: "page".into(),
            tool: InkTool::Pen,
            color: "ink".into(),
            width: 2.0,
            pointer: PointerKind::Pen,
            xs: pts.iter().map(|p| (p.0 * 10.0) as i32).collect(),
            ys: pts.iter().map(|p| (p.1 * 10.0) as i32).collect(),
            ps: vec![128; pts.len()],
            ts: (0..pts.len() as u32).map(|i| i * 8).collect(),
            started_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::stroke;
    use super::*;

    #[test]
    fn bbox_and_hit_test() {
        let s = stroke("a", 0, &[(10.0, 10.0), (50.0, 10.0), (50.0, 30.0)]);
        assert_eq!(bbox(&s), Rect { x: 10.0, y: 10.0, w: 40.0, h: 20.0 });
        assert!(hits(&s, Point { x: 30.0, y: 12.0 }, 2.0));
        assert!(hits(&s, Point { x: 52.0, y: 20.0 }, 2.0));
        assert!(!hits(&s, Point { x: 30.0, y: 20.0 }, 2.0));
        let dot = stroke("b", 0, &[(5.0, 5.0)]);
        assert!(hits(&dot, Point { x: 6.0, y: 5.0 }, 1.0));
        let both = bbox_all([&s, &dot]).unwrap();
        assert_eq!((both.x, both.y, both.right(), both.bottom()), (5.0, 5.0, 50.0, 30.0));
    }

    #[test]
    fn simplify_keeps_corners_and_drops_straight_runs() {
        let line: Vec<(f32, f32)> =
            (0..=20).map(|i| (i as f32, 0.0)).chain((1..=20).map(|i| (20.0, i as f32))).collect();
        let s = stroke("a", 0, &line);
        let t = simplify_stroke(&s, 0.5);
        assert_eq!(t.len(), 3);
        assert_eq!((t.xs[1], t.ys[1]), (200, 0));
        assert_eq!(t.ts, [0, 160, 320]);
        assert!(t.is_well_formed());
    }

    #[test]
    fn long_strokes_split_into_joined_pieces() {
        let pts: Vec<(f32, f32)> = (0..4500).map(|i| (i as f32 * 0.1, 0.0)).collect();
        let s = stroke("a", 1000, &pts);
        let mut n = 0;
        let pieces = split_stroke(s, || {
            n += 1;
            format!("a{n}")
        });
        assert_eq!(pieces.len(), 3);
        assert!(pieces.iter().all(InkStroke::is_well_formed));
        assert_eq!(pieces[0].id, "a");
        assert_eq!(pieces[1].id, "a1");
        assert_eq!(pieces[0].xs.last(), pieces[1].xs.first());
        assert_eq!(pieces[1].ts[0], 0);
        assert_eq!(pieces[1].started_ms, 1000 + 1999 * 8);
        assert_eq!(pieces.iter().map(InkStroke::len).sum::<usize>(), 4500 + 2);
    }
}

#[cfg(test)]
mod ai_tests;
