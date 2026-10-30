//! Journey map rendering and collision-aware map geometry.

use super::*;

pub(super) const PAN_ZOOM_JS: &str = include_str!("../../assets/journey.js");
pub(super) const INDONESIA_SVG: &str = include_str!("../../assets/journey/indonesia.svg");
pub(super) const JAVA_SVG: &str = include_str!("../../assets/journey/java.svg");
/// The `d` of the generated `<path class="{class}">`.
pub(super) fn land_path(svg: &'static str, class: &str) -> &'static str {
    let Some(at) = svg.find(&format!("class=\"{class}\"")) else { return "" };
    let rest = &svg[at..];
    let Some(start) = rest.find(" d=\"").map(|i| i + 4) else { return "" };
    let end = rest[start..].find('"').map_or(start, |i| start + i);
    &rest[start..end]
}
// ------------------------------------------------------- tablet & Mac ----

#[component]
pub(super) fn MapView(j: Journey, picked: Signal<Option<Option<String>>>) -> Element {
    let ui = use_context::<Ui>();
    let layout = LESSONS_VIEW.pick(&use_context::<crate::Db>()).layout;
    let cur = j.curriculum();
    let map = j.map();
    let progress = j.progress();
    let elsewhere = j.done_elsewhere();
    let initial = progress
        .current()
        .and_then(|c| cur.levels.iter().find(|l| l.lessons().any(|x| x.id == c)))
        .or(cur.levels.first())
        .map(|l| l.id.clone());
    let mut picked = picked;
    let chosen = picked.read().clone().unwrap_or_else(|| initial.clone());
    let stage_id = format!("jm-stage-{}", j.route.target_lang());
    // Open on the whole journey, every stop in view (#307, #308); Reset
    // comes back to the same view.
    let (x0, y0, x1, y1) =
        stops_box(map).unwrap_or((map.view.x, map.view.y, map.view.x + map.view.w, map.view.y + map.view.h));
    let script = format!(
        "{PAN_ZOOM_JS}({stage_id:?}, [{x0:.3}, {y0:.3}, {x1:.3}, {y1:.3}], {HOME_PAD}, {HOME_MAX_ZOOM});"
    );
    let zoom = {
        let stage_id = stage_id.clone();
        move |how: &str| {
            document::eval(&format!("document.getElementById({stage_id:?})?.__kz?.{how};"));
        }
    };
    let (zoom_in, zoom_out, zoom_reset) = (zoom.clone(), zoom.clone(), zoom);
    let map_label = match j.route {
        Route::Indonesian => Msg::JourneyMapOfIndonesia,
        Route::Javanese => Msg::JourneyMapOfJava,
    };
    let base = j.route.base_map();
    let (land_id, land_other) = (land_path(base, "land-id"), land_path(base, "land-other"));
    // Pins are drawn in a small design space, scaled so they keep the same
    // on-screen size on both maps (the JS counter-scales them while zooming).
    let k = pin_scale(map);
    let dot = map.radius * 0.22;
    let current_level = chosen.clone();
    let panel_level = current_level.as_deref().and_then(|id| cur.level(id));
    let spots = map_label_spots(cur, map);
    let rail = map.style == RouteStyle::Rail;
    let stickers = sticker_spots(&design_pins(cur, map), map_bounds(map));

    rsx! {
        div { class: "jm", "data-panel": if panel_level.is_some() { "open" } else { "closed" },
            div { class: "jm-map",
                div {
                    id: "{stage_id}",
                    class: "jm-stage",
                    style: "aspect-ratio: {map.view.w:.1} / {map.view.h:.1}",
                    tabindex: "0",
                    aria_label: "{ui.t(map_label)}",
                    onmounted: move |_| {
                        document::eval(&script);
                    },
                    svg {
                        view_box: "{map.view.attr()}",
                        preserve_aspect_ratio: "xMidYMid slice",
                        role: "group",
                        "aria-label": "{ui.t(map_label)}",
                        rect {
                            class: "sea",
                            x: "{map.view.x - map.view.w}",
                            y: "{map.view.y - map.view.h}",
                            width: "{map.view.w * 3.0}",
                            height: "{map.view.h * 3.0}",
                        }
                        g { class: "jm-viewport",
                            path { class: "land-other", "fill-rule": "evenodd", d: "{land_other}" }
                            path { class: "land-id", "fill-rule": "evenodd", d: "{land_id}" }
                            g { class: if rail { "jm-route rail" } else { "jm-route islands" }, "aria-hidden": "true",
                                for (i, seg) in map.segments.iter().enumerate() {
                                    g { key: "{i}",
                                    if seg.kind != SegmentKind::Flight {
                                        path { class: "rail-bed", d: "{seg.path_d()}" }
                                    }
                                    path {
                                        class: match seg.kind {
                                            SegmentKind::Flight => "seg flight",
                                            SegmentKind::Rail => "seg rail",
                                            SegmentKind::Branch => "seg branch",
                                        },
                                        d: "{seg.path_d()}",
                                    }
                                    }
                                }
                            }
                            g { class: "jm-stickers",
                                for ((level, pin), spot) in cur.levels.iter().zip(map.pins.iter()).zip(stickers.iter().copied()) {
                                    if let (Some((sx, sy)), Some(src)) = (spot, map_art::vignette(&level.id, ArtSize::Small)) {
                                        {
                                            let id = level.id.clone();
                                            let is_sel = current_level.as_deref() == Some(level.id.as_str());
                                            rsx! {
                                                // Moves with its pin when Bali and Lombok are spread apart.
                                                g { key: "{level.id}", class: "jm-nudge", style: nudge_style(pin),
                                                    g {
                                                        class: "jm-sticker-at",
                                                        transform: "translate({pin.x} {pin.y}) scale({k})",
                                                        g {
                                                            class: "jm-sticker",
                                                            "aria-hidden": "true",
                                                            "data-selected": if is_sel { "true" } else { "false" },
                                                            onclick: move |_| picked.set(Some(Some(id.clone()))),
                                                            {
                                                                // A short leader from the pin's edge to the sticker's,
                                                                // so a sticker never reads as its neighbour's.
                                                                let len = sx.hypot(sy);
                                                                let (ux, uy) = (sx / len, sy / len);
                                                                let (a, b) = (PIN + 0.5, len - STICKER / 2.0 - 0.8);
                                                                rsx! {
                                                                    line {
                                                                        class: "sticker-leader",
                                                                        x1: "{ux * a}",
                                                                        y1: "{uy * a}",
                                                                        x2: "{ux * b}",
                                                                        y2: "{uy * b}",
                                                                    }
                                                                }
                                                            }
                                                            circle { class: "sticker-edge", cx: "{sx}", cy: "{sy}", r: "{STICKER / 2.0 + 0.8}" }
                                                            image {
                                                                href: "{src}",
                                                                x: "{sx - STICKER / 2.0}",
                                                                y: "{sy - STICKER / 2.0}",
                                                                width: "{STICKER}",
                                                                height: "{STICKER}",
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            for ((level, pin), spot) in cur.levels.iter().zip(map.pins.iter()).zip(spots.iter().copied()) {
                                {
                                    let (lx, ly, anchor) = spot.attrs();
                                    let (done, total) = progress.level_counts(level);
                                    let pct = (done * 100).checked_div(total).unwrap_or(0);
                                    let state = if progress.level_done(&level.id) {
                                        "done"
                                    } else if level.units.iter().any(|u| progress.holds_current(u)) {
                                        "current"
                                    } else {
                                        "open"
                                    };
                                    let is_sel = current_level.as_deref() == Some(level.id.as_str());
                                    let id = level.id.clone();
                                    let id2 = level.id.clone();
                                    let label = format!("{}, {}: {done}/{total}", level.place(), level.code);
                                    let short = short_place(level.place());
                                    rsx! {
                                        // A nudged pin moves with its stops, so they stay under it.
                                        g { key: "{level.id}", class: "jm-level jm-nudge", "data-state": state, style: nudge_style(pin),
                                            path { class: "unit-loop", d: "{pin.units_d()}" }
                                            for (u, (ux, uy)) in level.units.iter().zip(pin.units.iter()) {
                                                circle {
                                                    key: "{u.id}",
                                                    class: if progress.unit_done(u) { "unit-dot done" } else { "unit-dot" },
                                                    cx: "{ux}",
                                                    cy: "{uy}",
                                                    r: "{dot}",
                                                }
                                            }
                                            g {
                                                class: "jm-pin-at",
                                                transform: "translate({pin.x} {pin.y}) scale({k})",
                                                g {
                                                    class: "jm-pin",
                                                    tabindex: "0",
                                                    role: "button",
                                                    "aria-label": "{label}",
                                                    "aria-pressed": if is_sel { "true" } else { "false" },
                                                    onclick: move |_| picked.set(Some(Some(id.clone()))),
                                                    onkeydown: move |e: KeyboardEvent| {
                                                        let key = e.key();
                                                        if key == Key::Enter || key == Key::Character(" ".into()) {
                                                            e.prevent_default();
                                                            picked.set(Some(Some(id2.clone())));
                                                        }
                                                    },
                                                    circle { class: "pin-hit", r: "8" }
                                                    circle { class: "pin-disc", r: "3.6" }
                                                    if pct > 0 {
                                                        circle {
                                                            class: "pin-ring",
                                                            r: "4.6",
                                                            "pathLength": "100",
                                                            "stroke-dasharray": "{pct} 100",
                                                            transform: "rotate(-90)",
                                                        }
                                                    }
                                                    text {
                                                        class: "pin-label",
                                                        x: "{lx}",
                                                        y: "{ly}",
                                                        "text-anchor": anchor,
                                                        direction: "ltr",
                                                        lang: j.route.target_lang(),
                                                        "{short}"
                                                    }
                                                    text { class: "pin-code", y: "1.25", "text-anchor": "middle", "{level.code.replace(\"BIPA \", \"\")}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // Under the map, never over it, so no zoom button sits on a pin
                // or a landmark in either reading direction.
                div { class: "jm-controls", role: "group",
                    button { aria_label: "{ui.t(Msg::JourneyZoomIn)}", title: "{ui.t(Msg::JourneyZoomIn)}", onclick: move |_| zoom_in("zoom(1.5)"), "+" }
                    button { aria_label: "{ui.t(Msg::JourneyZoomOut)}", title: "{ui.t(Msg::JourneyZoomOut)}", onclick: move |_| zoom_out("zoom(1 / 1.5)"), "−" }
                    button { aria_label: "{ui.t(Msg::JourneyZoomReset)}", title: "{ui.t(Msg::JourneyZoomReset)}", onclick: move |_| zoom_reset("reset()"), {icon_reset()} }
                }
                p { class: "muted jm-hint", "{ui.t(Msg::JourneyMapHint)}" }
                {continue_bar(&ui, &j, &progress)}
            }
            if let Some(level) = panel_level {
                aside { class: "jm-panel", aria_label: "{level.place()}",
                    div { class: "jm-panel-close",
                        button { aria_label: "{ui.t(Msg::JourneyClose)}", title: "{ui.t(Msg::JourneyClose)}", onclick: move |_| picked.set(Some(None)), "×" }
                    }
                    {level_head(&ui, &j, level, &progress, &elsewhere)}
                    crate::views::ViewBar { of: LESSONS_VIEW }
                    {unit_stops(&ui, &j, level, &progress, false, layout)}
                }
            }
        }
    }
}
/// Map units per pin design unit: pins and labels are drawn at this scale
/// so they keep the same size on both maps.
pub(super) fn pin_scale(map: &MapLayout) -> f64 {
    map.view.w / 200.0
}
/// The CSS that draws a nudged pin (Bali, Lombok) beside its real place at
/// the default zoom and slides it home as the learner zooms in, following
/// [`LevelPin::drawn_at`]: the offset is `nudge * max(0, (fade / s - 1) /
/// (fade - 1))`, and the JS keeps `--inv` at `1 / s`. Empty for a pin with
/// room around it.
pub(super) fn nudge_style(pin: &LevelPin) -> String {
    if pin.nudge == (0.0, 0.0) || pin.fade_zoom <= 1.0 {
        return String::new();
    }
    let per = pin.fade_zoom - 1.0;
    format!(
        "--fade: {:.4}; --nx: {:.3}px; --ny: {:.3}px",
        pin.fade_zoom,
        pin.nudge.0 / per,
        pin.nudge.1 / per
    )
}
/// Label spots for a whole map, from where the pins are drawn at the
/// default zoom, kept inside the map's edges.
pub(super) fn map_label_spots(cur: &Curriculum, map: &MapLayout) -> Vec<LabelSpot> {
    label_spots(&design_pins(cur, map), map_bounds(map))
}
/// "Bali (Ubud)" → "Bali": the name drawn next to a map pin.
pub(super) fn short_place(place: &str) -> String {
    place.split([',', '(']).next().unwrap_or_default().trim().to_string()
}
/// Where a pin's name goes. Above by default, clear of the pin's selection
/// halo; beside or below it when that would cover a neighbouring pin or its
/// name (Bali and Lombok sit almost on top of each other).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LabelSpot {
    Above,
    Below,
    East,
    West,
}
/// Pin design units (see `k` in the map): the selection halo's radius, the
/// pin's own radius with its progress ring, and the label's font metrics.
pub(super) const HALO: f64 = 8.0;
pub(super) const PIN: f64 = 5.0;
/// The closest two pins may sit at the default zoom: clear of each other's
/// disc and progress ring, with the selection halo just touching.
pub(super) const PIN_GAP: f64 = HALO + 4.0;
pub(super) const CHAR_W: f64 = 2.4;
pub(super) const ASCENT: f64 = 3.2;
pub(super) const DESCENT: f64 = 1.0;
impl LabelSpot {
    /// (x, y, text-anchor) relative to the pin, in pin design units.
    pub(super) fn attrs(self) -> (f64, f64, &'static str) {
        match self {
            Self::Above => (0.0, -(HALO + 1.6), "middle"),
            Self::Below => (0.0, HALO + 1.4 + ASCENT, "middle"),
            Self::East => (HALO + 1.4, 1.2, "start"),
            Self::West => (-(HALO + 1.4), 1.2, "end"),
        }
    }

    /// The label's box (x0, y0, x1, y1) for a pin at (x, y).
    pub(super) fn rect(self, x: f64, y: f64, chars: usize) -> (f64, f64, f64, f64) {
        let (dx, dy, anchor) = self.attrs();
        let w = chars as f64 * CHAR_W;
        let x0 = match anchor {
            "middle" => x + dx - w / 2.0,
            "start" => x + dx,
            _ => x + dx - w,
        };
        (x0, y + dy - ASCENT, x0 + w, y + dy + DESCENT)
    }
}
pub(super) fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}
/// Every pin as (x, y, label length) in pin design units. Laid out at the
/// default zoom, the most crowded the stops get on a map-shaped stage:
/// zooming in only spreads the places under the pins.
pub(super) fn design_pins(cur: &Curriculum, map: &MapLayout) -> Vec<(f64, f64, usize)> {
    let k = pin_scale(map);
    cur.levels
        .iter()
        .zip(map.pins.iter())
        .map(|(level, pin)| {
            // Where the pin is drawn at default zoom (Bali and Lombok spread apart).
            let (x, y) = pin.drawn_at(1.0);
            (x / k, y / k, short_place(level.place()).chars().count())
        })
        .collect()
}
/// The map's view box (x0, y0, x1, y1) in pin design units.
pub(super) fn map_bounds(map: &MapLayout) -> (f64, f64, f64, f64) {
    let k = pin_scale(map);
    let v = &map.view;
    (v.x / k, v.y / k, (v.x + v.w) / k, (v.y + v.h) / k)
}
/// The view a map opens at, and Reset returns to, fits every stop with this
/// much room around them (pin design units: a landmark sticker's reach and a
/// name), zoomed in no further than `HOME_MAX_ZOOM` (journey.js `home`).
pub(super) const HOME_PAD: f64 = HALO + STICKER + 6.0;
pub(super) const HOME_MAX_ZOOM: f64 = 4.0;
/// The box (x0, y0, x1, y1) in map units around every pin and unit stop.
pub(super) fn stops_box(map: &MapLayout) -> Option<(f64, f64, f64, f64)> {
    let points = map.pins.iter().flat_map(|p| std::iter::once((p.x, p.y)).chain(p.units.iter().copied()));
    points.fold(None, |b, (x, y)| {
        let (x0, y0, x1, y1) = b.unwrap_or((x, y, x, y));
        Some((x0.min(x), y0.min(y), x1.max(x), y1.max(y)))
    })
}
/// Diameter of a landmark sticker, in pin design units.
pub(super) const STICKER: f64 = 14.0;
/// Where each pin's landmark sticker goes, as the sticker's centre relative
/// to the pin: the first diagonal or side spot that stays clear of every
/// pin's halo, every name, the stickers already placed and the map's edge.
/// `None` when nothing fits; the art still shows in the level's header.
pub(super) fn sticker_spots(
    pins: &[(f64, f64, usize)],
    bounds: (f64, f64, f64, f64),
) -> Vec<Option<(f64, f64)>> {
    let labels: Vec<_> =
        pins.iter().zip(label_spots(pins, bounds)).map(|(&(x, y, n), s)| s.rect(x, y, n)).collect();
    let r = STICKER / 2.0;
    // Up-right first, then round the pin in eighths and sixteenths, a bit
    // farther out each time the closer rings are taken (Bali and Lombok
    // nearly share one spot).
    let spots: Vec<(f64, f64)> = (0..5)
        .map(|ring| HALO + r + 1.0 + f64::from(ring) * 5.0)
        .flat_map(|d| {
            [
                -45.0_f64, -135.0, 45.0, 135.0, 0.0, 180.0, 90.0, -90.0, -22.5, -157.5, 22.5, 157.5, -67.5,
                -112.5, 67.5, 112.5,
            ]
            .map(|a: f64| (d * a.to_radians().cos(), d * a.to_radians().sin()))
        })
        .collect();
    let mut placed: Vec<(f64, f64)> = Vec::new();
    pins.iter()
        .map(|&(x, y, _)| {
            let free = |&(dx, dy): &(f64, f64)| {
                let (cx, cy) = (x + dx, y + dy);
                let bx = (cx - r, cy - r, cx + r, cy + r);
                bx.0 >= bounds.0 + 1.0
                    && bx.1 >= bounds.1 + 1.0
                    && bx.2 <= bounds.2 - 1.0
                    && bx.3 <= bounds.3 - 1.0
                    && pins.iter().all(|&(px, py, _)| (cx - px).hypot(cy - py) >= HALO + r + 0.5)
                    && !labels.iter().any(|l| overlaps(bx, *l))
                    && placed.iter().all(|&(qx, qy)| (cx - qx).hypot(cy - qy) >= STICKER + 1.0)
            };
            let spot = spots.iter().copied().find(free);
            if let Some((dx, dy)) = spot {
                placed.push((x + dx, y + dy));
            }
            spot
        })
        .collect()
}
/// A label spot, its box, and its cost on its own (see `label_spots`).
pub(super) type SpotOption = (LabelSpot, (f64, f64, f64, f64), f64);
pub(super) fn inside(r: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    r.0 >= b.0 && r.1 >= b.1 && r.2 <= b.2 && r.3 <= b.3
}
/// Picks a spot per pin, given (x, y, label length) in pin design units and
/// the map's edges in the same units. Every name stays on the map and off
/// the other pins and names when that's possible at all; among such layouts
/// the one with the fewest names moved off "above" wins (below next, then
/// beside), and a tie goes to the names leaning toward the middle of the
/// route, where the map is in view (Bali's name west, Lombok's above, rather
/// than Lombok's pushed out east).
pub(super) fn label_spots(pins: &[(f64, f64, usize)], bounds: (f64, f64, f64, f64)) -> Vec<LabelSpot> {
    const ORDER: [LabelSpot; 4] = [LabelSpot::Above, LabelSpot::Below, LabelSpot::West, LabelSpot::East];
    const RANK: [f64; 4] = [0.0, 1.0, 2.0, 2.0];
    /// Far more than any choice of spots: a name off the map or on another
    /// pin or name only when nothing else fits.
    const CLASH: f64 = 100.0;
    if pins.is_empty() {
        return Vec::new();
    }
    let n = pins.len() as f64;
    let (mx, my) = pins.iter().fold((0.0, 0.0), |(a, b), &(x, y, _)| (a + x / n, b + y / n));
    let discs: Vec<_> = pins.iter().map(|&(x, y, _)| (x - PIN, y - PIN, x + PIN, y + PIN)).collect();
    // Each pin's four spots: (spot, label box, cost on its own).
    let options: Vec<Vec<SpotOption>> = pins
        .iter()
        .enumerate()
        .map(|(i, &(x, y, chars))| {
            let mut o: Vec<_> = ORDER
                .iter()
                .zip(RANK)
                .map(|(&spot, rank)| {
                    let r = spot.rect(x, y, chars);
                    let lean = ((r.0 + r.2) / 2.0 - mx).hypot((r.1 + r.3) / 2.0 - my) / 100.0;
                    let off = if inside(r, bounds) { 0.0 } else { CLASH };
                    let covers = discs.iter().enumerate().filter(|&(j, d)| j != i && overlaps(r, *d)).count();
                    (spot, r, rank + lean + off + CLASH * covers as f64)
                })
                .collect();
            o.sort_by(|a, b| a.2.total_cmp(&b.2));
            o
        })
        .collect();

    // Branch and bound over the pins in order (a map has a handful of pins;
    // the budget keeps a pathological input from taking long).
    struct Search<'a> {
        options: &'a [Vec<SpotOption>],
        pick: Vec<usize>,
        best: Vec<usize>,
        best_cost: f64,
        budget: u32,
    }
    impl Search<'_> {
        fn go(&mut self, i: usize, cost: f64) {
            if cost >= self.best_cost || self.budget == 0 {
                return;
            }
            self.budget -= 1;
            if i == self.options.len() {
                self.best_cost = cost;
                self.best.clone_from(&self.pick);
                return;
            }
            for k in 0..self.options[i].len() {
                let (_, r, own) = self.options[i][k];
                let clashes = (0..i).filter(|&j| overlaps(r, self.options[j][self.pick[j]].1)).count() as f64;
                self.pick.push(k);
                self.go(i + 1, cost + own + CLASH * clashes);
                self.pick.pop();
            }
        }
    }
    let mut search = Search {
        options: &options,
        pick: Vec::with_capacity(pins.len()),
        best: vec![0; pins.len()],
        best_cost: f64::INFINITY,
        budget: 200_000,
    };
    search.go(0, 0.0);
    search.best.iter().zip(&options).map(|(&k, o)| o[k].0).collect()
}
