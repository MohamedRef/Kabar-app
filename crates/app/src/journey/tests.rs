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

// ------------------------------------------------------------- the screen

mod screen {
    use super::*;
    use crate::Screen;
    use crate::store::Store;
    use crate::test_screens::{self as td, Setup};
    use kabar_protocol::coach::{CoachProfile, LanguageFocus};

    const EN: UiLanguage = UiLanguage::En;
    const PHONE: f64 = 400.0;

    fn view(screen: Screen) -> Option<Element> {
        (screen == Screen::Journey).then(|| rsx! { JourneyScreen {} })
    }

    fn done(store: &mut Store, ids: impl IntoIterator<Item = String>) {
        for id in ids {
            store.record_lesson_event(&id, "completed", &serde_json::json!({ "score": 0.9 })).unwrap();
        }
    }

    fn unit_lessons(unit: &str) -> Vec<String> {
        find_unit(unit).unwrap().lessons.iter().map(|l| l.id.clone()).collect()
    }

    fn phone(store: Store) -> VirtualDom {
        td::mount(Setup::new(store, Screen::Journey, view).width(PHONE))
    }

    fn mac(store: Store) -> VirtualDom {
        td::mount(Setup::new(store, Screen::Journey, view))
    }

    /// Presses a route's button in the header.
    fn route(dom: &mut VirtualDom, msg: Msg) {
        td::click_where(dom, |l| {
            l.attr("aria-pressed").is_some() && l.text.ends_with(crate::i18n::t(EN, msg))
        });
    }

    fn fresh() -> Store {
        Store::open_in_memory().unwrap()
    }

    #[test]
    fn a_new_learner_starts_at_the_first_stop_and_every_lesson_opens() {
        let mut dom = phone(fresh());
        let html = td::html(&dom);
        let first = &ROUTES.as_ref().unwrap().id.levels[0];
        let unit = &first.units[0];
        // The phone shows the route as a list, every level of it.
        assert!(html.contains("route-list"), "{html}");
        assert_eq!(html.matches("class=\"rl-level\"").count(), 7, "{html}");
        // The now card: the first unit, none of it done yet.
        assert!(html.contains("journey-now"), "{html}");
        assert!(td::shows(&html, &unit.stop), "{html}");
        let total = unit.lessons.len().to_string();
        assert!(td::shows(&html, &text::lessons_of(EN, "0", &total)), "{html}");
        // Its first lesson is the current one; the rest are upcoming, none locked.
        assert!(html.contains("data-state=\"current\""), "{html}");
        assert!(!html.contains("data-state=\"done\""), "{html}");
        assert!(td::shows(&html, text::tt(EN, text::T::Upcoming)));

        // The now card opens the next lesson.
        td::click_where(&mut dom, |l| l.class().contains("now-card"));
        assert_eq!(td::screen(&dom), Screen::Lesson);
        assert_eq!(td::lesson(&dom).as_deref(), Some(unit.lessons[0].id.as_str()));

        // Any lesson opens, even one far ahead.
        let mut dom = phone(fresh());
        let far = &ROUTES.as_ref().unwrap().id.levels[3].units[1].lessons[2];
        td::click_where(&mut dom, |l| {
            l.class() == "lesson"
                && l.attr("aria-label").is_some_and(|a| a.starts_with(&far.title))
                && l.text.contains(&far.title)
        });
        assert_eq!(td::lesson(&dom).as_deref(), Some(far.id.as_str()));
    }

