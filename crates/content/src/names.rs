//! Two spelling checks on the Arabic text the learner reads.
//!
//! - One Egyptian Arabic spelling per name (#570): `content/names.yaml`
//!   (`kabar.names/v1`) gives the `ar_eg` spelling of each place, food or
//!   other name and the variants to `avoid`. Every `ar_eg` and `arz` text in
//!   the content YAML (golden sets included) and every `arz` gloss in the
//!   word sheets is checked. MSA (`ar`)
//!   keeps press spellings and is not checked.
//! - Any other name or Javanese word respelled with غ for a Latin g (#600):
//!   an `ar_eg`/`arz` word with غ whose consonants match a Latin word with
//!   g in the same item («توغو» next to Tugu) is flagged, list or no list.
//! - Aksara letters need distinct hints (#568): in letter-teaching lines the
//!   Arabic hint is all that tells the learner how a letter sounds, so two
//!   different letters must never share one (pa and ba were both «بَ»).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use crate::{LoadError, read, yaml_files, yaml_value};

pub const NAMES_SCHEMA_TAG: &str = "kabar.names/v1";
pub const NAMES_FILE: &str = "content/names.yaml";

/// One name: its ar_eg spelling and the spellings to flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub latin: String,
    pub ar_eg: String,
    pub avoid: Vec<String>,
}

/// Parses `content/names.yaml`; problems go to `out`.
pub fn parse(v: &Value, out: &mut Vec<String>) -> Vec<Name> {
    if v.get("schema").and_then(Value::as_str) != Some(NAMES_SCHEMA_TAG) {
        out.push(format!("{NAMES_FILE}: schema must be {NAMES_SCHEMA_TAG}"));
    }
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for (i, n) in v.get("names").and_then(Value::as_array).into_iter().flatten().enumerate() {
        let s = |k: &str| n.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let avoid: Vec<String> = n
            .get("avoid")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let (latin, ar_eg) = (s("latin"), s("ar_eg"));
        if latin.is_empty() || ar_eg.is_empty() || avoid.is_empty() {
            out.push(format!("{NAMES_FILE}: names[{i}] needs latin, ar_eg and a non-empty avoid list"));
            continue;
        }
        for a in &avoid {
            if *a == ar_eg {
                out.push(format!("{NAMES_FILE}: {latin}: avoid lists its own spelling «{a}»"));
            }
            if !seen.insert(a.clone()) {
                out.push(format!("{NAMES_FILE}: «{a}» is avoided twice"));
            }
        }
        names.push(Name { latin, ar_eg, avoid });
    }
    names
}

/// Arabic letters, marks and tatweel: the characters a word is made of.
fn is_letter(c: char) -> bool {
    matches!(c, '\u{0621}'..='\u{0652}' | '\u{0670}')
}

/// Proclitics a name may carry: و/ف, then ب/ك/ل or لل, then ال.
fn strip_clitics(word: &str) -> Vec<&str> {
    let mut out = vec![word];
    let stage = |prefixes: &[&str], out: &mut Vec<&str>| {
        let more: Vec<&str> =
            out.iter().flat_map(|w| prefixes.iter().filter_map(move |p| w.strip_prefix(p))).collect();
        out.extend(more);
    };
    stage(&["و", "ف"], &mut out);
    stage(&["ب", "ك", "ل", "لل"], &mut out);
    stage(&["ال"], &mut out);
    out
}

/// The avoided spellings `text` uses, each with the name it belongs to.
pub fn misspellings<'a>(text: &str, names: &'a [Name]) -> Vec<(&'a Name, &'a str)> {
    let mut found = Vec::new();
    for word in text.split(|c: char| !is_letter(c)).filter(|w| !w.is_empty()) {
        let bases = strip_clitics(word);
        for n in names {
            if let Some(a) = n.avoid.iter().find(|a| bases.contains(&a.as_str())) {
                found.push((n, a.as_str()));
            }
        }
    }
    found
}

