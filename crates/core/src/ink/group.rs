//! Groups strokes into lines of writing for the recognizer and for search:
//! strokes written close in time and space belong together. A drawing next
//! to the writing ends up in its own group; the recognizer then returns
//! little or nothing for it, which is fine.

use kabar_protocol::ink::InkStroke;

use super::{Rect, bbox};

/// A pause longer than this starts a new group, wherever the pen lands.
pub const PAUSE_MS: u64 = 2500;

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeGroup {
    /// The first stroke's id: stable as more strokes join.
    pub id: String,
    pub strokes: Vec<String>,
    pub bbox: Rect,
    /// Unix ms of the last stroke's end.
    pub last_ms: u64,
}

fn end_ms(s: &InkStroke) -> u64 {
    s.started_ms + u64::from(s.ts.last().copied().unwrap_or(0))
}

/// Groups strokes into lines, in writing order. A stroke joins the current
/// group when it starts within [`PAUSE_MS`] of the group's last stroke, sits
/// on the same line (vertical gap under half a line height) and isn't far
/// to the side (horizontal gap under two line heights).
pub fn group_lines(strokes: &[InkStroke]) -> Vec<StrokeGroup> {
    let mut order: Vec<&InkStroke> = strokes.iter().filter(|s| !s.is_empty()).collect();
    order.sort_by(|a, b| a.started_ms.cmp(&b.started_ms).then_with(|| a.id.cmp(&b.id)));

    let mut groups: Vec<StrokeGroup> = Vec::new();
    for s in order {
        let b = bbox(s);
        let joins = groups.last().is_some_and(|g| {
            let line = g.bbox.h.max(b.h).max(12.0);
            let (gx, gy) = g.bbox.gap(&b);
            s.started_ms.saturating_sub(g.last_ms) <= PAUSE_MS && gy <= line * 0.5 && gx <= line * 2.0
        });
        if joins {
            let g = groups.last_mut().expect("checked");
            g.strokes.push(s.id.clone());
            g.bbox = g.bbox.union(&b);
            g.last_ms = g.last_ms.max(end_ms(s));
        } else {
            groups.push(StrokeGroup {
                id: s.id.clone(),
                strokes: vec![s.id.clone()],
                bbox: b,
                last_ms: end_ms(s),
            });
        }
    }
    groups
}

/// Groups that are finished: no stroke for [`PAUSE_MS`] before `now_ms`.
/// The app sends these to the recognizer.
pub fn settled(groups: &[StrokeGroup], now_ms: u64) -> impl Iterator<Item = &StrokeGroup> {
    groups.iter().filter(move |g| now_ms.saturating_sub(g.last_ms) > PAUSE_MS)
}

#[cfg(test)]
mod tests {
    use super::super::testing::stroke;
    use super::*;

    /// A letter-sized squiggle at `x`, `y`, drawn at `t` ms.
    fn letter(id: &str, t: u64, x: f32, y: f32) -> InkStroke {
        stroke(id, t, &[(x, y), (x + 8.0, y + 20.0), (x + 14.0, y)])
    }

    #[test]
    fn letters_on_one_line_group_and_a_new_line_splits() {
        let strokes = vec![
            letter("a", 0, 0.0, 0.0),
            letter("b", 300, 18.0, 2.0),
            letter("c", 700, 60.0, 0.0),   // a word gap, same line
            letter("d", 1200, 0.0, 60.0),  // next line
            letter("e", 9000, 20.0, 60.0), // long pause
        ];
        let g = group_lines(&strokes);
        assert_eq!(g.len(), 3);
        assert_eq!(g[0].strokes, ["a", "b", "c"]);
        assert_eq!(g[0].id, "a");
        assert_eq!(g[1].strokes, ["d"]);
        assert_eq!(g[2].strokes, ["e"]);
        assert_eq!(g[0].bbox.x, 0.0);
        assert_eq!(g[0].bbox.right(), 74.0);
        assert_eq!(settled(&g, 9100).map(|g| g.id.as_str()).collect::<Vec<_>>(), ["a", "d"]);
    }

    #[test]
    fn order_comes_from_time_not_from_input() {
        let strokes = vec![letter("b", 300, 18.0, 2.0), letter("a", 0, 0.0, 0.0)];
        assert_eq!(group_lines(&strokes)[0].strokes, ["a", "b"]);
        assert!(group_lines(&[]).is_empty());
    }
}