    #[test]
    fn heads_continue_bar_and_flights_open_the_right_lesson() {
        let first = &ROUTES.as_ref().unwrap().id.levels[0];
        // "Lanjut" opens the current lesson.
        let mut dom = phone(fresh());
        td::click_where(&mut dom, |l| l.class() == "primary continue");
        assert_eq!(td::lesson(&dom).as_deref(), Some(first.units[0].lessons[0].id.as_str()));
        // A stop's name opens that unit's next lesson.
        let mut dom = phone(fresh());
        let second = &first.units[1];
        td::click_where(&mut dom, |l| {
            l.class().contains("stop-head")
                && l.text.contains(&second.stop)
                && l.text.contains(local(EN, &second.title))
        });
        assert_eq!(td::lesson(&dom).as_deref(), Some(second.lessons[0].id.as_str()));
        // A level's name opens its next lesson.
        let mut dom = phone(fresh());
        let bipa2 = &ROUTES.as_ref().unwrap().id.levels[1];
        td::click_where(&mut dom, |l| l.class().contains("level-title") && l.text.contains(&bipa2.code));
        assert_eq!(td::lesson(&dom).as_deref(), Some(bipa2.units[0].lessons[0].id.as_str()));
        // "Take a flight" opens the level's placement test.
        let mut dom = phone(fresh());
        let flights: Vec<_> = td::listeners(&dom).into_iter().filter(|l| l.class() == "flight").collect();
        assert_eq!(flights.len(), 7, "every unfinished level offers its test");
        assert!(flights[0].text.contains(crate::i18n::t(EN, Msg::JourneyTakeFlight)));
        td::click_on(&mut dom, &flights[0]);
        assert_eq!(td::lesson(&dom), first.level_test.as_ref().map(|t| t.id.clone()));
    }

    #[test]
    fn finished_lessons_show_done_and_the_now_card_moves_on() {
        let mut store = fresh();
        done(&mut store, unit_lessons("bipa1.u01"));
        let first = &ROUTES.as_ref().unwrap().id.levels[0];
        let dom = phone(store);
        let html = td::html(&dom);
        let n = first.units[0].lessons.len();
        assert_eq!(html.matches("data-state=\"done\"").count(), n, "{html}");
        // The now card is on the second stop.
        let now = td::find(&dom, "click", |l| l.class().contains("now-card"));
        assert!(now.text.contains(&first.units[1].stop), "{}", now.text);
        // The level's progress counts the finished lessons.
        let total = first.lessons().count();
        assert!(
            html.contains(&format!("aria-valuenow=\"{n}\"")) && html.contains(&format!("{n}/{total}")),
            "{html}"
        );
        // The suitcase holds the first stop's souvenir.
        let toggle = td::find(&dom, "click", |l| l.class() == "suitcase-toggle");
        assert!(toggle.text.ends_with('1'), "{}", toggle.text);
    }

    #[test]
    fn the_suitcase_shows_souvenirs_that_open_their_lesson() {
        // Empty for a new learner, with a word on how to fill it.
        let mut dom = phone(fresh());
        td::click_where(&mut dom, |l| l.class() == "suitcase-toggle");
        let html = td::html(&dom);
        assert!(td::shows(&html, crate::i18n::t(EN, Msg::JourneySuitcaseEmpty)), "{html}");
        assert!(!html.contains("journey-now"), "the now card steps aside: {html}");
        // Pressing it again goes back to the route.
        td::click_where(&mut dom, |l| l.class() == "suitcase-toggle");
        assert!(td::html(&dom).contains("route-list"));

        let mut store = fresh();
        done(&mut store, unit_lessons("bipa1.u01"));
        let mut dom = phone(store);
        td::click_where(&mut dom, |l| l.class() == "suitcase-toggle");
        let html = td::html(&dom);
        let (title, note) = local_souvenir("bipa1.u01", EN).unwrap();
        assert!(td::shows(&html, &title), "{html}");
        assert!(html.contains("id=\"souvenir-bipa1.u01\""), "{html}");
        assert!(note.split_whitespace().take(3).all(|w| html.contains(w)), "{note}");
        td::click_where(&mut dom, |l| l.class().contains("souvenir"));
        assert_eq!(td::lesson(&dom), souvenir_lesson("bipa1.u01"));
        // Choosing a route from the suitcase goes back to that route.
        let mut store = fresh();
        done(&mut store, unit_lessons("bipa1.u01"));
        let mut dom = phone(store);
        td::click_where(&mut dom, |l| l.class() == "suitcase-toggle");
        route(&mut dom, Msg::JourneyJavanese);
        let html = td::html(&dom);
        assert!(html.contains("route-list") && html.contains("rl-j1"), "{html}");
    }

