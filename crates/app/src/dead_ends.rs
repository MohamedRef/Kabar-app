//! No dead ends (Mohamed, 2026-10-28): a card, tile, row, chip or link
//! that shows a thing opens it. The last test reads the CSS: anything drawn
//! as a link (pointer, underline) must be pressable. The shared scanner from study-testkit fails when a
//! class that looks tappable sits on an element that can't be pressed.
//!
//! The rule for new UI: put such a class on a `button` (with `.tap`, see
//! main.css) or a `summary`. A card that only holds a form or text keeps
//! plain `card`, and a result that refers to nothing (a game's score)
//! uses `stat-static`; add a class here when a new tappable look appears.

#![cfg(test)]

use std::path::Path;
use study_testkit::ui_audit::{
    UiAuditPolicy, css_files, elements, link_problems, linkish_classes, one_button_cards,
    single_action_problems, sources, strip_comments, tappable_problems,
};

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
    // Shared components (components.css, redesign 2026-10-29).
    "activity-row",
    "answer",
    "rating",
    "ch-persona",
    "ch-scene",
    "ch-context",
    // Today and Journey (redesign 2026-10-29).
    "nur-card",
    "now-card",
    "now-milestone",
    // List screens' items in every layout (views.rs, components.css .coll):
    // cards, rows and compact lines are the same button or summary.
    "coll-btn",
    "view-menu-item",
];

/// Elements that can be pressed. A `li.lx-card.locked` is a broken episode
/// (an error report, nothing to open).
const ALLOWED: &[(&str, &str)] = &[("li", "lx-card")];

#[test]
fn tappable_looks_are_pressable() {
    let dead = tappable_problems(Path::new(env!("CARGO_MANIFEST_DIR")), &POLICY);
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

/// Buttons that leave a card rather than open what it shows.
const EXITS: &[&str] = &["Msg::Back)", "Msg::PlayAgain)", "Msg::RevBackHome)", "back_label("];

/// One-button cards whose button is a side action, not what the card
/// shows: freeing downloaded space (a stray tap must not delete), and
/// refreshing the month's cost.
const SIDE_ACTION_CARDS: &[&str] = &["lx-space", "cost-head"];

const POLICY: UiAuditPolicy<'static> = UiAuditPolicy {
    tappable: TAPPABLE,
    allowed: ALLOWED,
    exit_text: EXITS,
    side_action_cards: SIDE_ACTION_CARDS,
    skip_grouped: true,
};

#[test]
fn single_action_cards_open_anywhere() {
    let dead = single_action_problems(Path::new(env!("CARGO_MANIFEST_DIR")), &POLICY);
    assert!(dead.is_empty(), "single-action cards that only react to their button:\n{}", dead.join("\n"));
}

#[test]
fn the_card_check_sees_issue_433() {
    // Kabar Pagi before #433: only its button reacted.
    let before = "div { class: \"card pagi-card reveal\",\n    h2 { \"Kabar Pagi\" }\n    p { class: \"muted\", \"x\" }\n    button { class: \"primary\", onclick: move |_| go(), \"Listen\" }\n}\n";
    assert_eq!(one_button_cards(before, &POLICY), vec![1]);
    let after = "div { class: \"card card-action pagi-card\",\n    onclick: move |_| go(),\n    h2 { \"Kabar Pagi\" }\n    button { class: \"primary\", \"Listen\" }\n}\n";
    assert!(one_button_cards(after, &POLICY).is_empty());
    // Two choices (do it now / later) stay a card with two buttons.
    let choice = "div { class: \"card\",\n    button { onclick: move |_| a(), \"Now\" }\n    button { onclick: move |_| b(), \"Later\" }\n}\n";
    assert!(one_button_cards(choice, &POLICY).is_empty());
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

#[test]
fn link_looks_are_pressable() {
    let dead = link_problems(Path::new(env!("CARGO_MANIFEST_DIR")), &POLICY);
    assert!(dead.is_empty(), "link-looking text that opens nothing:\n{}", dead.join("\n"));
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

/// Components that put an opening control in the card that holds them
/// (a title that opens where its numbers come from).
const OPENING_COMPONENTS: &[&str] = &["CardHead {"];

/// Redesigned screens (2026-10-29) where every card must open something:
/// no card is only a picture of numbers. Each card's block holds an
/// `onclick` (its own or a button's) or an opening component.
const EVERY_CARD_OPENS: &[&str] = &["stats/mod.rs", "settings/mod.rs"];

/// Lines of cards in `src` that open nothing.
fn cards_that_open_nothing(src: &str) -> Vec<usize> {
    let lines: Vec<&str> = src.lines().collect();
    elements(src)
        .into_iter()
        .filter(|f| f.classes.iter().any(|c| c == "card"))
        .filter(|f| {
            let raw = lines[f.line - 1..f.end.min(lines.len())].join("\n");
            !raw.contains("onclick") && !OPENING_COMPONENTS.iter().any(|c| raw.contains(c))
        })
        .map(|f| f.line)
        .collect()
}

#[test]
fn redesigned_cards_all_open_something() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let dead: Vec<String> = EVERY_CARD_OPENS
        .iter()
        .flat_map(|file| {
            let text = std::fs::read_to_string(src.join(file)).unwrap();
            cards_that_open_nothing(&text).into_iter().map(move |line| format!("{file}:{line}"))
        })
        .collect();
    assert!(dead.is_empty(), "cards that open nothing:\n{}", dead.join("\n"));
}

#[test]
fn the_open_check_sees_a_picture_card() {
    let picture = "section { class: \"card st-card\",\n    h3 { \"Title\" }\n    p { \"42\" }\n}\n";
    assert_eq!(cards_that_open_nothing(picture), vec![1]);
    let opens = "section { class: \"card st-card\",\n    CardHead { title: t, to: Screen::Review }\n    p { \"42\" }\n}\n";
    assert!(cards_that_open_nothing(opens).is_empty());
}

/// List screens with a view switcher (views::ViewBar, 2026-10-29): in every
/// layout an item is the same `button.coll-btn` (or a details `summary`),
/// so a card, a row and a compact line all open what they show. Each `.coll`
/// list takes the screen's layout, its items are `coll-btn`s, every file
/// with a switcher reads its choice, and components.css draws all three.
#[test]
fn every_view_of_a_list_opens_its_items() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut bad = Vec::new();
    let mut lists = 0;
    for (path, src) in &files {
        if path.ends_with("dead_ends.rs") || path.ends_with("views.rs") {
            continue;
        }
        let lines: Vec<&str> = src.lines().collect();
        for f in elements(src) {
            if !f.classes.iter().any(|c| c == "coll") {
                continue;
            }
            lists += 1;
            let block = lines[f.line - 1..f.end].join("\n");
            if !lines[f.line - 1].contains("\"data-layout\"") {
                bad.push(format!("{path}:{}: a .coll list without the screen's data-layout", f.line));
            }
            // Items written inline, or a row component (DeckRow) that is.
            let row_component = block.contains("Row {");
            if !row_component && !block.contains("coll-btn") {
                bad.push(format!("{path}:{}: a .coll list whose items aren't coll-btn buttons", f.line));
            }
        }
        if src.contains("ViewBar { of: ") && !src.contains(".pick(&") {
            bad.push(format!("{path}: a ViewBar whose choice the list never reads"));
        }
    }
    let css =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/components.css")).unwrap();
    for layout in crate::views::Layout::ALL {
        if !css.contains(&format!("[data-layout=\"{}\"]", layout.as_str())) {
            bad.push(format!("components.css draws no `{}` layout", layout.as_str()));
        }
    }
    assert!(lists >= 7, "the scanner should see the list screens' .coll lists, saw {lists}");
    assert!(bad.is_empty(), "list views that can't open their items:\n{}", bad.join("\n"));
}

