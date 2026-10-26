//! URL import: a link pasted or shared into the Reader becomes a saved text.
//! Fetched natively (no CORS, unlike the WebView), so any public page works;
//! a page that can't be read asks the learner to paste the text instead.

use std::time::Duration;

use kabar_core::learn::text::{PageText, html_to_text};

/// Pages are articles, not downloads.
const MAX_BYTES: u64 = 4 * 1024 * 1024;
/// Below this, an `<article>` is a teaser card, not the story.
const MIN_ARTICLE_WORDS: usize = 30;

/// The text is a single http(s) link and nothing else.
pub fn as_url(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.is_empty() || s.contains(char::is_whitespace) {
        return None;
    }
    let rest = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    host.contains('.').then_some(s)
}

/// The readable part of a page: its `<article>` (or `<main>`) when it has a
/// real one, otherwise the whole page minus scripts and navigation.
pub fn article_text(html: &str) -> PageText {
    let full = html_to_text(html);
    // ASCII lowercasing keeps byte offsets, so they index `html` too.
    let lower = html.to_ascii_lowercase();
    for tag in ["article", "main"] {
        let (Some(a), Some(b)) = (lower.find(&format!("<{tag}")), lower.rfind(&format!("</{tag}>"))) else {
            continue;
        };
        if a < b {
            let part = html_to_text(&html[a..b + tag.len() + 3]);
            if part.text.split_whitespace().count() >= MIN_ARTICLE_WORDS {
                return PageText { title: full.title, text: part.text };
            }
        }
    }
    full
}

/// Blocking: run it off the UI thread ([`fetch`]).
pub fn fetch_page(url: &str) -> Result<PageText, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut res = agent
        .get(url)
        .header("Accept", "text/html,application/xhtml+xml,text/plain")
        .call()
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status()));
    }
    let content_type =
        res.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("text/html").to_lowercase();
    let body = res.body_mut().with_config().limit(MAX_BYTES).read_to_string().map_err(|e| e.to_string())?;
    let page = if content_type.contains("html") {
        article_text(&body)
    } else if content_type.starts_with("text/") {
        PageText { title: None, text: body }
    } else {
        return Err(format!("not a text page ({content_type})"));
    };
    if page.text.trim().is_empty() { Err("no readable text".into()) } else { Ok(page) }
}

pub async fn fetch(url: String) -> Result<PageText, String> {
    let (tx, rx) = futures_channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(fetch_page(&url));
    });
    rx.await.map_err(|_| "fetch thread stopped".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_lone_link_counts_as_a_url() {
        assert_eq!(
            as_url("  https://kompas.com/read/2026/10/26/abc \n"),
            Some("https://kompas.com/read/2026/10/26/abc")
        );
        assert_eq!(as_url("http://detik.com"), Some("http://detik.com"));
        assert_eq!(as_url("Baca ini https://kompas.com"), None);
        assert_eq!(as_url("https://localhost/x"), None);
        assert_eq!(as_url("ftp://kompas.com"), None);
        assert_eq!(as_url("Saya sedang membaca buku."), None);
    }

    #[test]
    fn the_article_wins_over_the_page_chrome() {
        let story = "Saya sedang membaca buku tentang Yogyakarta. ".repeat(8);
        let html = format!(
            "<html><head><title>Berita Hari Ini</title><script>var x = 1;</script></head><body>\
             <nav>Beranda Politik Olahraga</nav><ARTICLE class=\"post\"><h1>Judul</h1><p>{story}</p></ARTICLE>\
             <aside>Baca juga: berita lain</aside><footer>Hak cipta</footer></body></html>"
        );
        let page = article_text(&html);
        assert_eq!(page.title.as_deref(), Some("Berita Hari Ini"));
        assert!(page.text.contains("Yogyakarta"));
        assert!(!page.text.contains("Baca juga") && !page.text.contains("var x"));
    }

    #[test]
    fn a_teaser_article_falls_back_to_the_whole_page() {
        let html = "<body><article>Promo singkat</article><p>Kesehatan itu penting.</p></body>";
        let page = article_text(html);
        assert!(page.text.contains("Kesehatan itu penting."));
    }
}
