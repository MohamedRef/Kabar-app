use kabar_protocol::lesson::{DialogueLine, Example, Exercise, Localized, NewWord, Slide, Souvenir};

use super::*;

fn loc(s: &str) -> Localized {
    Localized { ar_eg: s.into(), ar: s.into(), en: s.into() }
}

fn line(speaker: &str, text: &str) -> DialogueLine {
    DialogueLine { speaker: speaker.into(), text: text.into(), meaning: loc("") }
}

fn lesson(id: &str, language: &str) -> LessonContent {
    LessonContent {
        schema: "kabar.lesson/v1".into(),
        id: id.into(),
        language: language.into(),
        title: "Salam".into(),
        can_do: loc("greet"),
        slides: vec![Slide {
            title: Some("Selamat pagi!".into()),
            body: loc("..."),
            examples: vec![
                Example { text: "Selamat pagi, Bu.".into(), meaning: loc("") },
                Example { text: "Apa  kabar?".into(), meaning: loc("") },
            ],
            grammar: None,
            table: None,
        }],
        exercises: vec![
            Exercise {
                id: "e01".into(),
                kind: ExerciseKind::Listen {
                    text: "Apa kabar?".into(),
                    audio: None,
                    options: vec![],
                    answer: None,
                    meaning: None,
                    question: None,
                },
                explain: None,
                words: vec![],
            },
            Exercise {
                id: "e02".into(),
                kind: ExerciseKind::MultipleChoice {
                    question: String::new(),
                    question_meaning: Some(loc("?")),
                    options: vec!["a".into()],
                    options_meaning: vec![],
                    answer: 0,
                },
                explain: None,
                words: vec![],
            },
            Exercise {
                id: "e03".into(),
                kind: ExerciseKind::Speaking { text: "Kabar baik.".into(), meaning: loc("") },
                explain: None,
                words: vec![],
            },
        ],
        dialogue: Some(Dialogue {
            title: None,
            lines: vec![line("Mohamed", "Selamat pagi, Bu Sari."), line("Bu Sari", "Pagi! Apa kabar?")],
        }),
        new_words: vec![NewWord {
            word: "kabar".into(),
            pos: None,
            meaning: loc("news"),
            arabic: Some("خبر".into()),
            bridge: None,
            false_friend: None,
            same_as_id: false,
        }],
        souvenir: Some(Souvenir { id: "s".into(), title: "t".into(), note: loc("") }),
        review: Default::default(),
        game: None,
    }
}

#[test]
fn keys_ignore_spacing_but_not_voice() {
    assert_eq!(audio_key(Voice::IdGadis, " Apa  kabar?\n"), audio_key(Voice::IdGadis, "Apa kabar?"));
    assert_ne!(audio_key(Voice::IdGadis, "Apa kabar?"), audio_key(Voice::IdArdi, "Apa kabar?"));
    assert_ne!(audio_key(Voice::IdGadis, "Apa kabar?"), audio_key(Voice::IdGadis, "apa kabar?"));
    assert!(is_hex_key(&audio_key(Voice::JvSiti, "Sugeng enjing")));
}