/// A row component inside a `.coll` list must itself be a `coll-btn`.
#[test]
fn the_bridge_rows_are_coll_items() {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/play/bridge_deck.rs"))
        .unwrap();
    assert!(src.contains("summary { class: \"coll-btn\""));
}

#[test]
fn shared_card_class_on_next_line_is_audited() {
    let source = "crate::ui::DivCard {\n    class: \"tap extra\",\n    button { \"Open\" }\n}";
    let cards = elements(source);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].element, "div");
    assert_eq!(cards[0].classes, ["card", "tap", "extra"]);
}

#[test]
fn shared_card_components_remain_visible_to_dead_end_checks() {
    let source = r#"
        crate::ui::Card {
            h2 { "Static card" }
        }
        crate::ui::DivCard { class: "flat extra",
            button { onclick: move |_| open(), "Open" }
        }
    "#;
    let cards = elements(source);
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].element, "section");
    assert_eq!(cards[0].classes, ["card"]);
    assert_eq!(cards[1].element, "div");
    assert_eq!(cards[1].classes, ["card", "flat", "extra"]);
}

#[test]
fn shared_keyed_and_semantic_cards_are_still_audited() {
    let source = "crate::ui::Card { element: crate::ui::CardElement::Article,\n key: \"item\",\n class: \"st-item\",\n}\ncrate::ui::CardButton { class: \"tap\", onclick: move |_| open(), \"Open\" }";
    let found = elements(source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].element, "article");
    assert!(found[0].shared && found[0].classes.iter().any(|c| c == "card"));
    assert_eq!(found[1].element, "button");
    assert!(found[1].shared);
}

#[test]
fn base_card_markup_is_owned_by_shared_components() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut duplicated = Vec::new();
    for (path, text) in files {
        if path.ends_with("/ui.rs") || path.ends_with("/ui/tests.rs") || path.ends_with("dead_ends.rs") {
            continue;
        }
        for found in elements(&text) {
            if !found.shared && found.classes.iter().any(|class| class == "card") {
                duplicated.push(format!("{path}:{}", found.line));
            }
        }
    }
    assert!(duplicated.is_empty(), "base cards must use Card/DivCard/CardButton: {}", duplicated.join(", "));
}

/// #575: a bare `[dir="rtl"] x` rule also matches the web page's own
/// `<html dir="rtl">`, so it fires in English too. Direction rules hang off
/// the app root (`.app[dir="rtl"] x`), which follows the learner's language.
#[test]
fn direction_rules_hang_off_the_app_root() {
    let mut files = Vec::new();
    css_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"), &mut files);
    css_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    assert!(files.len() > 10);
    let mut bad = Vec::new();
    for file in files {
        let css = strip_comments(&std::fs::read_to_string(&file).unwrap());
        for selector in css.split(['{', '}', ',']) {
            let sel = selector.trim();
            if sel.starts_with("[dir")
                || sel.contains(" [dir")
                || sel.contains(">[dir")
                || sel.contains("> [dir")
            {
                bad.push(format!("{}: {sel}", file.display()));
            }
        }
    }
    assert!(bad.is_empty(), "scope these to .app[dir=…]:\n{}", bad.join("\n"));
}
