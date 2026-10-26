//! Kabar's own tokens.
//!
//! - **Access token**: a JWT signed with Ed25519 (`alg: EdDSA`), 15 minutes.
//!   Verifying is cheap enough for the Worker's 10 ms CPU budget, and Kabar
//!   Home can check it with only the public key.
//! - **Refresh token**: `kr1.<account>.<device>.<secret>`, 60 days, rotated on
//!   every use. Only a SHA-256 of the secret is stored, in the account's
//!   Durable Object.

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use kabar_protocol::DeviceId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ACCESS_TTL_MS: u64 = 15 * 60 * 1000;
pub const REFRESH_TTL_MS: u64 = 60 * 24 * 60 * 60 * 1000;
const ISSUER: &str = "kabar-cloud";

pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// Account id for a Google account: stable, and doesn't reveal the `sub`.
pub fn account_id_for_google_sub(sub: &str) -> String {
    format!("g{}", &sha256_hex(format!("google:{sub}").as_bytes())[..24])
}

/// Device ids end up in tokens, URLs and SQL: keep them boring.
pub fn valid_device_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn valid_account_id(id: &str) -> bool {
    id.len() == 25 && id.starts_with('g') && id[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// The Ed25519 signing key (Worker secret `TOKEN_SIGNING_KEY`: base64 of a 32-byte seed).
#[derive(Clone)]
pub struct TokenKey(SigningKey);

impl fmt::Debug for TokenKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TokenKey(kid={})", self.kid())
    }
}

