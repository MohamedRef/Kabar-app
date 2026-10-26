//! The test texts from docs/SPEC.md §24, run through the whole deterministic
//! pipeline. M3 is done when these pass.

use kabar_core::lang::Lang;
use kabar_core::lang::analyze::{AnalyzedToken, Engine, Morph};

fn analyze(text: &str) -> Vec<AnalyzedToken> {
    Engine::default().analyze(text).into_iter().filter(|t| t.token.is_wordlike()).collect()
}

fn find<'a>(toks: &'a [AnalyzedToken], surface: &str) -> &'a AnalyzedToken {
    toks.iter().find(|t| t.token.text == surface).unwrap_or_else(|| panic!("no token {surface}"))
}

fn morph(t: &AnalyzedToken) -> String {
    match &t.morph {
        Some(Morph::Indonesian(a)) => a.display(),
        Some(Morph::Javanese(a)) => a.display(),
        None => String::new(),
    }
}

#[test]
fn javanese_chat() {
    let t = analyze("Sek tak lapor pusat");
    assert_eq!(find(&t, "Sek").lang.lang, Lang::Javanese);
    assert_eq!(find(&t, "tak").lang.lang, Lang::Javanese, "context decides 'tak'");
    assert_eq!(find(&t, "lapor").lang.lang, Lang::Javanese, "shared word follows the sentence");
    assert_eq!(find(&t, "pusat").lang.lang, Lang::Indonesian);

    let t = analyze("ng prabowo?");
    let ng = find(&t, "ng");
    assert_eq!(ng.normalized.as_ref().map(|n| n.full.as_str()), Some("nang"));
    assert_eq!(ng.lang.lang, Lang::Javanese);
    assert_ne!(find(&t, "prabowo").lang.lang, Lang::Indonesian, "a name is not an Indonesian word");

    let t = analyze("piye dhe");
    assert_eq!(find(&t, "piye").lang.lang, Lang::Javanese);

    let t = analyze("arp dicul piro");
    assert_eq!(find(&t, "arp").normalized.as_ref().unwrap().full, "arep");
    let dicul = find(&t, "dicul");
    assert_eq!(dicul.normalized.as_ref().unwrap().full, "diculke");
    assert_eq!(dicul.lang.lang, Lang::Javanese);
    assert_eq!(morph(dicul), "di- + cul + -ke");
    assert_eq!(find(&t, "piro").normalized.as_ref().unwrap().full, "pira");
}

#[test]
fn marketplace_listing() {
    let t = analyze("iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan minat wa");
    assert_eq!(find(&t, "iphone").lang.lang, Lang::Proper);
    assert_eq!(find(&t, "pm").normalized.as_ref().unwrap().full, "Pro Max");
    assert_eq!(find(&t, "nomin").normalized.as_ref().unwrap().full, "no minus");
    assert_eq!(find(&t, "wa").normalized.as_ref().unwrap().full, "WhatsApp");
    for w in ["resmi", "mulus", "garansi", "jalan", "minat"] {
        assert_eq!(find(&t, w).lang.lang, Lang::Indonesian, "{w}");
    }
}

#[test]
fn indonesian_morphology() {
    let t = analyze("Saya sedang membaca buku.");
    assert_eq!(morph(find(&t, "membaca")), "meN- + baca");
    assert!(t.iter().all(|x| x.lang.lang == Lang::Indonesian));

    let t = analyze("Rumahnya dibersihkan kemarin.");
    assert_eq!(morph(find(&t, "Rumahnya")), "rumah + -nya");
    assert_eq!(morph(find(&t, "dibersihkan")), "di- + bersih + -kan");

    let t = analyze("Anak-anak bermain di taman.");
    let anak = find(&t, "Anak-anak");
    assert_eq!(anak.lookup_form(), "anak-anak");
    assert_eq!(morph(anak), "anak");
    assert_eq!(morph(find(&t, "bermain")), "ber- + main");

    let t = analyze("Kesehatan itu penting.");
    assert_eq!(morph(find(&t, "Kesehatan")), "ke-an + sehat");

    let t = analyze("Apa kabar?");
    assert_eq!(find(&t, "kabar").lang.lang, Lang::Indonesian);
}

#[test]
fn javanese_speech_levels_are_javanese() {
    for s in ["Kowe wis mangan?", "Sampeyan sampun nedha?", "Panjenengan sampun dhahar?"] {
        let t = analyze(s);
        assert!(
            t.iter().all(|x| x.lang.lang == Lang::Javanese),
            "{s}: {:?}",
            t.iter().map(|x| (&x.token.text, x.lang.lang)).collect::<Vec<_>>()
        );
    }
}

#[test]
fn dialect_and_code_mixing() {
    let t = analyze("Koen wis mangan ta, rek?");
    for w in ["Koen", "wis", "mangan", "rek"] {
        assert_eq!(find(&t, w).lang.lang, Lang::Javanese, "{w}");
    }
    let t = analyze("Which is, gue literally capek banget.");
    assert_eq!(find(&t, "Which").lang.lang, Lang::English);
    assert_eq!(find(&t, "literally").lang.lang, Lang::English);
    assert_eq!(find(&t, "gue").lang.lang, Lang::Indonesian);
    assert_eq!(find(&t, "capek").lang.lang, Lang::Indonesian);
}

#[test]
fn offsets_cover_the_text() {
    let s = "arp dicul piro 👍";
    for t in Engine::default().analyze(s) {
        assert_eq!(&s[t.token.start..t.token.end], t.token.text);
    }
}
