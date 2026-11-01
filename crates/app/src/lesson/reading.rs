//! Bidirectional lesson text, pronunciation runs and headings.

use super::*;

// ---------------------------------------------------------------------------
// Bidi helpers
// ---------------------------------------------------------------------------

pub(super) fn is_latin_letter(c: char) -> bool {
    c.is_alphabetic() && (c as u32) < 0x0250
}
/// Splits mixed text into (is_latin, run) so Latin snippets inside Arabic
/// explanations get their own `<bdi>` (SPEC §9). A run keeps together what
/// reads as one left-to-right phrase: several Latin sentences in a row
/// ("Ni Luh membawa tas. Tasnya dari kain."), affix hyphens ("-nya", "ber-"),
/// chains like "ora ↔ tidak · wis ↔ sudah", and parentheses it opened.
/// A Latin run in a slide body worth a pronunciation: a word or phrase, not
/// a lone letter, a number, an abbreviation or a "X …" pattern.
pub(super) fn is_target_run(run: &str) -> bool {
    let letters: Vec<char> = run.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 2
        && !letters.iter().all(|c| c.is_uppercase())
        && !run.chars().any(|c| c.is_ascii_digit() || c == '…')
}
pub fn latin_runs(s: &str) -> Vec<(bool, String)> {
    let mut out: Vec<(bool, String)> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let latin = |i: usize| chars.get(i).is_some_and(|&c| is_latin_letter(c));
    // The next non-space character after `i` is a capital Latin letter.
    let capital_after = |i: usize| {
        chars[i..]
            .iter()
            .find(|c| !c.is_whitespace())
            .is_some_and(|&c| is_latin_letter(c) && c.is_uppercase())
    };
    // "-ku" after a space or an Arabic letter ("aku و-ku") is a suffix: its
    // hyphen stays inside the Latin run, never mirrored to "ku-".
    let affix_at = |i: usize| {
        chars[i] == '-'
            && latin(i + 1)
            && (i == 0 || !(is_latin_letter(chars[i - 1]) || chars[i - 1].is_ascii_digit()))
    };
    let mut i = 0;
    while i < chars.len() {
        let affix = affix_at(i);
        if latin(i) || affix {
            let start = i;
            let mut end = i;
            if affix {
                i += 1;
            }
            while i < chars.len() {
                let c = chars[i];
                // Letters and digits extend the run; ? ! and a suffix hyphen
                // join it only straight after a letter ("Apa kabar?", "ber-").
                // A trailing … belongs to the pattern it ends ("Menurut X, …").
                if is_latin_letter(c) || c.is_ascii_digit() || c == '…' || ("?!-".contains(c) && end == i) {
                    end = i + 1;
                } else if c == '.' && chars.get(i + 1).is_none_or(|n| n.is_whitespace()) {
                    // A full stop ends the Arabic sentence, unless another
                    // Latin sentence follows it.
                    if !capital_after(i + 1) {
                        // A run made of sentences keeps its last full stop.
                        if run_has_sentence_break(&chars[start..end]) {
                            end = i + 1;
                        }
                        break;
                    }
                } else if !" -'’.,…/↔→·=+()".contains(c) {
                    break;
                }
                i += 1;
            }
            // Close a parenthesis the run opened.
            let open = chars[start..end].iter().filter(|&&c| c == '(').count();
            let close = chars[start..end].iter().filter(|&&c| c == ')').count();
            if open > close && chars.get(end) == Some(&')') {
                end += 1;
            }
            out.push((true, chars[start..end].iter().collect()));
            i = end;
        } else {
            let start = i;
            while i < chars.len() && !latin(i) && !affix_at(i) {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            match out.last_mut() {
                Some((false, prev)) => prev.push_str(&run),
                _ => out.push((false, run)),
            }
        }
    }
    out
}
pub(super) fn is_arabic_letter(c: char) -> bool {
    matches!(c, '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' | '\u{08A0}'..='\u{08FF}' | '\u{FB50}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}
/// Splits UI-language text of an English line into (is_arabic, run): an
/// Arabic run spans Arabic words and the spaces between them, never the
/// guillemets, brackets or commas around it, which stay with the English.
pub fn arabic_runs(s: &str) -> Vec<(bool, String)> {
    let chars: Vec<char> = s.chars().collect();
    let arabic = |i: usize| chars.get(i).is_some_and(|&c| is_arabic_letter(c));
    let mut out: Vec<(bool, String)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        if arabic(i) {
            let mut end = i;
            while i < chars.len() {
                if arabic(i) || chars[i] == '\u{200C}' {
                    end = i + 1;
                } else if chars[i] != ' ' {
                    break;
                }
                i += 1;
            }
            out.push((true, chars[start..end].iter().collect()));
            i = end;
        } else {
            while i < chars.len() && !arabic(i) {
                i += 1;
            }
            out.push((false, chars[start..i].iter().collect()));
        }
    }
    out
}
/// A run that already holds a sentence break (". " before a capital) is
/// itself made of sentences, so its final full stop belongs to it too.
pub(super) fn run_has_sentence_break(run: &[char]) -> bool {
    run.windows(3).any(|w| w[0] == '.' && w[1] == ' ' && w[2].is_uppercase())
}
/// Explanation text with Latin runs isolated (explicitly left-to-right: some
/// WebViews don't resolve `dir=auto` on an inline `<bdi>`, which turned
/// «Apa kabar?» into «?Apa kabar»). Digits outside the Latin runs follow the
/// Eastern-digits setting, whichever digits the content was written with.
/// `**words**` in the content are shown in bold.
#[component]
pub(crate) fn Rich(text: String, target: &'static str, #[props(default)] pron: bool) -> Element {
    let text = glue_articles(&text);
    rsx! {
        for (j, (bold, part)) in bold_parts(&text).into_iter().enumerate() {
            if bold {
                strong { key: "b{j}", RichRuns { text: part, target, pron } }
            } else {
                RichRuns { key: "p{j}", text: part, target, pron }
            }
        }
    }
}
/// "الـ kopi", "بالـ **Bu Rina**": the Arabic article is written apart from
/// the Latin word it belongs to. The space after it becomes a no-break space,
/// so a line never ends on "الـ" with its word on the next one.
pub(crate) fn glue_articles(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let article = i > 0 && chars[i - 1] == '\u{640}';
        let word = chars[i + 1..].iter().find(|&&n| n != '*').is_some_and(|&n| is_latin_letter(n));
        out.push(if c == ' ' && article && word { '\u{a0}' } else { c });
    }
    out
}
/// Splits `**bold**` markup out of content text: (is bold, text) parts in
/// order. An unpaired `**` stays as written.
pub(crate) fn bold_parts(text: &str) -> Vec<(bool, String)> {
    let pieces: Vec<&str> = text.split("**").collect();
    // An even piece count means one `**` has no partner: the last one.
    let paired = if pieces.len() % 2 == 1 { pieces.len() } else { pieces.len() - 1 };
    let mut parts: Vec<(bool, String)> = pieces[..paired]
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.is_empty())
        .map(|(i, p)| (i % 2 == 1, (*p).to_string()))
        .collect();
    if paired < pieces.len() {
        parts.push((false, format!("**{}", pieces[paired])));
    }
    parts
}
#[component]
pub(super) fn RichRuns(text: String, target: &'static str, pron: bool) -> Element {
    let ui = use_context::<Ui>();
    let (lang, eastern) = {
        let s = ui.settings.read();
        (s.ui_language, s.eastern_digits)
    };
    let dir = if lang.dir() == kabar_core::locale::Direction::Rtl { "rtl" } else { "ltr" };
    // Slides teach words inside Arabic sentences: say each one right there.
    let say = pron && lang != UiLanguage::En;
    let runs = move |key: String, runs: Vec<(bool, String)>| {
        rsx! {
            for (j , unit) in pron_units(runs, target, say).into_iter().enumerate() {
                match unit {
                    Unit::Plain(t) if dir == "ltr" => rsx! {
                        // Arabic in an English line («إنت») gets its own
                        // right-to-left isolate, the mirror of Latin above, so
                        // it can't pull the quotes and commas around it in.
                        span { key: "{key}.{j}",
                            for (k , (arabic, part)) in arabic_runs(&t).into_iter().enumerate() {
                                if arabic {
                                    bdi { key: "{k}", lang: "ar", dir: "rtl", "{part}" }
                                } else {
                                    span { key: "{k}", "{localize_digits(&part, lang, eastern)}" }
                                }
                            }
                        }
                    },
                    Unit::Plain(t) => rsx! {
                        span { key: "{key}.{j}", "{localize_digits(&in_direction(&t, dir == \"rtl\"), lang, eastern)}" }
                    },
                    Unit::Latin { phrases, close, short } => rsx! {
                        // The phrase, its closing quote and its pronunciation
                        // stay together; a short one stays on one line.
                        span { key: "{key}.{j}", class: if short { "pron-unit nowrap" } else { "pron-unit" },
                            for (k , bit) in layout(&phrases, &close, dir == "rtl").into_iter().enumerate() {
                                match bit {
                                    Bit::Latin(t) => rsx! {
                                        bdi { key: "{k}", lang: target, dir: "ltr", class: "latin", "{t}" }
                                    },
                                    Bit::Close(t) | Bit::Sep(t) => rsx! {
                                        span { key: "{k}", "{t}" }
                                    },
                                    Bit::Say(t) => rsx! {
                                        span { key: "{k}",
                                            Say { text: t.clone(), tag: target }
                                            Pron { text: t, tag: target, inline: true }
                                        }
                                    },
                                }
                            }
                        }
                    },
                }
            }
        }
    };
    rsx! {
        for (i, piece) in bracket_groups(latin_runs(&text)).into_iter().enumerate() {
            match piece {
                Piece::Run(latin, text) => runs(format!("{i}"), vec![(latin, text)]),
                Piece::Group(g) => {
                    let short = g.iter().map(|(_, r)| r.chars().count()).sum::<usize>() <= 28;
                    rsx! {
                        span { key: "{i}", dir, class: if short { "bidi-group nowrap" } else { "bidi-group" },
                            {runs(format!("{i}"), g)}
                        }
                    }
                }
            }
        }
    }
}
/// One phrase of a Latin run ("Selamat pagi" in "Selamat pagi → Sugeng
/// enjing"), whether to say it, and the separator after it.
#[derive(Debug, PartialEq)]
pub(super) struct Phrase {
    pub(super) text: String,
    pub(super) say: bool,
    pub(super) sep: String,
}
/// What a slide line is drawn from: UI-language text, or a Latin run split
/// into phrases with the closing quote or bracket that ends it.
#[derive(Debug, PartialEq)]
pub(super) enum Unit {
    Plain(String),
    Latin { phrases: Vec<Phrase>, close: String, short: bool },
}
pub(super) const CLOSERS: &[char] = &['»', '"', '”', ')'];
pub(super) const SEPARATORS: &[&str] = &[" → ", " ↔ ", " = ", " / ", " · "];
/// Splits runs into units: each Latin run takes the closing punctuation
/// straight after it (so «selamat jalan» isn't split by its pronunciation),
/// and mapping lines ("A → B") get one pronunciation per side. On Javanese
/// slides the Indonesian side isn't said: the learner knows it from BIPA.
pub(super) fn pron_units(runs: Vec<(bool, String)>, target: &str, say: bool) -> Vec<Unit> {
    let mut out = Vec::new();
    let mut runs = runs.into_iter().peekable();
    while let Some((latin, text)) = runs.next() {
        if !latin {
            out.push(Unit::Plain(text));
            continue;
        }
        let mut close = String::new();
        if let Some((false, next)) = runs.peek_mut() {
            let n: usize = next.chars().take_while(|c| CLOSERS.contains(c)).map(char::len_utf8).sum();
            close = next[..n].to_string();
            next.replace_range(..n, "");
        }
        let short = text.split_whitespace().count() <= 3;
        let phrases = split_phrases(&text)
            .into_iter()
            .map(|(t, sep)| Phrase {
                say: say && is_target_run(&t) && !(target == "jv" && is_indonesian(&t)),
                text: t,
                sep,
            })
            .collect();
        out.push(Unit::Latin { phrases, close, short });
        if let Some((false, next)) = runs.peek()
            && next.is_empty()
        {
            runs.next();
        }
    }
    out
}
/// UI-language text as drawn in a line of direction `rtl`: an arrow written
/// between an Arabic word and a Latin one ("besar = كبير → lebih besar")
/// points along the reading, so it turns around in a right-to-left line.
pub(super) fn in_direction(text: &str, rtl: bool) -> std::borrow::Cow<'_, str> {
    if rtl && text.contains('→') { text.replace('→', "←").into() } else { text.into() }
}
/// One piece of a drawn Latin unit, in reading order.
#[derive(Debug, PartialEq)]
pub(super) enum Bit {
    Latin(String),
    Close(String),
    /// The Arabic-script pronunciation of the phrase before it.
    Say(String),
    Sep(String),
}
/// The order a Latin unit is drawn in: each phrase in its own LTR isolate,
/// then the closing quote, then its pronunciation outside the isolate so in
/// an Arabic line it reads after the word (to its left), never before it.
/// Arrows turn around in a right-to-left line so "A ← B" still points from
/// the Indonesian to the Javanese side.
pub(super) fn layout(phrases: &[Phrase], close: &str, rtl: bool) -> Vec<Bit> {
    let last = phrases.len().saturating_sub(1);
    let mut out = Vec::new();
    for (k, ph) in phrases.iter().enumerate() {
        out.push(Bit::Latin(ph.text.clone()));
        if k == last && !close.is_empty() {
            out.push(Bit::Close(close.to_string()));
        }
        if ph.say {
            out.push(Bit::Say(ph.text.clone()));
        }
        if !ph.sep.is_empty() {
            out.push(Bit::Sep(if rtl { ph.sep.replace('→', "←") } else { ph.sep.clone() }));
        }
    }
    out
}
/// "Selamat pagi → Sugeng enjing" → [("Selamat pagi", " → "), ("Sugeng enjing", "")].
pub(super) fn split_phrases(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some((at, sep)) = SEPARATORS.iter().filter_map(|s| rest.find(s).map(|i| (i, *s))).min() {
        out.push((rest[..at].to_string(), sep.to_string()));
        rest = &rest[at + sep.len()..];
    }
    out.push((rest.to_string(), String::new()));
    out
}
/// Every word of the phrase reads as Indonesian (the engine's guess).
pub(super) fn is_indonesian(phrase: &str) -> bool {
    use kabar_core::lang::Lang;
    let tokens = crate::reader::doc::engine().analyze(phrase);
    let mut words = tokens.iter().filter(|t| t.token.is_wordlike()).peekable();
    words.peek().is_some() && words.all(|t| t.lang.lang == Lang::Indonesian)
}
/// A run of [`latin_runs`] output, or a bracketed group that mixes the UI
/// language with a Latin run ("(هو ngendika)"). The group is isolated in
/// the UI's direction so both brackets pair up around it, and a short one
/// is kept on one line.
#[derive(Debug, PartialEq)]
pub(super) enum Piece {
    Run(bool, String),
    Group(Vec<(bool, String)>),
}
pub(super) fn bracket_groups(runs: Vec<(bool, String)>) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut group: Option<Vec<(bool, String)>> = None;
    for (latin, text) in runs {
        if latin {
            match &mut group {
                Some(g) => g.push((true, text)),
                None => out.push(Piece::Run(true, text)),
            }
            continue;
        }
        let mut rest = text.as_str();
        loop {
            match &mut group {
                Some(g) => match rest.find(')') {
                    // The group closes in this run.
                    Some(at) => {
                        let (inside, after) = rest.split_at(at + 1);
                        g.push((false, inside.to_string()));
                        out.push(Piece::Group(group.take().unwrap_or_default()));
                        rest = after;
                    }
                    None => {
                        g.push((false, rest.to_string()));
                        break;
                    }
                },
                None => {
                    // An opening bracket whose partner isn't in this same run
                    // starts a group; brackets closed within the run stay put.
                    let open = rest
                        .char_indices()
                        .filter(|&(_, c)| c == '(')
                        .find(|&(at, _)| !rest[at..].contains(')'));
                    match open {
                        Some((at, _)) => {
                            if at > 0 {
                                out.push(Piece::Run(false, rest[..at].to_string()));
                            }
                            group = Some(Vec::new());
                            rest = &rest[at..];
                        }
                        None => {
                            if !rest.is_empty() {
                                out.push(Piece::Run(false, rest.to_string()));
                            }
                            break;
                        }
                    }
                }
            }
            if rest.is_empty() {
                break;
            }
        }
    }
    // A bracket that never closes: leave its runs as they were.
    if let Some(g) = group {
        out.extend(g.into_iter().map(|(l, t)| Piece::Run(l, t)));
    }
    out
}
/// A slide title: target-language text isolated left to right, or, when it
/// mixes in Arabic ("-nya = الـ / بتاعه"), laid out like [`Rich`] text.
#[component]
pub(super) fn Title(text: String, target: &'static str) -> Element {
    if text.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)) {
        rsx! { Rich { text, target } }
    } else {
        rsx! { bdi { lang: target, dir: "ltr", "{text}" } }
    }
}