    #[test]
    fn the_journals_souvenir_line_opens_the_suitcase_on_it() {
        let mut store = fresh();
        done(&mut store, unit_lessons("bipa1.u01"));
        let mut dom = td::mount(Setup::new(store, Screen::Today, view));
        dom.in_scope(ScopeId::APP, || open_suitcase(consume_context::<crate::Ui>(), "bipa1.u01"));
        dom.render_immediate_to_vec();
        assert_eq!(td::screen(&dom), Screen::Journey);
        let html = td::html(&dom);
        assert!(html.contains("souvenir tap found"), "the souvenir is picked out: {html}");
        // Scrolled into view once mounted (the effect runs without a page here).
        td::settle(&mut dom, 50);
        assert!(td::html(&dom).contains("souvenir tap found"));
        // Shown once: the next visit opens on the route.
        assert!(dom.in_scope(ScopeId::APP, || SUITCASE_AT.peek().is_none()));
    }

    #[test]
    fn the_javanese_line_waits_gently_for_bipa_1_and_is_unverified() {
        let mut dom = phone(fresh());
        route(&mut dom, Msg::JourneyJavanese);
        let html = td::html(&dom);
        let jv = &ROUTES.as_ref().unwrap().jv;
        assert_eq!(html.matches("class=\"rl-level\"").count(), jv.levels.len(), "{html}");
        // Nothing locked, just a note that it's best after BIPA 1.
        assert!(td::shows(&html, crate::i18n::t(EN, Msg::JourneyBestAfter)), "{html}");
        assert!(td::shows(&html, crate::i18n::t(EN, Msg::JourneyOpenAnyway)), "{html}");
        // Javanese content is flagged until approved; trains, not flights.
        assert!(td::shows(&html, crate::i18n::t(EN, Msg::JourneyUnverified)), "{html}");
        assert!(td::shows(&html, crate::i18n::t(EN, Msg::JourneyTakeExpress)), "{html}");
        assert!(!td::shows(&html, crate::i18n::t(EN, Msg::JourneyTakeFlight)), "{html}");
        let pressed = td::find(&dom, "click", |l| {
            l.attr("aria-pressed").is_some() && l.text.ends_with(crate::i18n::t(EN, Msg::JourneyJavanese))
        });
        assert_eq!(pressed.attr("aria-pressed"), Some("true"));

        // With BIPA 1 done the note goes.
        let mut store = fresh();
        done(&mut store, ROUTES.as_ref().unwrap().id.levels[0].lessons().map(|l| l.id.clone()));
        let mut dom = phone(store);
        route(&mut dom, Msg::JourneyJavanese);
        let html = td::html(&dom);
        let j1 = html.split("id=\"rl-j1\"").nth(1).unwrap().split("</section>").next().unwrap().to_string();
        assert!(!td::shows(&j1, crate::i18n::t(EN, Msg::JourneyBestAfter)), "{j1}");
    }

    #[test]
    fn a_javanese_only_learner_opens_on_the_train_line() {
        let mut store = fresh();
        store
            .save_coach_profile(&CoachProfile { focus: LanguageFocus::Javanese, ..CoachProfile::default() })
            .unwrap();
        let dom = phone(store);
        let html = td::html(&dom);
        assert!(html.contains("rl-j1") && !html.contains("rl-bipa1"), "{html}");
    }

    #[test]
    fn placement_starts_the_journey_further_on() {
        let mut store = fresh();
        let placed = kabar_protocol::test::PlacementResult {
            start_level: "bipa2".into(),
            start_unit: Some("bipa2.u03".into()),
            sections: Vec::new(),
        };
        store.record_lesson_event("placement", "placed", &placed).unwrap();
        let dom = phone(store);
        let now = td::find(&dom, "click", |l| l.class().contains("now-card"));
        let unit = find_unit("bipa2.u03").unwrap();
        assert!(now.text.contains(&unit.stop) && now.text.contains("BIPA 2"), "{}", now.text);
    }