#[test]
fn ssml_matches_the_proxy_shape() {
    assert_eq!(
        clip_ssml(Voice::IdGadis, "Apa kabar?"),
        r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="id-ID"><voice name="id-ID-GadisNeural">Apa kabar?</voice></speak>"#
    );
    assert!(clip_ssml(Voice::JvDimas, "a & <b>").contains("a &amp; &lt;b&gt;"));
    // Azure bills the escapes as written, not the speak/voice tags.
    assert_eq!(billable_chars(Voice::IdGadis, "Apa kabar?"), 10);
    assert_eq!(billable_chars(Voice::IdArdi, "Jum'at"), "Jum&apos;at".len() as u32);
    assert_eq!(
        ssml_billable_chars(r#"<speak a="1"><voice name="x"><break time="1s"/>ok</voice></speak>"#),
        20
    );
}

#[test]
fn speakers_get_fitting_voices() {
    let d = Dialogue {
        title: None,
        lines: vec![line("Mohamed", "x"), line("Bu Siti", "y"), line("Penjual", "z")],
    };
    assert_eq!(speaker_voice("id", &d, "Mohamed"), Voice::IdArdi);
    assert_eq!(speaker_voice("jv", &d, "Bu Siti"), Voice::JvSiti);
    assert_eq!(speaker_voice("id", &d, "Mbak Rara"), Voice::IdGadis);
    assert_eq!(speaker_voice("jv", &d, "Mas Heru"), Voice::JvDimas);
    assert_eq!(speaker_voice("id", &d, "Ni Luh"), Voice::IdGadis);
    assert_eq!(speaker_voice("jv", &d, "Mbah Putri"), Voice::JvSiti);
    assert_eq!(speaker_voice("jv", &d, "Mbah Darmi"), Voice::JvSiti);
    assert_eq!(speaker_voice("id", &d, "Rina"), Voice::IdGadis);
    assert_eq!(speaker_voice("id", &d, "Kak Dewi"), Voice::IdGadis);
    assert_eq!(speaker_voice("id", &d, "Kak Mei"), Voice::IdGadis);
    assert_eq!(speaker_voice("id", &d, "Kak Yosua"), Voice::IdArdi);
    assert_eq!(speaker_voice("id", &d, "Kaka Benny"), Voice::IdArdi);
    assert_eq!(speaker_voice("id", &d, "Kaka Yohana"), Voice::IdGadis);
    assert_eq!(speaker_voice("id", &d, "Mama Ruth"), Voice::IdGadis);
    // Unknown: the voice the others don't use (Mohamed is first and male).
    assert_eq!(speaker_voice("id", &d, "Penjual"), Voice::IdGadis);
    let two = Dialogue { title: None, lines: vec![line("Petugas", "a"), line("Pengumuman", "b")] };
    assert_ne!(speaker_voice("id", &two, "Petugas"), speaker_voice("id", &two, "Pengumuman"));
}

#[test]
fn lesson_lines_cover_what_the_player_plays() {
    let l = lesson("bipa1.u01.l01", "id");
    let lines = lesson_lines(&l);
    let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Selamat pagi, Bu.",
            "Apa kabar?",
            "Apa kabar?",
            "Kabar baik.",
            "Selamat pagi, Bu Sari.",
            "Pagi! Apa kabar?",
            "kabar"
        ]
    );
    assert_eq!(lines[4].voice, Voice::IdArdi);
    assert_eq!(lines[5].voice, Voice::IdGadis);
    assert_eq!(unit_of("bipa1.u01.l01"), "bipa1.u01");
}

#[test]
fn manifest_dedupes_and_versions() {
    let a = lesson("bipa1.u01.l01", "id");
    let b = lesson("bipa1.u01.l02", "id");
    let inputs = [
        PackInput { id: "bipa1.u01.l02", yaml_sha256: "b".into(), lesson: &b },
        PackInput { id: "bipa1.u01.l01", yaml_sha256: "a".into(), lesson: &a },
    ];
    let m = build_manifest("id", "bipa1.u01", &inputs, None);
    assert_eq!(m.id, "id/bipa1.u01");
    assert_eq!(m.version, 1);
    assert_eq!(m.lessons[0].id, "bipa1.u01.l01");
    // Same lines in both lessons: one clip each.
    assert_eq!(m.totals.clips, 6);
    assert_eq!(m.totals.missing, 6);
    assert_eq!(m.totals.chars, m.audio.iter().map(|a| u64::from(a.chars)).sum::<u64>());

    // Rebuild unchanged: same version; synthesized hashes carry over.
    let mut synthesized = m.clone();
    synthesized.audio[0].sha256 = Some("f".repeat(64));
    synthesized.audio[0].size = Some(1234);
    refresh_totals(&mut synthesized);
    let again = build_manifest("id", "bipa1.u01", &inputs, Some(&synthesized));
    assert_eq!(again.version, 1);
    assert_eq!(again.audio[0].size, Some(1234));
    assert_eq!(again.totals.audio_bytes, 1234);
    assert_eq!(again.totals.missing, 5);

    // A changed lesson bumps the version.
    let inputs2 = [PackInput { id: "bipa1.u01.l01", yaml_sha256: "a2".into(), lesson: &a }];
    let changed = build_manifest("id", "bipa1.u01", &inputs2, Some(&again));
    assert_eq!(changed.version, 2);
    assert_ne!(changed.content_hash, again.content_hash);
}

#[test]
fn budget_refuses_overspending() {
    let b = Budget { used: 270_000, limit: DEFAULT_MONTHLY_BUDGET };
    assert_eq!(b.left(), 10_000);
    assert!(b.check(10_000).is_ok());
    assert_eq!(b.check(10_001), Err(OverBudget { needed: 10_001, left: 10_000 }));
    assert_eq!(Budget { used: 600_000, limit: FREE_MONTHLY_CHARS }.left(), 0);
}

#[test]
fn synthesis_skips_cached_and_shared_clips() {
    let a = lesson("bipa1.u01.l01", "id");
    let m1 = build_manifest(
        "id",
        "bipa1.u01",
        &[PackInput { id: "bipa1.u01.l01", yaml_sha256: "a".into(), lesson: &a }],
        None,
    );
    let m2 = build_manifest(
        "id",
        "bipa1.u02",
        &[PackInput { id: "bipa1.u02.l01", yaml_sha256: "c".into(), lesson: &a }],
        None,
    );
    let cached: BTreeSet<String> = [m1.audio[0].key.clone()].into();
    let (todo, chars) = to_synthesize([&m1, &m2], &cached);
    assert_eq!(todo.len(), m1.audio.len() - 1);
    assert_eq!(chars, m1.totals.chars - u64::from(m1.audio[0].chars));
}

