//! kaikki.org Wiktionary import (§17): reads wiktextract JSONL for Indonesian
//! or Javanese and writes normalized [`DictEntry`] lines.
//!
//! Outputs:
//! - `build/dict/<lang>.jsonl`: every entry (rebuilt, not committed; the
//!   dictionary/SQLite build reads it).
//! - `content/bridge/wiktionary-<lang>.jsonl`: only entries with an Arabic
//!   etymology, committed so the Arabic Bridge cross-check is reproducible.
//!
//! wiktextract's format drifts over time (categories as strings or objects,
//! example translations under `english` or `translation`), so parsing goes
//! through `serde_json::Value` and tolerates both.

use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use kabar_protocol::content::{ArabicEtymon, DictEntry, DictExample, DictForm, DictSense, Lang};
use serde_json::Value;

use crate::bridge::wiktionary_path;
use crate::freq::parse_lang;
use crate::paths::Root;

#[derive(clap::Args)]
pub struct Args {
    /// `id` or `jv`.
    #[arg(long, default_value = "id")]
    lang: String,
    /// kaikki JSONL to read (defaults to data/raw/kaikki-<lang>.jsonl).
    #[arg(long)]
    input: Option<PathBuf>,
}

/// Etymology templates that name a source language in arg 2.
const SOURCE_TEMPLATES: &[&str] =
    &["bor", "bor+", "bor-lite", "der", "der+", "der-lite", "inh", "inh+", "lbor", "ubor", "uder", "slbor"];

pub fn run(root: &Root, args: Args) -> Result<()> {
    let lang = parse_lang(&args.lang)?;
    let input = args.input.unwrap_or_else(|| root.raw().join(format!("kaikki-{}.jsonl", lang.code())));
    let file = fs::File::open(&input)
        .with_context(|| format!("{} is missing: run `scripts/fetch-sources.sh kaikki`", input.display()))?;

    let dict_path = root.build().join(format!("dict/{}.jsonl", lang.code()));
    fs::create_dir_all(dict_path.parent().unwrap())?;
    let mut dict = BufWriter::new(fs::File::create(&dict_path)?);
    let mut arabic: Vec<DictEntry> = Vec::new();
    let (mut read, mut kept, mut skipped) = (0usize, 0usize, 0usize);

    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        read += 1;
        let value: Value = serde_json::from_str(&line).with_context(|| format!("line {read}"))?;
        let Some(entry) = normalize(&value, lang) else {
            skipped += 1;
            continue;
        };
        kept += 1;
        serde_json::to_writer(&mut dict, &entry)?;
        dict.write_all(b"\n")?;
        if entry.arabic.is_some() {
            arabic.push(entry);
        }
    }
    dict.flush()?;

    arabic.sort_by(|a, b| (&a.word, &a.pos).cmp(&(&b.word, &b.pos)));
    let bridge_path = match lang {
        Lang::Id => wiktionary_path(root),
        Lang::Jv => root.bridge_dir().join("wiktionary-jv.jsonl"),
    };
    fs::create_dir_all(bridge_path.parent().unwrap())?;
    let mut out = String::new();
    for e in &arabic {
        out.push_str(&serde_json::to_string(e)?);
        out.push('\n');
    }
    fs::write(&bridge_path, out)?;
    println!(
        "read {read} entries: kept {kept}, skipped {skipped} (other language or no senses); {} with an Arabic etymology → {}",
        arabic.len(),
        bridge_path.display()
    );
    Ok(())
}

/// One wiktextract object → [`DictEntry`], or `None` for entries of another
/// language, redirects, or entries without senses.
pub fn normalize(v: &Value, lang: Lang) -> Option<DictEntry> {
    if str_at(v, "lang_code")? != lang.code() {
        return None;
    }
    let word = str_at(v, "word")?.to_string();
    let pos = str_at(v, "pos").unwrap_or("unknown").to_string();
    let senses: Vec<DictSense> = arr(v, "senses").filter_map(sense).collect();
    if senses.is_empty() {
        return None;
    }
    let forms = arr(v, "forms")
        .filter_map(|f| {
            let form = str_at(f, "form")?.to_string();
            let tags = strings(f, "tags");
            // Table headers and inflection-class markers are not forms.
            (!tags.iter().any(|t| t == "table-tags" || t == "inflection-template") && form != word)
                .then_some(DictForm { form, tags })
        })
        .collect();
    let mut ipa: Vec<String> =
        arr(v, "sounds").filter_map(|s| str_at(s, "ipa").map(str::to_string)).collect();
    ipa.dedup();
    let etymology = str_at(v, "etymology_text").map(str::to_string).filter(|t| !t.is_empty());

    let mut speech_levels = Vec::new();
    let mut tag_sources: Vec<String> = categories(v);
    for s in arr(v, "senses") {
        tag_sources.extend(strings(s, "tags"));
        tag_sources.extend(categories(s));
    }
    for t in &tag_sources {
        let t = t.to_lowercase();
        for level in ["krama-inggil", "krama-andhap", "krama", "ngoko"] {
            let spaced = level.replace('-', " ");
            if (t == level || t.contains(&spaced) || t.contains(level))
                && !speech_levels.iter().any(|l| l == level)
            {
                speech_levels.push(level.to_string());
                break;
            }
        }
    }
    speech_levels.sort();

    Some(DictEntry {
        lang,
        word,
        pos,
        senses,
        forms,
        ipa,
        etymology,
        arabic: arabic_etymon(v),
        speech_levels,
    })
}

