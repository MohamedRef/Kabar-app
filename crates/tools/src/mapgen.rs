//! Polygon geometry for the Journey Map generator: clip to a box, project,
//! simplify (Douglas–Peucker), and write compact SVG path data.

use kabar_core::journey::geo;

pub type Pt = (f64, f64);

/// A lon/lat box: (lon_min, lat_min, lon_max, lat_max).
pub type Bounds = (f64, f64, f64, f64);

/// Sutherland–Hodgman clip of a closed ring (first point not repeated) to a box.
pub fn clip_ring(ring: &[Pt], b: Bounds) -> Vec<Pt> {
    let (x0, y0, x1, y1) = b;
    let mut out = ring.to_vec();
    // Each edge: inside test and intersection with the edge line.
    type Inside = fn(Pt, f64) -> bool;
    let edges: [(Inside, f64, bool); 4] = [
        (|p, v| p.0 >= v, x0, true),
        (|p, v| p.0 <= v, x1, true),
        (|p, v| p.1 >= v, y0, false),
        (|p, v| p.1 <= v, y1, false),
    ];
    for (inside, v, vertical) in edges {
        if out.is_empty() {
            break;
        }
        let input = std::mem::take(&mut out);
        let mut prev = *input.last().expect("non-empty");
        for &cur in &input {
            let (ci, pi) = (inside(cur, v), inside(prev, v));
            if ci != pi {
                out.push(cross(prev, cur, v, vertical));
            }
            if ci {
                out.push(cur);
            }
            prev = cur;
        }
    }
    out
}

fn cross(a: Pt, b: Pt, v: f64, vertical: bool) -> Pt {
    if vertical {
        let t = (v - a.0) / (b.0 - a.0);
        (v, a.1 + t * (b.1 - a.1))
    } else {
        let t = (v - a.1) / (b.1 - a.1);
        (a.0 + t * (b.0 - a.0), v)
    }
}

/// Shoelace area (absolute).
pub fn area(ring: &[Pt]) -> f64 {
    let n = ring.len();
    let s: f64 = (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum();
    s.abs() / 2.0
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) };
    let (qx, qy) = (a.0 + t * dx - p.0, a.1 + t * dy - p.1);
    (qx * qx + qy * qy).sqrt()
}

/// Douglas–Peucker on an open polyline (iterative, so long coasts can't
/// overflow the stack).
pub fn simplify_line(pts: &[Pt], tol: f64) -> Vec<Pt> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((s, e)) = stack.pop() {
        let (mut best, mut idx) = (0.0, s);
        for i in s + 1..e {
            let d = seg_dist(pts[i], pts[s], pts[e]);
            if d > best {
                (best, idx) = (d, i);
            }
        }
        if best > tol {
            keep[idx] = true;
            stack.push((s, idx));
            stack.push((idx, e));
        }
    }
    pts.iter().zip(keep).filter_map(|(p, k)| k.then_some(*p)).collect()
}

/// Douglas–Peucker on a closed ring: split at the point farthest from the
/// first, simplify both halves.
pub fn simplify_ring(ring: &[Pt], tol: f64) -> Vec<Pt> {
    if ring.len() < 4 {
        return ring.to_vec();
    }
    let a = ring[0];
    let far = (1..ring.len())
        .max_by(|&i, &j| {
            let d = |k: usize| (ring[k].0 - a.0).powi(2) + (ring[k].1 - a.1).powi(2);
            d(i).total_cmp(&d(j))
        })
        .expect("len >= 4");
    let mut first = simplify_line(&ring[..=far], tol);
    let mut second: Vec<Pt> = ring[far..].to_vec();
    second.push(a);
    let second = simplify_line(&second, tol);
    first.pop();
    first.extend_from_slice(&second[..second.len() - 1]);
    first
}

