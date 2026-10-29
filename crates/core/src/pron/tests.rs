use super::*;

fn id(w: &str) -> String {
    PronDict::builtin().text(w, Lang::Indonesian)
}

fn jv(w: &str) -> String {
    PronDict::builtin().text(w, Lang::Javanese)
}

fn id_codes(w: &str) -> String {
    codes(&PronDict::builtin().phones(w, Lang::Indonesian))
}

fn jv_codes(w: &str) -> String {
    codes(&PronDict::builtin().phones(w, Lang::Javanese))
}

#[test]
fn builtin_data_parses() {
    let d = PronDict::builtin();
    assert!(d.id.len() > 10_000, "lexicon has {} words", d.id.len());
}

#[test]
fn rules_cover_regular_spelling() {
    assert_eq!(codes(&rule_phones("nyanyi", Lang::Indonesian)), "ɲaɲi");
    assert_eq!(codes(&rule_phones("tanggal", Lang::Indonesian)), "taŋgal");
    assert_eq!(codes(&rule_phones("syukur", Lang::Indonesian)), "ʃukur");
    assert_eq!(codes(&rule_phones("tidak", Lang::Indonesian)), "tidaʔ");
    assert_eq!(codes(&rule_phones("rakyat", Lang::Indonesian)), "raʔyat");
    assert_eq!(codes(&rule_phones("taksi", Lang::Indonesian)), "taksi");
    assert_eq!(codes(&rule_phones("maaf", Lang::Indonesian)), "maʔaf");
    assert_eq!(codes(&rule_phones("sore", Lang::Indonesian)), "sore");
    assert_eq!(codes(&rule_phones("besar", Lang::Indonesian)), "bəsar");
}

#[test]
fn lexicon_settles_e() {
    assert_eq!(id_codes("enak"), "enaʔ");
    assert_eq!(id_codes("enam"), "ənam");
    assert_eq!(id_codes("meja"), "meja");
    assert_eq!(id_codes("kereta"), "kəreta");
}

#[test]
fn affixed_forms_borrow_the_root_e() {
    // Not in the lexicon as such: the e of "enak" stays open.
    let w = "keenakannya";
    assert!(!PronDict::builtin().id.contains_key(w));
    assert_eq!(id_codes(w), "kəenakanɲa");
}

#[test]
fn spec_morphology_sentences() {
    assert_eq!(id("Apa kabar?"), "آبَا كَابَارْ؟");
    assert_eq!(id("Saya sedang membaca buku."), "سَايَا سِدَانْجْ مِمْبَاتْشَا بُوكُو.");
    assert_eq!(id("Anak-anak bermain di taman."), "آنَاءْ-آنَاءْ بِرْمَائِينْ دِي تَامَانْ.");
    assert_eq!(id("Kesehatan itu penting."), "كِسَيْهَاتَانْ إِيتُو بِنْتِينْجْ.");
}

#[test]
fn letters_arabic_lacks() {
    assert_eq!(id("pagi"), "بَاجِي");
    assert_eq!(id("cari"), "تْشَارِي");
    assert_eq!(id("jalan"), "دْجَالَانْ");
    assert_eq!(id("sore"), "سَوْرَيْ");
    assert_eq!(id("orang"), "أَوْرَانْجْ");
    assert_eq!(id("tanggal"), "تَانْجَالْ");
    assert_eq!(id("toko"), "تَوْكَوْ");
    assert_eq!(id("ekor"), "أَيْكَوْرْ");
}

#[test]
fn glides_and_diphthongs() {
    assert_eq!(id("dia"), "دِيَا");
    assert_eq!(id("buah"), "بُوَاهْ");
    assert_eq!(id("pantai"), "بَانْتَايْ");
    assert_eq!(id("kalau"), "كَالَاوْ");
    assert_eq!(id("survei"), "سُورْفَيْ");
    assert_eq!(jv("piye"), "بِيَيْ");
}

#[test]
fn bridge_words() {
    assert_eq!(id("selamat"), "سِلَامَاتْ");
    assert_eq!(id("pikir"), "بِيكِيرْ");
    assert_eq!(id("kursi"), "كُورْسِي");
}

#[test]
fn javanese_open_o_and_heavy_consonants() {
    assert_eq!(jv_codes("apa"), "ɔpɔ");
    assert_eq!(jv_codes("lunga"), "luŋɔ");
    assert_eq!(jv_codes("kanca"), "kɔncɔ");
    assert_eq!(jv_codes("rasane"), "rasane");
    assert_eq!(jv("dhahar"), "ضَاهَارْ");
    assert_eq!(jv_codes("kowe"), "kowe");
    assert_eq!(jv_codes("kabeh"), "kabeh");
    assert_eq!(jv_codes("dereng"), "dereŋ");
    assert_eq!(jv_codes("mbak"), "mbaʔ");
}

#[test]
fn spec_javanese_speech_levels() {
    assert_eq!(jv("Kowe wis mangan?"), "كَوْوَيْ وِيسْ مَانْجَانْ؟");
    assert_eq!(jv("Sampeyan sampun nedha?"), "سَامْبَيْيَانْ سَامْبُونْ نِضَوْ؟");
    assert_eq!(jv("Panjenengan sampun dhahar?"), "بَانْدْجِنِنْجَانْ سَامْبُونْ ضَاهَارْ؟");
}

#[test]
fn overrides_must_match_their_word() {
    assert!(PronDict::parse("", "jv\tkabeh\tkabèh\n").is_ok());
    assert!(PronDict::parse("", "jv\tkabeh\tkebèh\n").is_err());
    assert!(PronDict::parse("", "xx\tkabeh\tkabèh\n").is_err());
    assert!(PronDict::parse("besar\tbəsar\nbesar\tbesar\n", "").is_err());
}

/// Only the original Arabic letters (hamza to ya) and the vowel marks: no
/// Persian or Kurdish extras such as پ چ ڠ ێ ۆ.
fn pure_arabic(c: char) -> bool {
    ('\u{0621}'..='\u{064A}').contains(&c) || ('\u{064E}'..='\u{0652}').contains(&c) || c == '-'
}

#[test]
fn every_lexicon_word_renders_in_original_arabic_letters() {
    let d = PronDict::builtin();
    for (word, phones) in &d.id {
        let a = arabic(phones);
        assert!(!a.is_empty(), "{word}");
        assert!(a.chars().all(pure_arabic), "{word} → {a}");
    }
}

#[test]
fn every_phone_renders_in_original_arabic_letters() {
    for (p, _) in CODES {
        for seg in [vec![*p], vec![*p, Ph::A], vec![Ph::B, *p], vec![Ph::B, *p, Ph::A, Ph::I]] {
            let a = arabic(&seg);
            assert!(a.chars().all(pure_arabic), "{seg:?} → {a}");
        }
    }
}
