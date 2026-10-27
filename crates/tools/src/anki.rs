//! `batna-tools anki [--lecture hb.L01] [--verdicts FILE] [--out DIR]`: Anki
//! decks from the content YAML, so Mohamed can study before the app is done.
//!
//! By default it writes one file, `dist/anki/batna.apkg`, with a `Batna`
//! deck and one subdeck per lecture (`Batna::L01 Liver Cirrhosis`).
//! `--lecture` writes just that lecture to `batna-hb-L01.apkg`.
//!
//! So the file never shrinks while content threads work, a lecture that
//! isn't merged yet comes from the project's shared folder, like
//! make_apkg.py does: `<medstudy>/content-wip/hb/LNN.yaml`, then
//! `<medstudy>/drafts/LNN.yaml` or `LNN.draft.yaml`. `<medstudy>` is
//! `--drafts DIR`, or `/mnt/project-files/medstudy` when it exists;
//! `--merged-only` turns this off. Those notes get a `draft` tag.
//!
//! An `.apkg` is a zip of an Anki SQLite collection (`collection.anki2`, the
//! schema-11 layout every Anki and AnkiDroid imports) and a `media` index.
//! Model, deck and note ids, field names and GUIDs match the interim
//! `/mnt/project-files/medstudy/anki/make_apkg.py` (genanki), so decks
//! imported from either one update in place and keep their review history.
//!
//! Note types: Batna Basic (Front, Back, Source, Update, Id) and Batna Cloze
//! (Text, Extra, Source, Update, Id). MCQ, enumerate, essay, order and calc
//! items become Basic notes; a case becomes one note per step and a table one
//! note per blank. Tags: `hb::L01`, `topic::<slug>`, `approved` or
//! `unverified` (from `content/approved.txt`), and the item's own tags.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use batna_core::content::{Body, Item, Lecture, OutlineEntry};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::verdicts::{Verdict, Verdicts};

