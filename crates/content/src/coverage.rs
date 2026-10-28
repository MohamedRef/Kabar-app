//! Reading coverage (#138): the share of word tokens in a Sinetron episode,
//! Kabar Radio script or lesson dialogue that the curriculum has not taught by
//! the time the learner meets it. Comfortable reading needs about 95–98% known
//! words (Laufer 1989; Hu & Nation 2000), so items above [`Item::limit`] get
//! flagged for glossing or re-wording. Glosses in notes may close the gap, but
//! only so far: the raw share, with notes ignored, has its own cap
//! ([`Item::raw_limit`]), so a script cannot be glossed into shape.
//!
//! A word counts as known when a `new_words` entry at or before the item's
//! position teaches it (a taught phrase covers its words; a suffixed or
//! reduplicated form is covered by its root; a street form such as "nggak" by
//! its formal twin in `content/freq/id.street.tsv`, a se- measure such as
//! "seratus" by its root, a chat spelling such as "tdk" by its full form, and
//! a plainly inflected form such as "kirim" or "dikirim" by a taught word on
//! the same root, "mengirim"), when the note of that line
//! or an earlier line of the same item explains it (the learner met the gloss), or when it is not vocabulary at all: a name (capitalised and
//! either a cast name or absent from the frequency list), a number, or an
//! entry in `content/freq/<lang>.core-exclude.tsv`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use kabar_core::lang::morph_id::IndonesianMorph;
use kabar_core::lang::{Lang, normalize, street};
use serde_json::Value;

use crate::core_words::{mask, suffixes, tokens, word_list};
use crate::model::Curriculum;
use crate::retrieval::forms;
use crate::{LoadError, yaml_files, yaml_value};

/// One text the learner reads or hears.
#[derive(Debug, Clone)]
pub struct Item {
    /// `sinetron.e07`, `radio.bipa1.u03` or a lesson id.
    pub id: String,
    /// `sinetron`, `radio` or `dialogue`.
    pub kind: &'static str,
    /// The unit (or, for a dialogue, the lesson) it is measured against.
    pub after: String,
    /// Index of the level `after` belongs to, from 0.
    pub level: usize,
    /// Word tokens counted (names and numbers left out).
    pub tokens: usize,
    /// Unknown tokens, with repeats.
    pub unknown: Vec<String>,
    /// Unknown tokens when note glosses are ignored (raw coverage).
    pub raw: Vec<String>,
}

impl Item {
    /// Unknown tokens as a percentage of all counted tokens.
    pub fn percent(&self) -> f64 {
        if self.tokens == 0 { 0.0 } else { 100.0 * self.unknown.len() as f64 / self.tokens as f64 }
    }

    /// The highest acceptable unknown share: 8% in the first two levels of a
    /// track, 12% later.
    pub fn limit(&self) -> f64 {
        if self.level < 2 { 8.0 } else { 12.0 }
    }

    pub fn over(&self) -> bool {
        self.percent() > self.limit()
    }

    /// Unknown share when note glosses are ignored: what the script asks of
    /// the learner before any help.
    pub fn raw_percent(&self) -> f64 {
        if self.tokens == 0 { 0.0 } else { 100.0 * self.raw.len() as f64 / self.tokens as f64 }
    }

    /// The highest acceptable raw share: 15% in the first two levels, 18% later.
    /// Glosses may close the gap to [`Item::limit`], but only this far.
    pub fn raw_limit(&self) -> f64 {
        if self.level < 2 { 15.0 } else { 18.0 }
    }

    pub fn raw_over(&self) -> bool {
        self.raw_percent() > self.raw_limit()
    }

    /// The most frequent unknown words, most frequent first.
    pub fn top(&self, n: usize) -> Vec<(String, usize)> {
        top(&self.unknown, n)
    }

    /// Like [`Item::top`], for the raw unknowns (notes ignored).
    pub fn top_raw(&self, n: usize) -> Vec<(String, usize)> {
        top(&self.raw, n)
    }
}

