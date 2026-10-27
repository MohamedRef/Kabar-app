//! The cast registry (`content/cast.yaml`, `kabar.cast/v1`) and the
//! no-orphans rules (`content/pending.yaml`, `kabar.pending/v1`).
//!
//! Every recurring character is listed once, with the units they belong to,
//! and nobody else may share their name: the same name always means the same
//! person. Every unit has a Radio episode, a Sinetron episode and a chat
//! partner, or is listed as pending. Rules and the cast itself: docs/CAST.md.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::lessons::{Check, Index};

pub const CAST_SCHEMA_TAG: &str = "kabar.cast/v1";
pub const PENDING_SCHEMA_TAG: &str = "kabar.pending/v1";

/// A speaker in this many units' lessons (or tests) must be in the registry.
pub const MAX_EXTRA_UNITS: usize = 2;
/// A speaker in this many Sinetron episodes must be in the registry.
pub const MAX_EXTRA_EPISODES: usize = 4;

const PROGRAMS: &[&str] =
    &["lessons", "tests", "sinetron", "radio", "chat", "placement", "games", "pagi", "coach"];

/// Titles and honorifics a name may start with; the rest is the name itself.
const HONORIFICS: &[&str] = &[
    "Pak", "Bapak", "Bu", "Ibu", "Mas", "Mbak", "Kak", "Kaka", "Bang", "Uda", "Encik", "Om", "Tante", "Mama",
    "Mbah", "Cak", "Gus", "Dokter", "Dr", "Hagga", "Ni", "Mak", "Kang", "Teh", "Ning", "Inaq", "Amaq",
    "Daeng", "Haji", "Bli", "Andi", "Nenek", "Kakek", "Opung", "Pakdhe", "Budhe", "Mbok", "Ki", "Nyai",
    "Raden", "Adik",
];

/// A name without its honorifics, lower-cased: "Bu Siti" → "siti". A name
/// that is only an honorific ("Andi") stays as it is.
pub fn base_name(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let skip = words.iter().take_while(|w| HONORIFICS.contains(w)).count().min(words.len().saturating_sub(1));
    words[skip..].join(" ").to_lowercase()
}

/// Where one speaker label occurs.
#[derive(Debug, Default)]
pub struct Seen {
    /// Units whose lessons (or unit-level content) use the label.
    pub units: BTreeSet<String>,
    /// Sinetron episodes with the label in their cast.
    pub episodes: BTreeSet<String>,
    pub programs: BTreeSet<&'static str>,
    /// One file it occurs in, for messages.
    pub example: String,
}

/// Every speaker and every unit's programs, collected while the other checks
/// load their files.
#[derive(Debug, Default)]
pub struct Appearances {
    pub speakers: BTreeMap<String, Seen>,
    pub radio_units: BTreeSet<String>,
    pub sinetron_units: BTreeSet<String>,
    pub persona_units: BTreeSet<String>,
    /// Unggah-Ungguh scene ids → partner name.
    pub game_scenes: BTreeMap<String, String>,
    /// Game links lessons make (`game: uu.friend`).
    pub game_links: BTreeSet<String>,
}

/// `bipa1.u03.l02` → `bipa1.u03`.
fn unit_of(lesson: &str) -> Option<String> {
    let mut parts = lesson.split('.');
    match (parts.next(), parts.next()) {
        (Some(l), Some(u)) if u.starts_with('u') => Some(format!("{l}.{u}")),
        _ => None,
    }
}

fn speakers_in(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            if let Some(s) = m.get("speaker").and_then(Value::as_str) {
                out.insert(s.to_owned());
            }
            m.values().for_each(|x| speakers_in(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| speakers_in(x, out)),
        _ => {}
    }
}

impl Appearances {
    fn seen(&mut self, name: &str, program: &'static str, file: &str) -> &mut Seen {
        let s = self.speakers.entry(name.to_owned()).or_default();
        s.programs.insert(program);
        if s.example.is_empty() {
            s.example = file.to_owned();
        }
        s
    }