/// Every string under an `ar_eg` key (a string or a list of strings).
fn ar_eg_texts<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a str>) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                if k == key {
                    match v {
                        Value::String(s) => out.push(s),
                        Value::Array(a) => out.extend(a.iter().filter_map(Value::as_str)),
                        _ => ar_eg_texts(v, key, out),
                    }
                } else {
                    ar_eg_texts(v, key, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|v| ar_eg_texts(v, key, out)),
        _ => {}
    }
}

fn report(file: &str, texts: &[&str], names: &[Name], out: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for t in texts {
        for (n, a) in misspellings(t, names) {
            if seen.insert(a) {
                out.push(format!("{file}: ar_eg spells {} «{a}»; use «{}» ({NAMES_FILE})", n.latin, n.ar_eg));
            }
        }
    }
}

/// Checks every ar_eg text under `content/` (packs aside: they are built from
/// the rest) and the word sheets' `arz` glosses against `content/names.yaml`.
pub fn check(root: &Path, out: &mut Vec<String>) -> Result<(), LoadError> {
    let path = root.join(NAMES_FILE);
    if !path.exists() {
        return Ok(());
    }
    let names = parse(&yaml_value(&path)?, out);
    let content = root.join("content");
    let mut files = Vec::new();
    yaml_files(&content, &mut files)?;
    for file in files.iter().filter(|f| !f.starts_with(content.join("packs")) && **f != path) {
        let v = yaml_value(file)?;
        let mut texts = Vec::new();
        ar_eg_texts(&v, "ar_eg", &mut texts);
        // The golden sets write Egyptian Arabic under `arz` (#590): an engine
        // that spells names the house way must not score as a miss.
        ar_eg_texts(&v, "arz", &mut texts);
        report(&file.display().to_string(), &texts, &names, out);
        check_ghain(&v, &mut Vec::new(), &file.display().to_string(), &mut BTreeSet::new(), out);
    }
    for lang in ["id", "jv"] {
        let dir = content.join("glosses").join(lang);
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut sheets: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
            .collect();
        sheets.sort();
        for file in sheets {
            let mut texts = Vec::new();
            let lines: Vec<Value> =
                read(&file)?.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
            let mut seen = BTreeSet::new();
            for v in &lines {
                ar_eg_texts(v, "arz", &mut texts);
                check_ghain(v, &mut Vec::new(), &file.display().to_string(), &mut seen, out);
            }
            report(&file.display().to_string(), &texts, &names, out);
        }
    }
    Ok(())
}

// --- غ for a Latin g (#600) ---------------------------------------------

/// An Arabic transliteration read back as Latin letters, vowels included
/// (ا a, و u/o, ي i/e), غ and ج both g, marks dropped and doubled letters
/// folded: «ألينغكا» → "alingka". A hamza seat before و or ي is silent
/// («إيموغيري» → "imugiri"). `None` when a letter has no reading here.
fn arabic_reading(word: &str) -> Option<String> {
    let cs: Vec<char> = word.chars().collect();
    let mut out = String::new();
    for (i, &c) in cs.iter().enumerate() {
        let l = match c {
            'أ' | 'إ' | 'آ' if matches!(cs.get(i + 1), Some('و' | 'ي')) => "",
            'ا' | 'أ' | 'إ' | 'آ' | 'ى' => "a",
            'و' => "u",
            'ي' => "i",
            'ب' => "b",
            'ت' | 'ط' => "t",
            'ث' | 'س' | 'ص' | 'ش' => "s",
            'ج' | 'غ' => "g",
            'ح' | 'ه' | 'ة' => "h",
            'خ' | 'ق' | 'ك' => "k",
            'د' | 'ض' => "d",
            'ذ' | 'ز' | 'ظ' => "z",
            'ر' => "r",
            'ف' => "f",
            'ل' => "l",
            'م' => "m",
            'ن' => "n",
            'ء' | 'ؤ' | 'ئ' | 'ع' | '\u{064B}'..='\u{0652}' | '\u{0670}' | 'ـ' => "",
            _ => return None,
        };
        if !l.is_empty() && !out.ends_with(l) {
            out.push_str(l);
        }
    }
    Some(out)
}

