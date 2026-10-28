//! Who a speaker label is: the cast registry (`content/cast.yaml`) read for
//! the app, so a Radio host, Sinetron character or chat partner can show
//! their portrait. `kabar-content` checks the registry itself; this only
//! reads ids, names, aliases and programs and ignores the rest.

use serde::Deserialize;

use crate::journey::ParseError;

#[derive(Debug, Deserialize)]
struct File {
    characters: Vec<Character>,
}

#[derive(Debug, Deserialize)]
struct Character {
    id: String,
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default, rename = "in")]
    programs: Vec<String>,
}

/// Speaker label → cast id.
#[derive(Debug, Default)]
pub struct Cast {
    names: Vec<(String, String)>,
    /// (cast id, programs, name and aliases as written), in registry order.
    people: Vec<(String, Vec<String>, Vec<String>)>,
}

impl Cast {
    pub fn parse(yaml: &str) -> Result<Self, ParseError> {
        let file: File = serde_yaml_ng::from_str(yaml).map_err(|e| ParseError(e.to_string()))?;
        let mut names = Vec::new();
        let mut people = Vec::new();
        for c in file.characters {
            names.push((c.name.to_lowercase(), c.id.clone()));
            names.extend(c.aliases.iter().map(|a| (a.to_lowercase(), c.id.clone())));
            let written = std::iter::once(c.name).chain(c.aliases).collect();
            people.push((c.id, c.programs, written));
        }
        Ok(Self { names, people })
    }

    /// The characters of `program` ("pagi", "sinetron", ...) whose name or
    /// alias appears as whole words in `text`, in registry order, each once.
    pub fn mentioned<'a>(&'a self, program: &str, text: &str) -> Vec<&'a str> {
        let words: Vec<&str> =
            text.split(|c: char| !c.is_alphanumeric() && c != '-').filter(|w| !w.is_empty()).collect();
        let has = |name: &str| {
            let parts: Vec<&str> = name.split_whitespace().collect();
            !parts.is_empty() && words.windows(parts.len()).any(|w| w == parts.as_slice())
        };
        self.people
            .iter()
            .filter(|(_, programs, written)| {
                programs.iter().any(|p| p == program) && written.iter().any(|n| has(n))
            })
            .map(|(id, _, _)| id.as_str())
            .collect()
    }

    /// Every word of every name and alias as written ("Mbak Gadis" gives
    /// "Mbak" and "Gadis"), honorifics included.
    pub fn name_words(&self) -> impl Iterator<Item = &str> {
        self.people.iter().flat_map(|(_, _, written)| written.iter()).flat_map(|n| n.split_whitespace())
    }

    /// The registry name of a cast id ("bu-murni" → "Bu Murni").
    pub fn name_of(&self, id: &str) -> Option<&str> {
        self.people
            .iter()
            .find(|(i, _, _)| i == id)
            .and_then(|(_, _, written)| written.first())
            .map(String::as_str)
    }

    /// The cast id of a speaker label ("Kak Tiara", "Tiara", "Ardi"), or a
    /// cast id itself. Roles ("Penjual") and one-off speakers have none.
    pub fn id_of(&self, speaker: &str) -> Option<&str> {
        let key = speaker.trim().to_lowercase();
        self.names.iter().find(|(name, id)| *name == key || *id == key).map(|(_, id)| id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "schema: kabar.cast/v1\nroles: [Penjual]\ncharacters:\n  - id: kak-tiara\n    name: Kak Tiara\n    home: Jakarta\n  - id: mbak-gadis\n    name: Mbak Gadis\n    aliases: [Gadis]\n  - id: omar\n    name: Omar\n    in: [pagi]\n  - id: bu-murni\n    name: Bu Murni\n    in: [pagi]\n";

    #[test]
    fn finds_who_a_story_mentions() {
        let cast = Cast::parse(YAML).unwrap();
        let text = "Pagi ini Omar lapar. Dia makan di warung Bu Murni. Omar senang.";
        assert_eq!(cast.mentioned("pagi", text), ["omar", "bu-murni"]);
        assert!(cast.mentioned("pagi", "Omari dan Murni.").is_empty());
        assert!(cast.mentioned("sinetron", text).is_empty());
        assert_eq!(cast.name_of("bu-murni"), Some("Bu Murni"));
        let words: Vec<&str> = cast.name_words().collect();
        assert_eq!(words, ["Kak", "Tiara", "Mbak", "Gadis", "Gadis", "Omar", "Bu", "Murni"]);
    }

    #[test]
    fn finds_names_aliases_and_ids() {
        let cast = Cast::parse(YAML).unwrap();
        assert_eq!(cast.id_of("Kak Tiara"), Some("kak-tiara"));
        assert_eq!(cast.id_of(" gadis "), Some("mbak-gadis"));
        assert_eq!(cast.id_of("mbak-gadis"), Some("mbak-gadis"));
        assert_eq!(cast.id_of("Penjual"), None);
    }
}