    pub fn lesson(&mut self, file: &str, stem: &str, v: &Value) {
        let mut names = BTreeSet::new();
        speakers_in(v, &mut names);
        let unit = unit_of(stem);
        for n in names {
            let s = self.seen(&n, "lessons", file);
            s.units.extend(unit.clone());
        }
        if let Some(g) = v.get("game").and_then(Value::as_str) {
            self.game_links.insert(g.to_owned());
        }
    }

    pub fn test(&mut self, file: &str, v: &Value) {
        let mut names = BTreeSet::new();
        speakers_in(v, &mut names);
        for n in names {
            self.seen(&n, "tests", file);
        }
    }

    pub fn story(&mut self, file: &str, v: &Value) {
        let id = v.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
        let unit = v.get("unlock_after").and_then(Value::as_str).unwrap_or_default().to_owned();
        for n in v.get("cast").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            self.seen(n, "sinetron", file).episodes.insert(id.clone());
        }
        self.sinetron_units.insert(unit);
    }

    pub fn radio(&mut self, file: &str, v: &Value) {
        let unit = v.get("unit").and_then(Value::as_str).unwrap_or_default().to_owned();
        for h in v.get("hosts").and_then(Value::as_array).into_iter().flatten() {
            if let Some(n) = h.get("name").and_then(Value::as_str) {
                self.seen(n, "radio", file).units.insert(unit.clone());
            }
        }
        self.radio_units.insert(unit);
    }

    pub fn persona(&mut self, file: &str, v: &Value) {
        if let Some(n) = v.get("name").and_then(Value::as_str) {
            self.seen(n, "chat", file);
        }
        let units = v.get("units").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str);
        self.persona_units.extend(units.map(str::to_owned));
    }

    /// The Unggah-Ungguh scenes (`[[scenes]]` with `id` and `partner.name`)
    /// of `content/games/unggah-ungguh.toml`. Read line by line: the game's
    /// own parser in kabar-core checks the format.
    pub fn unggah_ungguh(&mut self, file: &str, toml: &str) {
        let mut scene = None;
        for line in toml.lines() {
            let value = || line.split_once('=').map(|(_, v)| v.trim().trim_matches('"').to_owned());
            if line.trim() == "[[scenes]]" {
                scene = Some(None);
            } else if line.starts_with("id = ")
                && let Some(None) = scene
            {
                scene = Some(value());
            } else if line.starts_with("partner.name = ")
                && let Some(Some(id)) = &scene
                && let Some(name) = value()
            {
                self.seen(&name, "games", file);
                self.game_scenes.insert(id.clone(), name);
            }
        }
    }
}

/// One registry entry.
struct Character {
    id: String,
    name: String,
    aliases: Vec<String>,
    programs: BTreeSet<String>,
    units: BTreeSet<String>,
    family: Vec<String>,
}