/// A Latin word in the alphabet of [`arabic_reading`]: p as b, j as g, c as
/// ts, o as u, sy as s; `E` marks an e, which a transliteration writes as ي
/// or leaves out (sengkalan → «سنغكالان»).
fn latin_reading(word: &str) -> String {
    let w = word.to_lowercase().replace("sy", "s").replace("kh", "k").replace("dh", "d").replace("th", "t");
    let mut out = String::new();
    for c in w.chars() {
        let l = match c {
            'a' => "a",
            'i' | 'y' => "i",
            'e' => "E",
            'o' | 'u' | 'w' => "u",
            'b' | 'p' => "b",
            't' => "t",
            'c' => "ts",
            's' | 'x' => "s",
            'g' | 'j' => "g",
            'h' => "h",
            'k' | 'q' => "k",
            'd' => "d",
            'z' => "z",
            'r' => "r",
            'f' | 'v' => "f",
            'l' => "l",
            'm' => "m",
            'n' => "n",
            _ => "",
        };
        for ch in l.chars() {
            if !out.ends_with(ch) {
                out.push(ch);
            }
        }
    }
    out
}

/// Whether the Arabic reading spells the Latin one (`E` is i or nothing).
fn reads_as(ar: &[u8], la: &[u8]) -> bool {
    match (ar.first(), la.first()) {
        (_, Some(b'E')) => {
            reads_as(ar, &la[1..]) || (ar.first() == Some(&b'i') && reads_as(&ar[1..], &la[1..]))
        }
        (None, None) => true,
        (Some(a), Some(l)) if a == l => reads_as(&ar[1..], &la[1..]),
        _ => false,
    }
}

fn latin_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| w.len() > 1)
}

/// Arabic words in `text` that spell, with غ, a Latin word from `latin`
/// that has a g: «توغو» for Tugu. Arabic loans written gh (maghrib) are
/// fine, since gh is غ.
/// Words that keep غ: Arabic words that happen to read like a Latin word
/// with g, and Chinese and Korean names, which Arabic writes with غ by
/// convention (kung fu «كونغ فو», Jiang «جيانغ»).
const GHAIN_WORDS: &[&str] = &[
    "غايب",
    "غائب",
    "غريم",
    "فوتوغرافي",
    "فوتوغرافيا",
    "غوريلا",
    "كونغ",
    "جيانغ",
    "جانغ",
    "بونغ",
    "بانغ",
    "لينغ",
    "كيونغ",
];

pub fn ghain_for_g<'a>(text: &'a str, latin: &[&str]) -> Vec<(&'a str, String)> {
    let targets: Vec<(String, &str)> = latin
        .iter()
        .flat_map(|t| latin_words(t))
        .filter(|w| w.to_lowercase().contains('g'))
        .map(|w| (latin_reading(w), w))
        .collect();
    let mut found = Vec::new();
    for word in text.split(|c: char| !is_letter(c)).filter(|w| w.contains('غ')) {
        for base in strip_clitics(word) {
            if GHAIN_WORDS.contains(&base) {
                break;
            }
            let Some(reading) = arabic_reading(base) else { continue };
            // A Latin gh (maghrib) is what غ is for, unless the Arabic
            // writes the h too (Beringharjo «بيرينغهارجو»).
            let fits = |l: &str| {
                reads_as(reading.as_bytes(), l.as_bytes()) && (!l.contains("gh") || reading.contains("gh"))
            };
            if let Some((_, latin)) = targets.iter().find(|(l, _)| fits(l)) {
                found.push((word, (*latin).to_owned()));
                break;
            }
        }
    }
    found
}

