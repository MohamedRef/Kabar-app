//! Username + password sign-in, the device's half: the password is
//! stretched here with Argon2id and only the key goes to Batna Cloud
//! (`kabar_protocol::cloud::PasswordSignIn`, docs/CLOUD.md § Username and password).

use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kabar_protocol::cloud::{
    MIN_PASSWORD_CHARS, PASSWORD_PARAMS, normalize_username, password_salt, valid_username,
};

/// What's wrong with a username or password before anything is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    Username,
    TooShort,
}

/// Checks a new username and password (setting them, not signing in).
pub fn check_new(username: &str, password: &str) -> Result<(), Invalid> {
    if !valid_username(&normalize_username(username)) {
        return Err(Invalid::Username);
    }
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(Invalid::TooShort);
    }
    Ok(())
}

/// The 32-byte Argon2id key for this username and password, base64url.
/// Takes about a quarter of a second on a phone; run it off the UI thread
/// where there is one.
pub fn password_key(username: &str, password: &str) -> Result<String, String> {
    let (m, t, p) = PASSWORD_PARAMS;
    let params = Params::new(m, t, p, Some(32)).map_err(|e| e.to_string())?;
    let mut key = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), &password_salt(username), &mut key)
        .map_err(|e| e.to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_username_and_password_give_the_same_key_on_every_device() {
        let a = password_key("Mo.Refaat", "correct horse battery").unwrap();
        // The username is matched however it was typed.
        assert_eq!(a, password_key(" mo.refaat ", "correct horse battery").unwrap());
        assert_eq!(URL_SAFE_NO_PAD.decode(&a).unwrap().len(), 32);
        assert_ne!(a, password_key("mo.refaat", "correct horse batterY").unwrap());
        assert_ne!(a, password_key("dr.mo", "correct horse battery").unwrap());
        // Pinned, so a change to the parameters or the salt can't slip by:
        // it would lock the learner out of every username already set.
        assert_eq!(a, PINNED);
    }

    /// From the reference implementation (argon2-cffi `hash_secret_raw`,
    /// Type.ID, the same parameters), so it is known to be real Argon2id.
    const PINNED: &str = "yTJrOc94B9qFT8rgrB8Za8khOn2ULrY7YSZqW6CHTPQ";

    #[test]
    fn new_logins_are_checked_before_anything_is_sent() {
        assert_eq!(check_new("mo", "long enough password"), Err(Invalid::Username));
        assert_eq!(check_new("محمد", "long enough password"), Err(Invalid::Username));
        assert_eq!(check_new("Mo.Refaat", "short"), Err(Invalid::TooShort));
        assert_eq!(check_new("Mo.Refaat", "١٢٣٤٥٦٧٨٩"), Err(Invalid::TooShort), "nine characters");
        assert_eq!(
            check_new("Mo.Refaat", "١٢٣٤٥٦٧٨٩٠"),
            Ok(()),
            "Arabic passwords count characters, not bytes"
        );
    }
}
