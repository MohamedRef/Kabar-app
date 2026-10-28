//! Character portraits (docs/VISUAL-ASSETS.md, prompt 6). Each file is named
//! by the character's cast id (`content/cast.yaml`), plus an expression for
//! the people you talk to, and build.rs embeds every export, so a new batch
//! needs no code change. Speaker labels ("Kak Tiara") find their cast id
//! through the registry; anyone without a portrait wears a monogram.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use base64::Engine as _;
use dioxus::prelude::*;
use kabar_core::cast::Cast;

use crate::embedded::{CAST, text};

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
    Cast::parse(&text(CAST)).unwrap_or_else(|e| {
        crate::diag::report!("reading content/cast.yaml failed: {e}");
        Cast::default()
    })
});

/// Forms of address that come before a name; they keep their own meaning.
const HONORIFICS: &[&str] = &[
    "bu", "ibu", "pak", "bapak", "mas", "mbak", "kak", "dik", "om", "tante", "mbah", "bude", "pakde",
    "budhe", "pakdhe", "bulik", "paklik", "lik", "eyang", "cak", "ning",
];

static NAME_WORDS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    REGISTRY.name_words().map(str::to_lowercase).filter(|w| !HONORIFICS.contains(&w.as_str())).collect()
});

/// The word is part of a cast member's name ("Gadis", "Murni"), not an
/// honorific: written with a capital, it's the character, not "girl".
pub fn is_cast_name(word: &str) -> bool {
    NAME_WORDS.contains(&word.to_lowercase())
}

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

/// The characters of `program` named in `text` who have a face:
/// `(registry name, face)`.
pub fn mentioned(program: &str, text: &str) -> Vec<(&'static str, &'static str)> {
    REGISTRY
        .mentioned(program, text)
        .into_iter()
        .filter_map(|id| Some((REGISTRY.name_of(id)?, url(id, Mood::Neutral)?)))
        .collect()
}

/// A decorative face; the name next to it carries the meaning.
#[component]
pub fn Face(src: &'static str, class: &'static str) -> Element {
    rsx! {
        img { class: "face {class}", src: "{src}", alt: "", aria_hidden: "true", draggable: "false" }
    }
}

/// Tints for monograms, from tokens already contrast-checked against
/// surfaces in every theme (crates/core/src/theme.rs).
const MONOGRAM_TINTS: &[&str] = &["sawah", "wc-noun", "gold", "wc-adj", "wp-prefix", "wp-suffix", "indigo"];

/// A small sign for speakers who are a role, not a person (`roles:` in
/// content/cast.yaml): the warung seller, the doctor, the driver.
fn role_glyph(speaker: &str) -> Option<&'static str> {
    Some(match speaker.trim().to_lowercase().as_str() {
        "penjual" | "bakul" => "🛒",
        "pelayan" => "🍽",
        "dokter" => "🩺",
        "sopir" => "🚗",
        "guru" => "📚",
        "petugas" => "🪪",
        "resepsionis" => "🛎",
        "pembawa acara" | "penyiar" => "🎙",
        "narrator" => "📖",
        _ => return None,
    })
}

/// What a speaker without a portrait wears: a role glyph, else the initial
/// of their name past any honorific ("Bu Yani" → "Y"), in a tint picked by
/// the name, so the same person looks the same in every lesson (#312).
pub fn monogram(speaker: &str) -> (String, &'static str) {
    let tint = {
        // FNV-1a: stable across builds and platforms, unlike the std hasher.
        let h = speaker
            .trim()
            .to_lowercase()
            .bytes()
            .fold(0x811c_9dc5_u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193));
        MONOGRAM_TINTS[h as usize % MONOGRAM_TINTS.len()]
    };
    if let Some(glyph) = role_glyph(speaker) {
        return (glyph.to_string(), tint);
    }
    let words: Vec<&str> = speaker.split_whitespace().collect();
    let name = words.iter().find(|w| !HONORIFICS.contains(&w.to_lowercase().as_str())).or(words.first());
    let initial = name.and_then(|w| w.chars().next()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    (initial, tint)
}

/// A speaker's portrait, or their monogram until Codex draws them: every
/// line of dialogue has a face. Decorative; the name next to it speaks.
#[component]
pub fn SpeakerFace(speaker: String, class: &'static str) -> Element {
    if let Some(src) = of(&speaker, Mood::Neutral) {
        return rsx! { Face { src, class } };
    }
    let (mark, tint) = monogram(&speaker);
    rsx! {
        span {
            class: "face monogram {class}",
            style: "--mono: var(--{tint})",
            aria_hidden: "true",
            "{mark}"
        }
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

    #[test]
    fn faceless_speakers_get_a_monogram() {
        assert_eq!(monogram("Bu Yani").0, "Y");
        assert_eq!(monogram("Pak Hasan").0, "H");
        assert_eq!(monogram("Pakdhe Darmo").0, "D");
        assert_eq!(monogram("Mbah").0, "M", "an honorific alone keeps its own initial");
        assert_eq!(monogram("ilham").0, "I");
        assert_eq!(monogram("Penjual").0, "🛒");
        assert_eq!(monogram("Pembawa acara").0, "🎙");
        assert_eq!(monogram("").0, "");
        // The same person, the same tint, wherever they speak.
        assert_eq!(monogram("Bu Retno").1, monogram("bu retno ").1);
        let tints: HashSet<_> = ["Pak Hasan", "Bu Yani", "Kak Selvi", "Pak Sastro", "Bu Retno", "Pak Elias"]
            .into_iter()
            .map(|s| monogram(s).1)
            .collect();
        assert!(tints.len() >= 3, "names spread over the tints: {tints:?}");
    }
}
