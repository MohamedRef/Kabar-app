//! The signed-in session. The refresh token lives in the macOS Keychain or
//! the Android Keystore (through `keyring-core`), never in the database. The
//! web build keeps it in the browser's `localStorage` for this site only
//! ([`WebVault`]): the closest thing a browser has to a credential store.

use kabar_protocol::cloud::Session;

use super::api::{Api, CloudError};

/// Where the session is kept between launches.
pub trait SessionVault: Send {
    fn load(&self) -> Option<Session>;
    /// Whether the refresh token was persisted for the next launch.
    fn save(&self, session: &Session) -> bool;
    fn clear(&self);
}

/// The platform credential store.
#[cfg(not(target_arch = "wasm32"))]
pub struct KeyringVault {
    entry: Option<keyring_core::Entry>,
}

#[cfg(not(target_arch = "wasm32"))]
const SERVICE: &str = "app.kabar.cloud";
#[cfg(not(target_arch = "wasm32"))]
const USER: &str = "session";

#[cfg(not(target_arch = "wasm32"))]
impl KeyringVault {
    pub fn new() -> Self {
        init_default_store();
        let entry = keyring_core::Entry::new(SERVICE, USER)
            .map_err(|e| crate::diag::report!("no credential store: {e}"))
            .ok();
        Self { entry }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SessionVault for KeyringVault {
    fn load(&self) -> Option<Session> {
        let json = self.entry.as_ref()?.get_password().ok()?;
        serde_json::from_str(&json).ok()
    }

    fn save(&self, session: &Session) -> bool {
        let Some(entry) = &self.entry else { return false };
        let Ok(json) = serde_json::to_string(session) else { return false };
        entry.set_password(&json).map_err(|e| crate::diag::report!("saving the session failed: {e}")).is_ok()
    }

    fn clear(&self) {
        if let Some(entry) = &self.entry {
            let _ = entry.delete_credential();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

/// The web build's vault, readable only by this site's pages:
/// `localStorage` (kept across visits) or, on a computer that isn't the
/// learner's, `sessionStorage` ([`WebVault::tab_only`]: gone when the tab
/// closes). Signing out clears both.
#[cfg(target_arch = "wasm32")]
pub struct WebVault {
    tab_only: bool,
}

#[cfg(target_arch = "wasm32")]
impl WebVault {
    const KEY: &str = "kabar:session";

    /// Kept for the next visit.
    pub fn kept() -> Self {
        Self { tab_only: false }
    }

    /// Only for this tab.
    #[allow(dead_code)] // the "not my computer" sign-in uses it
    pub fn tab_only() -> Self {
        Self { tab_only: true }
    }

    fn storage(tab_only: bool) -> Option<web_sys::Storage> {
        let window = web_sys::window()?;
        if tab_only { window.session_storage() } else { window.local_storage() }.ok().flatten()
    }
}

#[cfg(target_arch = "wasm32")]
impl SessionVault for WebVault {
    fn load(&self) -> Option<Session> {
        [true, false].into_iter().find_map(|tab_only| {
            let json = Self::storage(tab_only)?.get_item(Self::KEY).ok().flatten()?;
            serde_json::from_str(&json).ok()
        })
    }

    fn save(&self, session: &Session) -> bool {
        let Some(storage) = Self::storage(self.tab_only) else { return false };
        let Ok(json) = serde_json::to_string(session) else { return false };
        if let Some(other) = Self::storage(!self.tab_only) {
            let _ = other.remove_item(Self::KEY);
        }
        storage
            .set_item(Self::KEY, &json)
            .map_err(|e| crate::diag::report!("saving the session failed: {e:?}"))
            .is_ok()
    }

    fn clear(&self) {
        for tab_only in [true, false] {
            if let Some(storage) = Self::storage(tab_only) {
                let _ = storage.remove_item(Self::KEY);
            }
        }
    }
}

/// In memory only (tests, and the end-to-end check).
#[derive(Default)]
pub struct MemoryVault(std::sync::Mutex<Option<Session>>);

impl SessionVault for MemoryVault {
    fn load(&self) -> Option<Session> {
        self.0.lock().ok()?.clone()
    }

    fn save(&self, session: &Session) -> bool {
        self.0.lock().map(|mut s| *s = Some(session.clone())).is_ok()
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
    pub fn new(session: Session, vault: Box<dyn SessionVault>) -> Result<Self, Box<dyn SessionVault>> {
        if !vault.save(&session) {
            return Err(vault);
        }
        Ok(Self { session, vault })
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
    /// it is about to expire.
    pub async fn access_token(&mut self, api: &Api, now_ms: u64, force: bool) -> Result<String, CloudError> {
        if force || now_ms + REFRESH_MARGIN_MS >= self.session.access_expires_at_ms {
            let fresh = api.refresh(&self.session.refresh_token).await?;
            self.session = if self.vault.save(&fresh) {
                fresh
            } else {
                // The hub still accepts the previous refresh token until the
                // replacement is presented; preserve it for the next launch.
                Session {
                    refresh_token: std::mem::take(&mut self.session.refresh_token),
                    refresh_expires_at_ms: self.session.refresh_expires_at_ms,
                    ..fresh
                }
            };
        }
        Ok(self.session.access_token.clone())
    }

    /// Forgets the session on this device. Returns the vault for the next sign-in.
    pub fn forget(self) -> Box<dyn SessionVault> {
        self.vault.clear();
        self.vault
    }
}
