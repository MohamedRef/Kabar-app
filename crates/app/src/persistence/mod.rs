//! Observable preference save failures. Retry only idempotent preference
//! writes; never enqueue append-only study events. Capture owned desired
//! values and long-lived storage handles, not component-local signals.
mod queue;

use dioxus::core::Runtime;
use dioxus::prelude::*;
use queue::Queue;
use std::rc::Rc;

#[derive(Clone, Copy)]
struct Saves(Signal<Queue>);

pub fn use_provide_saves() {
    use_context_provider(|| Saves(Signal::new(Queue::default())));
}

fn current() -> Option<Saves> {
    Runtime::try_current()
        .and_then(|runtime| runtime.try_current_scope_id())
        .and_then(|_| try_consume_context::<Saves>())
}

/// Execute once, replacing any older failed choice for this same key.
pub fn save(key: impl Into<String>, write: impl Fn() -> Result<(), String> + 'static) -> bool {
    let result = write();
    record(key, result, write)
}

/// Record an already attempted atomic save with a narrow, per-key retry.
pub fn record(
    key: impl Into<String>,
    result: Result<(), String>,
    retry: impl Fn() -> Result<(), String> + 'static,
) -> bool {
    let key = key.into();
    if let Err(error) = &result {
        crate::diag::report!("saving {key} failed: {error}");
    }
    let succeeded = result.is_ok();
    if let Some(mut saves) = current()
        && (!succeeded || saves.0.peek().pending())
    {
        saves.0.write().record(key, succeeded, Rc::new(retry));
    }
    succeeded
}

/// Subscribe to failures so diagnostics and notices also refresh on recovery.
pub fn use_pending_saves() -> bool {
    current().is_some_and(|saves| saves.0.read().pending())
}

impl Saves {
    fn retry(mut self) -> bool {
        let attempts = self.0.peek().attempts();
        for attempt in attempts {
            if !self.0.peek().is_current(&attempt) {
                continue;
            }
            // IO and notifications must not run while the queue is borrowed.
            let result = (attempt.write)();
            if let Err(error) = &result {
                crate::diag::report!("retrying {} failed: {error}", attempt.key);
            }
            self.0.write().finish(&attempt, result.is_ok());
        }
        !self.0.peek().pending()
    }
}

#[component]
pub fn SaveNotice() -> Element {
    let saves = use_context::<Saves>();
    let pending = use_pending_saves();
    let ui = use_context::<crate::Ui>();
    let (message, retry) = match ui.lang() {
        kabar_core::locale::UiLanguage::ArEg => (
            "فيه إعدادات ما اتحفظتش. اختياراتك شغالة دلوقتي، بس ممكن تضيع لما تقفل التطبيق.",
            "حاول تحفظ تاني",
        ),
        kabar_core::locale::UiLanguage::Ar => {
            ("تعذّر حفظ بعض الإعدادات. اختياراتك مطبّقة الآن، وقد تُفقد عند إغلاق التطبيق.", "إعادة محاولة الحفظ")
        }
        kabar_core::locale::UiLanguage::En => (
            "Some preferences could not be saved. Your choices apply now but may be lost when you close the app.",
            "Retry saving",
        ),
    };
    if !pending {
        return rsx! {};
    }
    rsx! {
        div { class: "save-notice", role: "alert", aria_atomic: "true",
            p { "{message}" }
            button { class: "secondary save-retry",
                onclick: move |_| { saves.retry(); },
                "{retry}"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! { static OBSERVED: Cell<bool> = const { Cell::new(false) }; }

    fn app() -> Element {
        use_provide_saves();
        rsx! { Observer {} }
    }

    #[component]
    fn Observer() -> Element {
        OBSERVED.set(use_pending_saves());
        rsx! {}
    }

    #[test]
    fn helpers_work_without_a_dioxus_runtime() {
        assert!(save("ok", || Ok(())));
        assert!(!save("blocked", || Err("read only".into())));
    }

    #[test]
    fn notice_reacts_to_failure_and_success() {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        assert!(!OBSERVED.get());
        dom.in_scope(ScopeId::ROOT, || {
            save("theme", || Err("blocked".into()));
        });
        let _ = dom.render_immediate_to_vec();
        assert!(OBSERVED.get());
        dom.in_scope(ScopeId::ROOT, || {
            save("theme", || Ok(()));
        });
        let _ = dom.render_immediate_to_vec();
        assert!(!OBSERVED.get());
    }

    #[test]
    fn real_write_failure_retries_latest_choice_and_keeps_other_failures() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("prefs");
        std::fs::write(&folder, "blocked").unwrap();
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dom.in_scope(ScopeId::ROOT, || {
            for value in ["normal", "large"] {
                let path = folder.clone();
                save("size", move || {
                    std::fs::create_dir_all(&path)
                        .and_then(|()| std::fs::write(path.join("value"), value))
                        .map_err(|e| e.to_string())
                });
            }
            save("unrelated", || Ok(()));
            assert!(use_pending_saves());
            assert!(!current().unwrap().retry());
            std::fs::remove_file(&folder).unwrap();
            assert!(current().unwrap().retry());
            assert!(!use_pending_saves());
        });
        assert_eq!(std::fs::read_to_string(folder.join("value")).unwrap(), "large");
    }
}
