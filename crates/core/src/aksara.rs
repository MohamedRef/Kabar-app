//! Aksara Jawa calligraphy (PLAN §18): the practice set with its stroke
//! order (`content/aksara/strokes.json`) and the offline shape check that
//! runs on every attempt before (or instead of) the `check_strokes` task.
//!
//! The check aligns the learner's ink with the reference by a small search
//! over scale and anchoring, then compares both ways: how much of each
//! reference stroke the ink covers, and how much ink lies off the letter.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// One practice item: a letter, a syllable with a sign, or a word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glyph {
    pub id: String,
    /// The aksara, as typed (`ꦏꦶ`).
    pub text: String,
    pub latin: String,
    pub kind: GlyphKind,
    /// The sandhangan a syllable practises (`wulu`, `cecak`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign: Option<String>,
    /// Line width in 1/1000 em.
    pub width: i32,
    /// Strokes in writing order; points in 1/1000 em from the pen origin,
    /// y down, baseline at 0.
    pub strokes: Vec<Vec<[i32; 2]>>,
    /// The stroke order was checked by a person.
    #[serde(default)]
    pub verified: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlyphKind {
    /// One of the 20 base letters.
    Nglegena,
    /// A vowel sign on a letter.
    Sandhangan,
    /// A final sign (cecak, layar, wignyan, pangkon).
    Panyigeg,
    Word,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AksaraSet {
    pub schema: String,
    pub glyphs: Vec<Glyph>,
}

impl AksaraSet {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn get(&self, id: &str) -> Option<&Glyph> {
        self.glyphs.iter().find(|g| g.id == id)
    }
}

impl Glyph {
    /// Bounding box `[x0, y0, x1, y1]` of the strokes, 1/1000 em.
    pub fn bbox(&self) -> [i32; 4] {
        let mut b = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
        for p in self.strokes.iter().flatten() {
            b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
        }
        b
    }

    /// Total length of each stroke, for pacing the stroke-order animation.
    pub fn stroke_lengths(&self) -> Vec<f32> {
        self.strokes.iter().map(|s| s.windows(2).map(|w| dist(pt(w[0]), pt(w[1]))).sum()).collect()
    }
}

/// A point in any unit; the check normalizes.
pub type P = (f32, f32);

fn pt(p: [i32; 2]) -> P {
    (p[0] as f32, p[1] as f32)
}

fn dist(a: P, b: P) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Points along a polyline, one every `step`.
fn resample(line: &[P], step: f32) -> Vec<P> {
    let mut out = Vec::new();
    let Some(&first) = line.first() else { return out };
    out.push(first);
    let mut carry = 0.0;
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = dist(a, b);
        if len == 0.0 {
            continue;
        }
        let mut t = step - carry;
        while t <= len {
            out.push((a.0 + (b.0 - a.0) * t / len, a.1 + (b.1 - a.1) * t / len));
            t += step;
        }
        carry = len - (t - step);
    }
    if let Some(&last) = line.last()
        && out.last().is_some_and(|&l| dist(l, last) > step * 0.3)
    {
        out.push(last);
    }
    out
}

fn bbox(points: &[P]) -> Option<(P, P)> {
    let first = *points.first()?;
    Some(points.iter().fold((first, first), |(lo, hi), &p| {
        ((lo.0.min(p.0), lo.1.min(p.1)), (hi.0.max(p.0), hi.1.max(p.1)))
    }))
}

/// Where on the letter something is, as a teacher would point at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    Left,
    Middle,
    Right,
    Above,
    Below,
}

impl Part {
    fn en(self) -> &'static str {
        match self {
            Self::Left => "left-hand part",
            Self::Middle => "middle",
            Self::Right => "right-hand part",
            Self::Above => "mark above",
            Self::Below => "part below the line",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Issue {
    /// Reference stroke `stroke` (from 0) is mostly not there.
    Missing { stroke: usize, part: Part },
    /// Ink where the letter has none.
    Extra { part: Part },
    /// The letter is drawn mirrored left to right.
    Mirrored,
    /// Another practice item fits the ink clearly better.
    ReadsAs { id: String, text: String, latin: String },
    /// Nothing written.
    Empty,
}

impl Issue {
    /// A short English note, in the golden set's style.
    pub fn note(&self, target: &Glyph) -> String {
        match self {
            Self::Missing { stroke, part } => {
                format!("Stroke {} of {} (the {}) is missing.", stroke + 1, target.latin, part.en())
            }
            Self::Extra { part } => {
                format!("There is extra ink at the {} that {} doesn't have.", part.en(), target.latin)
            }
            Self::Mirrored => {
                format!("{} is mirrored left-to-right; the opening should face the other way.", target.latin)
            }
            Self::ReadsAs { text, latin, .. } => {
                format!("This reads as {latin} ({text}), not {} ({}).", target.latin, target.text)
            }
            Self::Empty => "Nothing written yet.".into(),
        }
    }
}

/// The offline verdict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapeReport {
    pub correct: bool,
    /// 0–1: how well the ink matches, both ways.
    pub score: f32,
    pub issues: Vec<Issue>,
}

/// How far (as a share of the letter's height) a point may sit from the
/// other drawing and still count as on it.
const NEAR: f32 = 0.11;
/// A pass needs this much overlap both ways.
const PASS: f32 = 0.8;

/// Points bucketed by `cell` so "is anything within `cell` of p" looks at
/// nine buckets instead of every point.
struct Grid {
    cell: f32,
    buckets: HashMap<(i32, i32), Vec<P>>,
}

impl Grid {
    fn new(points: impl IntoIterator<Item = P>, cell: f32) -> Self {
        let mut buckets: HashMap<(i32, i32), Vec<P>> = HashMap::new();
        for p in points {
            buckets.entry(Self::key(p, cell)).or_default().push(p);
        }
        Self { cell, buckets }
    }

