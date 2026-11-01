//! Diagnostics: the one place the app reports something that went wrong
//! but doesn't stop it (a read that failed, a sync op the server refused).
//!
//! `report!("…", args)` writes the line where it can be read later:
//! logcat on Android (an app's stderr goes nowhere there), stderr on the Mac
//! and in development, and a ring of the last [`KEEP`] lines that Settings →
//! Your data shows and copies. A line that carries a sign-in token is
//! redacted before it goes anywhere. `eprintln!` is disallowed in this crate
//! (clippy.toml) so new code reports through here.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use dioxus::prelude::*;

use crate::Ui;
use crate::i18n::Msg;

/// Lines kept for Settings; older ones drop off.
pub const KEEP: usize = 200;

static LINES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// `report!("reading the review log failed: {e}")`: see the module docs.
macro_rules! report {
    ($($arg:tt)*) => {
        $crate::diag::emit(format_args!($($arg)*))
    };
}
pub(crate) use report;

pub fn emit(args: fmt::Arguments<'_>) {
    let line = scrub(args.to_string());
    write_out(&line);
    let stamped = format!("{} {line}", utc_time(now_ms()));
    let mut lines = LINES.lock().unwrap_or_else(|e| e.into_inner());
    if lines.len() == KEEP {
        lines.pop_front();
    }
    lines.push_back(stamped);
}

/// The kept lines, oldest first.
pub fn lines() -> Vec<String> {
    LINES.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
}

/// Reports a panic before the default hook runs, so a crash on the phone
/// leaves its message in logcat.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        emit(format_args!("panic: {info}"));
        default(info);
    }));
}

fn scrub(line: String) -> String {
    if crate::store::export::carries_token(&line) {
        "[a line that held a sign-in token was left out]".into()
    } else {
        line
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or_default()
}

/// `HH:MM:SS` in UTC: enough to line reports up with what happened.
fn utc_time(ms: u64) -> String {
    let s = ms / 1000 % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

#[cfg(target_os = "android")]
fn write_out(line: &str) {
    android::write(line);
}

#[cfg(not(target_os = "android"))]
#[allow(clippy::disallowed_macros)]
fn write_out(line: &str) {
    eprintln!("kabar: {line}");
}

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
mod android {
    use std::ffi::{CString, c_char, c_int};

    const ANDROID_LOG_WARN: c_int = 5;

    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }

    /// `adb logcat -s kabar` shows these.
    pub fn write(line: &str) {
        let text = CString::new(line.replace('\0', " ")).unwrap_or_default();
        // SAFETY: both pointers are NUL-terminated strings that outlive the call.
        unsafe {
            __android_log_write(ANDROID_LOG_WARN, c"kabar".as_ptr(), text.as_ptr());
        }
    }
}

/// Settings → Offline & storage: what went wrong lately, to copy into a message.
#[component]
pub fn DiagnosticsCard() -> Element {
    let ui = use_context::<Ui>();
    let mut shown = use_signal(Vec::<String>::new);
    let mut copied = use_signal(|| false);
    let refresh = move |_| {
        shown.set(lines());
        copied.set(false);
    };
    let copy = move |_| {
        let text = format!("Kabar {}\n{}", env!("CARGO_PKG_VERSION"), shown.read().join("\n"));
        let literal = serde_json::to_string(&text).unwrap_or_default();
        document::eval(&format!("navigator.clipboard.writeText({literal})"));
        copied.set(true);
    };
    use_hook(move || shown.set(lines()));
    let count = shown.read().len();

    rsx! {
        crate::ui::DivCard { class: "diagnostics-card",
            h2 { "{ui.t(Msg::DiagTitle)}" }
            if count == 0 {
                p { "{ui.t(Msg::DiagEmpty)}" }
            } else {
                details {
                    summary { {ui.t(Msg::DiagCount).replace("{n}", &ui.num(count as u64))} }
                    pre { class: "mono diag-lines", dir: "ltr",
                        for (i, line) in shown.read().iter().enumerate() {
                            bdi { key: "{i}", "{line}\n" }
                        }
                    }
                }
            }
            div { class: "row",
                button { onclick: refresh, "{ui.t(Msg::DiagRefresh)}" }
                if count > 0 {
                    button { onclick: copy,
                        if copied() { "{ui.t(Msg::DiagCopied)}" } else { "{ui.t(Msg::DiagCopy)}" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_last_lines_and_never_a_token() {
        for i in 0..KEEP + 5 {
            report!("line {i}");
        }
        report!("refresh failed for kr1.acc.dev.{}", "x".repeat(43));
        let kept = lines();
        assert_eq!(kept.len(), KEEP);
        assert!(kept.iter().all(|l| !l.contains("kr1.acc")));
        assert!(kept.last().unwrap().ends_with("[a line that held a sign-in token was left out]"));
        assert!(kept.iter().any(|l| l.ends_with(&format!("line {}", KEEP + 4))));
    }

    #[test]
    fn utc_time_of_day() {
        assert_eq!(utc_time(1_793_011_856_000), "10:50:56");
    }
}
