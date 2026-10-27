//! The Arabic Bridge dataset (§12): `content/bridge/id.jsonl` (one card per
//! line, sorted by word) and `content/bridge/sound-rules.json`.
//!
//! Cards are written by Claude Code following `prompts/bridge@v1.md` and
//! cross-checked against Wiktionary etymologies (`kabar-tools kaikki` writes
//! `content/bridge/wiktionary-id.jsonl`): `crosscheck` adds `wiktionary` to
//! a card's sources when Wiktionary agrees, flags disagreements, and lists
//! Wiktionary's Arabic loans that still need a card.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use kabar_protocol::content::{BridgeEntry, DictEntry, GlossEntry, Lang, MeaningShift, Pos, SoundRule};

use crate::gloss::check_tri;
use crate::paths::Root;
use crate::text::{arabic_skeleton, has_arabic, is_arabic_letter, is_latin_word};

#[derive(Subcommand)]
pub enum Cmd {
    /// Validate the Bridge cards and sound rules.
    Validate,
    /// Compare the cards with Wiktionary etymologies (needs `kabar-tools kaikki` first).
    Crosscheck {
        /// Add `wiktionary` to the sources of cards Wiktionary confirms.
        #[arg(long)]
        write: bool,
    },
    /// Rewrite the cards file sorted by word, one card per line.
    Sort,
    /// Counts by meaning shift, confidence and sound rule.
    Stats,
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Validate => {
            let entries = load_entries(root)?;
            let rules = load_rules(root)?;
            let issues = validate(&entries, &rules);
            for i in &issues {
                println!("bridge: {i}");
            }
            if !issues.is_empty() {
                bail!("{} bridge issue(s)", issues.len());
            }
            println!("bridge ok ({} cards, {} sound rules)", entries.len(), rules.len());
            Ok(())
        }
        Cmd::Crosscheck { write } => crosscheck(root, write),
        Cmd::Sort => {
            let mut entries = load_entries(root)?;
            entries.sort_by(|a, b| a.word.cmp(&b.word));
            save_entries(root, &entries)
        }
        Cmd::Stats => {
            let entries = load_entries(root)?;
            let mut by: BTreeMap<String, usize> = BTreeMap::new();
            for e in &entries {
                *by.entry(format!("shift {:?}", e.shift)).or_default() += 1;
                *by.entry(format!("confidence {:?}", e.confidence)).or_default() += 1;
                for r in &e.sound_rules {
                    *by.entry(format!("rule {r}")).or_default() += 1;
                }
            }
            println!("{} cards", entries.len());
            for (k, n) in by {
                println!("{n:5}  {k}");
            }
            Ok(())
        }
    }
}

fn entries_path(root: &Root) -> PathBuf {
    root.bridge_dir().join("id.jsonl")
}

fn rules_path(root: &Root) -> PathBuf {
    root.bridge_dir().join("sound-rules.json")
}

pub fn wiktionary_path(root: &Root) -> PathBuf {
    root.bridge_dir().join("wiktionary-id.jsonl")
}

pub fn load_entries(root: &Root) -> Result<Vec<BridgeEntry>> {
    read_jsonl(&entries_path(root))
}

pub fn load_rules(root: &Root) -> Result<Vec<SoundRule>> {
    let path = rules_path(root);
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| path.display().to_string())
}

fn save_entries(root: &Root, entries: &[BridgeEntry]) -> Result<()> {
    let mut out = String::new();
    for e in entries {
        out.push_str(&serde_json::to_string(e)?);
        out.push('\n');
    }
    fs::write(entries_path(root), out)?;
    Ok(())
}

pub fn read_jsonl<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<Vec<T>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| serde_json::from_str(l).with_context(|| format!("{}:{}", path.display(), i + 1)))
        .collect()
}

