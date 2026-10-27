//! Souvenir card art (docs/VISUAL-ASSETS.md, prompts 4–4d), linked to the
//! unit whose souvenir it shows. A unit without its own card shows its
//! level's landmark from the Journey Map, so every souvenir has a picture
//! of the place it was earned in.

use std::collections::HashMap;
use std::sync::LazyLock;

use base64::Engine as _;
use dioxus::prelude::*;

use crate::map_art;

macro_rules! card {
    ($name:literal) => {
        (
            include_bytes!(concat!("../../../assets/export/souvenirs/", $name, ".webp")).as_slice(),
            include_bytes!(concat!("../../../assets/export/souvenirs/", $name, "-thumb.webp")).as_slice(),
        )
    };
}

/// A card's 768x1152 WebP and its 256x384 thumbnail.
type CardFiles = (&'static [u8], &'static [u8]);

/// Unit id → (card file name, its files). The unit's
/// souvenir (`<unit>.souvenir` in its last lesson) is what the card shows.
const CARDS: [(&str, &str, CardFiles); 6] = [
    // Toraja tongkonan houses.
    ("bipa4.u03", "tongkonan", card!("tongkonan")),
    // Rumah Gadang, Minangkabau.
    ("bipa6.u03", "rumah-gadang", card!("rumah-gadang")),
    // Rendang.
    ("bipa6.u09", "rendang", card!("rendang")),
    // Kampung Batik Kauman, where batik is drawn with a canting.
    ("j3.u03", "batik", card!("batik")),
    // Batik Klewer, Java's great batik market: a classic kawung motif.
    ("j3.u05", "kawung", card!("kawung")),
    // Perak Kotagedhe.
    ("j6.u01", "kotagede-silver", card!("kotagede-silver")),
];

/// Card files in the repo that no unit's souvenir shows yet, with why.
/// The Journey thread keeps this list empty or explained.
#[cfg(test)]
const UNLINKED: [(&str, &str); 3] = [
    ("angklung", "no unit's souvenir is angklung (no West Java stop on either route)"),
    ("kopi-tubruk", "no unit's souvenir is coffee"),
    ("sate", "no unit's souvenir is sate"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    /// The whole card: the lesson's souvenir moment.
    Card,
    /// The Suitcase grid.
    Thumb,
}

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/webp;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

static URLS: LazyLock<HashMap<(&'static str, Size), String>> = LazyLock::new(|| {
    CARDS
        .iter()
        .flat_map(|&(unit, _, (card, thumb))| {
            [((unit, Size::Card), data_url(card)), ((unit, Size::Thumb), data_url(thumb))]
        })
        .collect()
});

/// The unit's own souvenir card, as a `data:` URL.
pub fn card(unit_id: &str, size: Size) -> Option<&'static str> {
    let unit = CARDS.iter().find(|c| c.0 == unit_id)?.0;
    URLS.get(&(unit, size)).map(String::as_str)
}

/// The unit of a lesson souvenir id (`bipa1.u01.souvenir` → `bipa1.u01`).
pub fn unit_of(souvenir_id: &str) -> &str {
    souvenir_id.strip_suffix(".souvenir").unwrap_or(souvenir_id)
}