    fn key(p: P, cell: f32) -> (i32, i32) {
        ((p.0 / cell).floor() as i32, (p.1 / cell).floor() as i32)
    }

    fn near(&self, p: P) -> bool {
        let (kx, ky) = Self::key(p, self.cell);
        (kx - 1..=kx + 1).any(|x| {
            (ky - 1..=ky + 1).any(|y| {
                self.buckets.get(&(x, y)).is_some_and(|b| b.iter().any(|&q| dist(p, q) <= self.cell))
            })
        })
    }
}

struct Fit {
    score: f32,
    /// Per reference stroke: share of its points the ink covers.
    covered: Vec<f32>,
    /// Ink points far from the letter, in reference coordinates.
    stray: Vec<P>,
    ink_points: usize,
}

/// Compares learner points (any unit, y down) with a reference glyph.
fn fit(reference: &[Vec<P>], ink: &[P]) -> Option<Fit> {
    fit_with(reference, ink, true)
}

/// `full: false` tries only the plain fit (whole-ink scale, centered): a
/// quick screen before the full search for rival letters.
fn fit_with(reference: &[Vec<P>], ink: &[P], full: bool) -> Option<Fit> {
    let all: Vec<P> = reference.iter().flatten().copied().collect();
    let ((rx0, ry0), (rx1, ry1)) = bbox(&all)?;
    let ((ix0, iy0), (ix1, iy1)) = bbox(ink)?;
    let (rw, rh) = ((rx1 - rx0).max(1.0), (ry1 - ry0).max(1.0));
    let (iw, ih) = ((ix1 - ix0).max(1.0), (iy1 - iy0).max(1.0));
    let unit = rh.max(rw * 0.5);
    let near = NEAR * unit;
    let step = unit / 30.0;
    let ref_pts: Vec<Vec<P>> = reference.iter().map(|s| resample(s, step)).collect();
    let ref_grid = Grid::new(ref_pts.iter().flatten().copied(), near);

    // Scale from the size of the ink to the size of the letter, then try a
    // few anchorings so a missing first or last part doesn't skew the rest.
    let base = (rh / ih).min(rw / iw).max(f32::MIN_POSITIVE);
    let base_h = rh / ih;
    let base_w = rw / iw;
    let mut best: Option<Fit> = None;
    let scales: &[f32] = if full { &[base, base_h, base_w, base * 0.9, base * 1.1] } else { &[base] };
    let anchors: &[f32] = if full { &[0.0, 0.5, 1.0] } else { &[0.5] };
    for &scale in scales {
        if !scale.is_finite() || scale <= 0.0 {
            continue;
        }
        let (sw, sh) = (iw * scale, ih * scale);
        for &ax in anchors {
            for &ay in anchors {
                let ox = rx0 + (rw - sw) * ax - ix0 * scale;
                let oy = ry0 + (rh - sh) * ay - iy0 * scale;
                let mapped: Vec<P> = ink.iter().map(|&(x, y)| (x * scale + ox, y * scale + oy)).collect();
                let ink_grid = Grid::new(mapped.iter().copied(), near);
                let covered: Vec<f32> = ref_pts
                    .iter()
                    .map(|s| {
                        if s.is_empty() {
                            1.0
                        } else {
                            s.iter().filter(|&&p| ink_grid.near(p)).count() as f32 / s.len() as f32
                        }
                    })
                    .collect();
                let stray: Vec<P> = mapped.iter().copied().filter(|&p| !ref_grid.near(p)).collect();
                let total: usize = ref_pts.iter().map(Vec::len).sum();
                let recall = ref_pts.iter().zip(&covered).map(|(s, c)| s.len() as f32 * c).sum::<f32>()
                    / total.max(1) as f32;
                let precision = 1.0 - stray.len() as f32 / mapped.len().max(1) as f32;
                let score = if recall + precision > 0.0 {
                    2.0 * recall * precision / (recall + precision)
                } else {
                    0.0
                };
                if best.as_ref().is_none_or(|b| score > b.score) {
                    best = Some(Fit { score, covered, stray, ink_points: mapped.len() });
                }
            }
        }
    }
    best
}

fn part_of(p: P, reference: &[Vec<P>]) -> Part {
    let all: Vec<P> = reference.iter().flatten().copied().collect();
    let Some(((x0, _), (x1, _))) = bbox(&all) else { return Part::Middle };
    // Above the letter body (marks) or under the baseline.
    let body_top = -620.0;
    if p.1 < body_top {
        return Part::Above;
    }
    if p.1 > 120.0 {
        return Part::Below;
    }
    let t = (p.0 - x0) / (x1 - x0).max(1.0);
    if t < 0.36 {
        Part::Left
    } else if t > 0.64 {
        Part::Right
    } else {
        Part::Middle
    }
}

fn reference(g: &Glyph) -> Vec<Vec<P>> {
    g.strokes.iter().map(|s| s.iter().map(|&p| pt(p)).collect()).collect()
}

fn mirrored(g: &Glyph) -> Vec<Vec<P>> {
    g.strokes.iter().map(|s| s.iter().map(|&p| (-(p[0] as f32), p[1] as f32)).collect()).collect()
}

/// Checks ink (strokes of points, any unit with y down) against `target`.
/// `set` supplies the other items for "this reads as …".
pub fn check(target: &Glyph, ink: &[Vec<P>], set: &AksaraSet) -> ShapeReport {
    let points: Vec<P> = ink.iter().flat_map(|s| resample_any(s)).collect();
    let refs = reference(target);
    let Some(main) = fit(&refs, &points) else {
        return ShapeReport { correct: false, score: 0.0, issues: vec![Issue::Empty] };
    };
    let mut issues = Vec::new();

    // Another letter of the same kind that fits clearly better. Words are
    // judged stroke by stroke instead: one wrong letter shows as missing ink.
    let rival = set
        .glyphs
        .iter()
        // A rival must beat the main fit by 0.06, so a near-perfect fit has none.
        .filter(|_| target.kind != GlyphKind::Word && main.score < 0.94)
        .filter(|g| g.id != target.id && g.kind == target.kind && g.text != target.text)
        .filter_map(|g| {
            let r = reference(g);
            // Most letters are nowhere near: skip the full search for them.
            fit_with(&r, &points, false).filter(|f| f.score >= PASS - 0.2)?;
            fit(&r, &points).map(|f| (g, f.score))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((g, s)) = rival
        && s >= PASS
        && s > main.score + 0.06
    {
        issues.push(Issue::ReadsAs { id: g.id.clone(), text: g.text.clone(), latin: g.latin.clone() });
    } else if main.score < 0.92
        && let Some(m) = fit(&mirrored(target), &points)
        && m.score >= PASS
        && m.score > main.score + 0.08
    {
        issues.push(Issue::Mirrored);
    } else {
        for (i, c) in main.covered.iter().enumerate() {
            if *c < 0.55 {
                let s = &refs[i];
                let mid = s[s.len() / 2];
                issues.push(Issue::Missing { stroke: i, part: part_of(mid, &refs) });
            }
        }
        // Stray ink worth mentioning: a real share of what was written.
        if main.stray.len() as f32 > main.ink_points as f32 * 0.14 {
            let n = main.stray.len() as f32;
            let c = main.stray.iter().fold((0.0, 0.0), |acc, p| (acc.0 + p.0 / n, acc.1 + p.1 / n));
            issues.push(Issue::Extra { part: part_of(c, &refs) });
        }
    }
    let correct = issues.is_empty() && main.score >= PASS;
    ShapeReport { correct, score: main.score, issues }
}

/// Ink strokes resampled densely enough to compare (their unit is unknown,
/// so the step comes from the ink's own size).
fn resample_any(line: &[P]) -> Vec<P> {
    let Some(((x0, y0), (x1, y1))) = bbox(line) else { return Vec::new() };
    let size = (x1 - x0).max(y1 - y0).max(1.0);
    if line.len() == 1 {
        return line.to_vec();
    }
    resample(line, (size / 40.0).max(0.5))
}

/// The teacher's answer (`prompts/check_strokes@v1.md`), shaped like the
/// golden set: `{correct, issues[]}`.
pub use crate::ai::tasks::CheckStrokes as StrokeCheck;

/// The `check_strokes` request for a sheet uploaded as `image_sha256`, in
/// the golden set's shape. The learner's handwriting: personal.
pub fn request(target: &Glyph, image_sha256: &str) -> crate::ai::AiRequest {
    crate::ai::AiRequest {
        input: serde_json::json!({ "target": target.text, "script": "aksara_jawa" }),
        personal: true,
        image_sha256: Some(image_sha256.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AiOutput;

    fn set() -> AksaraSet {
        AksaraSet::parse(include_str!("../../../content/aksara/strokes.json")).unwrap()
    }

    /// A glyph's own strokes as "ink": scaled to page pixels, shifted, with
    /// a little deterministic wobble.
    fn ink(g: &Glyph, wobble: f32) -> Vec<Vec<P>> {
        g.strokes
            .iter()
            .enumerate()
            .map(|(i, s)| {
                s.iter()
                    .enumerate()
                    .map(|(j, p)| {
                        let w = ((i * 7 + j * 13) % 5) as f32 - 2.0;
                        (p[0] as f32 * 0.31 + 120.0 + w * wobble, p[1] as f32 * 0.31 + 400.0 - w * wobble)
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_set_is_complete_and_well_formed() {
        let s = set();
        assert_eq!(s.glyphs.iter().filter(|g| g.kind == GlyphKind::Nglegena).count(), 20);
        for g in &s.glyphs {
            assert!(!g.strokes.is_empty() && g.strokes.iter().all(|st| st.len() >= 2), "{}", g.id);
            assert!(g.width > 0 && !g.verified, "{} must stay unverified until reviewed", g.id);
            assert!(g.stroke_lengths().iter().all(|&l| l > 0.0));
        }
        let ids: std::collections::BTreeSet<_> = s.glyphs.iter().map(|g| &g.id).collect();
        assert_eq!(ids.len(), s.glyphs.len());
    }

    #[test]
    fn every_letter_passes_against_itself_with_a_wobble() {
        let s = set();
        for g in s.glyphs.iter().filter(|g| g.kind != GlyphKind::Word) {
            let r = check(g, &ink(g, 2.0), &s);
            assert!(r.correct, "{}: {r:?}", g.id);
        }
    }

    #[test]
    fn a_missing_last_stroke_is_found_and_located() {
        let s = set();
        let ha = s.get("ha").unwrap();
        let mut partial = ink(ha, 0.0);
        partial.pop();
        let r = check(ha, &partial, &s);
        assert!(!r.correct);
        let last = ha.strokes.len() - 1;
        assert!(
            r.issues.iter().any(|i| matches!(i, Issue::Missing { stroke, .. } if *stroke == last)),
            "{r:?}"
        );
    }

    #[test]
    fn mirrored_and_other_letters_are_named() {
        let s = set();
        let ka = s.get("ka").unwrap();
        let flipped: Vec<Vec<P>> =
            ink(ka, 0.0).into_iter().map(|st| st.into_iter().map(|(x, y)| (-x, y)).collect()).collect();
        let r = check(ka, &flipped, &s);
        assert!(!r.correct);
        assert!(r.issues.contains(&Issue::Mirrored) || matches!(r.issues[0], Issue::ReadsAs { .. }), "{r:?}");

        let ga = s.get("ga").unwrap();
        let r = check(ka, &ink(ga, 1.0), &s);
        assert!(!r.correct);
        assert!(r.issues.iter().any(|i| matches!(i, Issue::ReadsAs { id, .. } if id == "ga")), "{r:?}");
        assert_eq!(r.issues[0].note(ka), "This reads as ga (ꦒ), not ka (ꦏ).");

        let da = s.get("da").unwrap();
        let r = check(da, &ink(s.get("dha").unwrap(), 1.0), &s);
        assert!(r.issues.iter().any(|i| matches!(i, Issue::ReadsAs { id, .. } if id == "dha")), "{r:?}");
    }

    #[test]
    fn scribbles_and_empty_sheets_fail_kindly() {
        let s = set();
        let ha = s.get("ha").unwrap();
        assert_eq!(check(ha, &[], &s).issues, [Issue::Empty]);
        let zigzag: Vec<Vec<P>> =
            vec![(0..40).map(|i| (i as f32 * 5.0, if i % 2 == 0 { 0.0 } else { 80.0 })).collect()];
        assert!(!check(ha, &zigzag, &s).correct);
    }

    #[test]
    fn stroke_check_validation_and_request() {
        let ok = StrokeCheck { correct: true, issues: vec![] };
        assert!(ok.validate().is_ok());
        assert!(StrokeCheck { correct: false, issues: vec![] }.validate().is_err());
        let s = set();
        let req = request(s.get("ka").unwrap(), "ab");
        assert!(req.personal);
        assert_eq!(req.input["target"], "ꦏ");
    }
}
