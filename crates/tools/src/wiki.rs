//! Word counts from a MediaWiki XML dump (`*-pages-articles.xml.bz2`), used
//! for the Javanese frequency list. Markup is stripped roughly: templates,
//! tables, references, file links and tags go; link labels and prose stay.
//! That is plenty for frequency counts.

use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::text::words;

pub fn count_words(dump: &Path) -> Result<HashMap<String, u64>> {
    let file = fs::File::open(dump)
        .with_context(|| format!("{} is missing: run `scripts/fetch-sources.sh jvwiki`", dump.display()))?;
    let reader = BufReader::new(bzip2::read::MultiBzDecoder::new(BufReader::new(file)));
    count_xml(reader)
}

fn count_xml<R: std::io::BufRead>(input: R) -> Result<HashMap<String, u64>> {
    let mut reader = Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut counts = HashMap::new();
    let (mut in_ns, mut in_text, mut ns_main, mut redirect) = (false, false, false, false);
    let mut text = String::new();
    let mut pages = 0u64;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.name().as_ref() {
                b"page" => {
                    ns_main = false;
                    redirect = false;
                }
                b"ns" => in_ns = true,
                b"text" => {
                    in_text = true;
                    text.clear();
                }
                _ => {}
            },
            Event::Empty(e) if e.name().as_ref() == b"redirect" => redirect = true,
            Event::Text(t) if in_ns => ns_main = t.decode()?.trim() == "0",
            Event::Text(t) if in_text => text.push_str(&quick_xml::escape::unescape(&t.decode()?)?),
            Event::GeneralRef(r) if in_text => {
                // quick-xml reports entities separately; decode the common ones.
                text.push_str(match r.as_ref() {
                    b"lt" => "<",
                    b"gt" => ">",
                    b"amp" => "&",
                    b"quot" => "\"",
                    b"apos" => "'",
                    _ => " ",
                });
            }
            Event::End(e) => match e.name().as_ref() {
                b"ns" => in_ns = false,
                b"text" => {
                    in_text = false;
                    if ns_main && !redirect {
                        pages += 1;
                        for w in words(&strip_markup(&text)) {
                            *counts.entry(w).or_default() += 1;
                        }
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    eprintln!("counted {pages} articles");
    Ok(counts)
}

/// Rough wikitext → prose.
pub fn strip_markup(src: &str) -> String {
    let src = remove_nested(src, "{{", "}}");
    let src = remove_nested(&src, "{|", "|}");
    let src = remove_between(&src, "<ref", "</ref>");
    let src = remove_between(&src, "<!--", "-->");
    let mut out = String::with_capacity(src.len());
    let mut rest = src.as_str();
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            rest = after;
            break;
        };
        let link = &after[..end];
        let lower = link.to_lowercase();
        let is_file = ["file:", "berkas:", "gambar:", "image:", "kategori:", "category:"]
            .iter()
            .any(|p| lower.starts_with(p));
        if !is_file {
            out.push_str(link.rsplit('|').next().unwrap_or(link));
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    // Drop remaining tags and URLs.
    let mut clean = String::with_capacity(out.len());
    let mut in_tag = false;
    for c in out.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => clean.push(c),
            _ => {}
        }
    }
    clean.split_whitespace().filter(|w| !w.contains("://")).collect::<Vec<_>>().join(" ")
}

fn remove_nested(src: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut depth = 0usize;
    let mut i = 0;
    while i < src.len() {
        let rest = &src[i..];
        if rest.starts_with(open) {
            depth += 1;
            i += open.len();
        } else if depth > 0 && rest.starts_with(close) {
            depth -= 1;
            i += close.len();
        } else {
            let c = rest.chars().next().unwrap();
            if depth == 0 {
                out.push(c);
            }
            i += c.len_utf8();
        }
    }
    out
}

fn remove_between(src: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(s) = rest.find(open) {
        out.push_str(&rest[..s]);
        match rest[s..].find(close) {
            Some(e) => rest = &rest[s + e + close.len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_wikitext() {
        let src = "{{Infobox|jeneng={{lang|jv|Sala}}}}'''Surakarta''' iku [[kutha]] ing [[Jawa Tengah|Jawa Tengah]].<ref>{{cite}}</ref> [[Berkas:Solo.jpg|thumb|Foto]] <!-- note -->Kowé wis mangan?";
        assert_eq!(strip_markup(src), "'''Surakarta''' iku kutha ing Jawa Tengah. Kowé wis mangan?");
    }

    #[test]
    fn counts_only_main_namespace_articles() {
        let xml = r#"<mediawiki>
          <page><title>Solo</title><ns>0</ns><revision><text>Kowe wis mangan? Wis.</text></revision></page>
          <page><title>Alih</title><ns>0</ns><redirect title="Solo" /><revision><text>#ALIH [[Solo]]</text></revision></page>
          <page><title>Dhiskusi</title><ns>1</ns><revision><text>mangan mangan</text></revision></page>
          <page><title>Sega</title><ns>0</ns><revision><text>Aku mangan sega &amp; iwak.</text></revision></page>
        </mediawiki>"#;
        let counts = count_xml(xml.as_bytes()).unwrap();
        assert_eq!(counts["mangan"], 2);
        assert_eq!(counts["wis"], 2);
        assert_eq!(counts["iwak"], 1);
        assert!(!counts.contains_key("alih"));
    }
}