pub fn validate(entries: &[BridgeEntry], rules: &[SoundRule]) -> Vec<String> {
    let mut issues = Vec::new();
    let rule_ids: HashSet<&str> = rules.iter().map(|r| r.id.as_str()).collect();
    let words: HashSet<&str> = entries.iter().map(|e| e.word.as_str()).collect();
    let mut seen = HashSet::new();
    let mut prev: Option<&str> = None;
    for e in entries {
        let w = &e.word;
        let mut p = Vec::new();
        if !seen.insert(w.as_str()) {
            p.push("duplicate".to_string());
        }
        if prev.is_some_and(|prev| prev > w.as_str()) {
            p.push("not sorted (run `kabar-tools bridge sort`)".into());
        }
        prev = Some(w);
        if e.lang != Lang::Id {
            p.push("only Indonesian cards live in id.jsonl".into());
        }
        if !is_latin_word(w) {
            p.push("word must be lowercase Latin".into());
        }
        if !has_arabic(&e.arabic) {
            p.push("arabic must be in Arabic script".into());
        }
        if let Some(root) = &e.root {
            let letters: Vec<&str> = root.split(' ').collect();
            let ok = (2..=5).contains(&letters.len())
                && letters.iter().all(|l| l.chars().count() == 1 && l.chars().all(is_arabic_letter));
            if !ok {
                p.push(format!("root {root:?} should be 2–5 Arabic letters separated by spaces"));
            }
        }
        check_tri(&e.arabic_meaning, "arabic_meaning", &mut p);
        check_tri(&e.meaning, "meaning", &mut p);
        match (&e.shift, &e.shift_note) {
            (MeaningShift::Same, _) => {}
            (_, Some(note)) => check_tri(note, "shift_note", &mut p),
            (_, None) => p.push("shift_note is required when the meaning moved".into()),
        }
        for r in &e.sound_rules {
            if !rule_ids.contains(r.as_str()) {
                p.push(format!("unknown sound rule {r:?}"));
            }
        }
        if e.sources.is_empty() {
            p.push("no sources".into());
        }
        issues.extend(p.into_iter().map(|p| format!("{w}: {p}")));
    }
    let mut seen_rules = HashSet::new();
    for r in rules {
        let mut p = Vec::new();
        if !seen_rules.insert(&r.id) {
            p.push("duplicate rule".to_string());
        }
        check_tri(&r.explanation, "explanation", &mut p);
        if r.examples.is_empty() {
            p.push("no examples".into());
        }
        for x in &r.examples {
            match entries.iter().find(|e| &e.word == x) {
                None if !words.is_empty() => p.push(format!("example {x:?} has no card")),
                Some(e) if !e.sound_rules.contains(&r.id) => {
                    p.push(format!("example {x:?} doesn't list this rule"))
                }
                _ => {}
            }
        }
        issues.extend(p.into_iter().map(|p| format!("rule {}: {p}", r.id)));
    }
    issues
}

fn crosscheck(root: &Root, write: bool) -> Result<()> {
    let mut entries = load_entries(root)?;
    let wikt: Vec<DictEntry> = read_jsonl(&wiktionary_path(root))
        .context("run `kabar-tools kaikki` on the Indonesian extract first")?;
    let mut by_word: BTreeMap<&str, Vec<&DictEntry>> = BTreeMap::new();
    for d in &wikt {
        by_word.entry(d.word.as_str()).or_default().push(d);
    }
    let (mut confirmed, mut differ, mut unknown) = (0, 0, 0);
    for e in &mut entries {
        let Some(ds) = by_word.get(e.word.as_str()) else {
            unknown += 1;
            continue;
        };
        let ours = arabic_skeleton(&e.arabic);
        let agrees = ds.iter().filter_map(|d| d.arabic.as_ref()).any(|a| {
            let theirs = arabic_skeleton(&a.arabic);
            theirs.is_empty() || theirs == ours || theirs.contains(&ours) || ours.contains(&theirs)
        });
        if agrees {
            confirmed += 1;
            if !e.sources.iter().any(|s| s == "wiktionary") {
                e.sources.push("wiktionary".into());
            }
        } else {
            differ += 1;
            let theirs: Vec<&str> =
                ds.iter().filter_map(|d| d.arabic.as_ref()).map(|a| a.arabic.as_str()).collect();
            println!("differs  {}: ours {} · wiktionary {:?}", e.word, e.arabic, theirs);
        }
    }
    let ours: HashSet<&str> = entries.iter().map(|e| e.word.as_str()).collect();
    let mut todo: Vec<&str> =
        by_word.keys().copied().filter(|w| !ours.contains(w) && is_latin_word(w)).collect();
    todo.dedup();
    let todo_path = root.build().join("bridge-todo.txt");
    fs::create_dir_all(todo_path.parent().unwrap())?;
    fs::write(&todo_path, todo.join("\n") + "\n")?;
    println!(
        "{confirmed} confirmed by Wiktionary, {differ} differ, {unknown} not marked Arabic in Wiktionary; \
         {} Wiktionary loans without a card (listed in {})",
        todo.len(),
        todo_path.display()
    );
    if write {
        save_entries(root, &entries)?;
    }
    Ok(())
}

/// A lesson word whose `arabic:` names its Arabic source.
pub struct LessonLoan {
    pub lesson: String,
    pub lang: Lang,
    pub word: String,
    pub arabic: String,
    /// The card it shares when no lookup finds one (`bridge: selamat`).
    pub bridge: Option<String>,
}

