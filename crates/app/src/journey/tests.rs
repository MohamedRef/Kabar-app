//! Existing behavior regressions for the feature facade.

use super::*;

#[test]
fn curricula_parse_and_lay_out() {
    let r = ROUTES.as_ref().unwrap();
    assert_eq!(r.id.levels.len(), 7);
    assert_eq!(r.jv.levels.len(), 6);
    assert_eq!(r.id_map.style, RouteStyle::Islands);
    assert_eq!(r.jv_map.style, RouteStyle::Rail);
    assert!(r.jv.unverified());
    // J5 rides the branch line back to Madiun.
    assert!(r.jv_map.segments.iter().any(|s| s.kind == SegmentKind::Branch));
    // Every level and its test have an id the player can open.
    for c in [&r.id, &r.jv] {
        for l in &c.levels {
            assert!(l.level_test.is_some(), "{} has no level test", l.id);
            assert!(l.lessons().count() > 0, "{} has no lessons", l.id);
        }
    }
}

#[test]
fn map_labels_stay_off_other_pins_and_labels() {
    let open = (-1000.0, -1000.0, 1000.0, 1000.0);
    let clear = |pins: &[(f64, f64, usize)], spots: &[LabelSpot]| {
        let rects: Vec<_> = pins.iter().zip(spots).map(|(&(x, y, n), s)| s.rect(x, y, n)).collect();
        rects.iter().enumerate().all(|(i, a)| {
            rects[i + 1..].iter().all(|b| !overlaps(*a, *b))
                && pins
                    .iter()
                    .enumerate()
                    .all(|(j, &(x, y, _))| i == j || !overlaps(*a, (x - PIN, y - PIN, x + PIN, y + PIN)))
        })
    };
    // Two pins almost touching, like Bali and Lombok: one name moves.
    let pins = [(0.0, 0.0, 4), (7.0, 1.0, 6), (60.0, 0.0, 7)];
    let spots = label_spots(&pins, open);
    assert!(clear(&pins, &spots), "{spots:?}");
    assert_eq!(spots.iter().filter(|s| **s == LabelSpot::Above).count(), 2, "{spots:?}");
    assert_eq!(spots[2], LabelSpot::Above, "a lone pin keeps the default");
    // Above clears the selection halo.
    let (_, _, _, bottom) = LabelSpot::Above.rect(0.0, 0.0, 7);
    assert!(bottom <= -HALO + 0.01, "{bottom}");
    // Near the map's edges, labels turn back onto the map: Lombok sits
    // 10 units above the bottom edge, so no name can go below it, and
    // Bali's name moves west (toward the route) rather than Lombok's
    // being pushed out east.
    let edge = (0.0, 0.0, 200.0, 75.0);
    let pins = [(88.0, 64.0, 4), (98.0, 65.0, 6)];
    let spots = label_spots(&pins, edge);
    assert_eq!(spots, [LabelSpot::West, LabelSpot::Above]);
    assert!(clear(&pins, &spots));
    assert!(label_spots(&[], edge).is_empty());
    let spots = label_spots(&[(4.0, 40.0, 6), (196.0, 4.0, 6)], edge);
    // At the west edge the name goes east; in the north-east corner, west.
    assert_eq!(spots, [LabelSpot::East, LabelSpot::West]);

    // The real maps, as drawn in their home view.
    let r = ROUTES.as_ref().unwrap();
    for (cur, map) in [(&r.id, &r.id_map), (&r.jv, &r.jv_map)] {
        let k = pin_scale(map);
        let bounds = map_bounds(map);
        let pins = design_pins(cur, map);
        let spots = map_label_spots(cur, map);
        let rects: Vec<_> = pins.iter().zip(&spots).map(|(&(x, y, n), s)| s.rect(x, y, n)).collect();
        for (i, a) in rects.iter().enumerate() {
            // Every name stays inside the map, never clipped at its edge.
            assert!(inside(*a, bounds), "label {i} leaves the map: {a:?}");
            for (j, &(x, y, _)) in pins.iter().enumerate() {
                if i != j {
                    assert!(!overlaps(*a, (x - PIN, y - PIN, x + PIN, y + PIN)), "label {i} covers pin {j}");
                    // No two pins cover each other.
                    let (xi, yi, _) = pins[i];
                    assert!((x - xi).hypot(y - yi) >= PIN_GAP - 1e-3, "pins {i} and {j} overlap");
                }
            }
            for b in &rects[i + 1..] {
                assert!(!overlaps(*a, *b), "label {i} overlaps another");
            }
            // Every pin is drawn inside the map.
            let (x, y, _) = pins[i];
            assert!(x > bounds.0 && x < bounds.2 && y > bounds.1 && y < bounds.3, "pin {i} off the map");
        }
        // Zooming in never makes two pins overlap on screen.
        for zoom in [1.0, 1.2, 1.5, 2.0, 3.0, 6.0, 12.0] {
            for (i, a) in map.pins.iter().enumerate() {
                for b in &map.pins[i + 1..] {
                    let (pa, pb) = (a.drawn_at(zoom), b.drawn_at(zoom));
                    let screen = (pa.0 - pb.0).hypot(pa.1 - pb.1) / k * zoom;
                    assert!(screen >= 2.0 * PIN, "{} and {} at zoom {zoom}", a.level_id, b.level_id);
                }
            }
        }
    }
    // Bali and Lombok were the pair that touched: they are spread now,
    // and slide back to their real places by 3x zoom.
    let (bali, lombok) = (r.id_map.pin("bipa2").unwrap(), r.id_map.pin("bipa3").unwrap());
    let k = pin_scale(&r.id_map);
    assert!((bali.x - lombok.x).hypot(bali.y - lombok.y) / k < 2.0 * PIN);
    assert!(bali.nudge.0 < 0.0 && lombok.nudge.0 > 0.0);
    assert_eq!(bali.drawn_at(3.0), (bali.x, bali.y));
    assert!(nudge_style(bali).contains("--fade"));
    assert_eq!(nudge_style(r.id_map.pin("bipa1").unwrap()), "");
}

