//! HTTP transport for offline packs: 20 s per-read stall timeouts and
//! resumable `<unit>.tar.part` Range downloads (matching `crate::update`).

#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::io::{Read as _, Write as _};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use kabar_core::packs::{is_unit_id, sha256_hex};
use kabar_protocol::pack::PackSummary;
#[cfg(not(target_arch = "wasm32"))]
use kabar_protocol::pack::pack_file;
#[cfg(not(target_arch = "wasm32"))]
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use super::{MAX_PACK_BYTES, write_atomic};
#[cfg(target_arch = "wasm32")]
use super::NOT_CONFIGURED;
use super::PackStore;

/// Largest `index.json` accepted.
#[cfg(not(target_arch = "wasm32"))]
const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;
/// How long a DNS lookup, TCP connect or single read may stall without progress.
#[cfg(not(target_arch = "wasm32"))]
const STALL: Duration = Duration::from_secs(20);
#[cfg(not(target_arch = "wasm32"))]
const INDEX_TIMEOUT: Duration = Duration::from_secs(30);
/// Slow mobile/Wi-Fi links (~30 KB/s) need ~3 minutes for a 5 MB BIPA 7 unit;
/// `STALL` still fails a dead socket within 20 s.
#[cfg(not(target_arch = "wasm32"))]
const PACK_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PartMeta {
    sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) enum TarFetch {
    Verified(Vec<u8>),
    /// Full HTTP body arrived, but its SHA-256 differed from `expected`
    /// (for example after `<unit>.tar` was re-uploaded under `--clobber`).
    HashMismatch(Vec<u8>),
}

#[cfg(not(target_arch = "wasm32"))]
fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(timeout))
        .timeout_resolve(Some(STALL))
        .timeout_connect(Some(STALL))
        .timeout_per_read(Some(STALL))
        .timeout_per_write(Some(STALL))
        .build()
        .into()
}

#[cfg(not(target_arch = "wasm32"))]
fn range_start<B>(res: &ureq::http::Response<B>) -> Option<u64> {
    let v = res.headers().get("content-range")?.to_str().ok()?;
    v.strip_prefix("bytes ")?.split('-').next()?.trim().parse().ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn content_length<B>(res: &ureq::http::Response<B>) -> Option<u64> {
    res.headers().get("content-length")?.to_str().ok()?.trim().parse().ok()
}

#[cfg(target_arch = "wasm32")]
pub(super) fn get_index(_url: &str) -> Result<Vec<u8>, String> {
    Err(NOT_CONFIGURED.into())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn get_index(url: &str) -> Result<Vec<u8>, String> {
    let mut res = agent(INDEX_TIMEOUT).get(url).call().map_err(|e| format!("network: {e}"))?;
    let status = res.status().as_u16();
    if status != 200 {
        return Err(format!("{url}: HTTP {status}"));
    }
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .take(MAX_INDEX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("network: {e}"))?;
    if bytes.len() as u64 > MAX_INDEX_BYTES {
        return Err("index: too large".into());
    }
    Ok(bytes)
}

#[cfg(target_arch = "wasm32")]
pub(super) fn fetch_tar(_store: &PackStore, _url: &str, _expected: &PackSummary) -> Result<TarFetch, String> {
    Err(NOT_CONFIGURED.into())
}

/// Streams a pack tar to `<data>/packs/downloads/<unit>.tar.part`, resuming
/// with an HTTP `Range` request when an earlier attempt for the same SHA-256
/// was cut mid-file.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn fetch_tar(store: &PackStore, url: &str, expected: &PackSummary) -> Result<TarFetch, String> {
    if !is_unit_id(&expected.unit) || expected.file != pack_file(&expected.unit) {
        return Err("bad unit id".into());
    }
    let Some(want_sha) = expected.file_sha256.clone() else {
        return Err("this unit's audio isn't published yet".into());
    };
    let (part, meta) = store.part_paths(&expected.unit);
    let want = PartMeta { sha256: want_sha.clone(), size: expected.file_size };

    for attempt in 0..2 {
        let on_disk = fs::metadata(&part).map_or(0, |m| m.len());
        let saved: Option<PartMeta> = fs::read(&meta).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let mut have = if attempt == 0
            && saved.as_ref() == Some(&want)
            && on_disk > 0
            && on_disk < MAX_PACK_BYTES
            && expected.file_size.is_none_or(|s| on_disk < s)
        {
            on_disk
        } else {
            0
        };
        let mut req = agent(PACK_TIMEOUT).get(url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let mut res = req.call().map_err(|e| format!("network: {e}"))?;
        let status = res.status().as_u16();
        match status {
            206 if have > 0 && range_start(&res) == Some(have) => {}
            200 => have = 0,
            416 => {
                let _ = fs::remove_file(&part);
                let _ = fs::remove_file(&meta);
                if have > 0 {
                    continue;
                }
                return Err(format!("{url}: HTTP 416"));
            }
            _ => return Err(format!("{url}: HTTP {status}")),
        }

        fs::create_dir_all(store.downloads_dir()).map_err(|e| e.to_string())?;
        let mut all = Vec::new();
        let mut file = if have > 0 {
            match fs::read(&part) {
                Ok(existing) if existing.len() as u64 == have => {
                    all = existing;
                    fs::OpenOptions::new().append(true).open(&part).map_err(|e| e.to_string())?
                }
                _ => {
                    let _ = fs::remove_file(&part);
                    continue;
                }
            }
        } else {
            let meta_bytes = serde_json::to_vec(&want).map_err(|e| e.to_string())?;
            write_atomic(&meta, &meta_bytes)?;
            fs::File::create(&part).map_err(|e| e.to_string())?
        };

        let expected_body = content_length(&res);
        let limit = MAX_PACK_BYTES.saturating_sub(have) + 1;
        let mut reader = res.body_mut().as_reader().take(limit);
        let mut buf = [0u8; 64 * 1024];
        let mut read_this = 0u64;
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => {
                    let _ = file.flush();
                    return Err(format!("network: {e}"));
                }
            };
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            all.extend_from_slice(&buf[..n]);
            read_this += n as u64;
            if all.len() as u64 > MAX_PACK_BYTES {
                drop(file);
                let _ = fs::remove_file(&part);
                let _ = fs::remove_file(&meta);
                return Err("pack too large".into());
            }
        }
        let _ = file.flush();
        drop(file);

        if expected_body.is_some_and(|len| read_this < len) {
            return Err("network: unexpected end of download".into());
        }
        let _ = fs::remove_file(&part);
        let _ = fs::remove_file(&meta);

        if sha256_hex(&all) == want_sha {
            return Ok(TarFetch::Verified(all));
        }
        if have > 0 {
            // The kept `.part` disagreed with the server's bytes: fetch fresh once.
            continue;
        }
        return Ok(TarFetch::HashMismatch(all));
    }
    Err("the download doesn't match the index (hash)".into())
}
