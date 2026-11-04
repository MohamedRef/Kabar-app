//! The web build (`web/`, wasm32, docs/WEB.md): what the browser stands in for.
//!
//! - **The database.** SQLite runs in wasm, in memory. Its bytes are kept in
//!   IndexedDB (`kabar` › `files` › `kabar.db`): loaded once before the app
//!   starts, and written back a moment after anything changes, and when the
//!   tab is hidden or closed.
//! - **One tab writes.** The newest tab owns the database; an older one stops
//!   saving and says so, so two tabs never overwrite each other's work.
//! - **Exports** are browser downloads.
// adapted from batna-app main: crates/app/src/platform/web.rs

use std::cell::{Cell, RefCell};
use std::time::Duration;

use dioxus::prelude::*;
use wasm_bindgen::prelude::*;

use crate::Db;
use crate::i18n::Msg;

#[wasm_bindgen(inline_js = r#"
const DB = "kabar", STORE = "files";
function open() {
  return new Promise((ok, fail) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => ok(req.result);
    req.onerror = () => fail(req.error);
  });
}
export async function idb_get(key) {
  const db = await open();
  return await new Promise((ok, fail) => {
    const req = db.transaction(STORE, "readonly").objectStore(STORE).get(key);
    req.onsuccess = () => ok(req.result instanceof Uint8Array ? req.result : null);
    req.onerror = () => fail(req.error);
  });
}
export async function idb_put(key, bytes) {
  // Copy first: `bytes` is a view of wasm memory, which can move while this
  // awaits, and IndexedDB would otherwise clone the whole memory.
  const copy = bytes.slice();
  const db = await open();
  await new Promise((ok, fail) => {
    const tx = db.transaction(STORE, "readwrite");
    tx.objectStore(STORE).put(copy, key);
    tx.oncomplete = () => ok();
    tx.onerror = () => fail(tx.error);
    tx.onabort = () => fail(tx.error);
  });
}
export async function idb_delete(key) {
  const db = await open();
  await new Promise((ok, fail) => {
    const tx = db.transaction(STORE, "readwrite");
    tx.objectStore(STORE).delete(key);
    tx.oncomplete = () => ok();
    tx.onerror = () => fail(tx.error);
  });
}
export function persist() {
  if (navigator.storage && navigator.storage.persist) navigator.storage.persist();
}
export function download(name, mime, bytes) {
  const url = URL.createObjectURL(new Blob([bytes], { type: mime }));
  const a = document.createElement("a");
  a.href = url; a.download = name; a.style.display = "none";
  document.body.appendChild(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60000);
}
export function ls_get(key) { try { return localStorage.getItem(key); } catch (_) { return null; } }
export function ls_set(key, value) {
  try { localStorage.setItem(key, value); return true; } catch (_) { return false; }
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn idb_get(key: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn idb_put(key: &str, bytes: &[u8]) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn idb_delete(key: &str) -> Result<JsValue, JsValue>;
    fn persist();
    #[wasm_bindgen(js_name = download)]
    fn js_download(name: &str, mime: &str, bytes: &[u8]);
    fn ls_get(key: &str) -> Option<String>;
    fn ls_set(key: &str, value: &str) -> bool;
}

const DB_KEY: &str = "kabar.db";
/// Which tab owns the database (`localStorage`).
const OWNER_KEY: &str = "kabar:tab";

/// Why this tab isn't saving, for the notice on top of every screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trouble {
    OtherTab,
    Failed,
}

thread_local! {
    static SNAPSHOT: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    static SAVING: Cell<bool> = const { Cell::new(true) };
    /// SQLite's change count at the last save.
    static SAVED_AT: Cell<Option<u64>> = const { Cell::new(None) };
    static TAB: RefCell<String> = const { RefCell::new(String::new()) };
    static TROUBLE: Cell<Option<Trouble>> = const { Cell::new(None) };
}

/// Before the app starts: reads the kept database and claims this tab.
/// A browser that can't read IndexedDB starts empty and doesn't save, so
/// it never overwrites what it couldn't read.
pub async fn boot() {
    persist();
    let tab = format!("{:x}", js_sys::Date::now() as u64 ^ (js_sys::Math::random() * 1e12) as u64);
    ls_set(OWNER_KEY, &tab);
    TAB.with(|t| *t.borrow_mut() = tab);
    match idb_get(DB_KEY).await {
        Ok(v) if v.is_null() || v.is_undefined() => {}
        Ok(v) => SNAPSHOT.with(|s| *s.borrow_mut() = Some(js_sys::Uint8Array::new(&v).to_vec())),
        Err(e) => {
            stop_saving(Trouble::Failed);
            crate::diag::report!("browser storage unreadable: {e:?}");
        }
    }
    crate::platform::files::boot().await;
}

/// An IndexedDB entry's bytes (`None`: missing or unreadable).
pub async fn idb_get_bytes(key: &str) -> Option<Vec<u8>> {
    match idb_get(key).await {
        Ok(v) if v.is_null() || v.is_undefined() => None,
        Ok(v) => Some(js_sys::Uint8Array::new(&v).to_vec()),
        Err(e) => {
            crate::diag::report!("reading {key} from browser storage failed: {e:?}");
            None
        }
    }
}

