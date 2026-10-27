//! Character portraits (docs/VISUAL-ASSETS.md, prompt 6). Each file is named
//! by the character's cast id (`content/cast.yaml`), plus an expression for
//! the people you talk to, and build.rs embeds every export, so a new batch
//! needs no code change. Speaker labels ("Kak Tiara") find their cast id
//! through the registry; anyone without a portrait keeps their initial.

use std::collections::HashMap;
use std::sync::LazyLock;

use base64::Engine as _;
use dioxus::prelude::*;
use kabar_core::cast::Cast;

use crate::embedded::{CAST, unpack};

// PORTRAITS: (cast id[-expression], 256x256 WebP).
include!(concat!(env!("OUT_DIR"), "/portraits.rs"));

/// The face a character makes. Only the coach and the chat partners have
/// expressions; everyone else always shows `Neutral`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mood {
    #[default]
    Neutral,
    /// Waiting for you to answer.
    Listening,
    /// Pleased: you got it right, or a lesson is done.
    Laughing,
    /// Working out a reply.
    Thinking,
    /// Correcting or teaching.
    Explaining,
}

impl Mood {
    fn suffix(self) -> &'static str {
        match self {
            Mood::Neutral => "",
            Mood::Listening => "-listening",
            Mood::Laughing => "-laughing",
            Mood::Thinking => "-thinking",
            Mood::Explaining => "-explaining",
        }
    }
}

static URLS: LazyLock<HashMap<&'static str, String>> = LazyLock::new(|| {
    PORTRAITS
        .iter()
        .map(|&(name, bytes)| {
            (
                name,
                format!("data:image/webp;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)),
            )
        })
        .collect()
});

static REGISTRY: LazyLock<Cast> = LazyLock::new(|| {
    Cast::parse(&unpack(CAST)).unwrap_or_else(|e| {
        eprintln!("kabar: reading content/cast.yaml failed: {e}");
        Cast::default()
    })
});

/// The cast id behind a speaker label or persona name, if the character is
/// in the registry.
pub fn cast_id(speaker: &str) -> Option<&'static str> {
    REGISTRY.id_of(speaker)
}

/// A character's face as a `data:` URL: the asked-for expression, else their
/// neutral portrait, else any portrait of theirs.
pub fn url(cast_id: &str, mood: Mood) -> Option<&'static str> {
    let get = |m: Mood| URLS.get(format!("{cast_id}{}", m.suffix()).as_str()).map(String::as_str);
    get(mood).or_else(|| get(Mood::Neutral)).or_else(|| {
        [Mood::Listening, Mood::Explaining, Mood::Laughing, Mood::Thinking].into_iter().find_map(get)
    })
}

/// A round portrait for a speaker label or cast id, or nothing when the
/// character has none yet (the caller keeps its own fallback).
pub fn of(speaker: &str, mood: Mood) -> Option<&'static str> {
    url(cast_id(speaker)?, mood)
}

/// A decorative face; the name next to it carries the meaning.
#[component]
pub fn Face(src: &'static str, class: &'static str) -> Element {
    rsx! {
        img { class: "face {class}", src: "{src}", alt: "", aria_hidden: "true", draggable: "false" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_portrait_belongs_to_the_cast() {
        for (name, bytes) in PORTRAITS {
            assert!(bytes.starts_with(b"RIFF"), "{name} is WebP");
            let id = ["-listening", "-laughing", "-thinking", "-explaining"]
                .iter()
                .find_map(|s| name.strip_suffix(s))
                .unwrap_or(name);
            assert_eq!(cast_id(id), Some(id), "{name}: not a cast id");
        }
    }

    #[test]
    fn speakers_find_their_faces() {
        assert_eq!(cast_id("Kak Tiara"), Some("kak-tiara"));
        assert_eq!(cast_id("Gadis"), Some("mbak-gadis"));
        assert_eq!(cast_id("Penjual"), None);
        // Every mood falls back to some face of hers once she has one.
        let faces: Vec<_> =
            [Mood::Neutral, Mood::Listening, Mood::Laughing, Mood::Thinking, Mood::Explaining]
                .into_iter()
                .filter_map(|m| of("Bu Siti", m))
                .collect();
        assert!(faces.len() == 5 || PORTRAITS.iter().all(|(n, _)| !n.starts_with("bu-siti")));
        assert!(of("Nobody", Mood::Neutral).is_none());
    }
}