/// Same CSS as make_apkg.py; Anki's night mode adds `.nightMode`.
const CSS: &str = ".card{font-family:system-ui,sans-serif;font-size:18px;line-height:1.5;text-align:left;color:#1d2420;background:#fff}
.nightMode .card{color:#e6ebe7;background:#1b211e}
.src{margin-top:14px;font-size:13px;color:#6b766f}
.upd{margin-top:10px;padding:6px 8px;border-left:3px solid #c08a1e;font-size:14px}
.why{font-size:15px}.cloze{font-weight:bold;color:#2f6b4f}";

const BACK_TAIL: &str =
    r#"<div class="src">{{Source}}</div>{{#Update}}<div class="upd">Update: {{Update}}</div>{{/Update}}"#;

const LETTERS: &[u8] = b"ABCDEFGH";

pub fn run(root: &Path, args: &[String]) -> Result<String, Vec<String>> {
    let out = crate::flag(args, "--out").map(PathBuf::from).unwrap_or_else(|| root.join("dist/anki"));
    let only = crate::flag(args, "--lecture");
    let verdicts = match crate::flag(args, "--verdicts") {
        Some(f) => Some(Verdicts::read(Path::new(f)).map_err(|e| vec![e])?),
        None => None,
    };
    let (index, mut lectures) = crate::packs::load(root)?;
    let mut warnings = Vec::new();
    let mut drafts = BTreeSet::new();
    let medstudy = match crate::flag(args, "--drafts") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => Some(PathBuf::from(MEDSTUDY)).filter(|d| d.is_dir()),
    };
    if let Some(dir) = medstudy.filter(|_| !args.iter().any(|a| a == "--merged-only")) {
        for entry in &index.lectures {
            if lectures.iter().any(|l| l.lecture == entry.id) {
                continue;
            }
            match draft_for(&dir, &entry.id) {
                Some(Ok(lec)) => {
                    drafts.insert(lec.lecture.clone());
                    lectures.push(lec);
                }
                Some(Err(e)) => warnings.push(format!("{e} (left out)")),
                None => {}
            }
        }
        let order = |id: &str| index.lectures.iter().position(|e| e.id == id).unwrap_or(usize::MAX);
        lectures.sort_by_key(|l| order(&l.lecture));
    }
    if let Some(id) = only {
        lectures.retain(|l| l.lecture == id);
        if lectures.is_empty() {
            return Err(vec![format!("{id}: no such lecture written yet")]);
        }
    }
    let approved = crate::read_approved(&root.join("content/approved.txt"));
    let opts =
        Options { approved: &approved, verdicts: verdicts.as_ref(), drafts: &drafts, now_ms: now_ms() };
    let mut deck = build(&lectures, &opts)?;
    deck.warnings.extend(warnings);

    let name = match only {
        Some(id) => format!("batna-{}.apkg", id.replace('.', "-")),
        None => "batna.apkg".into(),
    };
    let path = out.join(name);
    std::fs::create_dir_all(&out).map_err(|e| vec![format!("{}: {e}", out.display())])?;
    write_apkg(&deck, &path, opts.now_ms).map_err(|e| vec![format!("{}: {e}", path.display())])?;
    let mut report = format!("{}: {} notes, {} cards", path.display(), deck.notes.len(), deck.card_count());
    for (lecture, n) in &deck.per_lecture {
        let from = if drafts.contains(lecture) { " (draft, not merged yet)" } else { "" };
        report.push_str(&format!("\n  {lecture}: {n} notes{from}"));
    }
    if deck.skipped > 0 {
        report.push_str(&format!("\n  left out: {} item(s) marked ✗ or rejected", deck.skipped));
    }
    for w in &deck.warnings {
        report.push_str(&format!("\n  warning: {w}"));
    }
    Ok(report)
}

/// The project's shared folder, where unmerged lectures wait.
const MEDSTUDY: &str = "/mnt/project-files/medstudy";

/// The newest unmerged version of a lecture, if there is one.
fn draft_for(medstudy: &Path, id: &str) -> Option<Result<Lecture, String>> {
    let num = id.split_once('.').map(|(_, n)| n).unwrap_or(id);
    let candidates = [
        medstudy.join("content-wip/hb").join(format!("{num}.yaml")),
        medstudy.join("drafts").join(format!("{num}.yaml")),
        medstudy.join("drafts").join(format!("{num}.draft.yaml")),
    ];
    let path = candidates.into_iter().find(|p| p.exists())?;
    Some(crate::read_yaml::<Lecture>(&path).and_then(|lec| {
        if lec.lecture == id {
            Ok(lec)
        } else {
            Err(format!("{}: says it is {}", path.display(), lec.lecture))
        }
    }))
}

pub struct Options<'a> {
    pub approved: &'a BTreeSet<String>,
    /// Lectures read from the shared folder, not merged yet: tagged `draft`.
    pub drafts: &'a BTreeSet<String>,
    pub verdicts: Option<&'a Verdicts>,
    /// Note and card ids start here, as Anki's own do.
    pub now_ms: i64,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Basic,
    Cloze,
}

impl Model {
    pub fn id(self) -> i64 {
        match self {
            Model::Basic => stable_id("batna-basic-v1"),
            Model::Cloze => stable_id("batna-cloze-v1"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Note {
    pub model: Model,
    pub deck: i64,
    /// Front/Back/Source/Update/Id, or Text/Extra/Source/Update/Id.
    pub fields: [String; 5],
    pub tags: Vec<String>,
    pub guid: String,
}

impl Note {
    /// Card ordinals: one for Basic, one per distinct `{{cN::…}}` for Cloze.
    fn card_ords(&self) -> Vec<u32> {
        match self.model {
            Model::Basic => vec![0],
            Model::Cloze => {
                let ords: BTreeSet<u32> = batna_core::content::cloze_blanks(&self.fields[0])
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|(n, _)| *n > 0)
                    .map(|(n, _)| n - 1)
                    .collect();
                if ords.is_empty() { vec![0] } else { ords.into_iter().collect() }
            }
        }
    }
}

pub struct Deck {
    /// (deck id, full name, description)
    pub decks: Vec<(i64, String, String)>,
    pub notes: Vec<Note>,
    pub per_lecture: Vec<(String, usize)>,
    pub skipped: usize,
    pub warnings: Vec<String>,
}

impl Deck {
    fn card_count(&self) -> usize {
        self.notes.iter().map(|n| n.card_ords().len()).sum()
    }
}

/// `Batna::L01 Liver Cirrhosis`, as make_apkg.py names it (Anki matches
/// decks by name on import, so this must not change).
pub fn deck_name(lec: &Lecture) -> String {
    let num = lec.lecture.split_once('.').map(|(_, n)| n).unwrap_or(&lec.lecture);
    format!("Batna::{num} {}", lec.title)
}

pub fn build(lectures: &[Lecture], opts: &Options<'_>) -> Result<Deck, Vec<String>> {
    let latest = opts.verdicts.map(|v| v.latest()).unwrap_or_default();
    let mut deck = Deck {
        decks: vec![(stable_id("batna-deck-root"), "Batna".into(), ROOT_DESC.into())],
        notes: Vec::new(),
        per_lecture: Vec::new(),
        skipped: 0,
        warnings: Vec::new(),
    };
    for lec in lectures {
        let did = stable_id(&format!("batna-deck-{}", lec.lecture));
        let desc = format!(
            "{} · Book pp. {}–{}{}",
            esc(&lec.title),
            lec.book.pages[0],
            lec.book.pages[1],
            lec.author.as_deref().map(|a| format!(" · {}", esc(a))).unwrap_or_default()
        );
        deck.decks.push((did, deck_name(lec), desc));
        let before = deck.notes.len();
        for item in &lec.items {
            if item.status == batna_core::content::Status::Rejected {
                deck.skipped += 1;
                continue;
            }
            let mut approved = opts.approved.contains(&item.id);
            let edited;
            let item = match latest.get(item.id.as_str()) {
                Some(v) if v.verdict == Verdict::Wrong => {
                    deck.skipped += 1;
                    continue;
                }
                Some(v) if v.verdict == Verdict::Edited => match crate::verdicts::apply_edit(item, &v.edit) {
                    Ok(fixed) => {
                        edited = fixed;
                        approved = false;
                        &edited
                    }
                    Err(e) => {
                        deck.warnings.push(format!("{e} (kept the book version)"));
                        item
                    }
                },
                Some(v) if v.verdict == Verdict::Approved => {
                    approved = true;
                    item
                }
                _ => item,
            };
            let mut notes = notes_for(item, lec, did, approved);
            if opts.drafts.contains(&lec.lecture) {
                notes.iter_mut().for_each(|n| n.tags.push("draft".into()));
            }
            deck.notes.extend(notes);
        }
        deck.per_lecture.push((lec.lecture.clone(), deck.notes.len() - before));
    }
    Ok(deck)
}

const ROOT_DESC: &str = "Batna: Alexandria Internal Medicine Postgraduate Lectures, Chapter 1 (Hepato-Biliary). \
Re-import a newer batna.apkg to update the cards; your reviews are kept. Cards tagged <b>unverified</b> \
are not checked yet: trust the book page on the back.";

/// The notes for one item, ported from make_apkg.py's `notes_for`.
pub fn notes_for(item: &Item, lec: &Lecture, deck: i64, approved: bool) -> Vec<Note> {
    let src = format!("{} · Book p. {}", esc(&lec.title), item.src.page);
    let upd =
        item.update.as_ref().map(|u| format!("{} ({})", esc(&u.text), esc(&u.reference))).unwrap_or_default();
    let (chapter, num) = lec.lecture.split_once('.').unwrap_or((&lec.lecture, ""));
    let mut tags = vec![
        format!("{chapter}::{num}"),
        format!("topic::{}", item.topic.as_deref().unwrap_or("misc")),
        if approved { "approved" } else { "unverified" }.to_string(),
    ];
    tags.extend(item.tags.iter().cloned());
    let tags: Vec<String> = tags.iter().map(|t| t.split_whitespace().collect::<Vec<_>>().join("-")).collect();
    let id = item.id.as_str();

    let basic = |front: String, back: String, suffix: &str| Note {
        model: Model::Basic,
        deck,
        fields: [front, back, src.clone(), upd.clone(), format!("{id}{suffix}")],
        tags: tags.clone(),
        guid: guid_for(&format!("{id}{suffix}")),
    };
    let letter = |i: usize| LETTERS.get(i).map(|&b| b as char).unwrap_or('?');
    let options = |opts: &[String]| {
        opts.iter()
            .enumerate()
            .map(|(i, o)| format!("{}. {}", letter(i), esc(o)))
            .collect::<Vec<_>>()
            .join("<br>")
    };

    match &item.body {
        Body::Card { front, back } => vec![basic(esc(front), esc(back), "")],
        Body::Cloze { text, extra } => vec![Note {
            model: Model::Cloze,
            deck,
            fields: [
                esc(text),
                esc(extra.as_deref().unwrap_or("")),
                src.clone(),
                upd.clone(),
                id.to_string(),
            ],
            tags: tags.clone(),
            guid: guid_for(id),
        }],
        Body::Mcq { stem, options: opts, answer, why } => {
            let why: String = why
                .iter()
                .enumerate()
                .map(|(i, w)| format!(r#"<div class="why"><b>{}</b>: {}</div>"#, letter(i), esc(w)))
                .collect();
            vec![basic(
                format!("{}<br><br>{}", esc(stem), options(opts)),
                format!("<b>{}</b><br>{why}", letter(*answer)),
                "",
            )]
        }
        Body::Enumerate { prompt, points, pass } => vec![basic(
            format!("{}<br><i>({} points; pass at {pass})</i>", esc(prompt), points.len()),
            list(points),
            "",
        )],
        Body::Essay { prompt, outline } => {
            let mut body = String::from("<ol>");
            for entry in outline {
                match entry {
                    OutlineEntry::Heading(h) => body.push_str(&format!("<li>{}</li>", esc(h))),
                    OutlineEntry::Section(map) => {
                        for (head, points) in map {
                            body.push_str(&format!("<li><b>{}</b>{}</li>", esc(head), list(points)));
                        }
                    }
                }
            }
            body.push_str("</ol>");
            vec![basic(format!("Essay outline: {}", esc(prompt)), body, "")]
        }
        Body::Case { stem, steps } => {
            let mut shown = esc(stem);
            let mut out = Vec::new();
            for (n, step) in steps.iter().enumerate() {
                let n = n + 1;
                if let Some(reveal) = step.reveal.as_deref().filter(|r| !r.is_empty()) {
                    shown.push_str(&format!("<br><br>{}", esc(reveal)));
                }
                let answer = match step.answer {
                    Some(a) if !step.options.is_empty() => format!("<b>{}</b> ", letter(a)),
                    _ => String::new(),
                };
                out.push(basic(
                    format!(
                        "{shown}<br><br><b>Step {n}:</b> {}<br>{}",
                        esc(&step.question),
                        options(&step.options)
                    ),
                    format!("{answer}{}", esc(&step.why)),
                    &format!("#s{n}"),
                ));
            }
            out
        }
        Body::Table { title, columns, rows, blanks } => blanks
            .iter()
            .filter_map(|&[r, c]| {
                let row = rows.get(r)?;
                Some(basic(
                    format!(
                        "{}: <b>{}</b> → <b>{}</b>?",
                        esc(title),
                        esc(row.first()?),
                        esc(columns.get(c)?)
                    ),
                    esc(row.get(c)?),
                    &format!("#r{r}c{c}"),
                ))
            })
            .collect(),
        Body::Order { prompt, steps } => {
            vec![basic(format!("Put in order: {}", esc(prompt)), list(steps), "")]
        }
        Body::Calc { stem, question, expect, interpret, .. } => vec![basic(
            format!(
                "{}<br><br>{}",
                esc(stem),
                esc(question.as_deref().unwrap_or("Calculate and interpret."))
            ),
            format!("{}<br>{}", esc(expect), esc(interpret)),
            "",
        )],
    }
}

/// Python's `html.escape`, then newlines as `<br>` (make_apkg.py's `esc`).
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            '\n' => out.push_str("<br>"),
            c => out.push(c),
        }
    }
    out
}

fn list(items: &[String]) -> String {
    let lis: String = items.iter().map(|i| format!("<li>{}</li>", esc(i))).collect();
    format!("<ol>{lis}</ol>")
}

/// make_apkg.py's `mid()`: the first 8 hex digits of the name's SHA-1.
pub fn stable_id(name: &str) -> i64 {
    let digest = Sha1::digest(name.as_bytes());
    i64::from(u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]))
}

/// genanki's `guid_for`: the first 8 bytes of SHA-256, in Anki's base91.
pub fn guid_for(value: &str) -> String {
    const TABLE: &[u8] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!#$%&()*+,-./:;<=>?@[]^_`{|}~";
    let digest = Sha256::digest(value.as_bytes());
    let mut n = u64::from_be_bytes(digest[..8].try_into().unwrap());
    let mut out = Vec::new();
    while n > 0 {
        out.push(TABLE[(n % TABLE.len() as u64) as usize]);
        n /= TABLE.len() as u64;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

/// Anki's duplicate check: the first 8 hex digits of the SHA-1 of the
/// first field without HTML.
fn checksum(field: &str) -> i64 {
    let mut text = String::new();
    let mut in_tag = false;
    for ch in field.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    let digest = Sha1::digest(text.trim().as_bytes());
    i64::from(u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]))
}

fn model_json(model: Model, deck: i64, now_s: i64) -> Value {
    let (name, fields, qfmt, afmt, kind) = match model {
        Model::Basic => (
            "Batna Basic",
            ["Front", "Back", "Source", "Update", "Id"],
            "{{Front}}".to_string(),
            format!(r#"{{{{FrontSide}}}}<hr id="answer">{{{{Back}}}}{BACK_TAIL}"#),
            0,
        ),
        Model::Cloze => (
            "Batna Cloze",
            ["Text", "Extra", "Source", "Update", "Id"],
            "{{cloze:Text}}".to_string(),
            format!("{{{{cloze:Text}}}}<br>{{{{Extra}}}}{BACK_TAIL}"),
            1,
        ),
    };
    let flds: Vec<Value> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            json!({"name": f, "ord": i, "font": "Liberation Sans", "media": [], "rtl": false, "size": 20, "sticky": false})
        })
        .collect();
    json!({
        "css": CSS,
        "did": deck,
        "flds": flds,
        "id": model.id().to_string(),
        "latexPost": "\\end{document}",
        "latexPre": "\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage[utf8]{inputenc}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n",
        "latexsvg": false,
        "mod": now_s,
        "name": name,
        "req": [[0, "all", [0]]],
        "sortf": 0,
        "tags": [],
        "tmpls": [{
            "name": if model == Model::Basic { "Card 1" } else { "Cloze" },
            "ord": 0, "qfmt": qfmt, "afmt": afmt,
            "bafmt": "", "bqfmt": "", "bfont": "", "bsize": 0, "did": null
        }],
        "type": kind,
        "usn": -1,
        "vers": []
    })
}

fn deck_json(id: i64, name: &str, desc: &str, now_s: i64) -> Value {
    json!({
        "collapsed": false, "conf": 1, "desc": desc, "dyn": 0, "extendNew": 0, "extendRev": 50,
        "id": id, "mod": now_s, "name": name, "usn": -1,
        "lrnToday": [0, 0], "newToday": [0, 0], "revToday": [0, 0], "timeToday": [0, 0]
    })
}

const SCHEMA: &str = "
CREATE TABLE col (id integer primary key, crt integer not null, mod integer not null, scm integer not null,
  ver integer not null, dty integer not null, usn integer not null, ls integer not null, conf text not null,
  models text not null, decks text not null, dconf text not null, tags text not null);
CREATE TABLE notes (id integer primary key, guid text not null, mid integer not null, mod integer not null,
  usn integer not null, tags text not null, flds text not null, sfld integer not null, csum integer not null,
  flags integer not null, data text not null);
CREATE TABLE cards (id integer primary key, nid integer not null, did integer not null, ord integer not null,
  mod integer not null, usn integer not null, type integer not null, queue integer not null, due integer not null,
  ivl integer not null, factor integer not null, reps integer not null, lapses integer not null, left integer not null,
  odue integer not null, odid integer not null, flags integer not null, data text not null);
CREATE TABLE revlog (id integer primary key, cid integer not null, usn integer not null, ease integer not null,
  ivl integer not null, lastIvl integer not null, factor integer not null, time integer not null, type integer not null);
CREATE TABLE graves (usn integer not null, oid integer not null, type integer not null);
CREATE INDEX ix_notes_usn on notes (usn);
CREATE INDEX ix_cards_usn on cards (usn);
CREATE INDEX ix_revlog_usn on revlog (usn);
CREATE INDEX ix_cards_nid on cards (nid);
CREATE INDEX ix_cards_sched on cards (did, queue, due);
CREATE INDEX ix_revlog_cid on revlog (cid);
CREATE INDEX ix_notes_csum on notes (csum);
";

/// genanki's collection config and default deck options.
const CONF: &str = r#"{"activeDecks":[1],"addToCur":true,"collapseTime":1200,"curDeck":1,"curModel":null,"dueCounts":true,"estTimes":true,"newBury":true,"newSpread":0,"nextPos":1,"sortBackwards":false,"sortType":"noteFld","timeLim":0}"#;
const DCONF: &str = r#"{"1":{"autoplay":true,"id":1,"lapse":{"delays":[10],"leechAction":0,"leechFails":8,"minInt":1,"mult":0},"maxTaken":60,"mod":0,"name":"Default","new":{"bury":true,"delays":[1,10],"initialFactor":2500,"ints":[1,4,7],"order":1,"perDay":20,"separate":true},"replayq":true,"rev":{"bury":true,"ease4":1.3,"fuzz":0.05,"ivlFct":1,"maxIvl":36500,"minSpace":1,"perDay":100},"timer":0,"usn":0}}"#;

/// Writes the collection into `path` as an `.apkg`.
pub fn write_apkg(deck: &Deck, path: &Path, now_ms: i64) -> Result<(), String> {
    let s = |e: &dyn std::fmt::Display| e.to_string();
    let now_s = now_ms / 1000;
    let tmp = path.with_extension("anki2.tmp");
    let _ = std::fs::remove_file(&tmp);
    {
        let db = Connection::open(&tmp).map_err(|e| s(&e))?;
        db.execute_batch(SCHEMA).map_err(|e| s(&e))?;

        let first_deck = deck.decks.last().map(|d| d.0).unwrap_or(1);
        let models: BTreeMap<String, Value> = [Model::Basic, Model::Cloze]
            .into_iter()
            .map(|m| (m.id().to_string(), model_json(m, first_deck, now_s)))
            .collect();
        let mut decks = BTreeMap::new();
        decks.insert("1".to_string(), deck_json(1, "Default", "", now_s));
        for (id, name, desc) in &deck.decks {
            decks.insert(id.to_string(), deck_json(*id, name, desc, now_s));
        }
        db.execute(
            "INSERT INTO col VALUES (1, ?1, ?2, ?2, 11, 0, 0, 0, ?3, ?4, ?5, ?6, '{}')",
            params![
                now_s - now_s % 86_400,
                now_ms,
                CONF,
                serde_json::to_string(&models).unwrap(),
                serde_json::to_string(&decks).unwrap(),
                DCONF
            ],
        )
        .map_err(|e| s(&e))?;

        let tx = db.unchecked_transaction().map_err(|e| s(&e))?;
        let mut next_id = now_ms;
        let mut due = 0;
        for note in &deck.notes {
            let nid = next_id;
            next_id += 1;
            tx.execute(
                "INSERT INTO notes VALUES (?1, ?2, ?3, ?4, -1, ?5, ?6, ?7, ?8, 0, '')",
                params![
                    nid,
                    note.guid,
                    note.model.id(),
                    now_s,
                    format!(" {} ", note.tags.join(" ")),
                    note.fields.join("\x1f"),
                    note.fields[0],
                    checksum(&note.fields[0]),
                ],
            )
            .map_err(|e| s(&e))?;
            for ord in note.card_ords() {
                // New cards come in book order: `due` is their position.
                due += 1;
                tx.execute(
                    "INSERT INTO cards VALUES (?1, ?2, ?3, ?4, ?5, -1, 0, 0, ?6, 0, 0, 0, 0, 0, 0, 0, 0, '')",
                    params![next_id, nid, note.deck, ord, now_s, due],
                )
                .map_err(|e| s(&e))?;
                next_id += 1;
            }
        }
        tx.commit().map_err(|e| s(&e))?;
    }
    let collection = std::fs::read(&tmp).map_err(|e| s(&e))?;
    let _ = std::fs::remove_file(&tmp);

    let file = std::fs::File::create(path).map_err(|e| s(&e))?;
    let mut zip = zip::ZipWriter::new(file);
    let deflated =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("collection.anki2", deflated).map_err(|e| s(&e))?;
    zip.write_all(&collection).map_err(|e| s(&e))?;
    zip.start_file("media", deflated).map_err(|e| s(&e))?;
    zip.write_all(b"{}").map_err(|e| s(&e))?;
    zip.finish().map_err(|e| s(&e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use super::*;

    fn fixture() -> Lecture {
        serde_yaml_ng::from_str(include_str!("../../../fixtures/content/sample.yaml")).unwrap()
    }

    #[test]
    fn ids_match_the_python_exporter() {
        // Values from genanki.guid_for and make_apkg.py's mid().
        assert_eq!(guid_for("hb.L01.001"), "kO@wJGlHO&");
        assert_eq!(guid_for("hb.L01.006#s2"), "HxUBC_F~bH");
        assert_eq!(Model::Basic.id(), 89_591_256);
        assert_eq!(Model::Cloze.id(), 4_060_380_611);
        assert_eq!(stable_id("batna-deck-hb.L01"), 755_872_679);
        assert_eq!(deck_name(&fixture()), "Batna::L01 Liver Cirrhosis");
        assert_eq!(esc("a<b & \"c\"\nd'"), "a&lt;b &amp; &quot;c&quot;<br>d&#x27;");
    }

    #[test]
    fn every_kind_becomes_notes() {
        let lec = fixture();
        let approved = BTreeSet::from(["hb.L01.001".to_string()]);
        let none_drafts = BTreeSet::new();
        let opts =
            Options { approved: &approved, verdicts: None, drafts: &none_drafts, now_ms: 1_800_000_000_000 };
        let deck = build(std::slice::from_ref(&lec), &opts).unwrap();
        // 7 one-note items + 2 case steps + 3 table blanks.
        assert_eq!(deck.notes.len(), 12);
        assert_eq!(deck.card_count(), 14, "the cloze has three blanks");
        assert_eq!(deck.notes[0].tags, ["hb::L01", "topic::definition", "approved"]);
        assert_eq!(
            deck.notes[1].tags,
            ["hb::L01", "topic::child-pugh", "unverified", "numbers", "high-yield"]
        );
        let mcq = &deck.notes[2];
        assert!(mcq.fields[0].ends_with("B. Class B<br>C. Class C<br>D. Cannot be scored without the INR"));
        assert!(
            mcq.fields[1].starts_with("<b>B</b><br><div class=\"why\"><b>A</b>: Class A needs 5-6 points")
        );
        let step2 = deck.notes.iter().find(|n| n.fields[4] == "hb.L01.006#s2").unwrap();
        assert!(step2.fields[0].contains("PaO2 falls by 6 mmHg on standing.<br><br><b>Step 2:</b>"));
        assert_eq!(step2.fields[1], "<b>C</b> LT improves oxygenation at 1 year. TIPS can worsen shunting.");
        let blank = deck.notes.iter().find(|n| n.fields[4] == "hb.L01.007#r2c1").unwrap();
        assert_eq!(blank.fields[0], "Child-Pugh points: <b>Bilirubin (mg/dL)</b> → <b>1 point</b>?");
        assert_eq!(blank.fields[1], "&lt;2");
        let calc = deck.notes.last().unwrap();
        assert_eq!(calc.fields[2], "Liver Cirrhosis · Book p. 14");
        assert!(calc.fields[3].starts_with("US organ allocation moved to MELD 3.0"));
        assert_eq!(
            deck.decks.iter().map(|d| d.1.as_str()).collect::<Vec<_>>(),
            ["Batna", "Batna::L01 Liver Cirrhosis"]
        );
    }

    #[test]
    fn verdicts_drop_wrong_items_and_use_edits() {
        let verdicts: Verdicts = serde_yaml_ng::from_str(
            "schema: batna.verdicts/v1\nverdicts:\n  - { id: hb.L01.003, verdict: wrong }\n  - { id: hb.L01.001, verdict: edited, edit: { back: Fibrosis plus nodules. } }\n  - { id: hb.L01.008, verdict: approved }\n",
        )
        .unwrap();
        let none = BTreeSet::new();
        let opts = Options { approved: &none, drafts: &none, verdicts: Some(&verdicts), now_ms: 0 };
        let deck = build(&[fixture()], &opts).unwrap();
        assert_eq!(deck.skipped, 1);
        assert_eq!(deck.notes.len(), 11);
        assert_eq!(deck.notes[0].fields[1], "Fibrosis plus nodules.");
        assert!(deck.notes[0].tags.contains(&"unverified".to_string()));
        let order = deck.notes.iter().find(|n| n.fields[4] == "hb.L01.008").unwrap();
        assert!(order.tags.contains(&"approved".to_string()));
    }

    #[test]
    fn unmerged_lectures_come_from_the_shared_folder() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::create_dir_all(d.join("content/hb")).unwrap();
        std::fs::create_dir_all(d.join("medstudy/drafts")).unwrap();
        std::fs::create_dir_all(d.join("medstudy/content-wip/hb")).unwrap();
        std::fs::write(
            d.join("content/index.yaml"),
            "schema: batna.content/v1\nlectures:\n  - { id: hb.L01, title: Liver Cirrhosis, pages: [1, 22], chunk: 10 }\n  - { id: hb.L02, title: Two, pages: [23, 36], chunk: 10 }\n  - { id: hb.L03, title: Three, pages: [37, 64], chunk: 11 }\n",
        )
        .unwrap();
        let sample = include_str!("../../../fixtures/content/sample.yaml");
        std::fs::write(d.join("content/hb/L01.yaml"), sample).unwrap();
        // Merged wins over wip; wip wins over drafts.
        std::fs::write(d.join("medstudy/content-wip/hb/L01.yaml"), "not: read").unwrap();
        std::fs::write(d.join("medstudy/content-wip/hb/L02.yaml"), sample.replace("hb.L01", "hb.L02"))
            .unwrap();
        std::fs::write(d.join("medstudy/drafts/L02.draft.yaml"), "not: read").unwrap();
        std::fs::write(d.join("medstudy/drafts/L03.draft.yaml"), "lecture: [").unwrap();
        let args = |extra: &[&str]| {
            let mut a: Vec<String> =
                ["--drafts", "medstudy", "--out", "out"].iter().map(|s| s.to_string()).collect();
            a[1] = d.join("medstudy").display().to_string();
            a[3] = d.join("out").display().to_string();
            a.extend(extra.iter().map(|s| s.to_string()));
            a
        };
        let report = run(d, &args(&[])).unwrap();
        assert!(report.contains("batna.apkg: 24 notes"), "{report}");
        assert!(report.contains("hb.L02: 12 notes (draft, not merged yet)"), "{report}");
        assert!(report.contains("L03.draft.yaml") && report.contains("(left out)"), "{report}");
        let merged = run(d, &args(&["--merged-only"])).unwrap();
        assert!(merged.contains("batna.apkg: 12 notes"), "{merged}");
    }

    #[test]
    fn the_apkg_is_an_anki_collection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("batna.apkg");
        let none = BTreeSet::new();
        let opts = Options { approved: &none, drafts: &none, verdicts: None, now_ms: 1_800_000_000_000 };
        let deck = build(&[fixture()], &opts).unwrap();
        write_apkg(&deck, &path, opts.now_ms).unwrap();

        let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut media = String::new();
        zip.by_name("media").unwrap().read_to_string(&mut media).unwrap();
        assert_eq!(media, "{}");
        let mut collection = Vec::new();
        zip.by_name("collection.anki2").unwrap().read_to_end(&mut collection).unwrap();
        let db_path = dir.path().join("collection.anki2");
        std::fs::write(&db_path, &collection).unwrap();

        // Read it back the way the brief asks: with Python's sqlite3.
        let script = r#"
import json, sqlite3, sys
db = sqlite3.connect(sys.argv[1])
assert db.execute("pragma integrity_check").fetchone()[0] == "ok"
notes = db.execute("select count(*) from notes").fetchone()[0]
cards = db.execute("select count(*) from cards").fetchone()[0]
orphans = db.execute("select count(*) from cards where nid not in (select id from notes)").fetchone()[0]
models, decks = map(json.loads, db.execute("select models, decks from col").fetchone())
fields = db.execute("select flds from notes where guid = 'kO@wJGlHO&'").fetchone()[0].split("\x1f")
names = sorted(d["name"] for d in decks.values())
print(notes, cards, orphans, sorted(m["name"] for m in models.values()), names, fields[4])
"#;
        let out =
            match std::process::Command::new("python3").args(["-I", "-c", script]).arg(&db_path).output() {
                Ok(out) => out,
                Err(e) => {
                    eprintln!("python3 not found ({e}); checking with rusqlite only");
                    let db = Connection::open(&db_path).unwrap();
                    let n: i64 = db.query_row("select count(*) from notes", [], |r| r.get(0)).unwrap();
                    assert_eq!(n, 12);
                    return;
                }
            };
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "12 14 0 ['Batna Basic', 'Batna Cloze'] ['Batna', 'Batna::L01 Liver Cirrhosis', 'Default'] hb.L01.001"
        );
    }
}
