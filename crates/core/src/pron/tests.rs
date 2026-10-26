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
    assert_eq!(id("Apa kabar?"), "آپَا كَابَارْ؟");
    assert_eq!(id("Saya sedang membaca buku."), "سَايَا سِدَاڠْ مِمْبَاتْشَا بُوكُو.");
    assert_eq!(id("Anak-anak bermain di taman."), "آنَاءْ-آنَاءْ بِرْمَائِينْ دِي تَامَانْ.");
    assert_eq!(id("Kesehatan itu penting."), "كِسێهَاتَانْ إِيتُو پِنْتِيڠْ.");
}

#[test]
fn letters_arabic_lacks() {
    assert_eq!(id("pagi"), "پَاجِي");
    assert_eq!(id("cari"), "تْشَارِي");
    assert_eq!(id("jalan"), "چَالَانْ");
    assert_eq!(id("sore"), "سۆرێ");
}

#[test]
fn glides_and_diphthongs() {
    assert_eq!(id("dia"), "دِيَا");
    assert_eq!(id("buah"), "بُوَاهْ");
    assert_eq!(id("pantai"), "پَانْتَايْ");
    assert_eq!(id("kalau"), "كَالَاوْ");
}

#[test]
fn bridge_words() {
    assert_eq!(id("selamat"), "سِلَامَاتْ");
    assert_eq!(id("pikir"), "پِيكِيرْ");
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
    assert_eq!(jv("Kowe wis mangan?"), "كۆوێ وِيسْ مَاڠَانْ؟");
    assert_eq!(jv("Sampeyan sampun nedha?"), "سَامْپێيَانْ سَامْپُونْ نِضۆ؟");
    assert_eq!(jv("Panjenengan sampun dhahar?"), "پَانْچِنِڠَانْ سَامْپُونْ ضَاهَارْ؟");
}

#[test]
fn overrides_must_match_their_word() {
    assert!(PronDict::parse("", "jv\tkabeh\tkabèh\n").is_ok());
    assert!(PronDict::parse("", "jv\tkabeh\tkebèh\n").is_err());
    assert!(PronDict::parse("", "xx\tkabeh\tkabèh\n").is_err());
    assert!(PronDict::parse("besar\tbəsar\nbesar\tbesar\n", "").is_err());
}

#[test]
fn every_lexicon_word_renders_in_arabic_letters() {
    let d = PronDict::builtin();
    for (word, phones) in &d.id {
        let a = arabic(phones);
        assert!(!a.is_empty(), "{word}");
        assert!(a.chars().all(|c| ('\u{0600}'..='\u{06FF}').contains(&c) || c == '-'), "{word} → {a}");
    }
}
