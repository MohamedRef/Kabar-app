//! The fingerprint of the lesson content this build ships, by lesson id
//! (`kabar_core::fingerprint`, audit F25). Worked out once per lesson per
//! run: content only changes with an update, which takes effect on the next
//! start.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// `None` when this build doesn't ship the lesson or can't read it: a check
/// on it then can't be compared and stands as it is.
pub fn current(id: &str) -> Option<String> {
    #[cfg(test)]
    if let Some(over) = OVERRIDE.with(|o| o.borrow().get(id).cloned()) {
        return over;
    }
    let mut cache = cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    cache
        .entry(id.to_owned())
        .or_insert_with(|| {
            let yaml = super::lesson_content(id)?;
            let lesson = kabar_core::lesson::parse_lesson(&yaml).ok()?;
            Some(kabar_core::fingerprint::lesson(&lesson))
        })
        .clone()
}

#[cfg(test)]
thread_local! {
    static OVERRIDE: std::cell::RefCell<HashMap<String, Option<String>>> = Default::default();
}

/// Test-only: pretend the shipped content of `id` has this fingerprint.
#[cfg(test)]
pub fn pretend(id: &str, fingerprint: Option<&str>) {
    OVERRIDE.with(|o| o.borrow_mut().insert(id.to_owned(), fingerprint.map(String::from)));
}