    #[test]
    fn the_whole_route_done_says_selamat() {
        let mut store = fresh();
        let routes = ROUTES.as_ref().unwrap();
        done(&mut store, routes.id.lessons().map(|l| l.id.clone()));
        let dom = phone(store);
        let html = td::html(&dom);
        assert!(td::shows(&html, crate::i18n::t(EN, Msg::JourneyAllDone)), "{html}");
        assert!(!html.contains("journey-now"), "no next stop: {html}");
        assert!(!html.contains("class=\"flight\""), "no test left to offer: {html}");
    }

    #[test]
    fn the_last_unfinished_stop_points_to_the_level_test() {
        let routes = ROUTES.as_ref().unwrap();
        // Every BIPA 1 stop done but the last, which has no souvenir lesson
        // left to give: the milestone row names the level test.
        let level = &routes.id.levels[0];
        let last = level.units.last().unwrap();
        let mut store = fresh();
        done(
            &mut store,
            level
                .units
                .iter()
                .filter(|u| u.id != last.id)
                .flat_map(|u| u.lessons.iter().map(|l| l.id.clone())),
        );
        let mut dom = phone(store);
        let row = td::find(&dom, "click", |l| l.class().contains("now-milestone"));
        if last.culture.is_some() {
            assert!(row.text.contains(text::tt(EN, text::T::SouvenirHere)), "{}", row.text);
            td::click_on(&mut dom, &row);
            assert_eq!(td::lesson(&dom), souvenir_lesson(&last.id));
        } else {
            assert!(row.text.contains(text::tt(EN, text::T::LevelTestNext)), "{}", row.text);
            td::click_on(&mut dom, &row);
            assert_eq!(td::lesson(&dom), level.level_test.as_ref().map(|t| t.id.clone()));
        }
    }

    #[test]
    fn the_phone_map_picks_a_stop_and_shows_its_lessons() {
        let mut dom = phone(fresh());
        td::click_where(&mut dom, |l| l.class() == "map-toggle");
        let html = td::html(&dom);
        assert!(html.contains("class=\"jm\"") && html.contains("jm-stage-id"), "{html}");
        // Opens on the current level's panel.
        assert!(html.contains("data-panel=\"open\""), "{html}");
        let routes = ROUTES.as_ref().unwrap();
        let bipa3 = &routes.id.levels[2];
        // A pin picks its level: the panel shows that level's stops.
        td::click_where(&mut dom, |l| {
            l.class() == "jm-pin" && l.attr("aria-label").is_some_and(|a| a.contains(&bipa3.code))
        });
        let html = td::html(&dom);
        let panel = html.split("class=\"jm-panel\"").nth(1).unwrap();
        assert!(td::shows(panel, &bipa3.units[0].stop), "{panel}");
        let pin = td::find(&dom, "click", |l| {
            l.class() == "jm-pin" && l.attr("aria-label").is_some_and(|a| a.contains(&bipa3.code))
        });
        assert_eq!(pin.attr("aria-pressed"), Some("true"));
        // Its landmark sticker picks it too, and Close shuts the panel.
        let stickers: Vec<_> =
            td::listeners(&dom).into_iter().filter(|l| l.class() == "jm-sticker").collect();
        assert!(!stickers.is_empty(), "levels with art get a sticker");
        let first = stickers.iter().position(|l| l.attr("data-selected") == Some("false")).unwrap();
        td::click_on(&mut dom, &stickers[first]);
        let now: Vec<_> = td::listeners(&dom).into_iter().filter(|l| l.class() == "jm-sticker").collect();
        assert_eq!(now[first].attr("data-selected"), Some("true"));
        td::click_where(&mut dom, |l| l.attr("aria-label") == Some(crate::i18n::t(EN, Msg::JourneyClose)));
        assert!(td::html(&dom).contains("data-panel=\"closed\""));
        // Pressing Map again goes back to the list.
        td::click_where(&mut dom, |l| l.class() == "map-toggle");
        assert!(td::html(&dom).contains("route-list"));
    }

