//! No dead ends (Mohamed, 2026-10-28): a card, tile, row, chip or link
//! that shows a thing opens it. The last test reads the CSS: anything drawn
//! as a link (pointer, underline) must be pressable. Adapted from Batna-app main: crates/app/src/dead_ends.rs. This test reads the screens' source and fails when a
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
    "nur-card",
    "now-card",
    "now-milestone",
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
        for f in elements(src) {
            if PRESSABLE.contains(&f.element.as_str())
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
    for Found { element, classes, line, .. } in elements(src) {
        let classes: Vec<&str> = classes.iter().map(String::as_str).collect();
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

/// The cases #427's scanner missed (#433, ported from #437): a conditional class, a class
/// built in a variable, and one from `format!`.
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
}

/// Elements a pointer or an underline may sit on: real controls, and a
/// `label` (it toggles its checkbox).
const LINKISH_OK: &[&str] = &["button", "summary", "a", "label", "input", "select", "textarea"];

/// Class sets the CSS draws as a link or a control: a rule with
/// `cursor: pointer` or an underline whose selector is just those classes
/// (`.chip.toggle` → both), on no element or on one that can't be pressed.
fn linkish_classes(css: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let css = strip_comments(css);
    for rule in css.split('}') {
        let Some((selectors, decls)) = rule.rsplit_once('{') else { continue };
        let decls = decls.replace(' ', "");
        if !(decls.contains("cursor:pointer") || decls.contains("text-decoration:underline")) {
            continue;
        }
        for sel in selectors.split(',') {
            // Only a rule on the element itself: `.row .name { cursor }` is
            // about a name inside a row, which the row's own check covers.
            let sel = sel.trim();
            if sel.contains([' ', '>', '+', '~']) {
                continue;
            }
            // The subject, before any pseudo-class.
            let subject = sel;
            let subject = subject.split(':').next().unwrap_or("");
            let element: String = subject.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if LINKISH_OK.contains(&element.as_str()) {
                continue;
            }
            let set: Vec<String> = subject
                .split('.')
                .skip(1)
                .map(|c| {
                    c.chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                        .collect::<String>()
                })
                .filter(|c| !c.is_empty())
                .collect();
            // `mark.target` is about a mark only; `.row` about any element.
            let entry = (element, set);
            if !entry.1.is_empty() && !out.contains(&entry) {
                out.push(entry);
            }
        }
    }
    out
}

fn strip_comments(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        rest = rest[at..].find("*/").map_or("", |e| &rest[at + e + 2..]);
    }
    out.push_str(rest);
    out
}

#[test]
fn link_looks_are_pressable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut css = String::new();
    for dir in ["assets", "src"] {
        let mut files = Vec::new();
        css_files(&root.join(dir), &mut files);
        for f in files {
            css.push_str(&std::fs::read_to_string(f).unwrap());
            css.push('\n');
        }
    }
    let linkish = linkish_classes(&css);
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    let mut dead = Vec::new();
    for (path, src) in &files {
        if path.ends_with("dead_ends.rs") {
            continue;
        }
        let lines: Vec<&str> = src.lines().collect();
        for f in elements(src) {
            if LINKISH_OK.contains(&f.element.as_str())
                || !f.element.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                || has_onclick(&lines, f.line)
            {
                continue;
            }
            for (element, set) in &linkish {
                let on = element.is_empty() || *element == f.element;
                let allowed = set.iter().any(|c| ALLOWED.contains(&(f.element.as_str(), c.as_str())));
                if on && !allowed && set.iter().all(|c| f.classes.contains(c)) {
                    dead.push(format!(
                        "{path}:{}: `{}.{}` looks like a link or control (CSS: pointer or underline) but can't be pressed",
                        f.line,
                        f.element,
                        set.join(".")
                    ));
                }
            }
        }
    }
    dead.sort();
    dead.dedup();
    assert!(dead.is_empty(), "link-looking text that opens nothing:\n{}", dead.join("\n"));
}

/// Whether the element starting on `line` has its own `onclick` (before
/// its first child element).
fn has_onclick(lines: &[&str], line: usize) -> bool {
    lines[line - 1..]
        .iter()
        .take(12)
        .enumerate()
        .take_while(|(i, l)| *i == 0 || !l.trim_end().ends_with('{') || l.contains("onclick"))
        .any(|(_, l)| l.contains("onclick"))
}

fn css_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            css_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "css") {
            out.push(path);
        }
    }
}

#[test]
fn the_css_check_reads_link_looks() {
    let css = "/* .note { cursor: pointer } */\n.topic-link { text-decoration: underline dotted; }\nbutton.tap-chip { cursor: pointer; }\n.row:hover { cursor: pointer }\n.list .name { cursor: pointer }\nlabel.check { cursor: pointer }\nmark.hit { text-decoration: underline }\n.plain { color: red }";
    let want: Vec<(String, Vec<String>)> =
        vec![(String::new(), vec!["topic-link".into()]), (String::new(), vec!["row".into()])];
    let mut want = want;
    want.push(("mark".into(), vec!["hit".into()]));
    assert_eq!(linkish_classes(css), want);
}
