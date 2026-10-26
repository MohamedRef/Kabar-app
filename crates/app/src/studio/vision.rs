//! Getting ink to the vision tasks (`read_handwriting`, `guess_sketch`).
//!
//! The canvas hands over a PNG; the image goes to Kabar Cloud as a blob
//! (`PUT /v1/blobs/{sha256}`, ≤ 1 MB, kept 7 days) and the AI request names
//! it by hash ([`CloudLink::put_blob`]). Signed out, offline or in a build
//! without Kabar Cloud, [`upload`] says why and every screen offers its
//! typed or self-check fallback instead. Upload right before each call: the
//! server keeps images a week.

use base64::Engine as _;
use kabar_core::ai::FriendlyReason;

use crate::cloud::link::{CloudLink, Unreachable};

/// Kabar Cloud's limit for one blob.
pub const MAX_BLOB_BYTES: usize = 1024 * 1024;

/// The bytes of a `data:image/png;base64,…` URL.
pub fn png_bytes(data_url: &str) -> Option<Vec<u8>> {
    let b64 = data_url.strip_prefix("data:image/png;base64,")?;
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

/// Uploads a canvas snapshot and returns its hash for `AiRequest::image_sha256`.
pub async fn upload(link: &CloudLink, data_url: &str) -> Result<String, FriendlyReason> {
    let bytes = png_bytes(data_url).ok_or(FriendlyReason::TryAgain)?;
    if bytes.len() > MAX_BLOB_BYTES {
        return Err(FriendlyReason::TryAgain);
    }
    link.put_blob(bytes).await.map_err(|e| friendly(&e))
}

fn friendly(e: &Unreachable) -> FriendlyReason {
    match e {
        Unreachable::NotConfigured | Unreachable::SignedOut => FriendlyReason::NotAvailable,
        Unreachable::Offline => FriendlyReason::Unreachable,
        Unreachable::QuotaExhausted => FriendlyReason::ResetsLater,
        Unreachable::Server(_) => FriendlyReason::TryAgain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_urls_decode() {
        // A 1×1 PNG.
        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let bytes = png_bytes(url).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        assert!(png_bytes("data:image/jpeg;base64,AAAA").is_none());
    }
}
