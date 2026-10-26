//! Getting ink to the vision tasks (`read_handwriting`, `guess_sketch`).
//!
//! The canvas hands over a PNG; the image goes to Kabar Cloud as a blob
//! (`PUT /v1/blobs/{sha256}`, ≤ 1 MB, kept 7 days) and the AI request names
//! it by hash. The app-side upload (`CloudLink::put_blob`) is the Cloud
//! thread's next step; until it lands, [`upload`] reports `NotAvailable` and
//! every screen offers its typed or self-check fallback instead.

use base64::Engine as _;
use kabar_core::ai::FriendlyReason;
use sha2::{Digest, Sha256};

/// Kabar Cloud's limit for one blob.
pub const MAX_BLOB_BYTES: usize = 1024 * 1024;

/// The bytes of a `data:image/png;base64,…` URL.
pub fn png_bytes(data_url: &str) -> Option<Vec<u8>> {
    let b64 = data_url.strip_prefix("data:image/png;base64,")?;
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Uploads a canvas snapshot and returns its hash for `AiRequest::image_sha256`.
pub async fn upload(data_url: &str) -> Result<String, FriendlyReason> {
    let bytes = png_bytes(data_url).ok_or(FriendlyReason::TryAgain)?;
    if bytes.len() > MAX_BLOB_BYTES {
        return Err(FriendlyReason::TryAgain);
    }
    let _sha = sha256_hex(&bytes);
    // No blob upload in the app yet (see the module docs).
    Err(FriendlyReason::NotAvailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_urls_decode_and_hash() {
        // A 1×1 PNG.
        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let bytes = png_bytes(url).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert!(png_bytes("data:image/jpeg;base64,AAAA").is_none());
    }
}
