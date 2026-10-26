//! Notebook search: recognized handwriting, word cards, typed notes and
//! mind-map nodes, across every page. Forgiving, because recognizers slip a
//! letter: a query word also matches a word one edit away (when it has four
//! letters or more).

use kabar_protocol::ink::{InkTranscript, ItemKind, NotebookItem};

use super::Rect;

#[derive(Clone, Debug, PartialEq)]
pub enum HitSource {
    Handwriting { group: String },
    Item { id: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub doc: String,
    pub source: HitSource,
    pub text: String,
    /// Where to scroll to.
    pub bbox: Rect,
    /// Higher is better.
    pub score: u32,
}

#[derive(Clone, Debug)]
struct Entry {
    doc: String,
    source: HitSource,
    text: String,
    words: Vec<String>,
    bbox: Rect,
}

#[derive(Clone, Debug, Default)]
pub struct NotebookIndex {
    entries: Vec<Entry>,
}

/// Lowercase, accents and punctuation off, words split on the rest.
pub fn words(text: &str) -> Vec<String> {
    let folded: String = text
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            c if c.is_alphanumeric() || c == '-' => c,
            _ => ' ',
        })
        .collect();
    folded.split_whitespace().map(|w| w.trim_matches('-').to_string()).filter(|w| !w.is_empty()).collect()
}

/// Levenshtein distance, stopping early above `max`.
pub fn within(a: &str, b: &str, max: usize) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return false;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j - 1] + usize::from(a[i - 1] != b[j - 1])).min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        if cur.iter().min().copied().unwrap_or(0) > max {
            return false;
        }
        prev = cur;
    }
    prev[b.len()] <= max
}

impl NotebookIndex {
    pub fn add_transcript(&mut self, t: &InkTranscript) {
        let [x, y, w, h] = t.bbox;
        self.push(
            t.doc.clone(),
            HitSource::Handwriting { group: t.group.clone() },
            &t.text,
            Rect { x, y, w, h },
        );
    }

    pub fn add_item(&mut self, item: &NotebookItem) {
        let text = match &item.kind {
            ItemKind::WordCard { word, .. } => word.clone(),
            ItemKind::Text { text } => text.clone(),
            ItemKind::MapNode { label, gloss, .. } => match gloss {
                Some(g) => format!("{label} {g}"),
                None => label.clone(),
            },
            ItemKind::Arrow { .. } => return,
        };
        let bbox = Rect { x: item.x, y: item.y, w: item.w, h: item.h };
        self.push(item.doc.clone(), HitSource::Item { id: item.id.clone() }, &text, bbox);
    }

    fn push(&mut self, doc: String, source: HitSource, text: &str, bbox: Rect) {
        let w = words(text);
        if !w.is_empty() {
            self.entries.push(Entry { doc, source, text: text.to_string(), words: w, bbox });
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries where every query word matches some word: exactly (3 points),
    /// as a prefix (2) or one edit away (1). Best first.
    pub fn search(&self, query: &str) -> Vec<Hit> {
        let q = words(query);
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<Hit> = self
            .entries
            .iter()
            .filter_map(|e| {
                let mut score = 0;
                for qw in &q {
                    let best = e
                        .words
                        .iter()
                        .map(|w| {
                            if w == qw {
                                3
                            } else if w.starts_with(qw.as_str()) {
                                2
                            } else if qw.chars().count() >= 4 && within(w, qw, 1) {
                                1
                            } else {
                                0
                            }
                        })
                        .max()
                        .unwrap_or(0);
                    if best == 0 {
                        return None;
                    }
                    score += best;
                }
                Some(Hit {
                    doc: e.doc.clone(),
                    source: e.source.clone(),
                    text: e.text.clone(),
                    bbox: e.bbox,
                    score,
                })
            })
            .collect();
        hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.doc.cmp(&b.doc)));
        hits
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::content::Lang;
    use kabar_protocol::ink::TranscriptSource;

    use super::*;

    fn transcript(doc: &str, group: &str, text: &str) -> InkTranscript {
        InkTranscript {
            doc: doc.into(),
            group: group.into(),
            strokes: vec![group.into()],
            text: text.into(),
            lang: Lang::Id,
            source: TranscriptSource::MlKit,
            confidence: Some(0.9),
            bbox: [10.0, 20.0, 200.0, 30.0],
        }
    }

    #[test]
    fn finds_handwriting_cards_and_nodes_forgivingly() {
        let mut idx = NotebookIndex::default();
        idx.add_transcript(&transcript("p1", "g1", "Rumahnya dibersihkan kemarin."));
        idx.add_transcript(&transcript("p2", "g2", "Saya sedang membava buku")); // recognizer slip
        idx.add_item(&NotebookItem {
            id: "c1".into(),
            doc: "p2".into(),
            kind: ItemKind::WordCard { lang: Lang::Id, word: "pasar".into() },
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 40.0,
        });
        idx.add_item(&NotebookItem {
            id: "n1".into(),
            doc: "m1".into(),
            kind: ItemKind::MapNode {
                label: "penjual".into(),
                gloss: Some("seller".into()),
                parent: None,
                depth: 1,
            },
            x: 0.0,
            y: 0.0,
            w: 80.0,
            h: 30.0,
        });
        assert_eq!(idx.len(), 4);

        let h = idx.search("kemarin");
        assert_eq!((h.len(), h[0].doc.as_str(), h[0].score), (1, "p1", 3));
        assert_eq!(h[0].bbox.w, 200.0);
        assert_eq!(idx.search("dibersih")[0].score, 2);
        assert_eq!(idx.search("membaca")[0].doc, "p2");
        assert_eq!(idx.search("PASAR")[0].source, HitSource::Item { id: "c1".into() });
        assert_eq!(idx.search("seller")[0].doc, "m1");
        assert!(idx.search("rumahnya pasar").is_empty(), "all words must match one entry");
        assert!(idx.search("  ").is_empty());
        assert!(idx.search("bku").is_empty(), "short words need a prefix or exact match");
    }

    #[test]
    fn words_and_distance() {
        assert_eq!(words("Anak-anak, BERMAIN di taman!"), ["anak-anak", "bermain", "di", "taman"]);
        assert_eq!(words("café"), ["cafe"]);
        assert!(within("membaca", "membava", 1));
        assert!(!within("membaca", "mbaca", 1));
        assert!(within("", "", 0));
    }
}
