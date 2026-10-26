//! Turning ink into text and guesses. On Android the on-device recognizer
//! (ML Kit Digital Ink) answers first; the `read_handwriting` and
//! `guess_sketch` vision tasks are the fallback and the Mac path. Ink is the
//! learner's own: requests are always personal (never sent to an engine
//! that trains on inputs).

use kabar_protocol::ai::Task;
use kabar_protocol::content::Lang;
use kabar_protocol::ink::{InkTranscript, TranscriptSource};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::group::StrokeGroup;
use crate::ai::{AiOutput, AiRequest};

/// The model's transcription (`prompts/read_handwriting@v1.md`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handwriting {
    /// Exactly what is written, misspellings included; empty for scribbles.
    pub text: String,
}

impl AiOutput for Handwriting {
    const TASK: Task = Task::ReadHandwriting;

    fn validate(&self) -> Result<(), String> {
        if self.text.chars().count() > 2000 {
            return Err("text is too long for one canvas".into());
        }
        Ok(())
    }
}

/// `read_handwriting` for a PNG already uploaded as `image_sha256`.
/// `expected` (dictation) helps reading but must not be copied.
pub fn handwriting_request(lang: Lang, expected: Option<&str>, image_sha256: &str) -> AiRequest {
    AiRequest {
        input: json!({ "lang": lang.code(), "expected": expected }),
        personal: true,
        image_sha256: Some(image_sha256.into()),
    }
}

/// `guess_sketch` for a Gambar! canvas. The target is not sent: the model
/// must judge the drawing, not echo the word.
pub fn sketch_request(image_sha256: &str) -> AiRequest {
    AiRequest { input: json!({ "game": "gambar" }), personal: true, image_sha256: Some(image_sha256.into()) }
}

/// A transcript for a stroke group. Text is trimmed; empty text is still
/// stored (so the group isn't sent again) but never matches a search.
pub fn transcript(
    doc: &str,
    group: &StrokeGroup,
    text: &str,
    lang: Lang,
    source: TranscriptSource,
    confidence: Option<f32>,
) -> InkTranscript {
    InkTranscript {
        doc: doc.into(),
        group: group.id.clone(),
        strokes: group.strokes.clone(),
        text: text.trim().to_string(),
        lang,
        source,
        confidence,
        bbox: group.bbox.to_array(),
    }
}

/// Whether a new reading may replace the stored one: the learner's typing
/// always wins, and an on-device or AI reading never overwrites it.
pub fn should_replace(
    old: Option<&InkTranscript>,
    new_source: TranscriptSource,
    strokes_changed: bool,
) -> bool {
    match old {
        None => true,
        Some(o) if o.source == TranscriptSource::Typed => new_source == TranscriptSource::Typed,
        Some(o) => strokes_changed || new_source >= o.source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ink::Rect;

    #[test]
    fn requests_are_personal_and_hide_the_target() {
        let r = handwriting_request(Lang::Id, Some("Saya sedang membaca buku."), "ab12");
        assert!(r.personal);
        assert_eq!(r.image_sha256.as_deref(), Some("ab12"));
        assert_eq!(r.input["expected"], "Saya sedang membaca buku.");
        let s = sketch_request("cd34");
        assert!(s.personal && s.input.get("target_word").is_none());
    }

    #[test]
    fn typed_corrections_win() {
        let g = StrokeGroup {
            id: "s1".into(),
            strokes: vec!["s1".into(), "s2".into()],
            bbox: Rect { x: 1.0, y: 2.0, w: 3.0, h: 4.0 },
            last_ms: 0,
        };
        let ai = transcript("d", &g, " Saya lagi baca ", Lang::Id, TranscriptSource::Ai, None);
        assert_eq!(
            (ai.text.as_str(), ai.row_id().as_str(), ai.bbox),
            ("Saya lagi baca", "d:s1", [1.0, 2.0, 3.0, 4.0])
        );
        let typed = InkTranscript { source: TranscriptSource::Typed, ..ai.clone() };
        assert!(should_replace(None, TranscriptSource::Ai, false));
        assert!(should_replace(Some(&ai), TranscriptSource::MlKit, false));
        assert!(!should_replace(
            Some(&InkTranscript { source: TranscriptSource::MlKit, ..ai.clone() }),
            TranscriptSource::Ai,
            false
        ));
        assert!(should_replace(
            Some(&InkTranscript { source: TranscriptSource::MlKit, ..ai }),
            TranscriptSource::Ai,
            true
        ));
        assert!(!should_replace(Some(&typed), TranscriptSource::MlKit, true));
        assert!(should_replace(Some(&typed), TranscriptSource::Typed, false));
        assert!(Handwriting { text: String::new() }.validate().is_ok());
    }
}
