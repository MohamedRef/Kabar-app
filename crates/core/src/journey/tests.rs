use std::collections::BTreeSet;

use super::layout::{MapLayout, RouteStyle, SegmentKind};
use super::*;

const BIPA: &str = r#"
schema: kabar.curriculum/v1
language: id
title: { target: "Bahasa Indonesia", en: "Indonesian", ar: "الإندونيسية" }
framework: { name: ignored by the map }
levels:
  - id: bipa1
    code: BIPA 1
    title: { target: "Pemula", en: "Beginner", ar: "مبتدئ" }
    cefr: { approx: A1 }
    journey: { place: Jakarta, kind: city }
    units:
      - id: bipa1.u01
        title: { target: "Salam", en: "Greetings", ar: "التحية" }
        stop: Kota Tua
        grammar: [id.greeting.selamat]
        culture: { souvenir: Fatahillah Square, note: "kabar comes from Arabic خبر." }
        lessons:
          - { id: bipa1.u01.l01, kind: learn, title: "Apa kabar?", can_do: "Ask how someone is.", minutes: 10 }
          - { id: bipa1.u01.l02, kind: review, title: "Ulangan", minutes: 8 }
      - id: bipa1.u02
        title: { en: "Reading", ar: "القراءة" }
        stop: Menteng
        lessons:
          - { id: bipa1.u02.l01, kind: practice, title: "Saya sedang membaca buku.", minutes: 12 }
    level_test: { id: bipa1.test, minutes: 30, pass_mark: 0.75 }
  - id: bipa2
    code: BIPA 2
    title: { en: "Elementary", ar: "أساسي" }
    requires: [bipa1]
    journey: { place: Bali, kind: island }
    units:
      - id: bipa2.u01
        title: { en: "Market", ar: "السوق" }
        stop: Ubud
        culture: { souvenir: Canang sari, note: "Offerings." }
        lessons:
          - { id: bipa2.u01.l01, kind: mini_game, title: "Rumahnya dibersihkan kemarin.", minutes: 12 }
    level_test: { id: bipa2.test, minutes: 35 }
"#;

const JAVANESE: &str = r#"
language: jv
title: { target: "Basa Jawa", en: "Javanese", ar: "الجاوية" }
levels:
  - id: j1
    code: J1
    title: { en: "Survival ngoko", ar: "نغوكو" }
    requires: [bipa1]
    journey: { place: Surabaya, kind: train_line }
    units:
      - id: j1.u01
        title: { target: "Piye kabare, rek?", en: "How are you?", ar: "كيف حالك؟" }
        stop: Pasar Turi
        culture: { souvenir: Suroboyo Bus, note: "Koen wis mangan ta, rek?" }
        lessons:
          - { id: j1.u01.l01, kind: learn, title: "Sek tak lapor pusat", minutes: 10 }
    level_test: { id: j1.test, minutes: 25 }
  - id: j2
    code: J2
    title: { en: "Everyday ngoko", ar: "نغوكو يومية" }
    requires: [j1]
    journey: { place: Madiun, kind: train_line }
    units:
      - { id: j2.u01, title: { en: "Family", ar: "العائلة" }, stop: Madiun, lessons: [{ id: j2.u01.l01, title: "piye dhe", minutes: 10 }] }
  - id: j5
    code: J5
    title: { en: "Reading", ar: "القراءة" }
    requires: [j2]
    journey: { place: Semarang, kind: branch_line }
    units:
      - { id: j5.u01, title: { en: "News", ar: "الأخبار" }, stop: Tawang, lessons: [{ id: j5.u01.l01, title: "Warta", minutes: 14 }] }
  - id: j6
    code: J6
    title: { en: "Script", ar: "الخط" }
    journey: { place: "Kotagede, old Mataram capital", kind: city }
    units:
      - { id: j6.u01, title: { en: "Aksara", ar: "الأبجدية" }, stop: Kotagede, lessons: [{ id: j6.u01.l01, title: "Ha na ca ra ka", minutes: 14 }] }
"#;

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