fn top(words: &[String], n: usize) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for w in words {
        *counts.entry(w).or_default() += 1;
    }
    let mut v: Vec<(String, usize)> = counts.into_iter().map(|(w, c)| (w.to_owned(), c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.truncate(n);
    v
}

/// What a track has taught, lesson by lesson.
struct Track {
    lang: String,
    /// Lesson ids in curriculum order (only lessons that exist).
    order: Vec<String>,
    /// Word form -> position of the first lesson teaching it.
    taught: BTreeMap<String, usize>,
    /// Unit id -> (position of its last lesson, level index).
    units: BTreeMap<String, (usize, usize)>,
    /// Lesson id -> level index.
    lesson_level: BTreeMap<String, usize>,
    exclude: BTreeSet<String>,
    dictionary: BTreeSet<String>,
    /// Indonesian root -> first position a taught word built on it appears
    /// (mengirim teaches kirim; setiap teaches tiap). Empty for Javanese.
    roots: BTreeMap<String, usize>,
}

/// The root of `w` when it is built only with inflection a BIPA 1–3 learner
/// reads through (meN-, ber-, di-, ter-, se-, -kan, -i, clitics), else `None`.
/// Derivations that change the meaning (peN-, ke-an, per-an, -an) don't count.
fn inflected_root(w: &str) -> Option<String> {
    static MORPH: OnceLock<IndonesianMorph> = OnceLock::new();
    let a = MORPH.get_or_init(IndonesianMorph::default).analyze(w)?;
    let plain = a.confix.is_none()
        && a.prefixes.iter().all(|p| matches!(p.label.as_str(), "meN-" | "ber-" | "di-" | "ter-" | "se-"))
        && a.suffix.as_ref().is_none_or(|s| matches!(s.label.as_str(), "-kan" | "-i"));
    plain.then_some(a.root)
}

impl Track {
    fn known(&self, w: &str, pos: usize) -> bool {
        let at = |w: &str| self.taught.get(w).is_some_and(|&p| p <= pos);
        if at(w) || self.exclude.contains(w) {
            return true;
        }
        // A street form ("nggak", "makasih", Javanese "nggih") is known once
        // its formal twin is, and a street -in verb ("bacain", "dengerin")
        // once its root is.
        if self.lang == "jv" && street::formal(Lang::Javanese, w).is_some_and(at) {
            return true;
        }
        if self.lang == "id" {
            let street = |w: &str| at(w) || street::formal(Lang::Indonesian, w).is_some_and(at);
            if street(w) {
                return true;
            }
            // se- + a taught measure or number root: seratus, sekilo, sebotol.
            if w.strip_prefix("se").is_some_and(|r| r.len() > 2 && at(r)) {
                return true;
            }
            let root = w.strip_prefix("di").filter(|r| r.len() > 4).unwrap_or(w);
            if w.len() > 5 && root.strip_suffix("in").is_some_and(|r| r.len() > 2 && street(r)) {
                return true;
            }
            // Same root, plain inflection: kirim once mengirim is taught,
            // dibaca once membaca is.
            if inflected_root(w).and_then(|r| self.roots.get(&r)).is_some_and(|&p| p <= pos) {
                return true;
            }
        }
        // A chat spelling ("tp", "udh") is known once its full form is: the
        // reader shows the expansion.
        if normalize::lookup(w)
            .is_some_and(|n| n.lang == Lang::English || n.full.starts_with('(') || n.full.split(' ').all(at))
        {
            return true;
        }
        if let Some((a, b)) = w.split_once('-')
            && a == b
            && at(a)
        {
            return true;
        }
        // Krama "dipunwaos", "tumbasaken": known once the affix lesson and
        // the root are.
        if self.lang == "jv" {
            let affixed = |affix: &str, root: Option<&str>| {
                at(affix) && root.is_some_and(|r| r.len() > 2 && self.known(r, pos))
            };
            if affixed("dipun-", w.strip_prefix("dipun")) || affixed("-aken", w.strip_suffix("aken")) {
                return true;
            }
        }
        suffixes(&self.lang).iter().any(|s| w.len() > s.len() + 2 && w.strip_suffix(s).is_some_and(at))
    }

    /// Taught phrases at `pos`, longest first, each also with a clitic on
    /// its last word ("rumusan masalahnya", "sesi tanya jawabnya").
    fn phrases(&self, pos: usize) -> Vec<String> {
        let clitics: &[&str] =
            if self.lang == "id" { &["nya", "ku", "mu"] } else { &["e", "ne", "ku", "mu"] };
        let mut p: Vec<String> = self
            .taught
            .iter()
            .filter(|(w, p)| w.contains(' ') && **p <= pos)
            .flat_map(|(w, _)| {
                std::iter::once(w.clone()).chain(clitics.iter().map(move |c| format!("{w}{c}")))
            })
            .collect();
        p.sort_by_key(|p| std::cmp::Reverse(p.len()));
        p
    }
}

fn tokens_of(note: &str) -> Vec<String> {
    tokens(&note.to_lowercase())
}

fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// Every `{speaker, text}` line under `v`, with the text of its `note`.
fn lines<'a>(v: &'a Value, out: &mut Vec<(&'a str, String)>) {
    match v {
        Value::Object(o) => {
            if let (Some(Value::String(text)), true) = (o.get("text"), o.contains_key("speaker")) {
                let note = o.get("note").map(|n| n.to_string()).unwrap_or_default();
                out.push((text, note));
                return;
            }
            o.values().for_each(|v| lines(v, out));
        }
        Value::Array(a) => a.iter().for_each(|v| lines(v, out)),
        _ => {}
    }
}

/// [`mask`] that matches `phrase` (lower case) in any letter case, keeping
/// the rest of `text` as written so capitalised names stay recognisable.
fn mask_any_case(text: &str, phrase: &str) -> String {
    let lowered = text.to_lowercase();
    if lowered.len() != text.len() || !lowered.contains(phrase) {
        return text.to_owned();
    }
    let masked = mask(&lowered, phrase);
    // `mask` swaps each bounded match for " # "; copy the original text
    // around the matches.
    let mut out = String::new();
    let (mut i, mut j) = (0, 0);
    while i < lowered.len() {
        if lowered[i..].starts_with(phrase) && masked[j..].starts_with(" # ") {
            out.push_str(" # ");
            i += phrase.len();
            j += 3;
        } else {
            let c = text[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
            j += c.len_utf8();
        }
    }
    out
}

/// Javanese (aksara) or Arabic script.
fn script(c: char) -> bool {
    matches!(c, '\u{0600}'..='\u{06FF}' | '\u{A980}'..='\u{A9DF}')
}

/// Lowercased words of `text` the learner should know at `pos`, and the
/// unknown ones among them.
fn measure(
    track: &Track,
    names: &BTreeSet<String>,
    pos: usize,
    text: &str,
    explained: &BTreeSet<String>,
) -> (usize, Vec<String>) {
    let text = track.phrases(pos).iter().fold(text.to_owned(), |t, p| mask_any_case(&t, p));
    let mut count = 0;
    let mut unknown = Vec::new();
    let sentences = text.split(['.', '!', '?', '…', ':', ';', '"', '“', '”', '(', ')', '\n']);
    let words = sentences.flat_map(|s| tokens(s).into_iter().enumerate());
    for (at, tok) in words {
        // Numbers, single letters ("a" as in "written a, read o") and words
        // shown in Javanese or Arabic script are not vocabulary to read.
        if tok.chars().any(|c| c.is_ascii_digit() || script(c)) || tok.chars().count() == 1 {
            continue;
        }
        let w = tok.to_lowercase();
        // A capital inside a sentence marks a name or place; at the start of
        // a sentence it only does when the word is not in the dictionary.
        let capital = tok.chars().next().is_some_and(char::is_uppercase);
        let name = capital && (at > 0 || names.contains(&w) || !track.dictionary.contains(&w));
        if name && !track.known(&w, pos) {
            continue;
        }
        count += 1;
        if !track.known(&w, pos) && !explained.contains(&w) {
            unknown.push(w);
        }
    }
    (count, unknown)
}

fn tracks(root: &Path) -> Result<Vec<Track>, LoadError> {
    let mut files = Vec::new();
    yaml_files(&root.join("curriculum"), &mut files)?;
    let mut out = Vec::new();
    // (track index, level index, required level ids) for cross-track prerequisites.
    let mut requires = Vec::new();
    // Level id -> (track index, position of its last lesson).
    let mut level_end = BTreeMap::new();
    for file in files {
        let Ok(c) = serde_json::from_value::<Curriculum>(yaml_value(&file)?) else { continue };
        let lang = c.language.clone();
        let mut t = Track {
            exclude: word_list(&root.join(format!("content/freq/{lang}.core-exclude.tsv"))),
            dictionary: dictionary(&root.join(format!("content/freq/{lang}.tsv"))),
            lang,
            order: Vec::new(),
            taught: BTreeMap::new(),
            units: BTreeMap::new(),
            lesson_level: BTreeMap::new(),
            roots: BTreeMap::new(),
        };
        for (level, l) in c.levels.iter().enumerate() {
            requires.push((out.len(), t.order.len(), l.requires.clone()));
            for u in &l.units {
                for lesson in &u.lessons {
                    let path = root.join(format!("content/lessons/{}/{}.yaml", t.lang, lesson.id));
                    if !path.exists() {
                        continue;
                    }
                    let pos = t.order.len();
                    let v = yaml_value(&path)?;
                    let words = v.get("new_words").and_then(Value::as_array).into_iter().flatten();
                    for w in words.filter_map(|w| str_field(w, "word")) {
                        for f in forms(w) {
                            t.taught.entry(f).or_insert(pos);
                        }
                    }
                    t.order.push(lesson.id.clone());
                    t.units.insert(u.id.clone(), (pos, level));
                    t.lesson_level.insert(lesson.id.clone(), level);
                }
            }
            if let Some(end) = t.order.len().checked_sub(1) {
                level_end.insert(l.id.clone(), (out.len(), end));
            }
        }
        out.push(t);
    }
    // A level that requires another track's level (J1 requires BIPA 1) starts
    // with that level's words known: the learner has passed it.
    for (track, start, levels) in requires {
        for r in levels {
            let Some(&(other, end)) = level_end.get(&r) else { continue };
            if other == track {
                continue;
            }
            let words: Vec<String> =
                out[other].taught.iter().filter(|&(_, &p)| p <= end).map(|(w, _)| w.clone()).collect();
            for w in words {
                let p = out[track].taught.entry(w).or_insert(start);
                *p = (*p).min(start);
            }
        }
    }
    for t in out.iter_mut().filter(|t| t.lang == "id") {
        for (w, &p) in &t.taught {
            if let Some(r) = inflected_root(w) {
                let e = t.roots.entry(r).or_insert(p);
                *e = (*e).min(p);
            }
        }
    }
    Ok(out)
}

/// Every word of a `rank<TAB>word<TAB>…` frequency list.
fn dictionary(path: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split('\t').nth(1))
        .map(|w| w.trim().to_lowercase())
        .collect()
}

/// The words of every cast name ("Pak Budi" -> pak, budi).
fn cast_names(root: &Path) -> Result<BTreeSet<String>, LoadError> {
    let path = root.join("content/cast.yaml");
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let mut names = BTreeSet::new();
    fn walk(v: &Value, names: &mut BTreeSet<String>) {
        match v {
            Value::Object(o) => {
                if let Some(Value::String(n)) = o.get("name") {
                    names.extend(n.split_whitespace().map(str::to_lowercase));
                }
                o.values().for_each(|v| walk(v, names));
            }
            Value::Array(a) => a.iter().for_each(|v| walk(v, names)),
            _ => {}
        }
    }
    walk(&yaml_value(&path)?, &mut names);
    Ok(names)
}

fn item(
    track: &Track,
    names: &BTreeSet<String>,
    id: String,
    kind: &'static str,
    after: String,
    (pos, level): (usize, usize),
    doc: &Value,
) -> Item {
    let mut ls = Vec::new();
    lines(doc, &mut ls);
    let (mut tokens, mut unknown, mut raw) = (0, Vec::new(), Vec::new());
    // A word glossed in a note stays known for the rest of the item.
    let mut explained = BTreeSet::new();
    for (text, note) in ls {
        explained.extend(tokens_of(&note));
        let (n, u) = measure(track, names, pos, text, &explained);
        tokens += n;
        unknown.extend(u);
        raw.extend(measure(track, names, pos, text, &BTreeSet::new()).1);
    }
    Item { id, kind, after, level, tokens, unknown, raw }
}

/// Coverage of every Sinetron episode, Radio script and lesson dialogue.
pub fn report(root: &Path) -> Result<Vec<Item>, LoadError> {
    let tracks = tracks(root)?;
    let names = cast_names(root)?;
    let mut items = Vec::new();
    for t in &tracks {
        for (pos, id) in t.order.iter().enumerate() {
            let doc = yaml_value(&root.join(format!("content/lessons/{}/{id}.yaml", t.lang)))?;
            if let Some(d) = doc.get("dialogue") {
                items.push(item(t, &names, id.clone(), "dialogue", id.clone(), (pos, t.lesson_level[id]), d));
            }
        }
    }
    let mut files = Vec::new();
    for dir in ["content/stories", "content/radio"] {
        if root.join(dir).exists() {
            yaml_files(&root.join(dir), &mut files)?;
        }
    }
    for file in files {
        let doc = yaml_value(&file)?;
        let (Some(id), Some(lang)) = (str_field(&doc, "id"), str_field(&doc, "language")) else { continue };
        let (kind, after) = match (str_field(&doc, "unlock_after"), str_field(&doc, "unit")) {
            (Some(u), _) => ("sinetron", u),
            (None, Some(u)) => ("radio", u),
            _ => continue,
        };
        let Some(t) = tracks.iter().find(|t| t.lang == lang) else { continue };
        let Some(&at) = t.units.get(after) else { continue };
        items.push(item(t, &names, id.to_owned(), kind, after.to_owned(), at, &doc));
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> Track {
        Track {
            lang: "id".into(),
            order: vec!["a".into(), "b".into()],
            taught: [
                ("saya", 0),
                ("makan", 0),
                ("rumah sakit", 0),
                ("nasi", 1),
                ("tidak", 1),
                ("terima kasih", 1),
                ("baca", 1),
                ("dengar", 1),
            ]
            .into_iter()
            .map(|(w, p)| (w.to_owned(), p))
            .collect(),
            units: BTreeMap::new(),
            lesson_level: BTreeMap::new(),
            exclude: BTreeSet::new(),
            roots: BTreeMap::new(),
            dictionary: ["saya", "makan", "nasi", "pecel", "sari"].into_iter().map(String::from).collect(),
        }
    }

    fn ex(note: &str) -> BTreeSet<String> {
        tokens_of(note).into_iter().collect()
    }

    #[test]
    fn a_glossed_word_stays_known_for_the_rest_of_the_item() {
        let doc = serde_json::json!({"lines": [
            {"speaker": "A", "text": "Saya makan pecel.", "note": {"en": "pecel = salad with peanut sauce"}},
            {"speaker": "B", "text": "Pecel enak.", "note": {"en": ""}},
        ]});
        let i = item(&track(), &BTreeSet::new(), "x".into(), "radio", "u".into(), (0, 0), &doc);
        assert_eq!(i.unknown, vec!["enak".to_owned()]);
        assert_eq!(i.raw.len(), 3);
    }

    #[test]
    fn counts_unknown_words_against_the_position() {
        let names = BTreeSet::from(["sari".to_owned()]);
        let (n, u) = measure(&track(), &names, 0, "Sari, saya makan nasi di rumah sakit.", &ex(""));
        assert_eq!((n, u), (4, vec!["nasi".to_owned(), "di".to_owned()]));
        let (_, u) = measure(&track(), &names, 1, "Saya makan nasinya.", &ex(""));
        assert!(u.is_empty());
    }

    #[test]
    fn notes_explain_words_and_names_and_numbers_are_skipped() {
        let names = BTreeSet::new();
        let (n, u) =
            measure(&track(), &names, 0, "Saya makan pecel 2 kali, Pak Joko.", &ex("pecel = طبق خضار"));
        assert_eq!(n, 4);
        assert_eq!(u, vec!["kali".to_owned()]);
        let (n, u) = measure(&track(), &names, 0, "Saya makan a, ꦱꦺꦴꦠꦺꦴ, نور.", &ex(""));
        assert_eq!((n, u), (2, Vec::<String>::new()));
    }

    #[test]
    fn street_forms_count_once_their_formal_twin_is_taught() {
        let names = BTreeSet::new();
        let (_, u) = measure(&track(), &names, 0, "Saya nggak makan, makasih!", &ex(""));
        assert_eq!(u, vec!["nggak".to_owned(), "makasih".to_owned()]);
        let (_, u) = measure(&track(), &names, 1, "Saya nggak makan, makasih!", &ex(""));
        assert!(u.is_empty());
        let (_, u) = measure(&track(), &names, 1, "Saya bacain, dengerin, nggak dibacain.", &ex(""));
        assert!(u.is_empty(), "{u:?}");
        let (_, u) = measure(&track(), &names, 0, "Saya bacain.", &ex(""));
        assert_eq!(u, vec!["bacain".to_owned()]);
    }

    #[test]
    fn se_measures_and_chat_spellings_follow_their_roots() {
        let mut t = track();
        t.taught.insert("ratus".into(), 0);
        let names = BTreeSet::new();
        let (_, u) = measure(&t, &names, 0, "Seratus, tdk, thx, sebentar.", &ex(""));
        assert_eq!(u, vec!["tdk".to_owned(), "sebentar".to_owned()]);
        let (_, u) = measure(&t, &names, 1, "Seratus, tdk, wkwk.", &ex(""));
        assert!(u.is_empty(), "{u:?}");
    }

    #[test]
    fn plain_inflections_share_a_taught_root() {
        let mut t = track();
        t.roots = BTreeMap::from([("kirim".to_owned(), 0), ("baca".to_owned(), 1)]);
        let names = BTreeSet::new();
        let (_, u) = measure(&t, &names, 0, "Saya kirim, dikirimkan, pengiriman.", &ex(""));
        assert_eq!(u, vec!["pengiriman".to_owned()]);
        assert_eq!(inflected_root("dikirimkan").as_deref(), Some("kirim"));
        assert_eq!(inflected_root("pengiriman"), None);
    }

    #[test]
    fn krama_affixed_forms_count_once_affix_and_root_are_taught() {
        let mut t = track();
        t.lang = "jv".into();
        t.taught = [("waos", 0), ("tumbas", 0), ("dipun-", 1), ("-aken", 1)]
            .into_iter()
            .map(|(w, p)| (w.to_owned(), p))
            .collect();
        let names = BTreeSet::new();
        let line = "dipunwaos tumbasaken dipuntumbasaken dipunserat";
        let (_, u) = measure(&t, &names, 0, line, &ex(""));
        assert_eq!(u.len(), 4, "{u:?}");
        let (_, u) = measure(&t, &names, 1, line, &ex(""));
        assert_eq!(u, vec!["dipunserat".to_owned()]);
    }

    #[test]
    fn a_taught_phrase_with_a_clitic_is_still_the_phrase() {
        let names = BTreeSet::new();
        let (_, u) = measure(&track(), &names, 0, "Saya di rumah sakitnya.", &ex(""));
        assert_eq!(u, vec!["di".to_owned()]);
    }
}
