//! The full deterministic pipeline for one text: what the Reader's popovers
//! show before any AI is called.

use serde::{Deserialize, Serialize};

use super::langid::{DictHits, Item, LangGuess, identify};
use super::morph_id::{IdAnalysis, IndonesianMorph};
use super::morph_jv::{self, JvAnalysis};
use super::normalize::normalize;
use super::token::{Token, TokenKind, tokenize};
use super::{BuiltinLexicon, Lang, Lexicon};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Normalized {
    pub full: String,
    pub alt: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "lang", rename_all = "lowercase")]
pub enum Morph {
    Indonesian(IdAnalysis),
    Javanese(JvAnalysis),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnalyzedToken {
    pub token: Token,
    pub normalized: Option<Normalized>,
    pub lang: LangGuess,
    pub morph: Option<Morph>,
}

impl AnalyzedToken {
    /// The form to look up in the dictionary: normalized, lowercased.
    pub fn lookup_form(&self) -> String {
        self.normalized.as_ref().map_or_else(|| self.token.lower(), |n| n.full.to_lowercase())
    }
}

pub struct Engine {
    lexicon: Box<dyn Lexicon + Send + Sync>,
    id: IndonesianMorph,
}

impl Default for Engine {
    fn default() -> Self {
        Self { lexicon: Box::new(BuiltinLexicon::default()), id: IndonesianMorph::default() }
    }
}

impl Engine {
    pub fn new(lexicon: Box<dyn Lexicon + Send + Sync>, id: IndonesianMorph) -> Self {
        Self { lexicon, id }
    }

    pub fn analyze(&self, text: &str) -> Vec<AnalyzedToken> {
        let tokens = tokenize(text);
        let words: Vec<usize> = (0..tokens.len()).filter(|&i| tokens[i].is_wordlike()).collect();
        let lowers: Vec<String> = tokens.iter().map(Token::lower).collect();

        // Normalize with neighbouring words as context.
        let norms: Vec<_> = words
            .iter()
            .enumerate()
            .map(|(wi, &ti)| {
                let prev = wi.checked_sub(1).map(|p| lowers[words[p]].as_str());
                let next = words.get(wi + 1).map(|&n| lowers[n].as_str());
                normalize(&lowers[ti], prev, next)
            })
            .collect();

        let forms: Vec<String> = words
            .iter()
            .zip(&norms)
            .map(|(&ti, n)| n.map_or_else(|| self.base_form(&tokens[ti]), |n| n.full.to_lowercase()))
            .collect();

        let items: Vec<Item<'_>> = words
            .iter()
            .enumerate()
            .map(|(wi, &ti)| {
                let t = &tokens[ti];
                Item {
                    lower: &forms[wi],
                    slang: norms[wi].map(|n| n.lang),
                    dict: self.dict_hits(&forms[wi]),
                    capitalized: t.text.chars().next().is_some_and(char::is_uppercase),
                    sentence_start: wi == 0 || ti > 0 && tokens[ti - 1].kind == TokenKind::Punct,
                }
            })
            .collect();
        let guesses = identify(&items);

        let mut out: Vec<AnalyzedToken> = tokens
            .into_iter()
            .map(|token| AnalyzedToken { token, normalized: None, lang: LangGuess::UNKNOWN, morph: None })
            .collect();
        for (wi, &ti) in words.iter().enumerate() {
            let a = &mut out[ti];
            a.normalized = norms[wi].map(|n| Normalized { full: n.full.into(), alt: n.alt.map(Into::into) });
            a.lang = guesses[wi];
            a.morph = self.morph(&forms[wi], guesses[wi].lang);
        }
        out
    }

    /// Reduplicated words are analyzed through their base.
    fn base_form(&self, t: &Token) -> String {
        match &t.redup {
            Some((_, base)) => base.clone(),
            None => t.lower(),
        }
    }

    fn dict_hits(&self, w: &str) -> DictHits {
        let id = self.id.is_root(w) || self.id.root(w).is_some_and(|r| r != w || self.id.is_root(&r));
        let jv = self.lexicon.is_jv_root(w)
            || morph_jv::analyze(w, self.lexicon.as_ref()).is_some_and(|a| a.verified && a.root != w);
        DictHits { id: id || self.lexicon.is_id_root(w), jv }
    }

    fn morph(&self, form: &str, lang: Lang) -> Option<Morph> {
        if form.contains(' ') {
            return None;
        }
        match lang {
            Lang::Indonesian => self.id.analyze(form).map(Morph::Indonesian),
            Lang::Javanese => morph_jv::analyze(form, self.lexicon.as_ref()).map(Morph::Javanese),
            _ => None,
        }
    }
}
