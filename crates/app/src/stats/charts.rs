//! Inline SVG charts for Stats and the level check, built as strings so every
//! mark can carry an SVG `<title>` (the hover tooltip) and the whole chart a
//! `role="img"` with a label. Colours come from theme tokens in `stats.css`
//! (`--sawah`, `--indigo`, `--muted`, `--line`), never hex values here.
//!
//! Charts draw with `direction="ltr"` and mirror the time axis themselves in
//! RTL, so "newest" sits where an Arabic reader expects it (the left).
//! Every chart also has a text summary next to it in the screen.

use std::fmt::Write as _;

use kabar_core::stats::{HeatDay, Heatmap};
use kabar_protocol::coach::{LocalDate, Skill};
use kabar_protocol::level_check::RadarPoint;

const CELL: f32 = 12.0;
const STEP: f32 = 15.0;
const TOP: f32 = 16.0;

/// Escapes text for SVG content and attributes.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn open(view_w: f32, view_h: f32, class: &str, label: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="{class}" viewBox="0 0 {view_w} {view_h}" role="img" aria-label="{}" direction="ltr" preserveAspectRatio="xMidYMid meet"><title>{}</title>"#,
        esc(label),
        esc(label)
    )
}

/// Weeks as columns (Monday on top), newest column at the end of the reading
/// direction. `cell` labels each day; `month` labels the column where a month
/// starts.
pub fn heatmap(
    h: &Heatmap,
    rtl: bool,
    label: &str,
    cell: impl Fn(&HeatDay) -> String,
    month: impl Fn(LocalDate) -> String,
) -> String {
    let weeks = h.weeks().max(1);
    let w = weeks as f32 * STEP;
    let height = TOP + 7.0 * STEP;
    let mut s = open(w, height, "chart heatmap-svg", label);
    let col_x = |col: usize| {
        let c = if rtl { weeks - 1 - col } else { col };
        c as f32 * STEP
    };
    for (i, d) in h.days.iter().enumerate() {
        let (col, row) = (i / 7, i % 7);
        let (x, y) = (col_x(col), TOP + row as f32 * STEP);
        let _ = write!(
            s,
            r#"<rect class="hm hm-{}" x="{x}" y="{y}" width="{CELL}" height="{CELL}" rx="2"><title>{}</title></rect>"#,
            d.level,
            esc(&cell(d))
        );
        // A month label over the column holding its 1st day.
        if d.date.ymd().2 == 1 {
            let anchor = if rtl { "end" } else { "start" };
            let tx = if rtl { x + CELL } else { x };
            let _ = write!(
                s,
                r#"<text class="axis" x="{tx}" y="11" text-anchor="{anchor}">{}</text>"#,
                esc(&month(d.date))
            );
        }
    }
    s.push_str("</svg>");
    s
}

/// The heatmap legend's five swatches (less → more), as a tiny SVG.
pub fn heat_legend(rtl: bool) -> String {
    let mut s = String::from(
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="heat-legend" viewBox="0 0 75 12" aria-hidden="true" direction="ltr">"#,
    );
    for i in 0..5u8 {
        let level = if rtl { 4 - i } else { i };
        let _ = write!(
            s,
            r#"<rect class="hm hm-{level}" x="{}" y="0" width="{CELL}" height="{CELL}" rx="2"/>"#,
            f32::from(i) * STEP
        );
    }
    s.push_str("</svg>");
    s
}