/// Every object with an `ar_eg`/`arz` text, checked against the Latin
/// strings of that object and the objects around it (a dialogue line's
/// `text`, a note's `title`).
fn check_ghain(
    v: &Value,
    context: &mut Vec<String>,
    file: &str,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<String>,
) {
    match v {
        Value::Object(m) => {
            let pushed = m.values().filter_map(Value::as_str).filter(|s| !s.chars().any(is_letter)).count();
            context.extend(
                m.values().filter_map(Value::as_str).filter(|s| !s.chars().any(is_letter)).map(str::to_owned),
            );
            for (k, val) in m {
                if k == "ar_eg" || k == "arz" {
                    let mut texts = Vec::new();
                    match val {
                        Value::String(s) => texts.push(s.as_str()),
                        Value::Array(a) => texts.extend(a.iter().filter_map(Value::as_str)),
                        _ => {}
                    }
                    for t in texts {
                        // Latin inside the Arabic text counts too.
                        let mut latin: Vec<&str> = context.iter().map(String::as_str).collect();
                        latin.push(t);
                        for (ar, la) in ghain_for_g(t, &latin) {
                            if seen.insert(ar.to_owned()) {
                                out.push(format!(
                                    "{file}: {k} writes «{ar}» for {la} with غ; g is ج (ng is نج)"
                                ));
                            }
                        }
                    }
                } else {
                    check_ghain(val, context, file, seen, out);
                }
            }
            context.truncate(context.len() - pushed);
        }
        Value::Array(a) => a.iter().for_each(|v| check_ghain(v, context, file, seen, out)),
        _ => {}
    }
}

/// A single Javanese-script letter (the aksara block, U+A980–U+A9DF).
fn is_aksara_letter(s: &str) -> bool {
    let mut cs = s.chars();
    matches!((cs.next(), cs.next()), (Some('\u{A980}'..='\u{A9DF}'), None))
}

/// The Arabic hint of a letter line: its first parenthesised part
/// ("الحرف pa (بَ، بس p مش b)" → "بَ، بس p مش b"), else the whole text.
fn hint(text: &str) -> &str {
    match (text.find('('), text.find(')')) {
        (Some(a), Some(b)) if a < b => text[a + 1..b].trim(),
        _ => text.trim(),
    }
}

