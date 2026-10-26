//! Reader wire types: word statuses (synced), what Android's share target
//! hands to Rust, and on-device OCR results (ML Kit, Apple Vision,
//! Tesseract) in one engine-neutral shape.

use serde::{Deserialize, Serialize};

/// A word's learning status. Synced LWW per lexeme (docs/PLAN.md §5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WordStatus {
    #[default]
    New,
    Learning,
    Known,
    Ignored,
}

impl WordStatus {
    pub const ALL: [WordStatus; 4] = [Self::New, Self::Learning, Self::Known, Self::Ignored];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Learning => "learning",
            Self::Known => "known",
            Self::Ignored => "ignored",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.as_str() == s)
    }
}

/// Something another app shared to Kabar (Android share target, drag &
/// drop, paste). Kotlin writes it as JSON; Rust reads it once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SharedInput {
    Text {
        text: String,
        /// Package or app name of the sender, when Android tells us.
        #[serde(default)]
        source: Option<String>,
    },
    Image {
        /// A copy in Kabar's private cache, readable by Rust.
        path: String,
        #[serde(default)]
        mime: Option<String>,
        #[serde(default)]
        source: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrEngine {
    MlKit,
    AppleVision,
    Tesseract,
}

/// Axis-aligned box in image pixels, origin top-left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.w / 2.0
    }

    pub fn center_y(&self) -> f32 {
        self.y + self.h / 2.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OcrLine {
    pub text: String,
    pub bbox: Rect,
    /// 0..1 when the engine reports it.
    #[serde(default)]
    pub confidence: Option<f32>,
}

/// A paragraph-like group of lines, as the engine grouped them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OcrBlock {
    pub bbox: Rect,
    pub lines: Vec<OcrLine>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OcrResult {
    pub engine: OcrEngine,
    /// Image size in pixels.
    pub width: f32,
    pub height: f32,
    pub blocks: Vec<OcrBlock>,
}

impl OcrResult {
    /// All recognized text, one line per OCR line, blocks separated by a blank line.
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(|b| b.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n"))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_input_json_from_kotlin() {
        let t: SharedInput =
            serde_json::from_str(r#"{"kind":"text","text":"arp dicul piro","source":"com.whatsapp"}"#)
                .unwrap();
        assert_eq!(
            t,
            SharedInput::Text { text: "arp dicul piro".into(), source: Some("com.whatsapp".into()) }
        );
        let i: SharedInput = serde_json::from_str(r#"{"kind":"image","path":"/c/s.png"}"#).unwrap();
        assert!(matches!(i, SharedInput::Image { mime: None, .. }));
    }

    #[test]
    fn status_round_trips() {
        for s in WordStatus::ALL {
            assert_eq!(WordStatus::parse(s.as_str()), Some(s));
        }
    }

    #[test]
    fn plain_text_keeps_block_breaks() {
        let line = |t: &str| OcrLine { text: t.into(), bbox: Rect::default(), confidence: None };
        let r = OcrResult {
            engine: OcrEngine::MlKit,
            width: 1080.0,
            height: 2340.0,
            blocks: vec![
                OcrBlock { bbox: Rect::default(), lines: vec![line("Sek tak lapor pusat")] },
                OcrBlock { bbox: Rect::default(), lines: vec![line("ng prabowo?"), line("12:41")] },
            ],
        };
        assert_eq!(r.plain_text(), "Sek tak lapor pusat\n\nng prabowo?\n12:41");
    }
}
