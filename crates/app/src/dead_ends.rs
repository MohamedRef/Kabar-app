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
    "seed-word",
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

/// The rsx block that opens on `line` (1-based): its text up to the
/// matching brace, string literals emptied.
fn block_from(lines: &[&str], line: usize) -> String {
    let mut depth = 0i32;
    let mut out = String::new();
    for l in &lines[line - 1..] {
        let l = strip_strings(l);
        for c in l.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        out.push_str(&l);
        out.push('\n');
        if depth <= 0 {
            break;
        }
    }
    out
}

/// Buttons that leave a card rather than open what it shows.
const EXITS: &[&str] = &["Msg::Back)", "Msg::PlayAgain)", "Msg::RevBackHome)", "back_label("];

/// One-button cards whose button is a side action, not what the card
/// shows: freeing downloaded space (a stray tap must not delete), and
/// refreshing the month's cost.
const SIDE_ACTION_CARDS: &[&str] = &["lx-space", "cost-head"];

/// A card whose one job is one button (Kabar Pagi's "listen", the
/// placement invite) opens on a tap anywhere on it, not only on the
/// button: such a card carries `card-action` and an `onclick` of its own.
/// Cards with a choice (two buttons, a field, a link) are left alone.
/// Returns the lines of the cards in `src` that break this.
fn one_button_cards(src: &str) -> Vec<usize> {
    let lines: Vec<&str> = src.lines().collect();
    let mut dead = Vec::new();
    for (element, classes, line) in elements(src) {
        let classes: Vec<&str> = classes.split_whitespace().collect();
        if PRESSABLE.contains(&element.as_str()) || !classes.contains(&"card") {
            continue;
        }
        let block = block_from(&lines, line);
        let controls =
            ["button {", "input {", "textarea {", "select {", "a {", "summary {", "for ", "Link {"]
                .iter()
                .map(|c| block.matches(c).count())
                .sum::<usize>();
        let buttons = block.matches("button {").count();
        // Components inside may hold their own controls: only a plain card counts.
        let nested = block.lines().skip(1).any(|l| {
            let t = l.trim_start();
            t.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && t.contains(" {")
        });
        // A card whose button only leaves (Back, Play again, Review home) shows a result, not a thing.
        let raw: String = lines[line - 1..line - 1 + block.lines().count()].join("\n");
        let side =
            EXITS.iter().any(|e| raw.contains(e)) || classes.iter().any(|c| SIDE_ACTION_CARDS.contains(c));
        if buttons == 1 && controls == 1 && !nested && !side {
            let head: String = block.lines().take_while(|l| !l.contains("button {")).collect();
            if !classes.contains(&"card-action") || !head.contains("onclick") {
                dead.push(line);
            }
        }
    }
    dead
}

#[test]
fn single_action_cards_open_anywhere() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let dead: Vec<String> = files
        .iter()
        .filter(|(path, _)| !path.ends_with("dead_ends.rs"))
        .flat_map(|(path, src)| {
            one_button_cards(src).into_iter().map(move |line| {
                format!(
                    "{path}:{line}: a one-button card should open on a tap anywhere (`card-action` + onclick)"
                )
            })
        })
        .collect();
    assert!(dead.is_empty(), "single-action cards that only react to their button:\n{}", dead.join("\n"));
}

#[test]
fn the_card_check_sees_issue_433() {
    // Kabar Pagi before #433: only its button reacted.
    let before = "div { class: \"card pagi-card reveal\",\n    h2 { \"Kabar Pagi\" }\n    p { class: \"muted\", \"x\" }\n    button { class: \"primary\", onclick: move |_| go(), \"Listen\" }\n}\n";
    assert_eq!(one_button_cards(before), vec![1]);
    let after = "div { class: \"card card-action pagi-card\",\n    onclick: move |_| go(),\n    h2 { \"Kabar Pagi\" }\n    button { class: \"primary\", \"Listen\" }\n}\n";
    assert!(one_button_cards(after).is_empty());
    // Two choices (do it now / later) stay a card with two buttons.
    let choice = "div { class: \"card\",\n    button { onclick: move |_| a(), \"Now\" }\n    button { onclick: move |_| b(), \"Later\" }\n}\n";
    assert!(one_button_cards(choice).is_empty());
}