    #[test]
    fn the_stop_picked_on_the_map_survives_a_resize() {
        let mut dom = mac(fresh());
        let bipa4 = &ROUTES.as_ref().unwrap().id.levels[3];
        td::click_where(&mut dom, |l| {
            l.class() == "jm-pin" && l.attr("aria-label").is_some_and(|a| a.contains(&bipa4.code))
        });
        // The window narrows to a phone: the list, which brings that stop into view.
        dom.in_scope(ScopeId::APP, || {
            let mut w = consume_context::<crate::Ui>().width;
            w.set(WidthClass::Compact);
        });
        td::settle(&mut dom, 50);
        let html = td::html(&dom);
        assert!(html.contains("route-list") && html.contains(&format!("id=\"rl-{}\"", bipa4.id)), "{html}");
        // And wide again: the same stop is still picked.
        dom.in_scope(ScopeId::APP, || {
            let mut w = consume_context::<crate::Ui>().width;
            w.set(WidthClass::Expanded);
        });
        td::settle(&mut dom, 50);
        let pin = td::find(&dom, "click", |l| {
            l.class() == "jm-pin" && l.attr("aria-label").is_some_and(|a| a.contains(&bipa4.code))
        });
        assert_eq!(pin.attr("aria-pressed"), Some("true"));
    }

    #[test]
    fn the_mac_shows_the_map_with_zoom_controls_and_the_rail_for_javanese() {
        let mut dom = mac(fresh());
        let html = td::html(&dom);
        assert!(
            html.contains("class=\"jm\"") && !html.contains("map-toggle"),
            "a wide window has room for the map: {html}"
        );
        for msg in [Msg::JourneyZoomIn, Msg::JourneyZoomOut, Msg::JourneyZoomReset] {
            let b = td::find(&dom, "click", |l| l.attr("aria-label") == Some(crate::i18n::t(EN, msg)));
            td::click_on(&mut dom, &b);
        }
        assert_eq!(td::screen(&dom), Screen::Journey, "zooming stays on the map");
        assert!(html.contains("jm-route islands") && html.contains("seg flight"), "{html}");
        route(&mut dom, Msg::JourneyJavanese);
        let html = td::html(&dom);
        assert!(html.contains("jm-route rail") && html.contains("seg branch"), "{html}");
        assert!(html.contains("jm-stage-jv"), "{html}");
    }
}

// ------------------------------------------------------------- for Today and the journal

mod helpers {
    use super::*;
    use crate::store::Store;
    use kabar_protocol::coach::{CoachProfile, StartingPoint};

    fn db(store: Store) -> Db {
        Db(Rc::new(std::cell::RefCell::new(store)))
    }

    #[test]
    fn the_journal_names_every_lesson_test_and_souvenir() {
        let c = journal_catalog();
        let r = ROUTES.as_ref().unwrap();
        let lessons = r.id.lessons().count() + r.jv.lessons().count();
        let tests = r.id.levels.len() + r.jv.levels.len();
        assert_eq!(c.titles.len(), lessons + tests);
        assert_eq!(c.tests.len(), tests);
        let first = &r.id.levels[0];
        assert_eq!(c.titles.get(&first.units[0].lessons[0].id), Some(&first.units[0].lessons[0].title));
        // A level test reads as its level's code in the journal.
        let test = first.level_test.as_ref().unwrap();
        assert_eq!(c.titles.get(&test.id), Some(&first.code));
        assert!(c.tests.contains(&test.id));
        // Every stop with a culture card gives a souvenir, with its lessons.
        let unit = c.units.iter().find(|u| u.id == "bipa1.u01").unwrap();
        assert_eq!(unit.lessons, first.units[0].lessons.iter().map(|l| l.id.clone()).collect::<Vec<_>>());
        assert!(!unit.souvenir.is_empty());
    }

