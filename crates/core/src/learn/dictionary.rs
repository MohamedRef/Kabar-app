//! The personal dictionary: every entry the learner taught Kabar, as the
//! popover reads it. The app fills it from the synced `taught_entries`
//! table; this only indexes and orders.

use std::collections::BTreeMap;

use kabar_protocol::content::{GlossEntry, Lang, ReviewStatus};
use kabar_protocol::learn::{TaughtEntry, row_id};

#[derive(Clone, Debug, Default)]
pub struct PersonalDictionary {
    /// By sync row id (`{lang}:{word}`).
    entries: BTreeMap<String, TaughtEntry>,
}

impl PersonalDictionary {
    pub fn new(entries: impl IntoIterator<Item = TaughtEntry>) -> Self {
        let mut d = Self::default();
        for e in entries {
            d.insert(e);
        }
        d
    }

    pub fn insert(&mut self, entry: TaughtEntry) {
        self.entries.insert(entry.row_id(), entry);
    }

    pub fn remove(&mut self, lang: Lang, word: &str) -> Option<TaughtEntry> {
        self.entries.remove(&row_id(lang, word))
    }

    pub fn get(&self, lang: Lang, word: &str) -> Option<&TaughtEntry> {
        self.entries.get(&row_id(lang, word))
    }

    /// The entry for the first form that has one, trying forms in the
    /// Reader's order (surface, normalized, reduplication base, root): an
    /// exact word first, then an entry whose lemma is that form.
    pub fn lookup_forms(&self, lang: Lang, forms: &[&str]) -> Option<&TaughtEntry> {
        for form in forms {
            let form = form.trim().to_lowercase();
            if form.is_empty() {
                continue;
            }
            if let Some(e) = self.get(lang, &form) {
                return Some(e);
            }
            if let Some(e) = self.entries.values().find(|e| e.entry.lang == lang && e.entry.lemma == form) {
                return Some(e);
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drafts waiting for review (and entries marked for a fix), oldest
    /// word order.
    pub fn to_review(&self) -> impl Iterator<Item = &TaughtEntry> {
        self.entries.values().filter(|e| !e.is_approved())
    }

    pub fn approved(&self) -> impl Iterator<Item = &TaughtEntry> {
        self.entries.values().filter(|e| e.is_approved())
    }
}

/// One block in a word popover, in display order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GlossLayer<'a> {
    /// An approved entry: shown first, badged "taught by you".
    Taught(&'a TaughtEntry),
    /// The prebuilt gloss from the content pack.
    Prebuilt(&'a GlossEntry),
    /// A draft (or one marked for a fix): not an answer, only a chip that
    /// opens the review screen.
    Draft(&'a TaughtEntry),
}

/// Orders the popover: the learner's approved entry, then the prebuilt gloss
/// (never replaced, spec §26), then a draft chip.
pub fn popover_layers<'a>(
    taught: Option<&'a TaughtEntry>,
    prebuilt: Option<&'a GlossEntry>,
) -> Vec<GlossLayer<'a>> {
    let mut out = Vec::new();
    if let Some(t) = taught.filter(|t| t.entry.status == ReviewStatus::Approved) {
        out.push(GlossLayer::Taught(t));
    }
    if let Some(p) = prebuilt {
        out.push(GlossLayer::Prebuilt(p));
    }
    if let Some(t) = taught.filter(|t| t.entry.status != ReviewStatus::Approved) {
        out.push(GlossLayer::Draft(t));
    }
    out
}
