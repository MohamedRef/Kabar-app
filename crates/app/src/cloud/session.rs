//! The signed-in session. The refresh token lives in the macOS Keychain or
//! the Android Keystore (through `keyring-core`), never in the database.

use kabar_protocol::cloud::Session;

use super::api::{Api, CloudError};

/// Where the session is kept between launches.
pub trait SessionVault: Send {
    fn load(&self) -> Option<Session>;
    fn save(&self, session: &Session);
    fn clear(&self);
}

/// The platform credential store.
pub struct KeyringVault {
    entry: Option<keyring_core::Entry>,
}

const SERVICE: &str = "app.kabar.cloud";
const USER: &str = "session";

impl KeyringVault {
    pub fn new() -> Self {
        init_default_store();
        let entry = keyring_core::Entry::new(SERVICE, USER)
            .map_err(|e| crate::diag::report!("no credential store: {e}"))
            .ok();
        Self { entry }
    }
}

impl SessionVault for KeyringVault {
    fn load(&self) -> Option<Session> {
        let json = self.entry.as_ref()?.get_password().ok()?;
        serde_json::from_str(&json).ok()
    }

    fn save(&self, session: &Session) {
        let Some(entry) = &self.entry else { return };
        let json = serde_json::to_string(session).unwrap_or_default();
        if let Err(e) = entry.set_password(&json) {
            crate::diag::report!("saving the session failed: {e}");
        }
    }

    fn clear(&self) {
        if let Some(entry) = &self.entry {
            let _ = entry.delete_credential();
        }
    }
}

fn init_default_store() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    type Store = keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>>;
    ONCE.call_once(|| {
        #[cfg(target_os = "macos")]
        let store: Store = apple_native_keyring_store::keychain::Store::new().map(|s| s as _);
        #[cfg(target_os = "android")]
        let store: Store = android_native_keyring_store::Store::new().map(|s| s as _);
        // Linux is only for development and CI: an in-memory store, so a
        // session never lands in a plain file.
        #[cfg(not(any(target_os = "macos", target_os = "android")))]
        let store: Store = keyring_core::mock::Store::new().map(|s| s as _);
        match store {
            Ok(store) => keyring_core::set_default_store(store),
            Err(e) => crate::diag::report!("credential store unavailable: {e}"),
        }
    });
}

/// In memory only (tests, and the end-to-end check).
#[derive(Default)]
pub struct MemoryVault(std::sync::Mutex<Option<Session>>);

impl SessionVault for MemoryVault {
    fn load(&self) -> Option<Session> {
        self.0.lock().ok()?.clone()
    }

    fn save(&self, session: &Session) {
        if let Ok(mut s) = self.0.lock() {
            *s = Some(session.clone());
        }
    }

    fn clear(&self) {
        if let Ok(mut s) = self.0.lock() {
            *s = None;
        }
    }
}

/// Refresh this long before the access token expires.
const REFRESH_MARGIN_MS: u64 = 60_000;

pub struct Auth {
    session: Session,
    vault: Box<dyn SessionVault>,
}

impl Auth {
    pub fn new(session: Session, vault: Box<dyn SessionVault>) -> Self {
        vault.save(&session);
        Self { session, vault }
    }

    pub fn restore(vault: Box<dyn SessionVault>, now_ms: u64) -> Result<Self, Box<dyn SessionVault>> {
        match vault.load() {
            Some(session) if session.refresh_expires_at_ms > now_ms => Ok(Self { session, vault }),
            _ => Err(vault),
        }
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// A valid access token, refreshing (and rotating the refresh token) when
    /// it is about to expire. Blocking.
    pub fn access_token(&mut self, api: &Api, now_ms: u64, force: bool) -> Result<String, CloudError> {
        if force || now_ms + REFRESH_MARGIN_MS >= self.session.access_expires_at_ms {
            let fresh = api.refresh(&self.session.refresh_token)?;
            self.vault.save(&fresh);
            self.session = fresh;
        }
        Ok(self.session.access_token.clone())
    }

    /// Forgets the session on this device. Returns the vault for the next sign-in.
    pub fn forget(self) -> Box<dyn SessionVault> {
        self.vault.clear();
        self.vault
    }
}