pub async fn idb_put_bytes(key: &str, bytes: &[u8]) -> Result<(), String> {
    idb_put(key, bytes).await.map(drop).map_err(|e| format!("{e:?}"))
}

pub async fn idb_delete_key(key: &str) {
    if let Err(e) = idb_delete(key).await {
        crate::diag::report!("deleting {key} from browser storage failed: {e:?}");
    }
}

/// The database bytes `boot` found (empty for a first visit). Once.
pub fn take_snapshot() -> Vec<u8> {
    SNAPSHOT.with(|s| s.borrow_mut().take()).unwrap_or_default()
}

/// This tab keeps working in memory but never writes the browser's copy.
pub fn stop_saving(why: Trouble) {
    SAVING.with(|s| s.set(false));
    TROUBLE.with(|t| t.set(Some(why)));
}

/// Whether this tab is the newest one, which alone saves and syncs.
pub fn owns_tab() -> bool {
    TAB.with(|t| ls_get(OWNER_KEY).is_none_or(|owner| owner == *t.borrow()))
}

/// Writes the database to IndexedDB when it changed since the last save.
async fn save_now(db: &Db) {
    if !SAVING.with(Cell::get) {
        return;
    }
    if !owns_tab() {
        stop_saving(Trouble::OtherTab);
        return;
    }
    // Copy the bytes out before awaiting: the store stays free meanwhile.
    let snapshot = {
        let Ok(mut store) = db.0.try_borrow_mut() else { return };
        if SAVED_AT.with(Cell::get) == Some(store.changes()) {
            return;
        }
        store.snapshot()
    };
    let failed = match snapshot {
        Ok((changes, bytes)) => match idb_put(DB_KEY, &bytes).await {
            Ok(_) => {
                SAVED_AT.with(|s| s.set(Some(changes)));
                None
            }
            Err(e) => Some(format!("{e:?}")),
        },
        Err(e) => Some(e.to_string()),
    };
    match failed {
        Some(e) => {
            crate::diag::report!("saving to browser storage failed: {e}");
            TROUBLE.with(|t| t.set(Some(Trouble::Failed)));
        }
        None => TROUBLE.with(|t| t.set(None)),
    }
}

/// Keeps the database saved: shortly after changes, and when the tab is
/// hidden or closed. Call once in `Loaded`; [`StorageNotice`] says what's wrong.
pub fn use_saver(db: Db) {
    let mut trouble = use_context_provider(|| WebTrouble(Signal::new(TROUBLE.with(Cell::get)))).0;
    let db2 = db.clone();
    use_future(move || {
        let db = db.clone();
        async move {
            loop {
                // The first visit saves at once, so the device id sticks.
                save_now(&db).await;
                let now = TROUBLE.with(Cell::get);
                if *trouble.peek() != now {
                    trouble.set(now);
                }
                crate::platform::time::sleep(Duration::from_millis(1500)).await;
            }
        }
    });
    use_future(move || {
        let db = db2.clone();
        async move {
            let mut eval = document::eval(
                r#"
                const send = () => dioxus.send(true);
                document.addEventListener("visibilitychange", () => { if (document.hidden) send(); });
                window.addEventListener("pagehide", send);
                await new Promise(() => {});
                "#,
            );
            while eval.recv::<bool>().await.is_ok() {
                save_now(&db).await;
            }
        }
    });
}

#[derive(Clone, Copy)]
struct WebTrouble(Signal<Option<Trouble>>);

/// The line on top of every screen while this tab can't save.
#[component]
pub fn StorageNotice() -> Element {
    let ui = use_context::<crate::Ui>();
    let Some(why) = *use_context::<WebTrouble>().0.read() else { return rsx! {} };
    let text = ui.t(match why {
        Trouble::OtherTab => Msg::WebOtherTab,
        Trouble::Failed => Msg::WebSaveFailed,
    });
    rsx! {
        div { class: "save-notice", role: "alert", aria_atomic: "true",
            p { "{text}" }
        }
    }
}

/// Hands `bytes` to the browser as a download called `name`.
pub fn download(name: &str, mime: &str, bytes: &[u8]) -> Result<String, String> {
    js_download(name, mime, bytes);
    Ok(name.to_string())
}

/// The Devices screen's name for this browser, e.g. "Chrome on Android".
pub fn browser_name() -> String {
    let agent = web_sys::window().and_then(|w| w.navigator().user_agent().ok()).unwrap_or_default();
    browser_from_agent(&agent)
}

fn browser_from_agent(agent: &str) -> String {
    let browser = if agent.contains("Edg/") {
        "Edge"
    } else if agent.contains("Firefox/") {
        "Firefox"
    } else if agent.contains("SamsungBrowser/") {
        "Samsung Internet"
    } else if agent.contains("Chrome/") {
        "Chrome"
    } else if agent.contains("Safari/") {
        "Safari"
    } else {
        "a browser"
    };
    let os = if agent.contains("Android") {
        "Android"
    } else if agent.contains("iPhone") || agent.contains("iPad") {
        "iOS"
    } else if agent.contains("Mac OS X") {
        "macOS"
    } else if agent.contains("Windows") {
        "Windows"
    } else if agent.contains("Linux") {
        "Linux"
    } else {
        return format!("Kabar web ({browser})");
    };
    format!("Kabar web ({browser} on {os})")
}