#[test]
fn home_view_holds_every_stop() {
    let r = ROUTES.as_ref().unwrap();
    for map in [&r.id_map, &r.jv_map] {
        let (x0, y0, x1, y1) = stops_box(map).unwrap();
        for p in &map.pins {
            for (x, y) in std::iter::once((p.x, p.y)).chain(p.units.iter().copied()) {
                assert!(x >= x0 && x <= x1 && y >= y0 && y <= y1, "{} outside the home box", p.level_id);
            }
        }
    }
    // The Javanese line runs Jogja to Madiun only, so its home view is
    // a close-up of Central Java, not the whole map frame.
    let (x0, _, x1, _) = stops_box(&r.jv_map).unwrap();
    assert!((x1 - x0) < r.jv_map.view.w / 1.5);
    // No place name is drawn twice.
    let names: BTreeSet<String> = r.jv.levels.iter().map(|l| short_place(l.place())).collect();
    assert_eq!(names.len(), r.jv.levels.len(), "{names:?}");
}

#[test]
fn landmark_stickers_stay_clear_of_pins_names_and_each_other() {
    let r = ROUTES.as_ref().unwrap();
    for (cur, map) in [(&r.id, &r.id_map), (&r.jv, &r.jv_map)] {
        let pins = design_pins(cur, map);
        let stickers = sticker_spots(&pins, map_bounds(map));
        let labels: Vec<_> = pins
            .iter()
            .zip(label_spots(&pins, map_bounds(map)))
            .map(|(&(x, y, n), s)| s.rect(x, y, n))
            .collect();
        let placed: Vec<_> = pins
            .iter()
            .zip(&stickers)
            .filter_map(|(&(x, y, _), s)| s.map(|(dx, dy)| (x + dx, y + dy)))
            .collect();
        let missing: Vec<_> =
            cur.levels.iter().zip(&stickers).filter(|(_, s)| s.is_none()).map(|(l, _)| &l.id).collect();
        assert!(missing.is_empty(), "no room for the stickers of {missing:?}");
        let h = STICKER / 2.0;
        for (i, &(cx, cy)) in placed.iter().enumerate() {
            for &(px, py, _) in &pins {
                assert!((cx - px).hypot(cy - py) >= HALO + h, "sticker {i} covers a pin");
            }
            for l in &labels {
                assert!(!overlaps((cx - h, cy - h, cx + h, cy + h), *l), "sticker {i} covers a name");
            }
            for &(qx, qy) in &placed[i + 1..] {
                assert!((cx - qx).hypot(cy - qy) >= STICKER, "sticker {i} overlaps another");
            }
        }
    }
}

#[test]
fn generated_maps_have_land() {
    for svg in [INDONESIA_SVG, JAVA_SVG] {
        let d = land_path(svg, "land-id");
        assert!(d.starts_with('M') && d.len() > 1000, "{}", &d[..d.len().min(40)]);
    }
    assert!(land_path(INDONESIA_SVG, "land-other").starts_with('M'));
    assert_eq!(land_path(INDONESIA_SVG, "nope"), "");
}

#[test]
fn today_names_units_and_opens_the_next_lesson() {
    assert_eq!(unit_title(UiLanguage::ArEg, "bipa1.u01").as_deref(), Some("التحية · \u{2068}Salam\u{2069}"));
    assert_eq!(unit_title(UiLanguage::En, "bipa1.u01").as_deref(), Some("Greetings · \u{2068}Salam\u{2069}"));
    assert_eq!(unit_title(UiLanguage::En, "nope"), None);

    let mut done = BTreeSet::new();
    assert_eq!(next_lesson_in("bipa1.u01", &done).as_deref(), Some("bipa1.u01.l01"));
    assert_eq!(current_unit(&done, None, false).as_deref(), Some("bipa1.u01"));
    done.insert("bipa1.u01.l01".to_string());
    assert_eq!(next_lesson_in("bipa1.u01", &done).as_deref(), Some("bipa1.u01.l02"));
    assert_eq!(current_unit(&done, None, false).as_deref(), Some("bipa1.u01"));

    // A finished unit has no next lesson; the plan moves on to the next unit.
    let unit = find_unit("bipa1.u01").unwrap();
    done.extend(unit.lessons.iter().map(|l| l.id.clone()));
    assert_eq!(next_lesson_in("bipa1.u01", &done), None);
    assert_eq!(current_unit(&done, None, false).as_deref(), Some("bipa1.u02"));
    assert_eq!(next_lesson_in("nope", &done), None);

    // "I know the basics" starts at BIPA 2; Javanese has its own route.
    assert_eq!(current_unit(&BTreeSet::new(), Some("bipa2"), false).as_deref(), Some("bipa2.u01"));
    assert_eq!(current_unit(&BTreeSet::new(), Some("bipa2"), true).as_deref(), Some("j1.u01"));
}

#[test]
fn suitcase_souvenirs_speak_the_ui_language() {
    // The lesson content's souvenir, not the English curriculum card.
    let (title, note) = local_souvenir("bipa1.u01", UiLanguage::ArEg).unwrap();
    assert_eq!(title, "Taman Fatahillah");
    assert!(note.contains("جاكرتا"), "{note}");
    let (_, en) = local_souvenir("bipa1.u01", UiLanguage::En).unwrap();
    assert!(en.starts_with("Jakarta began"), "{en}");
    assert_eq!(local_souvenir("no.such.unit", UiLanguage::ArEg), None);
}
