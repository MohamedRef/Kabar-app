//! No dead ends (Mohamed, 2026-10-28): a card, tile or row that shows a
//! thing opens it. Adapted from Batna-app main: crates/app/src/dead_ends.rs. This test reads the screens' source and fails when a
//! class that looks tappable sits on an element that can't be pressed.
//!
//! The rule for new UI: put such a class on a `button` (with `.tap`, see
//! main.css) or a `summary`. A card that only holds a form or text keeps
//! plain `card`, and a result that refers to nothing (a game's score)
//! uses `stat-static`; add a class here when a new tappable look appears.

#![cfg(test)]

use std::path::Path;

/// Classes that look like they open something.
const TAPPABLE: &[&str] = &[
    "stat",
    "rev-stat",
    "reward",
    "plan-text",
    "souvenir",
    "lx-card",
    "lesson",
    "game-card",
    "journal-entry",
    "tap",
    "tap-head",
];

/// Elements that can be pressed. A `li.lx-card.locked` is a broken episode
/// (an error report, nothing to open).
const PRESSABLE: &[&str] = &["button", "summary", "a"];
const ALLOWED: &[(&str, &str)] = &[("li", "lx-card")];

fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push((path.display().to_string(), std::fs::read_to_string(&path).unwrap()));
        }
    }
}

/// `(element, class list)` for each `element { class: "…"` in rsx, the
/// element name taken from the text just before the brace.
fn elements(src: &str) -> Vec<(String, String, usize)> {
    let mut found = Vec::new();
    let lines: Vec<&str> = src.lines().collect();
    for (line_no, line) in lines.iter().enumerate() {
        let mut rest: &str = line;
        while let Some(at) = rest.find("class: \"") {
            let before = strip_strings(&rest[..at]);
            let classes = &rest[at + 8..];
            let classes = &classes[..classes.find('"').unwrap_or(classes.len())];
            // `li { key: "x", class: "y"` on one line, or `button {` on a line above.
            let element = match before.rfind('{') {
                Some(b) => ident_before(&before[..b]),
                None if before.trim().is_empty() || before.trim_end().ends_with(',') => lines[..line_no]
                    .iter()
                    .rev()
                    .map(|l| l.trim())
                    .find(|l| l.ends_with('{'))
                    .map_or(String::new(), |l| ident_before(&strip_strings(&l[..l.len() - 1]))),
                None => String::new(),
            };
            if !element.is_empty() {
                found.push((element, classes.to_string(), line_no + 1));
            }
            rest = &rest[at + 8..];
        }
    }
    found
}

/// The text with string literals emptied, so braces inside them don't count.
fn strip_strings(s: &str) -> String {
    let mut out = String::new();
    let mut in_str = false;
    for c in s.chars() {
        if c == '"' {
            in_str = !in_str;
            out.push(c);
        } else if !in_str {
            out.push(c);
        }
    }
    out
}

/// The identifier just before a `{`: `section { class` → `section`.
fn ident_before(s: &str) -> String {
    let s = s.trim_end();
    let start = s.rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':')).map_or(0, |i| i + 1);
    s[start..].to_string()
}

#[test]
fn tappable_looks_are_pressable() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut dead = Vec::new();
    for (path, src) in &files {
        if path.ends_with("dead_ends.rs") {
            continue;
        }
        for (element, classes, line) in elements(src) {
            if PRESSABLE.contains(&element.as_str())
                || !element.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            {
                continue;
            }
            for class in classes.split_whitespace() {
                if TAPPABLE.contains(&class) && !ALLOWED.contains(&(element.as_str(), class)) {
                    dead.push(format!("{path}:{line}: `{element}` has class `{class}` but can't be pressed"));
                }
            }
        }
    }
    assert!(
        dead.is_empty(),
        "dead cards or rows (make them buttons that open what they show):\n{}",
        dead.join("\n")
    );
}

#[test]
fn the_scanner_sees_elements() {
    let src =
        "li { key: \"{id}\", class: \"lecture\",\nbutton {\n    key: \"{i}\",\n    class: \"tile tap\",\n";
    let found = elements(src);
    assert_eq!(found[0].0, "li");
    assert_eq!(found[1], ("button".to_string(), "tile tap".to_string(), 4));
}