#[test]
fn parses_the_subset_and_ignores_the_rest() {
    let c = parse_curriculum(BIPA).unwrap();
    assert_eq!(c.language, "id");
    assert_eq!(c.levels.len(), 2);
    let l1 = &c.levels[0];
    assert_eq!(l1.kind(), PlaceKind::City);
    assert_eq!(l1.place(), "Jakarta");
    assert_eq!(l1.level_test.as_ref().unwrap().id, "bipa1.test");
    assert_eq!(l1.units[0].culture.as_ref().unwrap().souvenir, "Fatahillah Square");
    assert_eq!(l1.units[0].lessons[0].kind, LessonKind::Learn);
    assert_eq!(l1.units[0].lessons[1].kind, LessonKind::Review);
    assert_eq!(l1.units[1].title.target, None);
    // An unknown lesson kind falls back instead of failing the whole file.
    assert_eq!(c.levels[1].units[0].lessons[0].kind, LessonKind::Other);
    assert!(!c.unverified());
}

#[test]
fn unknown_place_kind_falls_back() {
    let yaml = BIPA.replace("kind: island", "kind: ferry");
    let c = parse_curriculum(&yaml).unwrap();
    assert_eq!(c.levels[1].kind(), PlaceKind::Other);
}

#[test]
fn broken_yaml_is_an_error_not_a_panic() {
    let err = parse_curriculum("levels: [ {").unwrap_err();
    assert!(err.to_string().starts_with("curriculum:"));
}

#[test]
fn first_not_done_lesson_is_current() {
    let c = parse_curriculum(BIPA).unwrap();
    let p = Progress::new(&c, &set(&[]));
    assert_eq!(p.current(), Some("bipa1.u01.l01"));
    assert_eq!(p.lesson_state("bipa1.u01.l01"), LessonState::Current);
    assert_eq!(p.lesson_state("bipa2.u01.l01"), LessonState::Open);

    // Skipping ahead is allowed: a later lesson done does not move "current".
    let p = Progress::new(&c, &set(&["bipa1.u01.l01", "bipa2.u01.l01"]));
    assert_eq!(p.current(), Some("bipa1.u01.l02"));
    assert_eq!(p.lesson_state("bipa2.u01.l01"), LessonState::Done);
    assert_eq!(p.level_counts(&c.levels[0]), (1, 3));
    assert!(!p.level_done("bipa1"));
}

#[test]
fn placement_moves_lanjut_forward_but_marks_nothing_done() {
    let c = parse_curriculum(BIPA).unwrap();
    let p = Progress::new(&c, &set(&[])).start_at(&c, "bipa2");
    assert_eq!(p.current(), Some("bipa2.u01.l01"));
    assert_eq!(p.lesson_state("bipa1.u01.l01"), LessonState::Open);
    assert!(!p.level_done("bipa1"));
    // Already past the placement level: unchanged.
    let p = Progress::new(&c, &set(&[])).start_at(&c, "bipa1");
    assert_eq!(p.current(), Some("bipa1.u01.l01"));
    // A level from another route changes nothing.
    let p = Progress::new(&c, &set(&[])).start_at(&c, "j2");
    assert_eq!(p.current(), Some("bipa1.u01.l01"));
}

#[test]
fn finished_units_earn_souvenirs() {
    let c = parse_curriculum(BIPA).unwrap();
    let p = Progress::new(&c, &set(&["bipa1.u01.l01"]));
    assert!(souvenirs(&c, &p).is_empty());

    let p = Progress::new(&c, &set(&["bipa1.u01.l01", "bipa1.u01.l02", "unrelated.id"]));
    let s = souvenirs(&c, &p);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].name, "Fatahillah Square");
    assert_eq!(s[0].stop, "Kota Tua");
    assert_eq!(s[0].level_code, "BIPA 1");
    assert!(!s[0].unverified);
    // A unit without a culture note is finished but earns no card.
    let p = Progress::new(&c, &set(&["bipa1.u02.l01"]));
    assert!(p.unit_done(&c.levels[0].units[1]));
    assert!(souvenirs(&c, &p).is_empty());
}

#[test]
fn taking_a_flight_finishes_the_level() {
    let c = parse_curriculum(BIPA).unwrap();
    let p = Progress::new(&c, &set(&["bipa1.test"]));
    assert!(p.level_done("bipa1"));
    assert_eq!(p.level_counts(&c.levels[0]), (3, 3));
    assert_eq!(p.current(), Some("bipa2.u01.l01"));

    let p = Progress::new(&c, &set(&["bipa1.test", "bipa2.test"]));
    assert_eq!(p.current(), None);
    assert_eq!(p.done_levels(), &set(&["bipa1", "bipa2"]));
}