/// Reviews due per day for the next `days.len()` days: thin bars from a
/// shared baseline, today first in the reading direction.
pub fn forecast(days: &[u32], rtl: bool, label: &str, bar: impl Fn(usize, u32) -> String) -> String {
    let n = days.len().max(1);
    let (bw, gap, h) = (8.0f32, 2.0f32, 96.0f32);
    let w = n as f32 * (bw + gap);
    let peak = days.iter().copied().max().unwrap_or(0).max(1) as f32;
    // Stretched to the card's width, so the bars line up with the "today" /
    // "in 30 days" labels under the chart at any width.
    let mut s = open(w, h + 2.0, "chart forecast-svg", label).replacen("xMidYMid meet", "none", 1);
    let _ = write!(s, r#"<line class="baseline" x1="0" x2="{w}" y1="{h}" y2="{h}"/>"#);
    for (i, &v) in days.iter().enumerate() {
        let slot = if rtl { n - 1 - i } else { i };
        let x = slot as f32 * (bw + gap) + gap / 2.0;
        let bh = if v == 0 { 0.0 } else { (v as f32 / peak * (h - 4.0)).max(2.0) };
        let class = if i == 0 { "fc today" } else { "fc" };
        // A transparent full-height hit target carries the tooltip.
        let _ = write!(
            s,
            r#"<g class="{class}"><title>{}</title><rect class="hit" x="{}" y="0" width="{}" height="{h}"/>"#,
            esc(&bar(i, v)),
            x - gap / 2.0,
            bw + gap
        );
        if v > 0 {
            let _ = write!(
                s,
                r#"<path class="bar" d="M{x} {h} V{y} q0 -2 2 -2 h{inner} q2 0 2 2 V{h} Z"/>"#,
                y = h - bh + 2.0,
                inner = bw - 4.0,
            );
        }
        s.push_str("</g>");
    }
    s.push_str("</svg>");
    s
}

const CX: f32 = 160.0;
const CY: f32 = 132.0;
const R: f32 = 92.0;

fn axis_point(i: usize, value: f32, rtl: bool) -> (f32, f32) {
    let angle = (-90.0 + 72.0 * i as f32).to_radians();
    let sign = if rtl { -1.0 } else { 1.0 };
    (CX + sign * R * value * angle.cos(), CY + R * value * angle.sin())
}

fn polygon(points: &[RadarPoint], rtl: bool) -> String {
    Skill::ALL
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let v = points.iter().find(|p| p.skill == *s).map_or(0.0, |p| p.value.clamp(0.0, 1.0));
            let (x, y) = axis_point(i, v, rtl);
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The 5-axis skill radar in [`Skill::ALL`] order, clockwise in the reading
/// direction from the top. `before` (last month) is a dashed outline under
/// this month's filled shape. Unmeasured axes sit at the centre and their
/// label says so.
pub fn radar(
    now: &[RadarPoint],
    before: Option<&[RadarPoint]>,
    rtl: bool,
    label: &str,
    axis_labels: &[String; 5],
    point_label: impl Fn(Skill, f32) -> String,
) -> String {
    let mut s = open(CX * 2.0, CY * 2.0 - 4.0, "chart radar-svg", label);
    for ring in [0.25f32, 0.5, 0.75, 1.0] {
        let pts: Vec<String> = (0..5)
            .map(|i| {
                let (x, y) = axis_point(i, ring, rtl);
                format!("{x:.1},{y:.1}")
            })
            .collect();
        let _ = write!(s, r#"<polygon class="grid" points="{}"/>"#, pts.join(" "));
    }
    for (i, axis_label) in axis_labels.iter().enumerate() {
        let (x, y) = axis_point(i, 1.0, rtl);
        let _ = write!(s, r#"<line class="grid" x1="{CX}" y1="{CY}" x2="{x:.1}" y2="{y:.1}"/>"#);
        let (lx, ly) = axis_point(i, 1.16, rtl);
        let anchor = if (lx - CX).abs() < 4.0 {
            "middle"
        } else if lx > CX {
            "start"
        } else {
            "end"
        };
        let dy = if ly < CY - R * 0.5 { -2.0 } else { 10.0 };
        let _ = write!(
            s,
            r#"<text class="axis" x="{lx:.1}" y="{:.1}" text-anchor="{anchor}">{}</text>"#,
            ly + dy - 4.0,
            esc(axis_label)
        );
    }
    if let Some(before) = before.filter(|b| !b.is_empty()) {
        let _ = write!(s, r#"<polygon class="before" points="{}"/>"#, polygon(before, rtl));
    }
    let _ = write!(s, r#"<polygon class="now" points="{}"/>"#, polygon(now, rtl));
    for (i, sk) in Skill::ALL.iter().enumerate() {
        if let Some(p) = now.iter().find(|p| p.skill == *sk) {
            let (x, y) = axis_point(i, p.value.clamp(0.0, 1.0), rtl);
            let _ = write!(
                s,
                r#"<g class="dot"><title>{}</title><circle class="hit" cx="{x:.1}" cy="{y:.1}" r="12"/><circle class="mark" cx="{x:.1}" cy="{y:.1}" r="4"/></g>"#,
                esc(&point_label(*sk, p.value))
            );
        }
    }
    s.push_str("</svg>");
    s
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use kabar_core::stats::{DayTotal, heatmap as build_heatmap};

    use super::*;

    fn d(y: i32, m: u32, day: u32) -> LocalDate {
        LocalDate::from_ymd(y, m, day).unwrap()
    }

    #[test]
    fn heatmap_has_a_cell_per_day_titles_and_month_labels() {
        let today = d(2026, 10, 26);
        let mut totals = BTreeMap::new();
        totals.insert(today, DayTotal { minutes: 20, reviews: 3, studied: true });
        let h = build_heatmap(&totals, today, 20);
        let svg =
            heatmap(&h, false, "Study <days>", |x| format!("{} {}", x.date, x.minutes), |m| m.to_string());
        assert_eq!(svg.matches("<rect").count(), h.days.len());
        assert!(svg.contains(r#"class="hm hm-3""#));
        assert!(svg.contains("<title>2026-10-26 20</title>"));
        assert!(svg.contains("Study &lt;days&gt;"), "labels are escaped");
        assert!(svg.contains(">2026-10-01</text>"));
        // RTL mirrors the columns: today's column moves to x = 0.
        let rtl = heatmap(&h, true, "", |_| String::new(), |_| String::new());
        let last = rtl.rfind("<rect").unwrap();
        assert!(rtl[last..].contains(r#"x="0""#));
    }

    #[test]
    fn forecast_bars_and_tooltips() {
        let svg = forecast(&[0, 4, 2], false, "Reviews", |i, n| format!("day {i}: {n}"));
        assert_eq!(svg.matches(r#"class="bar""#).count(), 2, "no bar for an empty day");
        assert_eq!(svg.matches("<title>day ").count(), 3, "every day has a tooltip");
        assert!(svg.contains(r#"<g class="fc today">"#));
    }

    #[test]
    fn radar_draws_rings_axes_and_both_months() {
        let labels = Skill::ALL.map(|s| format!("{s:?}"));
        let now = vec![RadarPoint { skill: Skill::Reading, value: 0.8 }];
        let before = vec![RadarPoint { skill: Skill::Reading, value: 0.6 }];
        let svg = radar(&now, Some(&before), false, "Skill map", &labels, |s, v| format!("{s:?} {v}"));
        assert_eq!(svg.matches(r#"<polygon class="grid""#).count(), 4);
        assert!(svg.contains(r#"class="before""#) && svg.contains(r#"class="now""#));
        assert_eq!(svg.matches(r#"class="mark""#).count(), 1, "only measured axes get a dot");
        assert!(svg.contains(">Vocabulary</text>"));
        // Reading at the top: x is the centre in both directions.
        assert!(svg.contains(&format!(r#"cx="{CX:.1}""#)));
    }
}
