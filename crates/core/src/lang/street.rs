//! Spoken (street) forms and their formal twins: "nggak" → "tidak",
//! "makasih" → "terima kasih". The list is `content/freq/id.street.tsv`,
//! shared by reading coverage and answer grading so there is one source.

use std::sync::OnceLock;

use super::Lang;

const ID: &str = include_str!("../../../../content/freq/id.street.tsv");

fn parse(text: &'static str) -> Vec<(&'static str, &'static str)> {
    text.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .collect()
}

fn table(lang: Lang) -> &'static [(&'static str, &'static str)] {
    static ID_TABLE: OnceLock<Vec<(&str, &str)>> = OnceLock::new();
    match lang {
        Lang::Indonesian => ID_TABLE.get_or_init(|| parse(ID)),
        _ => &[],
    }
}

/// The formal twin of a lowercase street form, if it has one.
pub fn formal(lang: Lang, form: &str) -> Option<&'static str> {
    let t = table(lang);
    t.binary_search_by(|(s, _)| s.cmp(&form)).ok().map(|i| t[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_is_sorted_unique_and_well_formed() {
        let t = table(Lang::Indonesian);
        assert!(t.len() > 30);
        assert!(t.windows(2).all(|w| w[0].0 < w[1].0), "keep id.street.tsv sorted by street form");
        for (s, f) in t {
            assert!(!s.is_empty() && !f.is_empty() && s != f, "{s} → {f}");
            assert_eq!(s.to_lowercase(), *s);
        }
        let rows = ID.lines().filter(|l| !l.is_empty() && !l.starts_with('#')).count();
        assert_eq!(rows, t.len(), "every row is street<TAB>formal");
    }

    #[test]
    fn looks_up_street_forms() {
        assert_eq!(formal(Lang::Indonesian, "nggak"), Some("tidak"));
        assert_eq!(formal(Lang::Indonesian, "makasih"), Some("terima kasih"));
        assert_eq!(formal(Lang::Indonesian, "tidak"), None);
        assert_eq!(formal(Lang::Javanese, "nggak"), None);
    }
}