/// Checks `cast.yaml` and `pending.yaml` (either may be `None` when absent,
/// which is itself a problem) against what the content uses.
pub fn check(
    cast: Option<&Value>,
    pending: Option<&Value>,
    personas: &[(String, Value)],
    seen: &Appearances,
    index: &Index,
    out: &mut Vec<String>,
) {
    let cast_file = "content/cast.yaml";
    let mut c = Check { file: cast_file, out };
    let Some(cast) = cast else {
        c.push("missing: every recurring character must be listed (docs/CAST.md)");
        return;
    };
    let Some(m) = c.fields("cast", cast, &["schema", "roles", "characters"], &[]) else { return };
    c.tag(m, CAST_SCHEMA_TAG);
    let roles: BTreeSet<String> = c.strings("roles", m.get("roles"), 1).into_iter().collect();
    let characters = characters(&mut c, m, index);

    // Names and aliases point at exactly one character.
    let mut by_name: BTreeMap<String, &Character> = BTreeMap::new();
    for ch in &characters {
        for n in std::iter::once(&ch.name).chain(&ch.aliases) {
            if let Some(other) = by_name.insert(n.clone(), ch) {
                c.push(format_args!("'{n}' names both {} and {}", other.id, ch.id));
            }
        }
    }
    let ids: BTreeMap<&str, &Character> = characters.iter().map(|ch| (ch.id.as_str(), ch)).collect();
    let related = |a: &Character, b: &Character| a.family.contains(&b.id) || b.family.contains(&a.id);
    for ch in &characters {
        for f in &ch.family {
            if !ids.contains_key(f.as_str()) {
                c.push(format_args!("{}: family '{f}' is not a character id", ch.id));
            }
        }
    }
    // Two characters never share a name, unless they are family.
    let mut by_base: BTreeMap<String, Vec<&Character>> = BTreeMap::new();
    for ch in &characters {
        by_base.entry(base_name(&ch.name)).or_default().push(ch);
    }
    for (base, list) in &by_base {
        for (i, a) in list.iter().enumerate() {
            for b in &list[i + 1..] {
                if !related(a, b) {
                    c.push(format_args!("{} and {} share the name '{base}'; rename one", a.id, b.id));
                }
            }
        }
    }

    for (name, s) in &seen.speakers {
        if roles.contains(name) {
            continue;
        }
        if let Some(ch) = by_name.get(name) {
            for p in &s.programs {
                if !ch.programs.contains(*p) {
                    c.push(format_args!("{}: add '{p}' to `in` ('{name}' speaks in {})", ch.id, s.example));
                }
            }
            continue;
        }
        let major = ["radio", "chat", "games"].iter().find(|p| s.programs.contains(**p));
        if let Some(p) = major {
            c.push(format_args!("'{name}' ({p}, {}) must be listed in the registry", s.example));
        } else if s.units.len() > MAX_EXTRA_UNITS || s.episodes.len() > MAX_EXTRA_EPISODES {
            c.push(format_args!(
                "'{name}' recurs ({} units, {} Sinetron episodes, e.g. {}); list them in the registry",
                s.units.len(),
                s.episodes.len(),
                s.example
            ));
        }
        if let Some(list) = by_base.get(&base_name(name)) {
            let ids: Vec<&str> = list.iter().map(|ch| ch.id.as_str()).collect();
            c.push(format_args!(
                "'{name}' ({}) shares a name with {}; rename them, or make them an alias if it is the same person",
                s.example,
                ids.join(", ")
            ));
        }
    }

    // Personas: one registry entry each, whose units cover the persona's.
    for (file, v) in personas {
        let id = v.get("id").and_then(Value::as_str).unwrap_or_default();
        let name = v.get("name").and_then(Value::as_str).unwrap_or_default();
        match ids.get(id) {
            None => c.out.push(format!("{file}: persona '{id}' must have a registry entry with the same id")),
            Some(ch) => {
                if ch.name != name {
                    c.out.push(format!("{file}: name '{name}' differs from the registry's '{}'", ch.name));
                }
                for u in
                    v.get("units").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str)
                {
                    if !ch.units.contains(u) {
                        c.out.push(format!("{file}: unit {u} is missing from {}'s registry units", ch.id));
                    }
                }
            }
        }
    }

    // Every Unggah-Ungguh scene is opened by some lesson.
    for scene in seen.game_scenes.keys() {
        if !seen.game_links.contains(scene) {
            c.push(format_args!("game scene {scene} is not linked from any lesson (`game: {scene}`)"));
        }
    }

    check_pending(pending, seen, index, c.out);
}

