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
    // Today's single-action cards (#433): the whole card presses its button.
    "pagi-card",
    "placement-invite",
    "st-link",
    "seed-word",
];

/// Elements that can be pressed. A `li.lx-card.locked` is a broken episode
/// (an error report, nothing to open).
const PRESSABLE: &[&str] = &["button", "summary", "a"];
const ALLOWED: &[(&str, &str)] = &[("li", "lx-card")];

/// A card whose one button covers it (main.css): it counts as pressable
/// when that `button.stretch` sits inside it.
const WHOLE_CARD: &str = "whole-card";
const STRETCH: &str = "stretch";

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

/// One rsx element with a class: its name, every class it can get, the
/// line it starts on, and the line its block ends on.
#[derive(Debug, PartialEq)]
struct Found {
    element: String,
    classes: Vec<String>,
    line: usize,
    end: usize,
}

/// Every `element { class: …` in rsx, the element name taken from the text
/// just before the brace. The class may be a literal (`"card tap"`), a
/// conditional or `format!` (every literal on the line counts), and a
/// `{name}` in it is looked up in the file's `let name = …;`.
fn elements(src: &str) -> Vec<Found> {
    const KEY: &str = "class: ";
    let mut found = Vec::new();
    let lines: Vec<&str> = src.lines().collect();
    for (line_no, line) in lines.iter().enumerate() {
        let mut rest: &str = line;
        while let Some(at) = rest.find(KEY) {
            let before = strip_strings(&rest[..at]);
            let value = &rest[at + KEY.len()..];
            rest = value;
            // `class: x` inside a string or a comment, or a struct field.
            if before.matches('"').count() % 2 == 1 || before.contains("//") {
                continue;
            }
            let mut classes = Vec::new();
            for lit in literals(value) {
                expand(src, &lit, &mut classes, 0);
            }
            // `li { key: "x", class: "y"` on one line, or `button {` on a line above.
            let (element, open_line) = match before.rfind('{') {
                Some(b) => (ident_before(&before[..b]), line_no),
                None if before.trim().is_empty() || before.trim_end().ends_with(',') => lines[..line_no]
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(i, l)| (i, l.trim()))
                    .find(|(_, l)| l.ends_with('{'))
                    .map_or((String::new(), line_no), |(i, l)| {
                        (ident_before(&strip_strings(&l[..l.len() - 1])), i)
                    }),
                None => (String::new(), line_no),
            };
            if !element.is_empty() && !classes.is_empty() {
                found.push(Found {
                    element,
                    classes,
                    line: line_no + 1,
                    end: block_end(&lines, open_line) + 1,
                });
            }
        }
    }
    found
}

/// The string literals in `value`, up to the end of the line: one for
/// `"card"`, both for `if x { "a" } else { "b" }`.
fn literals(value: &str) -> Vec<String> {
    let value = value.trim_start();
    // A plain literal is the class on its own; anything after it is another field.
    if let Some(body) = value.strip_prefix('"') {
        return vec![body[..body.find('"').unwrap_or(body.len())].to_string()];
    }
    let starts = ["if ", "match ", "format!(", "&format!("];
    if !starts.iter().any(|s| value.starts_with(s)) && !value.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Vec::new();
    }
    // A bare `class: name,` is a variable: expand it like `"{name}"`.
    let ident: String = value.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    if !starts.iter().any(|s| value.starts_with(s)) {
        return vec![format!("{{{ident}}}")];
    }
    value.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

/// Adds the classes in `lit`; a `{name}` adds the literals of the file's
/// `let name = …;` (a match or a conditional), so a class built in a
/// variable is still checked.
fn expand(src: &str, lit: &str, out: &mut Vec<String>, depth: u8) {
    for word in lit.split_whitespace() {
        let Some(name) = word.strip_prefix('{').and_then(|w| w.strip_suffix('}')) else {
            out.push(word.to_string());
            continue;
        };
        if depth > 2 {
            continue;
        }
        let Some(at) = src.find(&format!("let {name} =")).or_else(|| src.find(&format!("let mut {name} =")))
        else {
            continue;
        };
        let body = &src[at..];
        let body = &body[..body.find(";\n").unwrap_or(body.len())];
        for lit in body.split('"').skip(1).step_by(2) {
            expand(src, lit, out, depth + 1);
        }
    }
}

/// The line that closes the block opened on `open`: the first later line
/// indented no deeper than it.
fn block_end(lines: &[&str], open: usize) -> usize {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let base = indent(lines[open]);
    lines
        .iter()
        .enumerate()
        .skip(open + 1)
        .find(|(_, l)| !l.trim().is_empty() && indent(l) <= base)
        .map_or(lines.len() - 1, |(i, _)| i)
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

/// The identifier just before a `{`: `section { class` → `section`, and
/// `crate::art::Picture { class` → `Picture`.
fn ident_before(s: &str) -> String {
    let s = s.trim_end();
    let start = s.rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
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
        let found = elements(src);
        for f in &found {
            let has = |c: &str| f.classes.iter().any(|k| k == c);
            if has(WHOLE_CARD)
                && !found.iter().any(|b| {
                    b.element == "button"
                        && b.line > f.line
                        && b.line < f.end
                        && b.classes.iter().any(|k| k == STRETCH)
                })
            {
                dead.push(format!("{path}:{}: `.{WHOLE_CARD}` has no `button.{STRETCH}` inside", f.line));
                continue;
            }
            if PRESSABLE.contains(&f.element.as_str())
                || has(WHOLE_CARD)
                || !f.element.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            {
                continue;
            }
            for class in &f.classes {
                if TAPPABLE.contains(&class.as_str())
                    && !ALLOWED.contains(&(f.element.as_str(), class.as_str()))
                {
                    dead.push(format!(
                        "{path}:{}: `{}` has class `{class}` but can't be pressed",
                        f.line, f.element
                    ));
                }
            }
        }
    }
    dead.sort();
    dead.dedup();
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
    assert_eq!((found[0].element.as_str(), found[0].classes.clone()), ("li", vec!["lecture".to_string()]));
    assert_eq!(found[1].element, "button");
    assert_eq!(found[1].classes, ["tile", "tap"]);
    assert_eq!(found[1].line, 4);
}

/// The cases #427's scanner missed (#433): a conditional class, a class
/// built in a variable, and a whole card checked for its stretched button.
#[test]
fn the_scanner_sees_built_classes() {
    let src = "    let kind = match x {\n        A => \"reward lit\",\n        _ => \"reward\",\n    };\n\
               div { class: if on { \"stat on\" } else { \"stat\" },\n\
               span { class: \"{kind} big\", \"x\" }\n\
               div { class: format!(\"lesson {extra}\"),\n\
               Picture { class: \"stat\" }\n";
    let found = elements(src);
    let classes = |i: usize| found[i].classes.join(" ");
    assert_eq!(found[0].element, "div");
    assert_eq!(classes(0), "stat on stat");
    assert_eq!(found[1].element, "span");
    assert_eq!(classes(1), "reward lit reward big");
    assert_eq!(classes(2), "lesson");
    assert_eq!(found[3].element, "Picture");

    let card = "        div { class: \"card whole-card\",\n            p { \"hi\" }\n            button { class: \"stretch\", \"go\" }\n        }\n        button { class: \"stretch\" }\n";
    let found = elements(card);
    assert_eq!(found[0].end, 4);
    assert!(found[1].line < found[0].end && found[2].line > found[0].end);
}