#[test]
fn javanese_is_unverified_and_waits_gently_for_bipa1() {
    let c = parse_curriculum(JAVANESE).unwrap();
    assert!(c.unverified());
    let j1 = c.level("j1").unwrap();
    assert_eq!(c.external_requires(j1).collect::<Vec<_>>(), ["bipa1"]);
    // Requirements inside the same route are just its order.
    assert_eq!(c.external_requires(c.level("j2").unwrap()).count(), 0);
    // Still openable: J1's first lesson is current with nothing done.
    let p = Progress::new(&c, &set(&[]));
    assert_eq!(p.current(), Some("j1.u01.l01"));
    let p = Progress::new(&c, &set(&["j1.u01.l01"]));
    let s = souvenirs(&c, &p);
    assert!(s[0].unverified);
    assert_eq!(s[0].note, "Koen wis mangan ta, rek?");
}

#[test]
fn passing_a_units_lessons_approves_its_javanese() {
    let c = parse_curriculum(JAVANESE).unwrap();
    let done = set(&["j1.u01.l01"]);
    let p = Progress::new(&c, &done).with_approved(&done);
    assert!(p.unit_approved(&c.levels[0].units[0]));
    assert!(!souvenirs(&c, &p)[0].unverified);
    assert!(p.level_approved(c.level("j1").unwrap()));
    assert!(!p.level_approved(c.level("j2").unwrap()));
}

#[test]
fn islands_fly_and_java_rides_the_train() {
    let c = parse_curriculum(BIPA).unwrap();
    let m = MapLayout::new(&c);
    assert_eq!(m.style, RouteStyle::Islands);
    assert_eq!(m.pins.len(), 2);
    assert_eq!(m.segments.len(), 1);
    let s = &m.segments[0];
    assert_eq!(s.kind, SegmentKind::Flight);
    // Arcs bow north (smaller y).
    assert!(s.control.unwrap().1 < s.from.1.min(s.to.1));
    assert!(s.path_d().starts_with('M') && s.path_d().contains('Q'));
    // Jakarta is west of Bali; both pins inside the view.
    let (jk, bali) = (m.pin("bipa1").unwrap(), m.pin("bipa2").unwrap());
    assert!(jk.x < bali.x);
    for p in &m.pins {
        assert!(p.x > m.view.x && p.x < m.view.x + m.view.w);
        assert!(p.y > m.view.y && p.y < m.view.y + m.view.h);
    }
    assert_eq!(jk.units.len(), 2);
    assert!(jk.units_d().starts_with('M'));

    let c = parse_curriculum(JAVANESE).unwrap();
    let m = MapLayout::new(&c);
    assert_eq!(m.style, RouteStyle::Rail);
    let kinds: Vec<_> = m.segments.iter().map(|s| s.kind).collect();
    assert_eq!(kinds, [SegmentKind::Rail, SegmentKind::Branch, SegmentKind::Rail]);
    let madiun = m.pin("j2").unwrap();
    // The branch leaves the main line at Madiun, and the line carries on from there.
    assert_eq!(m.segments[1].from, (madiun.x, madiun.y));
    assert_eq!(m.segments[2].from, (madiun.x, madiun.y));
    for p in &m.pins {
        assert!(p.x > m.view.x && p.x < m.view.x + m.view.w, "{} off the Java view", p.level_id);
        assert!(p.y > m.view.y && p.y < m.view.y + m.view.h, "{} off the Java view", p.level_id);
    }
}

#[test]
fn every_place_is_on_the_map_and_found_by_name() {
    for p in geo::PLACES {
        assert!((geo::LON_MIN..geo::LON_MAX).contains(&p.lon), "{}", p.name);
        assert!((geo::LAT_MIN..geo::LAT_MAX).contains(&p.lat), "{}", p.name);
        // Nudged pins stay within ~20 km of the real place.
        assert!((p.lon - p.map_lon).abs() < 0.2 && (p.lat - p.map_lat).abs() < 0.2, "{}", p.name);
    }
    assert_eq!(geo::place_for("x", "Solo (Surakarta)").unwrap().level, "j3");
    assert_eq!(geo::place_for("x", "Raja Ampat, Papua").unwrap().level, "bipa7");
    assert!(geo::place_for("x", "Atlantis").is_none());
    let (x, y) = geo::project(geo::LON_MIN, geo::LAT_MAX);
    assert_eq!((x, y), (0.0, 0.0));
}