impl TokenKey {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(SigningKey::from_bytes(&seed))
    }

    pub fn from_base64(s: &str) -> Result<Self, String> {
        let bytes = STANDARD
            .decode(s.trim())
            .or_else(|_| URL_SAFE_NO_PAD.decode(s.trim()))
            .map_err(|e| format!("not base64: {e}"))?;
        let seed: [u8; 32] = bytes.try_into().map_err(|_| "must be 32 bytes".to_owned())?;
        Ok(Self::from_seed(seed))
    }

    pub fn public(&self) -> VerifyingKey {
        self.0.verifying_key()
    }

    /// Short id of the public key, so a future key rotation can tell keys apart.
    pub fn kid(&self) -> String {
        sha256_hex(self.public().as_bytes())[..16].to_owned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessClaims {
    pub iss: String,
    /// Account id.
    pub sub: String,
    pub dev: DeviceId,
    /// Seconds since the epoch, as JWT expects.
    pub iat: u64,
    pub exp: u64,
}

#[derive(Serialize, Deserialize)]
struct Header {
    alg: String,
    typ: String,
    kid: String,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TokenError {
    #[error("malformed token")]
    Malformed,
    #[error("bad signature")]
    BadSignature,
    #[error("expired")]
    Expired,
}

/// Returns the token and its expiry in milliseconds.
pub fn issue_access(key: &TokenKey, account_id: &str, device: &DeviceId, now_ms: u64) -> (String, u64) {
    let header = Header { alg: "EdDSA".into(), typ: "JWT".into(), kid: key.kid() };
    let exp_ms = now_ms + ACCESS_TTL_MS;
    let claims = AccessClaims {
        iss: ISSUER.into(),
        sub: account_id.into(),
        dev: device.clone(),
        iat: now_ms / 1000,
        exp: exp_ms / 1000,
    };
    let enc = |v: &[u8]| URL_SAFE_NO_PAD.encode(v);
    // Both structs always serialize.
    let signing_input = format!(
        "{}.{}",
        enc(&serde_json::to_vec(&header).unwrap_or_default()),
        enc(&serde_json::to_vec(&claims).unwrap_or_default())
    );
    let sig = key.0.sign(signing_input.as_bytes());
    (format!("{signing_input}.{}", enc(&sig.to_bytes())), exp_ms)
}

pub fn verify_access(key: &VerifyingKey, token: &str, now_ms: u64) -> Result<AccessClaims, TokenError> {
    let mut parts = token.split('.');
    let (Some(h), Some(c), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(TokenError::Malformed);
    };
    let sig_bytes: [u8; 64] =
        URL_SAFE_NO_PAD.decode(s).ok().and_then(|b| b.try_into().ok()).ok_or(TokenError::Malformed)?;
    let header: Header = URL_SAFE_NO_PAD
        .decode(h)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or(TokenError::Malformed)?;
    if header.alg != "EdDSA" {
        return Err(TokenError::Malformed);
    }
    let signing_input = &token[..h.len() + 1 + c.len()];
    key.verify(signing_input.as_bytes(), &Signature::from_bytes(&sig_bytes))
        .map_err(|_| TokenError::BadSignature)?;
    let claims: AccessClaims = URL_SAFE_NO_PAD
        .decode(c)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or(TokenError::Malformed)?;
    if claims.iss != ISSUER || !valid_device_id(&claims.dev.0) {
        return Err(TokenError::Malformed);
    }
    if claims.exp * 1000 <= now_ms {
        return Err(TokenError::Expired);
    }
    Ok(claims)
}

/// A freshly minted refresh token and the hash the hub stores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRefresh {
    pub token: String,
    pub hash: String,
}

pub fn new_refresh(account_id: &str, device: &DeviceId, secret: [u8; 32]) -> NewRefresh {
    let encoded = URL_SAFE_NO_PAD.encode(secret);
    NewRefresh { token: format!("kr1.{account_id}.{device}.{encoded}"), hash: sha256_hex(encoded.as_bytes()) }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedRefresh {
    pub account_id: String,
    pub device: DeviceId,
    pub hash: String,
}

pub fn parse_refresh(token: &str) -> Option<ParsedRefresh> {
    let mut parts = token.trim().split('.');
    let (Some("kr1"), Some(account), Some(device), Some(secret), None) =
        (parts.next(), parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if !valid_account_id(account) || !valid_device_id(device) || secret.len() != 43 {
        return None;
    }
    Some(ParsedRefresh {
        account_id: account.into(),
        device: DeviceId(device.into()),
        hash: sha256_hex(secret.as_bytes()),
    })
}

/// 32 random bytes from the platform (`crypto.getRandomValues` in the Worker).
pub fn random_bytes() -> Result<[u8; 32], getrandom::Error> {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b)?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_793_011_856_000;

    fn key() -> TokenKey {
        TokenKey::from_seed([7; 32])
    }

    fn dev() -> DeviceId {
        DeviceId("s24u".into())
    }

    #[test]
    fn access_token_round_trip() {
        let acc = account_id_for_google_sub("1234567890");
        let (token, exp) = issue_access(&key(), &acc, &dev(), NOW);
        assert_eq!(exp, NOW + ACCESS_TTL_MS);
        let claims = verify_access(&key().public(), &token, NOW + 1000).unwrap();
        assert_eq!(claims.sub, acc);
        assert_eq!(claims.dev, dev());
    }

    #[test]
    fn access_token_expires() {
        let (token, _) = issue_access(&key(), "gabc", &dev(), NOW);
        assert_eq!(verify_access(&key().public(), &token, NOW + ACCESS_TTL_MS), Err(TokenError::Expired));
    }

    #[test]
    fn access_token_rejects_other_keys_and_tampering() {
        let (token, _) = issue_access(&key(), "gabc", &dev(), NOW);
        let other = TokenKey::from_seed([8; 32]);
        assert_eq!(verify_access(&other.public(), &token, NOW), Err(TokenError::BadSignature));

        // Swap the claims for another device's, keep the signature.
        let (forged, _) = issue_access(&other, "gabc", &DeviceId("tab".into()), NOW);
        let parts: Vec<&str> = token.split('.').collect();
        let forged_parts: Vec<&str> = forged.split('.').collect();
        let spliced = format!("{}.{}.{}", parts[0], forged_parts[1], parts[2]);
        assert_eq!(verify_access(&key().public(), &spliced, NOW), Err(TokenError::BadSignature));
        assert_eq!(verify_access(&key().public(), "a.b", NOW), Err(TokenError::Malformed));
    }

    #[test]
    fn refresh_token_round_trip() {
        let acc = account_id_for_google_sub("42");
        let new = new_refresh(&acc, &dev(), [9; 32]);
        let parsed = parse_refresh(&new.token).unwrap();
        assert_eq!(parsed.account_id, acc);
        assert_eq!(parsed.device, dev());
        assert_eq!(parsed.hash, new.hash);
        assert!(parse_refresh("kr1.gnothex.s24u.abc").is_none());
        assert!(parse_refresh(&new.token.replace("kr1", "kr2")).is_none());
    }

    #[test]
    fn device_ids_are_restricted() {
        assert!(valid_device_id("tab-s11_2"));
        assert!(!valid_device_id(""));
        assert!(!valid_device_id("a.b"));
        assert!(!valid_device_id(&"x".repeat(65)));
    }

    #[test]
    fn signing_key_parses_from_base64_and_hides_itself() {
        let k = TokenKey::from_base64(&STANDARD.encode([7u8; 32])).unwrap();
        assert_eq!(k.kid(), key().kid());
        assert!(TokenKey::from_base64("c2hvcnQ=").is_err());
        assert!(format!("{k:?}").starts_with("TokenKey(kid="));
    }
}