fn sense(s: &Value) -> Option<DictSense> {
    let glosses = strings(s, "glosses");
    if glosses.is_empty() {
        return None;
    }
    let examples = arr(s, "examples")
        .filter_map(|x| {
            let text = str_at(x, "text")?.to_string();
            let translation = str_at(x, "translation").or_else(|| str_at(x, "english")).map(str::to_string);
            Some(DictExample { text, translation })
        })
        .collect();
    let form_of = ["form_of", "alt_of"]
        .iter()
        .flat_map(|k| arr(s, k))
        .find_map(|f| str_at(f, "word").map(str::to_string));
    Some(DictSense { glosses, tags: strings(s, "tags"), examples, form_of })
}

/// The Arabic link in the etymology chain, from templates first, then from
/// "terms borrowed/derived from Arabic" categories.
fn arabic_etymon(v: &Value) -> Option<ArabicEtymon> {
    let mut via = Vec::new();
    for t in arr(v, "etymology_templates") {
        let name = str_at(t, "name").unwrap_or_default();
        if !SOURCE_TEMPLATES.contains(&name) {
            continue;
        }
        let args = t.get("args");
        let arg = |k: &str| args.and_then(|a| a.get(k)).and_then(Value::as_str).filter(|s| !s.is_empty());
        let source = arg("2").unwrap_or_default();
        if source == "ar" || source.starts_with("ar-") {
            return Some(ArabicEtymon {
                arabic: arg("3").unwrap_or_default().to_string(),
                kind: name.trim_end_matches(['+']).trim_end_matches("-lite").to_string(),
                gloss: arg("t").or_else(|| arg("gloss")).or_else(|| arg("5")).map(str::to_string),
                via,
            });
        }
        via.push(source.to_string());
    }
    categories(v)
        .iter()
        .any(|c| {
            let c = c.to_lowercase();
            (c.contains("borrowed from arabic") || c.contains("derived from arabic"))
                && !c.contains("arabic script")
        })
        .then(|| ArabicEtymon { arabic: String::new(), kind: "cat".into(), gloss: None, via: Vec::new() })
}

fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn arr<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v.get(key).and_then(Value::as_array).into_iter().flatten()
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    arr(v, key).filter_map(Value::as_str).map(str::to_string).collect()
}

/// Categories as plain strings or `{ "name": … }` objects.
fn categories(v: &Value) -> Vec<String> {
    arr(v, "categories")
        .filter_map(|c| c.as_str().or_else(|| str_at(c, "name")).map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<Value> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/kaikki").join(name);
        fs::read_to_string(path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    fn find(entries: &[DictEntry], word: &str) -> DictEntry {
        entries.iter().find(|e| e.word == word).unwrap_or_else(|| panic!("{word} missing")).clone()
    }

    #[test]
    fn imports_indonesian_with_arabic_etymologies() {
        let entries: Vec<DictEntry> =
            fixture("id-sample.jsonl").iter().filter_map(|v| normalize(v, Lang::Id)).collect();
        // The Malay entry and the sense-less redirect are skipped.
        assert_eq!(entries.len(), 5);

        let kabar = find(&entries, "kabar");
        let a = kabar.arabic.unwrap();
        assert_eq!((a.arabic.as_str(), a.kind.as_str()), ("خَبَر", "bor"));
        assert_eq!(kabar.ipa, ["/ˈkabar/"]);
        assert_eq!(kabar.senses[0].glosses, ["news"]);
        assert_eq!(kabar.senses[0].examples[0].translation.as_deref(), Some("How are you?"));

        let pikir = find(&entries, "pikir").arabic.unwrap();
        assert_eq!(pikir.via, ["ms"], "Arabic reached Indonesian through Malay");
        assert_eq!(pikir.gloss.as_deref(), Some("thought"));

        let kalimat = find(&entries, "kalimat").arabic.unwrap();
        assert_eq!((kalimat.kind.as_str(), kalimat.arabic.as_str()), ("cat", ""), "category-only etymology");

        let membaca = find(&entries, "membaca");
        assert_eq!(membaca.senses[0].form_of.as_deref(), Some("baca"));
        assert!(membaca.arabic.is_none());
        assert_eq!(membaca.forms.len(), 1, "table-tags rows dropped");
    }

    #[test]
    fn imports_javanese_speech_levels() {
        let entries: Vec<DictEntry> =
            fixture("jv-sample.jsonl").iter().filter_map(|v| normalize(v, Lang::Jv)).collect();
        assert_eq!(find(&entries, "mangan").speech_levels, ["ngoko"]);
        assert_eq!(find(&entries, "nedha").speech_levels, ["krama"]);
        assert_eq!(find(&entries, "dhahar").speech_levels, ["krama-inggil"]);
    }
}
