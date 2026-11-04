//! The signed-in session. The refresh token lives in the Android Keystore
//! (through `keyring-core`) and on the Mac in a file only the user's account
//! can read (`FileVault`); never in the database. The web build keeps it in
//! the browser's `localStorage` for this site only ([`WebVault`]): the
//! closest thing a browser has to a credential store.
//!
//! Why not the Mac Keychain: Kabar.app is self-signed, so it has no Apple
//! team ID, and the Keychain then trusts only the exact build that saved an
//! item. Every new build asked for the login password again, and since the
//! refresh token rotates every 15 minutes, the prompt kept coming back.

use kabar_protocol::cloud::Session;

use super::api::{Api, CloudError};

/// Where the session is kept between launches.
pub trait SessionVault: Send {
    fn load(&self) -> Option<Session>;
    /// Whether the refresh token was persisted for the next launch.
    fn save(&self, session: &Session) -> bool;
    fn clear(&self);
    /// False for a session kept only until the app or tab closes.
    fn remembers(&self) -> bool {
        true
    }
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

/// A file in Kabar's data folder, readable by this user only (0600). The
/// Mac's vault: see the module docs for why not the Keychain. The file
/// holds JSON `null` once signed out, so a missing file means "never used",
/// which is when a session the Keychain kept before is moved over, once.
#[cfg(any(target_os = "macos", test))]
pub struct FileVault {
    path: std::path::PathBuf,
    /// Where a session was kept before this vault existed.
    legacy: Option<Box<dyn SessionVault>>,
}

#[cfg(any(target_os = "macos", test))]
impl FileVault {
    /// The Mac's vault, next to the database.
    #[cfg(target_os = "macos")]
    pub fn mac(data_dir: &std::path::Path) -> Self {
        Self::new(data_dir.join(FILE_NAME), Some(Box::new(KeyringVault::new())))
    }

    pub fn new(path: std::path::PathBuf, legacy: Option<Box<dyn SessionVault>>) -> Self {
        Self { path, legacy }
    }

    fn write(&self, session: Option<&Session>) -> bool {
        let json = serde_json::to_vec(&session).unwrap_or_default();
        write_private(&self.path, &json)
            .map_err(|e| crate::diag::report!("saving the session failed: {e}"))
            .is_ok()
    }
}

#[cfg(any(target_os = "macos", test))]
const FILE_NAME: &str = "cloud-session.json";

#[cfg(any(target_os = "macos", test))]
impl SessionVault for FileVault {
    fn load(&self) -> Option<Session> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice::<Option<Session>>(&bytes).ok().flatten(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // First launch with this vault: move the Keychain's session
                // over (the Keychain may ask one last time), then never read
                // the Keychain again. Its copy goes stale at the next refresh.
                let session = self.legacy.as_ref().and_then(|v| v.load());
                self.write(session.as_ref());
                session
            }
            Err(e) => {
                crate::diag::report!("reading the session failed: {e}");
                None
            }
        }
    }

    fn save(&self, session: &Session) -> bool {
        self.write(Some(session))
    }

    fn clear(&self) {
        self.write(None);
    }
}

/// Replaces `path` with `bytes` in one step, readable by this user only.
#[cfg(any(target_os = "macos", test))]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)
}

/// The web build's vault, readable only by this site's pages:
/// `localStorage` (kept across visits) or, on a computer that isn't the
/// learner's, `sessionStorage` ([`WebVault::tab_only`]: gone when the tab
/// closes). Signing out clears both.
#[cfg(target_arch = "wasm32")]
pub struct WebVault {
    /// Set by a tab-only sign-in, and by loading one after a reload, so a
    /// refresh never moves it to `localStorage`.
    tab_only: std::sync::atomic::AtomicBool,
}

#[cfg(target_arch = "wasm32")]
impl WebVault {
    const KEY: &str = "kabar:session";

    /// Kept for the next visit.
    pub fn kept() -> Self {
        Self { tab_only: false.into() }
    }

    /// Only for this tab.
    pub fn tab_only() -> Self {
        Self { tab_only: true.into() }
    }

