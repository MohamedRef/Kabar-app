//! Reader logic (spec §11): structuring an OCR'd screenshot into learnable
//! text, and split-mode alignment between a text and its explanation.
//! Pure: callers bring the OCR result and the AI analysis.

pub mod align;
pub mod screenshot;