    #[test]
    fn refreshers_titles_and_levels_by_id() {
        let unit = find_unit("bipa1.u02").unwrap();
        let refresher = unit_refresher("bipa1.u02").unwrap();
        let review = unit.lessons.iter().find(|l| l.kind == LessonKind::Review);
        assert_eq!(refresher, review.unwrap_or(&unit.lessons[0]).id, "the review lesson, else the first");
        assert_eq!(unit_refresher("nope"), None);
        assert_eq!(lesson_title(&unit.lessons[0].id).as_deref(), Some(unit.lessons[0].title.as_str()));
        assert_eq!(lesson_title("j1.u01.l01"), find_unit("j1.u01").map(|u| u.lessons[0].title.clone()));
        assert_eq!(lesson_title("nope"), None);
        assert_eq!(today::level_of("bipa2.u03"), "bipa2");
        assert_eq!(today::level_of("bipa2"), "bipa2");
        assert_eq!(curriculum("jv").map(|c| c.levels.len()), Some(6));
        assert!(curriculum("xx").is_none());
        assert_eq!(local(UiLanguage::Ar, &unit.title), unit.title.ar);
    }

    #[test]
    fn the_start_comes_from_placement_then_onboarding() {
        let profile = |start| CoachProfile { start, ..CoachProfile::default() };
        // Nothing yet: from the very beginning, BIPA 1.
        let d = db(Store::open_in_memory().unwrap());
        assert_eq!(start_level(&d, None), None);
        assert_eq!(learner_levels(&d, None), ("bipa1".to_string(), None));
        // "I know the basics" at onboarding: the level its first unit is in.
        let basics = profile(StartingPoint::KnowsBasics);
        let start = start_level(&d, Some(&basics)).unwrap();
        assert_eq!(
            start,
            today::level_of(kabar_core::coach::onboarding::starting_unit(StartingPoint::KnowsBasics))
        );
        assert_eq!(learner_levels(&d, Some(&basics)).0, start);
        // Placement wins, down to its unit when it could tell.
        let mut store = Store::open_in_memory().unwrap();
        let placed = kabar_protocol::test::PlacementResult {
            start_level: "bipa3".into(),
            start_unit: Some("bipa3.u02".into()),
            sections: Vec::new(),
        };
        store.record_lesson_event("placement", "placed", &placed).unwrap();
        let d = db(store);
        assert_eq!(start_level(&d, Some(&basics)).as_deref(), Some("bipa3.u02"));
        assert_eq!(learner_levels(&d, None).0, "bipa3");
        // A Javanese lesson started: its level shows too.
        d.0.borrow_mut().record_lesson_event("j1.u01.l01", "completed", &serde_json::json!({})).unwrap();
        assert_eq!(learner_levels(&d, None).1.as_deref(), Some("j1"));
    }

    #[test]
    fn journey_lines_exist_in_every_language() {
        use text::{T, tt};
        for t in [T::NowStop, T::Goal, T::Continue, T::Upcoming, T::SouvenirHere, T::LevelTestNext] {
            let lines = [tt(UiLanguage::En, t), tt(UiLanguage::ArEg, t), tt(UiLanguage::Ar, t)];
            assert!(lines.iter().all(|l| !l.trim().is_empty()), "{t:?}");
            assert!(
                lines[1..].iter().all(|l| l.chars().any(|c| ('\u{600}'..='\u{6ff}').contains(&c))),
                "{t:?}"
            );
        }
        // MSA has its own accusative form for 11+.
        assert_eq!(text::lessons_of(UiLanguage::Ar, "3", "14"), "3 من 14\u{a0}درسًا");
        assert_eq!(kind_attr(LessonKind::Other), "other");
        assert_eq!(kind_label(LessonKind::Other), Msg::LessonLearn);
        assert_eq!(kind_label(LessonKind::Review), Msg::LessonReview);
    }
}
