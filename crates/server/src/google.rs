//! Google ID token verification (RS256 against Google's published JWKS).
//! https://developers.google.com/identity/sign-in/web/backend-auth

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::signature::Verifier;
use rsa::{BigUint, RsaPublicKey};
use serde::Deserialize;
use sha2::Sha256;

pub const JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const ISSUERS: [&str; 2] = ["accounts.google.com", "https://accounts.google.com"];
/// Tolerated clock difference between Google and Cloudflare.
const LEEWAY_S: u64 = 60;

#[derive(Clone, Debug, Deserialize)]
pub struct Jwk {
    pub kid: String,
    pub kty: String,
    #[serde(default)]
    pub alg: Option<String>,
    pub n: String,
    pub e: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

#[derive(Debug, Deserialize)]
struct Claims {
    iss: String,
    aud: String,
    sub: String,
    exp: u64,
    #[serde(default)]
    iat: u64,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_verified: Option<EmailVerified>,
}

/// Google sends `true`, older tokens sent `"true"`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum EmailVerified {
    Bool(bool),
    Str(String),
}

impl EmailVerified {
    fn is_true(&self) -> bool {
        match self {
            EmailVerified::Bool(b) => *b,
            EmailVerified::Str(s) => s == "true",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoogleIdentity {
    pub sub: String,
    pub email: String,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum GoogleError {
    #[error("malformed ID token")]
    Malformed,
    #[error("unknown signing key (refresh JWKS)")]
    UnknownKey,
    #[error("bad signature")]
    BadSignature,
    #[error("wrong issuer")]
    WrongIssuer,
    #[error("issued for another app")]
    WrongAudience,
    #[error("expired")]
    Expired,
    #[error("email missing or not verified")]
    EmailNotVerified,
}

fn b64(s: &str) -> Result<Vec<u8>, GoogleError> {
    URL_SAFE_NO_PAD.decode(s).map_err(|_| GoogleError::Malformed)
}

/// Returns the `kid` the token was signed with, so the caller can refetch the
/// JWKS once when the key is new to it.
pub fn token_kid(id_token: &str) -> Result<String, GoogleError> {
    let header = id_token.split('.').next().ok_or(GoogleError::Malformed)?;
    let header: Header = serde_json::from_slice(&b64(header)?).map_err(|_| GoogleError::Malformed)?;
    Ok(header.kid)
}

pub fn verify_id_token(
    id_token: &str,
    jwks: &Jwks,
    client_ids: &[String],
    now_ms: u64,
) -> Result<GoogleIdentity, GoogleError> {
    let mut parts = id_token.split('.');
    let (Some(h), Some(c), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(GoogleError::Malformed);
    };
    let header: Header = serde_json::from_slice(&b64(h)?).map_err(|_| GoogleError::Malformed)?;
    if header.alg != "RS256" {
        return Err(GoogleError::Malformed);
    }
    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid == header.kid && k.kty == "RSA" && k.alg.as_deref().is_none_or(|a| a == "RS256"))
        .ok_or(GoogleError::UnknownKey)?;
    let public =
        RsaPublicKey::new(BigUint::from_bytes_be(&b64(&jwk.n)?), BigUint::from_bytes_be(&b64(&jwk.e)?))
            .map_err(|_| GoogleError::Malformed)?;
    let signature = Signature::try_from(b64(s)?.as_slice()).map_err(|_| GoogleError::Malformed)?;
    VerifyingKey::<Sha256>::new(public)
        .verify(&id_token.as_bytes()[..h.len() + 1 + c.len()], &signature)
        .map_err(|_| GoogleError::BadSignature)?;

    let claims: Claims = serde_json::from_slice(&b64(c)?).map_err(|_| GoogleError::Malformed)?;
    if !ISSUERS.contains(&claims.iss.as_str()) {
        return Err(GoogleError::WrongIssuer);
    }
    if !client_ids.contains(&claims.aud) {
        return Err(GoogleError::WrongAudience);
    }
    let now_s = now_ms / 1000;
    if claims.exp + LEEWAY_S <= now_s || claims.iat > now_s + LEEWAY_S {
        return Err(GoogleError::Expired);
    }
    match (claims.email, claims.email_verified) {
        (Some(email), Some(v)) if v.is_true() => Ok(GoogleIdentity { sub: claims.sub, email }),
        _ => Err(GoogleError::EmailNotVerified),
    }
}

/// How long to keep the JWKS, from Google's `Cache-Control: max-age`.
pub fn jwks_max_age_ms(cache_control: Option<&str>) -> u64 {
    const DEFAULT: u64 = 60 * 60 * 1000;
    cache_control
        .and_then(|cc| cc.split(',').find_map(|d| d.trim().strip_prefix("max-age=")?.parse::<u64>().ok()))
        .map_or(DEFAULT, |s| s * 1000)
}

#[cfg(test)]
pub(crate) mod testing {
    //! A fake Google: an RSA key that signs ID tokens, and its JWKS.
    use std::sync::OnceLock;

    use rsa::RsaPrivateKey;
    use rsa::pkcs1v15::SigningKey;
    use rsa::signature::{SignatureEncoding, Signer};
    use rsa::traits::PublicKeyParts;
    use serde_json::{Value, json};

    use super::*;

    pub const CLIENT_ID: &str = "kabar-android.apps.googleusercontent.com";
    pub const KID: &str = "test-kid";

    pub fn key() -> &'static RsaPrivateKey {
        static KEY: OnceLock<RsaPrivateKey> = OnceLock::new();
        KEY.get_or_init(|| RsaPrivateKey::new(&mut rand::thread_rng(), 2048).unwrap())
    }

    pub fn jwks() -> Jwks {
        let public = key().to_public_key();
        Jwks {
            keys: vec![Jwk {
                kid: KID.into(),
                kty: "RSA".into(),
                alg: Some("RS256".into()),
                n: URL_SAFE_NO_PAD.encode(public.n().to_bytes_be()),
                e: URL_SAFE_NO_PAD.encode(public.e().to_bytes_be()),
            }],
        }
    }

    pub fn claims(email: &str, now_ms: u64) -> Value {
        json!({
            "iss": "https://accounts.google.com",
            "aud": CLIENT_ID,
            "sub": "110169484474386276334",
            "email": email,
            "email_verified": true,
            "iat": now_ms / 1000,
            "exp": now_ms / 1000 + 3600,
        })
    }

    pub fn sign(claims: &Value) -> String {
        let enc = |v: &Value| URL_SAFE_NO_PAD.encode(serde_json::to_vec(v).unwrap());
        let input = format!("{}.{}", enc(&json!({ "alg": "RS256", "kid": KID, "typ": "JWT" })), enc(claims));
        let sig = SigningKey::<Sha256>::new(key().clone()).sign(input.as_bytes());
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::testing::*;
    use super::*;

    const NOW: u64 = 1_793_011_856_000;

    fn ids() -> Vec<String> {
        vec!["kabar-mac.apps.googleusercontent.com".into(), CLIENT_ID.into()]
    }

    #[test]
    fn accepts_a_valid_token() {
        let token = sign(&claims("learner@example.com", NOW));
        assert_eq!(token_kid(&token).unwrap(), KID);
        let id = verify_id_token(&token, &jwks(), &ids(), NOW).unwrap();
        assert_eq!(id.email, "learner@example.com");
        assert_eq!(id.sub, "110169484474386276334");
    }

    #[test]
    fn rejects_bad_tokens() {
        let mut c = claims("learner@example.com", NOW);
        c["aud"] = json!("someone-elses-app");
        assert_eq!(verify_id_token(&sign(&c), &jwks(), &ids(), NOW), Err(GoogleError::WrongAudience));

        let mut c = claims("learner@example.com", NOW);
        c["iss"] = json!("https://evil.example");
        assert_eq!(verify_id_token(&sign(&c), &jwks(), &ids(), NOW), Err(GoogleError::WrongIssuer));

        let c = claims("learner@example.com", NOW - 2 * 3600 * 1000);
        assert_eq!(verify_id_token(&sign(&c), &jwks(), &ids(), NOW), Err(GoogleError::Expired));

        let mut c = claims("learner@example.com", NOW);
        c["email_verified"] = json!(false);
        assert_eq!(verify_id_token(&sign(&c), &jwks(), &ids(), NOW), Err(GoogleError::EmailNotVerified));

        // Signed claims, then the email swapped.
        let token = sign(&claims("learner@example.com", NOW));
        let parts: Vec<&str> = token.split('.').collect();
        let other = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims("intruder@example.com", NOW)).unwrap());
        let forged = format!("{}.{}.{}", parts[0], other, parts[2]);
        assert_eq!(verify_id_token(&forged, &jwks(), &ids(), NOW), Err(GoogleError::BadSignature));

        let mut keys = jwks();
        keys.keys[0].kid = "rotated".into();
        assert_eq!(verify_id_token(&token, &keys, &ids(), NOW), Err(GoogleError::UnknownKey));
    }

    #[test]
    fn accepts_string_email_verified() {
        let mut c = claims("learner@example.com", NOW);
        c["email_verified"] = json!("true");
        assert!(verify_id_token(&sign(&c), &jwks(), &ids(), NOW).is_ok());
    }

    #[test]
    fn jwks_cache_age() {
        assert_eq!(jwks_max_age_ms(Some("public, max-age=19204, must-revalidate")), 19_204_000);
        assert_eq!(jwks_max_age_ms(None), 3_600_000);
    }
}