/// One land ring in lon/lat → projected, simplified ring in map units, or
/// `None` when it falls outside the box or is too small to see.
pub fn prepare(ring: &[Pt], bounds: Bounds, tol: f64, min_area: f64) -> Option<Vec<Pt>> {
    let (x0, y0, x1, y1) = bounds;
    let outside = ring.iter().all(|p| p.0 < x0)
        || ring.iter().all(|p| p.0 > x1)
        || ring.iter().all(|p| p.1 < y0)
        || ring.iter().all(|p| p.1 > y1);
    if outside {
        return None;
    }
    let mut ring = ring.to_vec();
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    let clipped = clip_ring(&ring, bounds);
    let projected: Vec<Pt> = clipped.iter().map(|&(lon, lat)| geo::project(lon, lat)).collect();
    if projected.len() < 3 || area(&projected) < min_area {
        return None;
    }
    let simple = simplify_ring(&projected, tol);
    (simple.len() >= 3 && area(&simple) >= min_area / 2.0).then_some(simple)
}

/// Compact SVG path data: absolute moves, relative lines, `decimals` places.
/// Relative steps are differences of rounded absolutes, so no drift builds up.
pub fn path_data(rings: &[Vec<Pt>], decimals: usize) -> String {
    let f = 10f64.powi(decimals as i32);
    let r = |v: f64| (v * f).round() as i64;
    let fmt = |v: i64| {
        let s = format!("{:.*}", decimals, v as f64 / f);
        let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
        if s == "-0" { "0".to_string() } else { s }
    };
    let mut d = String::new();
    for ring in rings {
        let mut prev: Option<(i64, i64)> = None;
        for &(x, y) in ring {
            let p = (r(x), r(y));
            match prev {
                None => d.push_str(&format!("M{} {}", fmt(p.0), fmt(p.1))),
                Some(q) if q == p => continue,
                Some(q) => {
                    let (dx, dy) = (p.0 - q.0, p.1 - q.1);
                    let sep = if dy < 0 { "" } else { " " };
                    let lead = if d.ends_with(|c: char| c.is_ascii_digit()) && dx >= 0 { " " } else { "" };
                    d.push_str(&format!("{lead}{}{sep}{}", fmt(dx), fmt(dy)));
                }
            }
            if prev.is_none() {
                d.push('l');
            }
            prev = Some(p);
        }
        d.push('Z');
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_a_square_to_a_box() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let c = clip_ring(&sq, (5.0, -1.0, 20.0, 5.0));
        assert!((area(&c) - 25.0).abs() < 1e-9);
        assert!(clip_ring(&sq, (20.0, 20.0, 30.0, 30.0)).is_empty());
    }

    #[test]
    fn simplify_keeps_corners_and_drops_noise() {
        let mut ring = vec![];
        for i in 0..=100 {
            ring.push((i as f64, if i % 2 == 0 { 0.0 } else { 0.01 }));
        }
        ring.extend([(100.0, 50.0), (0.0, 50.0)]);
        let s = simplify_ring(&ring, 0.1);
        assert_eq!(s.len(), 4);
        assert!((area(&s) - 5000.0).abs() < 1.0);
    }

    #[test]
    fn path_data_is_relative_and_closed() {
        let d = path_data(&[vec![(1.0, 1.0), (3.0, 1.0), (3.0, -2.0)]], 1);
        assert_eq!(d, "M1 1l2 0 0-3Z");
    }

    #[test]
    fn prepare_projects_into_map_units() {
        // A 1°×1° square around Jakarta.
        let sq = [(106.0, -7.0), (107.0, -7.0), (107.0, -6.0), (106.0, -6.0), (106.0, -7.0)];
        let r = prepare(&sq, (94.0, -11.0, 142.0, 7.0), 0.1, 1.0).unwrap();
        assert_eq!(r.len(), 4);
        assert!((area(&r) - 400.0).abs() < 1.0);
        assert!(prepare(&sq, (0.0, 0.0, 1.0, 1.0), 0.1, 1.0).is_none());
    }
}
