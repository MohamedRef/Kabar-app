//! Reads that failed (engineering audit F10). A store that can't be read is
//! never shown silently as "no data" or default settings: every such read
//! goes through [`read_or`], which reports it, and [`ReadNotice`] says so
//! above every screen with a button that reads the screen again.

use dioxus::core::Runtime;
use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;

/// The last read that failed, until a retry.
static READ_ERROR: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Bumped by Retry; the shell keys the screen on it, so it reads again.
pub static READ_RETRY: GlobalSignal<u32> = Signal::global(|| 0);

/// `r`'s value, or `T::default()` after reporting that `what` couldn't be
/// read (to the log, and on screen when called from the UI).
pub fn read_or<T: Default, E: std::fmt::Display>(what: &str, r: Result<T, E>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => {
            read_failed(what, &e);
            T::default()
        }
    }
}

/// Reports a read that failed.
pub fn read_failed(what: &str, e: &dyn std::fmt::Display) {
    crate::diag::report!("reading {what} failed: {e}");
    if Runtime::try_current().is_some() {
        let message = format!("{what}: {e}");
        if READ_ERROR.peek().as_deref() != Some(message.as_str()) {
            *READ_ERROR.write() = Some(message);
        }
    }
}

fn text(lang: UiLanguage) -> (&'static str, &'static str) {
    match lang {
        UiLanguage::ArEg => {
            ("ما عرفتش أقرأ بياناتك من الجهاز، فاللي قدامك ممكن يكون ناقص. ما ضاعش حاجة.", "اقرأ تاني")
        }
        UiLanguage::Ar => {
            ("تعذّرت قراءة بياناتك من الجهاز، فقد يكون المعروض ناقصًا. لم يُفقد شيء.", "أعد القراءة")
        }
        UiLanguage::En => (
            "Some of your data couldn't be read from this device, so what you see may be incomplete. Nothing was lost.",
            "Read again",
        ),
    }
}

/// Says a read failed, above every screen, with a way to read again.
#[component]
pub fn ReadNotice() -> Element {
    let Some(error) = READ_ERROR.read().clone() else {
        return rsx! {};
    };
    let ui = use_context::<crate::Ui>();
    let (message, retry) = text(ui.settings.read().ui_language);
    rsx! {
        div { class: "save-notice read-notice", role: "alert", aria_atomic: "true",
            p {
                "{message} "
                bdi { class: "small", dir: "ltr", "{error}" }
            }
            button {
                class: "secondary read-retry",
                onclick: move |_| {
                    *READ_ERROR.write() = None;
                    *READ_RETRY.write() += 1;
                },
                "{retry}"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dom::App;

    fn screen() -> Element {
        use_hook(|| read_or::<Vec<u8>, _>("the review log", Err("disk I/O error")));
        rsx! {
            ReadNotice {}
            p { class: "t-retry", "{READ_RETRY}" }
        }
    }

    #[test]
    fn a_failed_read_is_said_and_retry_reads_again() {
        let mut app = App::new(UiLanguage::En, screen);
        assert_eq!(app.by_class("read-notice").len(), 1);
        assert!(app.html().contains(text(UiLanguage::En).0) && app.html().contains("disk I/O error"));
        let retry = app.by_class("read-retry")[0];
        app.click(retry);
        assert!(app.by_class("read-notice").is_empty());
        assert!(app.html().contains(">1<"), "{}", app.html());
    }

    #[test]
    fn read_or_works_without_a_runtime() {
        assert_eq!(read_or::<u32, _>("x", Err("nope")), 0);
        assert_eq!(read_or::<u32, &str>("x", Ok(7)), 7);
    }
}