fn characters(c: &mut Check, m: &Map<String, Value>, index: &Index) -> Vec<Character> {
    let mut out = Vec::new();
    let list = m.get("characters").and_then(Value::as_array).cloned().unwrap_or_default();
    for (i, v) in list.iter().enumerate() {
        let at = format!("characters[{i}]");
        let Some(e) = c.fields(
            &at,
            v,
            &["id", "name", "home", "about", "in"],
            &["aliases", "voice", "units", "levels", "family"],
        ) else {
            continue;
        };
        let id = c.text(&format!("{at}.id"), e.get("id")).unwrap_or_default().to_owned();
        let at = if id.is_empty() { at } else { id.clone() };
        if !id.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-') {
            c.push(format_args!("{at}: id must be lower-case kebab-case"));
        }
        let name = c.text(&format!("{at}.name"), e.get("name")).unwrap_or_default().to_owned();
        c.text(&format!("{at}.home"), e.get("home"));
        c.text(&format!("{at}.about"), e.get("about"));
        let programs: BTreeSet<String> = c.strings(&format!("{at}.in"), e.get("in"), 1).into_iter().collect();
        for p in &programs {
            if !PROGRAMS.contains(&p.as_str()) {
                c.push(format_args!("{at}.in: '{p}' is not one of {PROGRAMS:?}"));
            }
        }
        let opt = |c: &mut Check, k: &str| {
            if e.contains_key(k) { c.strings(&format!("{at}.{k}"), e.get(k), 1) } else { Vec::new() }
        };
        let aliases = opt(c, "aliases");
        let units = opt(c, "units");
        let levels = opt(c, "levels");
        let family = opt(c, "family");
        if units.is_empty() && levels.is_empty() {
            c.push(format_args!(
                "{at}: needs `units` or `levels`: every character belongs to the curriculum"
            ));
        }
        for u in &units {
            if !index.units.contains(u.as_str()) {
                c.push(format_args!("{at}.units: '{u}' is not a curriculum unit id"));
            }
        }
        for l in &levels {
            if !index.levels.contains(l.as_str()) {
                c.push(format_args!("{at}.levels: '{l}' is not a curriculum level id"));
            }
        }
        // A level stands for all its units.
        let mut all: BTreeSet<String> = units.into_iter().collect();
        for l in &levels {
            all.extend(index.units.iter().filter(|u| u.starts_with(&format!("{l}."))).map(|u| u.to_string()));
        }
        out.push(Character { id, name, aliases, programs, units: all, family });
    }
    let mut ids = BTreeSet::new();
    for ch in &out {
        if !ids.insert(&ch.id) {
            c.push(format_args!("duplicate id {}", ch.id));
        }
    }
    out
}

/// Each unit has its Radio episode, Sinetron episode and (per level) a chat
/// partner, or is listed in `pending.yaml`; a listed unit that has one is an
/// error too, so the list only shrinks.
fn check_pending(pending: Option<&Value>, seen: &Appearances, index: &Index, out: &mut Vec<String>) {
    let mut c = Check { file: "content/pending.yaml", out };
    let Some(p) = pending else {
        c.push("missing (list the units still waiting for Radio, Sinetron or a chat partner)");
        return;
    };
    let Some(m) = c.fields("pending", p, &["schema", "radio", "sinetron", "personas"], &[]) else { return };
    c.tag(m, PENDING_SCHEMA_TAG);
    let list = |k: &str| -> BTreeSet<String> {
        m.get(k)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    };
    for (what, has) in [("radio", &seen.radio_units), ("sinetron", &seen.sinetron_units)] {
        let pending = list(what);
        for u in &index.units {
            match (has.contains(*u), pending.contains(*u)) {
                (false, false) => {
                    c.push(format_args!("{u} has no {what} episode; write one or list it under {what}"))
                }
                (true, true) => c.push(format_args!("{u} has its {what} episode now; remove it from {what}")),
                _ => {}
            }
        }
        for u in &pending {
            if !index.units.contains(u.as_str()) {
                c.push(format_args!("{what}: '{u}' is not a curriculum unit id"));
            }
        }
    }
    let pending = list("personas");
    for l in &index.levels {
        let prefix = format!("{l}.");
        let has = seen.persona_units.iter().any(|u| u.starts_with(&prefix));
        match (has, pending.contains(*l)) {
            (false, false) => {
                c.push(format_args!("no persona practises a {l} unit; add one or list {l} under personas"))
            }
            (true, true) => c.push(format_args!("{l} has a chat partner now; remove it from personas")),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::base_name;

    #[test]
    fn base_names_drop_honorifics() {
        assert_eq!(base_name("Bu Siti"), "siti");
        assert_eq!(base_name("Mas Dimas"), base_name("Dimas"));
        assert_eq!(base_name("Hagga Fatma"), "fatma");
        assert_eq!(base_name("Andi Tenri"), "tenri");
        assert_eq!(base_name("Andi"), "andi");
        assert_eq!(base_name("Ni Luh"), "luh");
    }
}
