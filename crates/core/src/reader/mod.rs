//! Reader logic (spec §11): structuring an OCR'd screenshot into learnable
//! text, split-mode alignment between a text and its explanation, and the
//! translation requests for the lines under the text.
//! Pure: callers bring the OCR result and the AI analysis.

pub mod align;
pub mod screen_ai;
pub mod screenshot;
pub mod translate;
