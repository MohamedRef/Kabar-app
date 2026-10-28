//! Chat-slang normalizer: "yg" → "yang", "arp" → "arep", "ng" → "nang".
//!
//! The seed list is curated by hand; the content pipeline extends it from
//! Wiktionary "text messaging" entries. Forms nothing here knows go to the
//! AI fallback and are queued for review (never silently guessed).

use super::Lang;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Norm {
    pub form: &'static str,
    /// The full form ("Pro Max" may be several words).
    pub full: &'static str,
    /// Language of the full form.
    pub lang: Lang,
    /// More than one reading is common; show both in the popover.
    pub alt: Option<&'static str>,
}

const fn n(form: &'static str, full: &'static str, lang: Lang) -> Norm {
    Norm { form, full, lang, alt: None }
}

const fn alt(form: &'static str, full: &'static str, lang: Lang, alt: &'static str) -> Norm {
    Norm { form, full, lang, alt: Some(alt) }
}

use Lang::{English as En, Indonesian as Id, Javanese as Jv};

/// Sorted by form for binary search (checked by a test).
static SEED: &[Norm] = &[
    alt("aja", "saja", Id, "aja (colloquial)"),
    n("ak", "aku", Id),
    n("ama", "sama", Id),
    n("arp", "arep", Jv),
    n("bgt", "banget", Id),
    n("blm", "belum", Id),
    n("brp", "berapa", Id),
    n("bs", "bisa", Id),
    n("bsk", "besok", Id),
    n("cod", "cash on delivery", En),
    n("dg", "dengan", Id),
    n("dgn", "dengan", Id),
    n("dicul", "diculke", Jv),
    n("dkk", "dan kawan-kawan", Id),
    n("dlm", "dalam", Id),
    n("dr", "dari", Id),
    n("g", "tidak", Id),
    alt("ga", "tidak", Id, "nggak"),
    alt("gak", "tidak", Id, "nggak"),
    n("gimana", "bagaimana", Id),
    alt("gk", "tidak", Id, "nggak"),
    n("gmn", "bagaimana", Id),
    n("gpp", "tidak apa-apa", Id),
    n("hrs", "harus", Id),
    n("jg", "juga", Id),
    n("jgn", "jangan", Id),
    n("kalo", "kalau", Id),
    n("karna", "karena", Id),
    n("klo", "kalau", Id),
    n("km", "kamu", Id),
    n("knp", "kenapa", Id),
    n("krn", "karena", Id),
    n("lg", "lagi", Id),
    n("mksh", "terima kasih", Id),
    n("msh", "masih", Id),
    alt("ng", "nang", Jv, "neng"),
    n("ngga", "tidak", Id),
    n("nggak", "tidak", Id),
    n("nomin", "no minus", En),
    n("opo", "apa", Jv),
    n("org", "orang", Id),
    n("pdhl", "padahal", Id),
    n("piro", "pira", Jv),
    n("pm", "Pro Max", En),
    n("ra", "ora", Jv),
    n("sbg", "sebagai", Id),
    n("sdh", "sudah", Id),
    alt("sek", "sek", Jv, "sik"),
    n("sik", "sek", Jv),
    n("skrg", "sekarang", Id),
    n("sm", "sama", Id),
    n("smpe", "sampai", Id),
    n("spt", "seperti", Id),
    n("sy", "saya", Id),
    n("td", "tadi", Id),
    n("tdk", "tidak", Id),
    n("thx", "thanks", En),
    n("tp", "tapi", Id),
    n("trs", "terus", Id),
    n("trus", "terus", Id),
    n("udah", "sudah", Id),
    n("udh", "sudah", Id),
    n("utk", "untuk", Id),
    n("wa", "WhatsApp", En),
    n("wes", "wis", Jv),
    n("wkwk", "(laughter)", Id),
    n("yg", "yang", Id),
];

/// Street forms whose formal twin is too loose to grade by: "terus" also
/// means "keep on", not only "lalu".
const NOT_FOR_GRADING: &[&str] = &["terus"];