fn pins_at(points: &[(f64, f64)]) -> MapLayout {
    let c = parse_curriculum(BIPA).unwrap();
    let mut m = MapLayout::new(&c);
    m.pins = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| layout::LevelPin {
            level_id: format!("p{i}"),
            x,
            y,
            units: Vec::new(),
            nudge: (0.0, 0.0),
            fade_zoom: 1.0,
        })
        .collect();
    m
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[test]
fn close_pins_spread_apart_and_settle_back_when_zoomed() {
    // Like Bali and Lombok: 4 apart where pins need 10; a third pin far away.
    let mut m = pins_at(&[(400.0, 300.0), (404.0, 301.0), (600.0, 100.0)]);
    m.spread_pins(10.0);
    let (a, b, far) = (&m.pins[0], &m.pins[1], &m.pins[2]);
    assert!(dist(a.drawn_at(1.0), b.drawn_at(1.0)) >= 10.0 - 1e-6);
    // Pushed apart along the line between them, half each way.
    assert!(a.drawn_at(1.0).0 < a.x && b.drawn_at(1.0).0 > b.x);
    assert!((dist(a.drawn_at(1.0), (a.x, a.y)) - dist(b.drawn_at(1.0), (b.x, b.y))).abs() < 1e-6);
    // A pin with room keeps its place.
    assert_eq!(far.nudge, (0.0, 0.0));
    assert_eq!(far.drawn_at(1.0), (600.0, 100.0));
    // Zooming in: the pins stay clear of each other at every zoom (the gap
    // they need shrinks as 1/s in map units), and the nudge fades away.
    for s in [1.0, 1.25, 1.5, 2.0, 2.4, 3.0, 6.0, 12.0] {
        let d = dist(a.drawn_at(s), b.drawn_at(s));
        assert!(d * s >= 10.0 - 1e-6, "zoom {s}: {d}");
    }
    assert!(a.fade_zoom > 1.0 && a.fade_zoom < 3.0, "{}", a.fade_zoom);
    assert_eq!(a.drawn_at(a.fade_zoom), (a.x, a.y));
    assert_eq!(b.drawn_at(12.0), (b.x, b.y));
}

#[test]
fn spread_handles_clusters_shared_spots_and_edges() {
    // Three pins in a tight row, and two on the very same spot.
    let mut m = pins_at(&[(300.0, 200.0), (303.0, 200.0), (306.0, 200.0), (100.0, 100.0), (100.0, 100.0)]);
    m.spread_pins(10.0);
    for (i, p) in m.pins.iter().enumerate() {
        for q in &m.pins[i + 1..] {
            let d = dist(p.drawn_at(1.0), q.drawn_at(1.0));
            assert!(d >= 10.0 - 1e-3, "{} and {}: {d}", p.level_id, q.level_id);
        }
    }
    // Two pins at the bottom edge: spread, but never off the map.
    let bottom = m.view.y + m.view.h;
    let mut m = pins_at(&[(500.0, bottom - 1.0), (500.0, bottom - 4.0)]);
    m.spread_pins(10.0);
    let (a, b) = (m.pins[0].drawn_at(1.0), m.pins[1].drawn_at(1.0));
    for p in [a, b] {
        assert!(p.1 <= bottom - 5.0 + 1e-9 && p.0 > m.view.x && p.0 < m.view.x + m.view.w, "{p:?}");
    }
    assert!(dist(a, b) >= 10.0 - 1e-3, "{a:?} {b:?}");
}

#[test]
fn placement_can_start_inside_a_level() {
    let c = parse_curriculum(BIPA).unwrap();
    let unit = c.levels.last().unwrap().units.last().unwrap();
    let p = Progress::new(&c, &set(&[])).start_at(&c, &unit.id);
    assert_eq!(p.current(), Some(unit.lessons[0].id.as_str()));
    // An unknown unit changes nothing.
    let p = Progress::new(&c, &set(&[])).start_at(&c, "bipa9.u01");
    assert_eq!(p.current(), Some(c.levels[0].units[0].lessons[0].id.as_str()));
}