/// Aksara letters taught in lessons (`new_words` with `pos: letter`, and
/// `match_pairs` whose `text` is one aksara letter): no two different
/// letters may share an Arabic hint in ar_eg or ar.
pub fn check_letter_hints(lessons: &BTreeMap<String, Value>, out: &mut Vec<String>) {
    for key in ["ar_eg", "ar"] {
        // hint -> letter -> first lesson
        let mut by_hint: BTreeMap<&str, BTreeMap<&str, &str>> = BTreeMap::new();
        for (lesson, v) in lessons {
            let words = v.get("new_words").and_then(Value::as_array).into_iter().flatten();
            let letters = words
                .filter(|w| w.get("pos").and_then(Value::as_str) == Some("letter"))
                .filter_map(|w| Some((w.get("word")?.as_str()?, w.get(key)?.as_str()?)));
            let pairs = v
                .get("exercises")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .flat_map(|e| e.get("pairs").and_then(Value::as_array).into_iter().flatten())
                .filter_map(|p| Some((p.get("text")?.as_str()?, p.get(key)?.as_str()?)))
                .filter(|(t, _)| is_aksara_letter(t));
            for (letter, text) in letters.chain(pairs) {
                let h = hint(text);
                if !h.is_empty() {
                    by_hint.entry(h).or_default().entry(letter).or_insert(lesson);
                }
            }
        }
        for (h, letters) in by_hint.into_iter().filter(|(_, l)| l.len() > 1) {
            let list: Vec<String> = letters.iter().map(|(l, lesson)| format!("{l} ({lesson})")).collect();
            out.push(format!(
                "aksara letters {} share the {key} hint «{h}»: give each its own (add the Latin contrast, e.g. «بَ، بس p مش b»)",
                list.join(", ")
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn names() -> Vec<Name> {
        let mut out = Vec::new();
        let v = json!({"schema": NAMES_SCHEMA_TAG, "names": [
            {"latin": "Jogja", "ar_eg": "جوجا", "avoid": ["جوكجا", "جوغجا"]},
            {"latin": "ngoko", "ar_eg": "نجوكو", "avoid": ["نغوكو"]},
        ]});
        let n = parse(&v, &mut out);
        assert!(out.is_empty(), "{out:?}");
        n
    }

    #[test]
    fn flags_avoided_spellings_with_clitics() {
        let n = names();
        let found = |t: &str| misspellings(t, &n).into_iter().map(|(_, a)| a.to_owned()).collect::<Vec<_>>();
        assert_eq!(found("القطر جه من جوكجا ناحية الشرق"), ["جوكجا"]);
        assert_eq!(found("ولجوغجا، وبالنغوكو"), ["جوغجا", "نغوكو"]);
        // The chosen spelling, longer words and MSA forms stay quiet.
        assert!(found("من جوجا بالنجوكو").is_empty());
        assert!(found("جوكجاكرتا").is_empty());
    }

    #[test]
    fn ghain_for_a_latin_g_is_flagged() {
        let hits = |t: &str, latin: &[&str]| {
            ghain_for_g(t, latin).into_iter().map(|(a, l)| format!("{a}={l}")).collect::<Vec<_>>()
        };
        assert_eq!(hits("روح ألينغكا.", &["Hanoman, kowe lunga menyang Alengka."]), ["ألينغكا=Alengka"]);
        assert_eq!(hits("عند التوغو", &["ing Tugu"]), ["التوغو=Tugu"]);
        assert_eq!(hits("سوق بيرينغهارجو", &["pasar Beringharjo"]), ["بيرينغهارجو=Beringharjo"]);
        // ج is right; Arabic words with غ and loans written gh stay quiet.
        assert!(hits("روح ألينجكا", &["Alengka"]).is_empty());
        assert!(hits("صلاة المغرب غالي", &["sholat maghrib", "larang"]).is_empty());
        // Arabic words that only share consonants with a Latin word stay quiet.
        assert!(hits("غير كده، في أغسطس", &["guru", "agar", "Agustus"]).is_empty());
        assert_eq!(hits("السنغكالان", &["sengkalan"]), ["السنغكالان=sengkalan"]);
        assert_eq!(hits("إيموغيري", &["Imogiri"]), ["إيموغيري=Imogiri"]);
    }

    #[test]
    fn names_file_problems() {
        let mut out = Vec::new();
        parse(
            &json!({"schema": "x", "names": [
                {"latin": "a", "ar_eg": "ب", "avoid": ["ب"]},
                {"latin": "c", "ar_eg": "د"},
            ]}),
            &mut out,
        );
        assert_eq!(out.len(), 3, "{out:?}");
    }

    #[test]
    fn letter_hints_must_differ() {
        let lesson = |w: &str, h: &str| {
            json!({"new_words": [{"word": w, "pos": "letter", "ar_eg": format!("الحرف x ({h})")}],
                   "exercises": [{"type": "match_pairs", "pairs": [{"text": w, "ar_eg": format!("x ({h})")}]}]})
        };
        let mut lessons = BTreeMap::new();
        lessons.insert("j6.u01.l03".to_owned(), lesson("ꦥ", "بَ"));
        lessons.insert("j6.u01.l04".to_owned(), lesson("ꦧ", "بَ"));
        let mut out = Vec::new();
        check_letter_hints(&lessons, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].contains("ꦥ (j6.u01.l03)") && out[0].contains("ꦧ (j6.u01.l04)"));

        lessons.insert("j6.u01.l04".to_owned(), lesson("ꦧ", "بَ، الـ b العادية"));
        out.clear();
        check_letter_hints(&lessons, &mut out);
        assert!(out.is_empty(), "{out:?}");
    }
}
