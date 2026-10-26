//! Turning what the learner hands Kabar (pasted text, a web page, OCR text,
//! a Wiktionary answer) into clean, short, scrubbed source text. Pure string
//! work: the app fetches, this only reads.

use kabar_protocol::content::Lang;
use serde_json::Value;

/// Replaces e-mail addresses and phone numbers, so nothing that identifies a
/// person leaves the device with learning text.
pub fn scrub(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        // Work token by token, keeping the whitespace between them.
        let ws = rest.len() - rest.trim_start().len();
        out.push_str(&rest[..ws]);
        rest = &rest[ws..];
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let token = &rest[..end];
        rest = &rest[end..];
        if is_email(token) {
            out.push_str("[email]");
        } else {
            out.push_str(token);
        }
    }
    scrub_phones(&out)
}

fn is_email(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !c.is_alphanumeric());
    match t.split_once('@') {
        Some((user, host)) => !user.is_empty() && host.contains('.') && !host.starts_with('.'),
        None => false,
    }
}

/// Runs of digits, spaces, dots, dashes, parentheses and `+` holding 9 or more
/// digits look like phone numbers (0812-3456-7890, +62 812 3456 7890).
/// Prices and model numbers ("512gb", "Rp 25.000") have fewer digits.
fn scrub_phones(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let starts = chars[i].is_ascii_digit()
            || chars[i] == '+' && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if starts && (i == 0 || !chars[i - 1].is_alphanumeric()) {
            let mut j = i;
            let mut digits = 0;
            let mut last_digit = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || " -.()+".contains(chars[j])) {
                if chars[j].is_ascii_digit() {
                    digits += 1;
                    last_digit = j;
                }
                j += 1;
            }
            let boundary = chars.get(last_digit + 1).is_none_or(|c| !c.is_alphanumeric());
            if digits >= 9 && boundary {
                out.push_str("[phone]");
                i = last_digit + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// A web page reduced to its readable text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageText {
    pub title: Option<String>,
    /// Paragraphs separated by newlines.
    pub text: String,
}

const SKIP: &[&str] = &["script", "style", "noscript", "svg", "template", "head", "nav", "footer", "form"];
const BLOCK: &[&str] = &[
    "p",
    "div",
    "br",
    "li",
    "ul",
    "ol",
    "dl",
    "dt",
    "dd",
    "tr",
    "table",
    "section",
    "article",
    "header",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "blockquote",
    "pre",
    "hr",
];

/// Strips tags, drops scripts/styles/navigation and decodes entities. Not a
/// full HTML parser: good enough for dictionary pages and blog posts.
pub fn html_to_text(html: &str) -> PageText {
    let mut title = None;
    let mut text = String::new();
    let mut skip_depth: Vec<String> = Vec::new();
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        if skip_depth.is_empty() {
            text.push_str(&rest[..lt]);
        }
        rest = &rest[lt..];
        if rest.starts_with("<!--") {
            rest = rest.find("-->").map_or("", |e| &rest[e + 3..]);
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if name == "title" && !closing {
            let end = rest.to_ascii_lowercase().find("</title").unwrap_or(rest.len());
            let t = collapse(&decode_entities(&rest[..end]));
            if !t.is_empty() {
                title = Some(t);
            }
            rest = &rest[end..];
            continue;
        }
        if SKIP.contains(&name.as_str()) && !tag.ends_with('/') {
            if closing {
                if skip_depth.last() == Some(&name) {
                    skip_depth.pop();
                }
            } else {
                skip_depth.push(name);
            }
            continue;
        }
        if skip_depth.is_empty() && BLOCK.contains(&name.as_str()) {
            text.push('\n');
        } else if skip_depth.is_empty() && matches!(name.as_str(), "td" | "th") {
            text.push(' ');
        }
    }
    if skip_depth.is_empty() {
        text.push_str(rest);
    }
    let text =
        decode_entities(&text).lines().map(collapse).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    PageText { title, text }
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let end = rest[..rest.len().min(12)].find(';');
        let decoded = end.and_then(|e| {
            let name = &rest[1..e];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some(' '),
                "ndash" => Some('–'),
                "mdash" => Some('—'),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// At most `max` characters of `text`, preferring the paragraphs that
/// mention `word`: long pages are mostly menus and unrelated articles.
pub fn excerpt(text: &str, word: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.trim().to_string();
    }
    let needle = word.to_lowercase();
    let paras: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let mut picked: Vec<usize> =
        (0..paras.len()).filter(|&i| paras[i].to_lowercase().contains(&needle)).collect();
    if picked.is_empty() {
        picked = (0..paras.len()).collect();
    }
    let mut out = String::new();
    for i in picked {
        let p = paras[i];
        let room = max.saturating_sub(out.chars().count() + 1);
        if room == 0 {
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.extend(p.chars().take(room));
    }
    out
}

/// Percent-encodes a query value (RFC 3986 unreserved characters kept).
pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A Google search for the word's meaning, opened in the browser (never
/// scraped). "artinya" / "tegese" is how Indonesians and Javanese ask.
pub fn google_search_url(lang: Lang, word: &str) -> String {
    let ask = match lang {
        Lang::Id => "artinya",
        Lang::Jv => "tegese bahasa jawa",
    };
    format!("https://www.google.com/search?q={}", url_encode(&format!("{} {ask}", word.trim())))
}

/// Wiktionary's free definition API (no key; CORS-enabled, so the WebView
/// can fetch it).
pub fn wiktionary_api_url(word: &str) -> String {
    format!("https://en.wiktionary.org/api/rest_v1/page/definition/{}", path_encode(word.trim()))
}

pub fn wiktionary_page_url(word: &str) -> String {
    format!("https://en.wiktionary.org/wiki/{}", path_encode(word.trim()))
}

fn path_encode(s: &str) -> String {
    url_encode(&s.replace(' ', "_")).replace('+', "_")
}

/// Reads a Wiktionary definition answer for one language: one line per
/// part of speech and definition, with examples. `None` when the word has no
/// entry in that language.
pub fn wiktionary_text(json: &Value, lang: Lang) -> Option<String> {
    let entries = json.get(lang.code())?.as_array()?;
    let mut out = String::new();
    for e in entries {
        let pos = e.get("partOfSpeech").and_then(Value::as_str).unwrap_or("");
        let mut n = 0;
        for d in e.get("definitions").and_then(Value::as_array).into_iter().flatten() {
            let def = d.get("definition").and_then(Value::as_str).map(|h| html_to_text(h).text);
            let Some(def) = def.filter(|d| !d.trim().is_empty()) else { continue };
            n += 1;
            out.push_str(&format!("{pos} {n}. {}\n", def.replace('\n', " ")));
            let examples = d.get("examples").and_then(Value::as_array).into_iter().flatten();
            for ex in examples.filter_map(Value::as_str).take(2) {
                out.push_str(&format!("   e.g. {}\n", html_to_text(ex).text.replace('\n', " ")));
            }
        }
    }
    let out = out.trim_end().to_string();
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubs_emails_and_phones_but_not_prices_or_models() {
        let s = scrub("minat wa 0812-3456-7890 atau +62 812 3456 7890, email budi.s@mail.co.id");
        assert_eq!(s, "minat wa [phone] atau [phone], email [email]");
        let listing = "iphone 17 pm 512gb resmi ibox mulus, Rp 25.000.000 nego";
        assert_eq!(scrub(listing), listing);
        assert_eq!(scrub("tahun 2024 ke 2025"), "tahun 2024 ke 2025");
    }

    #[test]
    fn html_to_text_keeps_the_readable_part() {
        let html = r#"<html><head><title>Arti Kata Mantul &amp; Contoh</title><style>p{}</style></head>
            <body><nav>Home | Login</nav><script>var x = "<p>no</p>";</script>
            <h1>Mantul</h1><p><b>mantul</b> artinya <i>mantap betul</i>&nbsp;(sangat bagus).</p>
            <!-- ad --><ul><li>Makanannya mantul!</li></ul><footer>© 2026</footer></body></html>"#;
        let page = html_to_text(html);
        assert_eq!(page.title.as_deref(), Some("Arti Kata Mantul & Contoh"));
        assert_eq!(page.text, "Mantul\nmantul artinya mantap betul (sangat bagus).\nMakanannya mantul!");
        assert_eq!(decode_entities("&#1582;&#x628;&#1585; &bogus; a&b"), "خبر &bogus; a&b");
    }

    #[test]
    fn excerpt_prefers_paragraphs_about_the_word() {
        let text = format!(
            "{}\nmantul = mantap betul\n{}\nContoh: mantul banget",
            "menu ".repeat(50),
            "x".repeat(300)
        );
        let e = excerpt(&text, "Mantul", 60);
        assert_eq!(e, "mantul = mantap betul\nContoh: mantul banget");
        assert_eq!(excerpt("short", "x", 60), "short");
        assert!(excerpt(&"a".repeat(500), "zzz", 100).chars().count() <= 100);
    }

    #[test]
    fn lookup_urls() {
        assert_eq!(google_search_url(Lang::Id, "mantul"), "https://www.google.com/search?q=mantul+artinya");
        assert_eq!(
            google_search_url(Lang::Jv, "dicul"),
            "https://www.google.com/search?q=dicul+tegese+bahasa+jawa"
        );
        assert_eq!(
            wiktionary_api_url("anak anak"),
            "https://en.wiktionary.org/api/rest_v1/page/definition/anak_anak"
        );
        assert_eq!(wiktionary_page_url("kabar"), "https://en.wiktionary.org/wiki/kabar");
    }

    #[test]
    fn reads_wiktionary_definitions_for_one_language() {
        let json: Value =
            serde_json::from_str(include_str!("../../../../fixtures/learn/wiktionary.kabar.json")).unwrap();
        let id = wiktionary_text(&json, Lang::Id).unwrap();
        assert!(id.starts_with("Noun 1. news"), "{id}");
        assert!(id.contains("e.g. Apa kabar?"), "{id}");
        assert!(!id.contains('<'));
        assert!(wiktionary_text(&json, Lang::Jv).is_none());
    }
}
