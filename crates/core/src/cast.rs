//! Who a speaker label is: the cast registry (`content/cast.yaml`) read for
//! the app, so a Radio host, Sinetron character or chat partner can show
//! their portrait. `kabar-content` checks the registry itself; this only
//! reads ids, names and aliases and ignores the rest.

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
}

/// Speaker label → cast id.
#[derive(Debug, Default)]
pub struct Cast {
    names: Vec<(String, String)>,
}

impl Cast {
    pub fn parse(yaml: &str) -> Result<Self, ParseError> {
        let file: File = serde_yaml_ng::from_str(yaml).map_err(|e| ParseError(e.to_string()))?;
        let mut names = Vec::new();
        for c in file.characters {
            names.push((c.name.to_lowercase(), c.id.clone()));
            names.extend(c.aliases.iter().map(|a| (a.to_lowercase(), c.id.clone())));
        }
        Ok(Self { names })
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

    const YAML: &str = "schema: kabar.cast/v1\nroles: [Penjual]\ncharacters:\n  - id: kak-tiara\n    name: Kak Tiara\n    home: Jakarta\n  - id: mbak-gadis\n    name: Mbak Gadis\n    aliases: [Gadis]\n";

    #[test]
    fn finds_names_aliases_and_ids() {
        let cast = Cast::parse(YAML).unwrap();
        assert_eq!(cast.id_of("Kak Tiara"), Some("kak-tiara"));
        assert_eq!(cast.id_of(" gadis "), Some("mbak-gadis"));
        assert_eq!(cast.id_of("mbak-gadis"), Some("mbak-gadis"));
        assert_eq!(cast.id_of("Penjual"), None);
    }
}