/// The picture on a souvenir: the unit's card (its picture panel; the app
/// writes the words), or else the landmark of the unit's level.
#[component]
pub fn SouvenirArt(unit_id: String, size: Size) -> Element {
    if let Some(src) = card(&unit_id, size) {
        return rsx! {
            div { class: "souvenir-art card-art", aria_hidden: "true",
                img { src: "{src}", alt: "", draggable: "false" }
            }
        };
    }
    let level = unit_id.split('.').next().unwrap_or_default();
    let vsize = if size == Size::Card { map_art::Size::Large } else { map_art::Size::Small };
    let Some(src) = map_art::vignette(level, vsize) else { return rsx! {} };
    rsx! {
        div { class: "souvenir-art place-art", aria_hidden: "true",
            img { src: "{src}", alt: "", draggable: "false" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_ids() -> Vec<String> {
        ["id", "jv"]
            .into_iter()
            .flat_map(|l| crate::journey::curriculum(l).unwrap().levels.iter())
            .flat_map(|level| level.units.iter().map(|u| u.id.clone()))
            .collect()
    }

    #[test]
    fn every_card_belongs_to_a_real_unit_with_a_souvenir() {
        let units = unit_ids();
        for (unit, name, _) in CARDS {
            assert!(units.iter().any(|u| u == unit), "{name}: no unit {unit}");
            let last = crate::journey::curriculum(if unit.starts_with('j') { "jv" } else { "id" })
                .unwrap()
                .levels
                .iter()
                .flat_map(|l| l.units.iter())
                .find(|u| u.id == unit)
                .and_then(|u| u.lessons.last())
                .unwrap();
            let lesson =
                kabar_core::lesson::parse_lesson(crate::lesson::lesson_content(&last.id).unwrap()).unwrap();
            let s = lesson.souvenir.unwrap_or_else(|| panic!("{unit} has no souvenir"));
            assert_eq!(unit_of(&s.id), unit);
        }
    }

    #[test]
    fn every_card_in_the_repo_is_linked_or_explained() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/export/souvenirs");
        for entry in std::fs::read_dir(dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            let Some(stem) = name.strip_suffix(".webp") else { continue };
            if stem.ends_with("-thumb") {
                continue;
            }
            let linked = CARDS.iter().any(|c| c.1 == stem);
            let explained = UNLINKED.iter().any(|u| u.0 == stem);
            assert!(linked ^ explained, "{stem}: link it to a unit in CARDS or explain it in UNLINKED");
        }
    }

    /// `assets/souvenirs.md`: every unit's souvenir and its art, built from
    /// the curriculum, so art prompts are written from it and nothing is
    /// drawn for a souvenir that doesn't exist. Rewrite it with
    /// `KABAR_UPDATE_MANIFEST=1 cargo test -p kabar-app souvenir_manifest`.
    fn manifest() -> String {
        let mut out = String::from(
            "# Souvenir art manifest\n\n\
             Generated from `curriculum/*.yaml` and the lessons by `crates/app/src/souvenir_art.rs`; \
             do not edit by hand. Every unit earns one souvenir. **art** is its card in \
             `assets/src/souvenirs/`, or *pending*: until the card exists the app shows the level's \
             landmark from `assets/src/map/`. Card files must be named in `CARDS` there; files no \
             unit uses are listed at the end.\n",
        );
        let mut linked = 0;
        let mut total = 0;
        for lang in ["id", "jv"] {
            for level in &crate::journey::curriculum(lang).unwrap().levels {
                out.push_str(&format!(
                    "\n## {} · {}\n\n| unit | stop | souvenir | art |\n|---|---|---|---|\n",
                    level.code,
                    level.place()
                ));
                for unit in &level.units {
                    let title = unit
                        .lessons
                        .iter()
                        .rev()
                        .find_map(|l| {
                            kabar_core::lesson::parse_lesson(crate::lesson::lesson_content(&l.id)?)
                                .ok()?
                                .souvenir
                        })
                        .map_or_else(|| "—".to_string(), |s| s.title);
                    let english = unit.culture.as_ref().map_or("", |c| c.souvenir.as_str());
                    let art = CARDS.iter().find(|c| c.0 == unit.id).map_or("*pending*".to_string(), |c| {
                        linked += 1;
                        format!("`{}`", c.1)
                    });
                    total += 1;
                    out.push_str(&format!(
                        "| {} | {} | {} ({}) | {} |\n",
                        unit.id, unit.stop, title, english, art
                    ));
                }
            }
        }
        out.push_str(&format!(
            "\n**{linked} of {total} units have their own card.**\n\n## Card files no unit uses\n\n"
        ));
        for (name, why) in UNLINKED {
            out.push_str(&format!("- `{name}`: {why}\n"));
        }
        out
    }

    #[test]
    fn souvenir_manifest_is_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/souvenirs.md");
        let want = manifest();
        if std::env::var_os("KABAR_UPDATE_MANIFEST").is_some() {
            std::fs::write(path, &want).unwrap();
        }
        let have = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            have == want,
            "assets/souvenirs.md is stale: KABAR_UPDATE_MANIFEST=1 cargo test -p kabar-app souvenir_manifest"
        );
    }

    #[test]
    fn every_unit_has_a_picture() {
        for unit in unit_ids() {
            let level = unit.split('.').next().unwrap();
            assert!(
                card(&unit, Size::Thumb).is_some()
                    || map_art::vignette(level, map_art::Size::Small).is_some(),
                "{unit} has no picture"
            );
        }
        assert_eq!(unit_of("bipa1.u01.souvenir"), "bipa1.u01");
        assert!(card("j6.u01", Size::Card).unwrap().starts_with("data:image/webp;base64,"));
    }
}