/// Every lesson word marked Arabic must lead to a Bridge card, and the card
/// must name the same source (#143).
pub fn check_lessons(root: &Root) -> Result<Vec<String>> {
    let cards = load_entries(root)?;
    let mut glosses = Vec::new();
    let mut loans = Vec::new();
    for lang in [Lang::Id, Lang::Jv] {
        let dir = root.gloss_dir(lang);
        if dir.exists() {
            for f in sorted_files(&dir, "jsonl")? {
                glosses.extend(read_jsonl::<GlossEntry>(&f)?);
            }
        }
        let dir = root.path().join(format!("content/lessons/{}", lang.code()));
        if !dir.exists() {
            continue;
        }
        for f in sorted_files(&dir, "yaml")? {
            let doc = kabar_content::yaml_value(&f).map_err(|e| anyhow::anyhow!("{e}"))?;
            let lesson = doc.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let words = doc.get("new_words").and_then(|v| v.as_array()).into_iter().flatten();
            for w in words {
                let text = |k: &str| w.get(k).and_then(|v| v.as_str()).map(str::to_owned);
                if let (Some(word), Some(arabic)) = (text("word"), text("arabic")) {
                    loans.push(LessonLoan {
                        lesson: lesson.into(),
                        lang,
                        word,
                        arabic,
                        bridge: text("bridge"),
                    });
                }
            }
        }
    }
    Ok(lesson_sources(&loans, &cards, &glosses))
}

fn sorted_files(dir: &std::path::Path, ext: &str) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect();
    files.sort();
    Ok(files)
}