#[test]
fn months_are_utc_calendar_months() {
    assert_eq!(month_of(0), "1970-01");
    assert_eq!(month_of(1_793_000_000), "2026-10");
    assert_eq!(month_of(951_782_400), "2000-02"); // 2000-02-29, a leap day
}

fn summary_row(unit: &str, language: &str, version: u32, ready: bool) -> PackSummary {
    PackSummary {
        id: pack_id(language, unit),
        language: language.into(),
        unit: unit.into(),
        stop: None,
        version,
        content_hash: "h".into(),
        lessons: 4,
        clips: 10,
        chars: 100,
        audio_bytes: 1000,
        missing: 0,
        file: pack_file(unit),
        file_sha256: ready.then(|| "s".repeat(64)),
        file_size: ready.then_some(2000),
    }
}

#[test]
fn prefetch_current_and_next_unit() {
    let index = PackIndex {
        format: PACK_FORMAT,
        packs: vec![
            summary_row("bipa1.u01", "id", 1, true),
            summary_row("bipa1.u02", "id", 2, true),
            summary_row("j1.u01", "jv", 1, true),
            summary_row("bipa2.u01", "id", 1, false),
        ],
    };
    let none = BTreeMap::new();
    assert_eq!(prefetch(&index, "bipa1.u01.l03", &none), ["id/bipa1.u01", "id/bipa1.u02"]);
    // The next unit skips the other language; not-yet-bundled packs wait.
    assert_eq!(prefetch(&index, "bipa1.u02.l01", &none), ["id/bipa1.u02"]);
    let installed: BTreeMap<String, u32> =
        [("id/bipa1.u01".to_owned(), 1), ("id/bipa1.u02".to_owned(), 1)].into();
    // Installed u02 is outdated (v1 < v2).
    assert_eq!(prefetch(&index, "bipa1.u01.l01", &installed), ["id/bipa1.u02"]);
    assert!(prefetch(&index, "j9.u01.l01", &none).is_empty());
    // Only installed packs with a newer version are refreshed.
    assert_eq!(outdated(&index, &installed), ["id/bipa1.u02"]);
    assert!(outdated(&index, &none).is_empty());
}

#[test]
fn removing_a_pack_keeps_shared_clips() {
    let a = lesson("bipa1.u01.l01", "id");
    let m = build_manifest(
        "id",
        "bipa1.u01",
        &[PackInput { id: "bipa1.u01.l01", yaml_sha256: "a".into(), lesson: &a }],
        None,
    );
    let shared = m.audio[0].key.clone();
    let cached = [shared.as_str(), "old"];
    assert_eq!(orphans(cached, std::slice::from_ref(&m)), ["old"]);
    assert_eq!(orphans(cached, &[]).len(), 2);
}

#[test]
fn downloaded_manifests_are_checked() {
    let a = lesson("bipa1.u01.l01", "id");
    let mut m = build_manifest(
        "id",
        "bipa1.u01",
        &[PackInput { id: "bipa1.u01.l01", yaml_sha256: "a".into(), lesson: &a }],
        None,
    );
    let row = summary(&m, None);
    assert!(check_manifest(&m, &row).unwrap_err().contains("without hashes"));
    for e in &mut m.audio {
        e.sha256 = Some("0".repeat(64));
        e.size = Some(10);
    }
    refresh_totals(&mut m);
    let row = summary(&m, None);
    assert_eq!(check_manifest(&m, &row), Ok(()));
    let mut newer = m.clone();
    newer.format = PACK_FORMAT + 1;
    assert!(check_manifest(&newer, &row).is_err());
    let mut other = row.clone();
    other.version = 9;
    assert!(check_manifest(&m, &other).is_err());

    // Unsafe or mismatched unit ids are refused before any file path is built.
    for bad in ["", "../cloud-session", ".hidden", "bipa1..u01", "bipa1.", "BIPA1.u01", "id/bipa1.u01"] {
        assert!(!is_unit_id(bad), "{bad}");
        let mut bad_m = m.clone();
        bad_m.unit = bad.into();
        let bad_row = PackSummary { unit: bad.into(), ..row.clone() };
        assert_eq!(check_manifest(&bad_m, &bad_row), Err("bad unit id".into()));
    }
    let mut wrong_unit = m.clone();
    wrong_unit.unit = "bipa1.u02".into();
    assert!(check_manifest(&wrong_unit, &row).is_err());
    let mut wrong_id = m.clone();
    wrong_id.id = "id/bipa1.u02".into();
    let wrong_row = PackSummary { id: "id/bipa1.u02".into(), ..row };
    assert!(check_manifest(&wrong_id, &wrong_row).is_err());
}