/// The formal twin of a lowercase word for grading, in `lang`'s street list
/// ([`super::street`]): Indonesian "udah" → "sudah"; Javanese once its list
/// lands (#297). Indonesian also folds its one-word chat forms ("tp" → "tapi"
/// → "tetapi"). One word maps to one word; `None` for a word that is already
/// formal, or a language without a list.
pub fn formal_twin(lang: Lang, word: &str) -> Option<&'static str> {
    let street = |w: &str| {
        super::street::formal(lang, w).filter(|f| !f.contains(' ') && !NOT_FOR_GRADING.contains(&w))
    };
    if let Some(twin) = street(word) {
        return Some(twin);
    }
    if lang != Lang::Indonesian {
        return None;
    }
    // "g" alone is too ambiguous (see `normalize`).
    let chat =
        lookup(word).filter(|n| n.lang == Lang::Indonesian && n.form != "g" && !n.full.contains(' '))?;
    Some(street(chat.full).unwrap_or(chat.full))
}

/// Looks up a lowercase form in the seed list.
pub fn lookup(form: &str) -> Option<&'static Norm> {
    SEED.binary_search_by(|e| e.form.cmp(form)).ok().map(|i| &SEED[i])
}

/// Normalizes a token in context. `prev`/`next` are neighbouring lowercase
/// words, used for forms that are only slang in some contexts.
pub fn normalize(form: &str, prev: Option<&str>, next: Option<&str>) -> Option<&'static Norm> {
    let norm = lookup(form)?;
    match form {
        // "pm" is Pro Max only after a phone name or model number.
        "pm" => {
            prev.filter(|p| p.chars().all(|c| c.is_ascii_digit()) || p.starts_with("iphone")).map(|_| norm)
        }
        // "wa" as WhatsApp: after "minat", "chat", "hub", "via", "kontak", or alone at the end.
        "wa" => (matches!(prev, Some("minat" | "chat" | "hub" | "via" | "kontak" | "no")) || next.is_none())
            .then_some(norm),
        // "g" alone is too ambiguous unless followed by a verb-ish word.
        "g" => next.map(|_| norm),
        _ => Some(norm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formal_twins_for_grading() {
        assert_eq!(formal_twin(Id, "nggak"), Some("tidak"));
        assert_eq!(formal_twin(Id, "ngga"), Some("tidak"));
        assert_eq!(formal_twin(Id, "udh"), Some("sudah"));
        assert_eq!(formal_twin(Id, "tp"), Some("tetapi"), "chat form → street form → formal");
        assert_eq!(formal_twin(Id, "makasih"), None, "two words: not a one-word twin");
        assert_eq!(formal_twin(Id, "terus"), None, "too loose to grade by");
        assert_eq!(formal_twin(Id, "gpp"), None);
        assert_eq!(formal_twin(Id, "ra"), None, "Javanese");
        assert_eq!(formal_twin(Jv, "aja"), None, "Javanese aja is \"don't\", not saja");
        assert_eq!(formal_twin(Id, "makan"), None);
        for w in ["tidak", "sudah", "tetapi", "saja", "bagaimana"] {
            assert_eq!(formal_twin(Id, w), None, "{w} is already formal");
        }
    }

    #[test]
    fn seed_is_sorted_and_unique() {
        assert!(SEED.windows(2).all(|w| w[0].form < w[1].form), "keep SEED sorted by form");
    }

    #[test]
    fn spec_examples() {
        assert_eq!(lookup("yg").unwrap().full, "yang");
        for f in ["gk", "ga", "gak"] {
            let n = lookup(f).unwrap();
            assert_eq!((n.full, n.alt), ("tidak", Some("nggak")));
        }
        assert_eq!(lookup("udh").unwrap().full, "sudah");
        assert_eq!(lookup("dr").unwrap().full, "dari");
        assert_eq!(lookup("arp").unwrap().full, "arep");
        let ng = lookup("ng").unwrap();
        assert_eq!((ng.full, ng.alt, ng.lang), ("nang", Some("neng"), Lang::Javanese));
        assert_eq!(lookup("sek").unwrap().alt, Some("sik"));
    }

    #[test]
    fn context_sensitive_forms() {
        assert_eq!(normalize("pm", Some("17"), Some("512gb")).map(|n| n.full), Some("Pro Max"));
        assert!(normalize("pm", Some("jam"), Some("nanti")).is_none());
        assert_eq!(normalize("wa", Some("minat"), None).map(|n| n.full), Some("WhatsApp"));
        assert!(normalize("wa", Some("si"), Some("datang")).is_none());
    }
}