/// The Arabic as the check compares it: letters only, no article, so
/// «القصّة» and «قِصَّة» agree.
fn source_key(arabic: &str) -> String {
    arabic
        .split_whitespace()
        .map(arabic_skeleton)
        .map(|w| match w.strip_prefix("ال") {
            Some(rest) if rest.chars().count() >= 2 => rest.to_owned(),
            _ => w,
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn lesson_sources(loans: &[LessonLoan], cards: &[BridgeEntry], glosses: &[GlossEntry]) -> Vec<String> {
    let cards: BTreeMap<&str, &BridgeEntry> = cards.iter().map(|c| (c.word.as_str(), c)).collect();
    let glosses: BTreeMap<(Lang, String), &GlossEntry> =
        glosses.iter().map(|g| ((g.lang, g.word.to_lowercase()), g)).collect();
    // A word, then its lemma and root; a Javanese word also its Indonesian
    // equivalent and that word's stems (the Reader's lookup, reader/content.rs).
    let stems = |lang: Lang, word: &str| -> Vec<String> {
        let mut out = vec![word.to_lowercase()];
        if let Some(g) = glosses.get(&(lang, word.to_lowercase())) {
            out.extend([Some(&g.lemma), g.root.as_ref()].into_iter().flatten().map(|s| s.to_lowercase()));
            let first =
                g.indonesian.as_deref().and_then(|i| i.split(',').next()).map(|f| f.trim().to_lowercase());
            if let (Lang::Jv, Some(first)) = (lang, first) {
                if let Some(id) = glosses.get(&(Lang::Id, first.clone())) {
                    out.extend(
                        [Some(&id.lemma), id.root.as_ref()].into_iter().flatten().map(|s| s.to_lowercase()),
                    );
                }
                out.push(first);
            }
        }
        out
    };
    let mut issues = Vec::new();
    for l in loans {
        let at = format!("{}: {}", l.lesson, l.word);
        let found: Vec<&BridgeEntry> = match &l.bridge {
            Some(b) => match cards.get(b.as_str()) {
                Some(c) => vec![c],
                None => {
                    issues.push(format!("{at}: bridge: {b} has no card"));
                    continue;
                }
            },
            // Phrases (hak pilih, asal-usul) also try each of their words.
            None => std::iter::once(l.word.as_str())
                .chain(l.word.split([' ', '-', '/', '(', ')']))
                .filter(|w| !w.is_empty())
                .flat_map(|w| stems(l.lang, w))
                .filter_map(|k| cards.get(k.as_str()).copied())
                .collect(),
        };
        if found.is_empty() {
            // Names (Mesir) carry an Arabic origin without being Bridge material.
            let name = glosses
                .get(&(l.lang, l.word.to_lowercase()))
                .is_some_and(|g| g.senses.iter().all(|s| matches!(s.pos, Pos::ProperNoun)));
            if !name {
                issues.push(format!(
                    "{at}: no Bridge card; name the card it shares (bridge: <word>) or add one to content/bridge/id.jsonl"
                ));
            }
            continue;
        }
        let key = source_key(&l.arabic);
        // A derived word may name the root it shares with its card
        // (peradilan ← عدل, card adil ← عادل); the card's own word may not.
        let agrees = |c: &&BridgeEntry| {
            let card = source_key(&c.arabic);
            let same = !key.is_empty() && !card.is_empty() && (key.contains(&card) || card.contains(&key));
            let root = c.root.as_deref().map(|r| arabic_skeleton(&r.replace(' ', "")));
            same || (!c.word.eq_ignore_ascii_case(&l.word) && root.is_some_and(|r| r == key))
        };
        if !found.iter().any(agrees) {
            issues.push(format!(
                "{at}: arabic {} but the Bridge card {} says {}; use one source in both",
                l.arabic, found[0].word, found[0].arabic
            ));
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_cards_and_rules() {
        let card: BridgeEntry = serde_json::from_str(
            r#"{"lang":"id","word":"kalimat","arabic":"كَلِمَة","root":"ك ل م",
            "arabic_meaning":{"arz":"كلمة","ar":"كلمة","en":"word"},
            "meaning":{"arz":"جملة","ar":"جملة","en":"sentence"},
            "shift":"false_friend","sound_rules":["ta_marbuta_at"],
            "confidence":"high","sources":["claude-code"]}"#,
        )
        .unwrap();
        let rule: SoundRule = serde_json::from_str(
            r#"{"id":"ta_marbuta_at","from":"ة","to":"-at",
            "explanation":{"arz":"التاء المربوطة بتبقى -at","ar":"تصير التاء المربوطة -at","en":"Final ta marbuta becomes -at"},
            "examples":["kalimat"]}"#,
        )
        .unwrap();
        let issues = validate(std::slice::from_ref(&card), std::slice::from_ref(&rule));
        assert_eq!(issues, ["kalimat: shift_note is required when the meaning moved"]);

        let mut bad_root = card.clone();
        bad_root.root = Some("كلم".into());
        bad_root.shift = MeaningShift::Same;
        bad_root.sound_rules = vec!["fa_p".into()];
        let issues = validate(&[bad_root], &[rule]);
        assert!(issues.iter().any(|i| i.contains("root")), "{issues:?}");
        assert!(issues.iter().any(|i| i.contains("unknown sound rule")), "{issues:?}");
        assert!(issues.iter().any(|i| i.contains("doesn't list this rule")), "{issues:?}");
    }

    #[test]
    fn lesson_loans_lead_to_a_card_with_the_same_source() {
        let card = |word: &str, arabic: &str, root: &str| -> BridgeEntry {
            serde_json::from_value(serde_json::json!({
                "lang": "id", "word": word, "arabic": arabic, "root": root,
                "arabic_meaning": {"arz": "x", "ar": "x", "en": "x"},
                "meaning": {"arz": "x", "ar": "x", "en": "x"},
                "shift": "same", "confidence": "high", "sources": ["claude-code"]
            }))
            .unwrap()
        };
        let cards =
            [card("adil", "عَادِل", "ع د ل"), card("kisah", "قِصَّة", "ق ص ص"), card("hak", "حَقّ", "ح ق ق")];
        let loan = |lesson: &str, lang, word: &str, arabic: &str, bridge: Option<&str>| LessonLoan {
            lesson: lesson.into(),
            lang,
            word: word.into(),
            arabic: arabic.into(),
            bridge: bridge.map(Into::into),
        };
        let loans = [
            // Article and vowels don't matter; a phrase finds its word's card.
            loan("a", Lang::Id, "alkisah", "القصّة", Some("kisah")),
            loan("b", Lang::Id, "hak pilih", "حقّ", None),
            // A derived word may name the shared root; the card's word may not.
            loan("c", Lang::Id, "peradilan", "عدل", Some("adil")),
            loan("d", Lang::Id, "adil", "عدل", None),
            loan("e", Lang::Jv, "ngelmu", "علم", None),
            loan("f", Lang::Id, "kisah", "قصص", Some("cerita")),
        ];
        let issues = lesson_sources(&loans, &cards, &[]);
        assert_eq!(issues.len(), 3, "{issues:?}");
        assert!(issues[0].starts_with("d: adil: arabic عدل"), "{issues:?}");
        assert!(issues[1].starts_with("e: ngelmu: no Bridge card"), "{issues:?}");
        assert!(issues[2].starts_with("f: kisah: bridge: cerita has no card"), "{issues:?}");
    }
}