    fn is_tab_only(&self) -> bool {
        self.tab_only.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn storage(tab_only: bool) -> Option<web_sys::Storage> {
        let window = web_sys::window()?;
        if tab_only { window.session_storage() } else { window.local_storage() }.ok().flatten()
    }
}

#[cfg(target_arch = "wasm32")]
impl SessionVault for WebVault {
    fn load(&self) -> Option<Session> {
        let (tab_only, session) = [true, false].into_iter().find_map(|tab_only| {
            let json = Self::storage(tab_only)?.get_item(Self::KEY).ok().flatten()?;
            Some((tab_only, serde_json::from_str(&json).ok()?))
        })?;
        self.tab_only.store(tab_only, std::sync::atomic::Ordering::Relaxed);
        Some(session)
    }

    fn remembers(&self) -> bool {
        !self.is_tab_only()
    }

    fn save(&self, session: &Session) -> bool {
        let tab_only = self.is_tab_only();
        let Some(storage) = Self::storage(tab_only) else { return false };
        let Ok(json) = serde_json::to_string(session) else { return false };
        if let Some(other) = Self::storage(!tab_only) {
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
        // The next sign-in decides afresh.
        self.tab_only.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Where a sign-in on someone else's device keeps its session: this tab
/// in the browser, memory on the Mac and Android, so it ends when the app
/// or the tab closes.
pub fn not_remembered() -> Box<dyn SessionVault> {
    #[cfg(target_arch = "wasm32")]
    return Box::new(WebVault::tab_only());
    #[cfg(not(target_arch = "wasm32"))]
    Box::new(MemoryVault::forgetful())
}

/// In memory only: tests, the end-to-end check, and a sign-in on someone
/// else's Mac or phone (`forgetful`), which ends when the app quits.
#[derive(Default)]
pub struct MemoryVault(std::sync::Mutex<Option<Session>>, bool);

impl MemoryVault {
    pub fn forgetful() -> Self {
        Self(Default::default(), true)
    }
}

impl SessionVault for MemoryVault {
    fn remembers(&self) -> bool {
        !self.1
    }

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

    /// False when this session ends with the app or the browser tab.
    pub fn remembered(&self) -> bool {
        self.vault.remembers()
    }

    /// Forgets the session on this device. Returns the vault for the next sign-in.
    pub fn forget(self) -> Box<dyn SessionVault> {
        self.vault.clear();
        self.vault
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(refresh: &str) -> Session {
        Session {
            account_id: "acc".into(),
            device_id: kabar_protocol::hlc::DeviceId("dev".into()),
            email: None,
            access_token: "a".into(),
            access_expires_at_ms: 1,
            refresh_token: refresh.into(),
            refresh_expires_at_ms: 2,
        }
    }

    /// Counts reads of the old store, to prove it is read once at most.
    struct Legacy(Option<Session>, std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl SessionVault for Legacy {
        fn load(&self) -> Option<Session> {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.0.clone()
        }
        fn save(&self, _: &Session) -> bool {
            panic!("the old store is never written")
        }
        fn clear(&self) {
            panic!("the old store is never written")
        }
    }

    fn vault(
        dir: &std::path::Path,
        legacy: Option<Session>,
    ) -> (FileVault, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let v = FileVault::new(dir.join(FILE_NAME), Some(Box::new(Legacy(legacy, reads.clone()))));
        (v, reads)
    }

    #[test]
    fn file_vault_moves_the_keychain_session_over_once() {
        let dir = tempfile::tempdir().unwrap();
        let (v, reads) = vault(dir.path(), Some(session("old")));
        assert_eq!(v.load(), Some(session("old")));
        assert_eq!(v.load(), Some(session("old")));
        assert!(v.save(&session("new")));
        assert_eq!(v.load(), Some(session("new")));
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn file_vault_stays_signed_out_without_asking_the_keychain_again() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = vault(dir.path(), Some(session("old")));
        assert!(v.save(&session("s")));
        v.clear();
        assert_eq!(v.load(), None);
        // A fresh launch: the file says "signed out", so the Keychain isn't read.
        let (v, reads) = vault(dir.path(), Some(session("old")));
        assert_eq!(v.load(), None);
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn an_unreadable_session_file_means_signed_out() {
        let dir = tempfile::tempdir().unwrap();
        // The path is a folder: reading fails, and nothing is guessed.
        let v = FileVault::new(dir.path().to_path_buf(), None);
        assert_eq!(v.load(), None);
        assert!(!v.save(&session("s")), "a folder can't hold the session");
    }

    #[cfg(unix)]
    #[test]
    fn file_vault_is_readable_by_this_user_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = vault(dir.path(), None);
        assert!(v.save(&session("s")));
        let mode = std::fs::metadata(dir.path().join(FILE_NAME)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
