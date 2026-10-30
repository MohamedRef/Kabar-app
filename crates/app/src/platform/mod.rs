//! Platform glue: where data lives, what device we're on, what other apps
//! shared with us, and on-device OCR.

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "macos")]
mod macos;
// Pure helpers; each native bridge uses a subset, other platforms only in tests.
#[allow(dead_code)]
mod ocr;

use std::path::{Path, PathBuf};

use kabar_protocol::reader::{OcrResult, SharedInput};

/// Directory for the local database.
pub fn data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        android::files_dir().map(PathBuf::from)
    }
    #[cfg(not(target_os = "android"))]
    {
        directories::ProjectDirs::from("app", "Kabar", "Kabar")
            .map(|d| d.data_dir().to_path_buf())
            .ok_or_else(|| "no home directory".to_string())
    }
}

/// Android only: a file in the app's `no_backup` folder, which Auto Backup
/// and device-to-device transfers never copy (`store/install.rs`).
pub fn install_marker() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        let files = PathBuf::from(android::files_dir().ok()?);
        Some(files.parent()?.join("no_backup").join("kabar-install"))
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Where Settings → Export writes its file: `~/Downloads` on the Mac (and
/// other desktops); on Android the app's external files directory
/// (`Android/data/app.kabar/files`), from where [`save_document`] copies it
/// to a place the learner picks (the Files app can't open `Android/data`).
pub fn export_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        android::external_files_dir().or_else(|_| android::files_dir()).map(PathBuf::from)
    }
    #[cfg(not(target_os = "android"))]
    {
        match directories::UserDirs::new() {
            Some(u) => Ok(u.download_dir().map_or_else(|| u.home_dir().to_path_buf(), Path::to_path_buf)),
            None => data_dir(),
        }
    }
}

/// Why native speech didn't play.
#[allow(dead_code)] // each platform builds only some variants
#[derive(Clone, Debug, PartialEq)]
pub enum SpeakError {
    /// Not Android: speak through the WebView's `speechSynthesis` instead.
    UseWebView,
    /// No text-to-speech engine answered (none installed, or disabled).
    NoEngine,
    /// The engine knows Indonesian but its voice data isn't downloaded.
    MissingVoice,
    /// The engine has no Indonesian voice at all.
    Unsupported,
    /// A JNI or engine error; logged, not shown.
    Failed(String),
}

/// What the native engine last reported, so the UI can say why it's silent.
#[allow(dead_code)] // each platform builds only some variants
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TtsStatus {
    /// Not Android: the WebView path handles speech.
    WebView,
    Starting,
    Ready,
    MissingVoice,
    Unsupported,
    NoEngine,
}

/// Android: on mobile data or with Data Saver on (unknown counts as metered).
/// Automatic downloads (app updates, audio packs) wait while this is true.
pub fn metered_network() -> bool {
    #[cfg(target_os = "android")]
    {
        android::network_metered().unwrap_or(true)
    }
    #[cfg(not(target_os = "android"))]
    {
        false
    }
}

/// A light tap of vibration for lesson feedback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Haptic {
    Correct,
    /// A wrong answer: barely there, never a scolding buzz.
    Missed,
    LessonEnd,
}

/// The device can vibrate (Android with a motor; most Galaxy tablets have
/// none). Settings shows the vibration switch only then.
pub fn can_vibrate() -> bool {
    #[cfg(target_os = "android")]
    {
        static HAS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *HAS.get_or_init(|| android::has_vibrator().unwrap_or(false))
    }
    #[cfg(not(target_os = "android"))]
    {
        false
    }
}

/// Whether the device is in dark mode, where the platform can tell
/// (Android: the system uiMode, which the WebView may misreport); `None`
/// elsewhere, where CSS `prefers-color-scheme` is right.
pub fn device_dark() -> Option<bool> {
    #[cfg(target_os = "android")]
    {
        static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        match android::is_night() {
            Ok(dark) => Some(dark),
            Err(e) => {
                if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    crate::diag::report!("night mode check failed: {e}");
                }
                None
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Paints Android's status and navigation bars in the theme's background,
/// with icons that read on it; `day` and `night` are `#rrggbb` (the same for
/// a fixed theme). Does nothing elsewhere: the Mac keeps its own title bar.
pub fn system_bars(day: &str, night: &str) {
    #[cfg(target_os = "android")]
    {
        let argb = |hex: &str| {
            let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
            (0xff00_0000 | rgb) as i32
        };
        if let Err(e) = android::system_bars(argb(day), argb(night)) {
            crate::diag::report!("system bars failed: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (day, night);
    }
}

/// Android: keeps a lost WebView renderer from crashing Kabar
/// (`app.kabar.WebViewGuard`). Android reclaims a backgrounded app's
/// renderer under memory pressure; unhandled, that kills Kabar as a crash
/// ("Kabar is crashing frequently"). Also reports how recent runs ended to
/// the diagnostics. Call once at start; nothing elsewhere.
pub fn guard_webview() {
    #[cfg(target_os = "android")]
    match android::webview_guard() {
        Ok(lines) => {
            for line in lines.unwrap_or_default().lines().filter(|l| !l.is_empty()) {
                crate::diag::report!("{line}");
            }
        }
        Err(e) => crate::diag::report!("WebView guard failed: {e}"),
    }
}

/// Vibrates once for `kind` where the device can; does nothing elsewhere.
pub fn vibrate(kind: Haptic) {
    #[cfg(target_os = "android")]
    {
        if !can_vibrate() {
            return;
        }
        // Haptics.CORRECT, MISSED, LESSON_END
        let code = match kind {
            Haptic::Correct => 0,
            Haptic::Missed => 1,
            Haptic::LessonEnd => 2,
        };
        if let Err(e) = android::vibrate(code) {
            crate::diag::report!("vibration failed: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = kind;
    }
}

/// Speech rate for normal and slow replays (matches the WebView path).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn speech_rate(slow: bool) -> f32 {
    if slow { 0.7 } else { 0.95 }
}

/// Speaks `text` with the device's own voice for `lang_tag` ("id-ID",
/// "jv-ID"; Javanese falls back to Indonesian). On Android this is the
/// system TextToSpeech engine; elsewhere it returns
/// [`SpeakError::UseWebView`] and the caller uses `speechSynthesis`.
pub fn speak(text: &str, lang_tag: &str, slow: bool) -> Result<(), SpeakError> {
    #[cfg(target_os = "android")]
    {
        android::speak(text, lang_tag, speech_rate(slow))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (text, lang_tag, slow);
        Err(SpeakError::UseWebView)
    }
}

/// Whether native speech is the path on this platform.
pub const NATIVE_TTS: bool = cfg!(target_os = "android");

/// What native speech last reported ([`TtsStatus::WebView`] off Android).
pub fn tts_status() -> TtsStatus {
    #[cfg(target_os = "android")]
    {
        android::tts_status()
    }
    #[cfg(not(target_os = "android"))]
    {
        TtsStatus::WebView
    }
}

/// Human-readable device model, e.g. "samsung SM-S928B" or "Mac14,2".
pub fn device_model() -> Option<String> {
    #[cfg(target_os = "android")]
    {
        android::device_model().ok()
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("sysctl").args(["-n", "hw.model"]).output().ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
    }
    #[cfg(not(any(target_os = "android", target_os = "macos")))]
    {
        None
    }
}

/// A share/intent that launched or resumed Kabar, taken once (None if nothing
/// pending or unsupported platform).
///
/// On Android, `ShareActivity` stores the item and brings Kabar forward; call
/// this on start-up and whenever the app becomes visible again. It is cheap
/// (one small file check) and returns each item only once.
pub fn take_shared_input() -> Option<SharedInput> {
    #[cfg(target_os = "android")]
    {
        match android::take_shared_input() {
            Ok(input) => input,
            Err(e) => {
                crate::diag::report!("reading the shared item failed: {e}");
                None
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Where the Android photo picker started by [`pick_image`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub enum PickState {
    /// Still showing.
    Open,
    /// Closed: the picked image (if any) waits in [`take_shared_input`].
    Done,
    /// The image couldn't be opened or copied.
    Failed,
}

/// Whether the Reader opens screenshots through [`pick_image`] instead of a
/// file input. Dioxus answers a file input with a native dialog that only
/// exists on desktop, so on Android it never returns a file.
pub const NATIVE_IMAGE_PICKER: bool = cfg!(target_os = "android");

/// Shows the system photo picker (Android). The choice arrives like a share:
/// poll [`pick_state`] and [`take_shared_input`].
pub fn pick_image() -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        android::pick_image()
    }
    #[cfg(not(target_os = "android"))]
    {
        Err("no native image picker on this platform".to_string())
    }
}

/// See [`pick_image`].
pub fn pick_state() -> PickState {
    #[cfg(target_os = "android")]
    {
        android::pick_state().unwrap_or_else(|e| {
            crate::diag::report!("image picker state: {e}");
            PickState::Failed
        })
    }
    #[cfg(not(target_os = "android"))]
    {
        PickState::Done
    }
}

/// Where the "Save to" dialog of [`save_document`] is.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // each platform builds only some variants
pub enum SaveState {
    Open,
    /// Copied; the name the learner gave it, when the provider says.
    Saved(Option<String>),
    Cancelled,
    Failed,
}

/// Whether a written file is handed to the system "Save to" dialog after
/// writing (Android: app files are out of the Files app's reach).
pub const SAVE_DIALOG: bool = cfg!(target_os = "android");

/// Shows the system "Save to" dialog for `path` (Android); poll [`save_state`].
pub fn save_document(path: &Path, name: &str) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        android::save_document(path.to_str().ok_or("path is not valid UTF-8")?, name)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (path, name);
        Err("no save dialog on this platform".to_string())
    }
}

/// See [`save_document`].
pub fn save_state() -> SaveState {
    #[cfg(target_os = "android")]
    {
        android::save_state().unwrap_or_else(|e| {
            crate::diag::report!("save dialog state: {e}");
            SaveState::Failed
        })
    }
    #[cfg(not(target_os = "android"))]
    {
        SaveState::Cancelled
    }
}

/// Name for the Devices screen, e.g. "Galaxy S24 Ultra" or "Mohamed's MacBook Air".
pub fn device_name() -> Option<String> {
    #[cfg(target_os = "android")]
    {
        android::device_name().ok().flatten().or_else(|| android::device_model().ok())
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("scutil").args(["--get", "ComputerName"]).output().ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
    }
    #[cfg(not(any(target_os = "android", target_os = "macos")))]
    {
        None
    }
}

/// Blocking on-device OCR of an image file. Called from a background thread by the UI.
///
/// Boxes are in pixels of the upright image, origin top-left. The error string
/// is meant for logs; show the user your own friendly text.
pub fn ocr_image(path: &Path) -> Result<OcrResult, String> {
    #[cfg(target_os = "android")]
    {
        android::ocr_image(path)
    }
    #[cfg(target_os = "macos")]
    {
        macos::ocr_image(path)
    }
    #[cfg(not(any(target_os = "android", target_os = "macos")))]
    {
        // Tesseract (`ind`/`jav`) as a fallback is phased for later (docs/PLAN.md §11).
        let _ = path;
        Err("Text recognition from images isn't available on this platform yet.".to_string())
    }
}

/// Whether [`ocr_image`] can work on this platform/build.
pub fn ocr_available() -> bool {
    cfg!(any(target_os = "android", target_os = "macos"))
}

/// Opens `url` in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        android::open_url(url)
    }
    #[cfg(not(target_os = "android"))]
    {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        std::process::Command::new(opener).arg(url).spawn().map(drop).map_err(|e| format!("{opener}: {e}"))
    }
}

/// A page of Kabar's own in the system settings (Android).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppPage {
    Notifications,
    /// App info: storage, battery, permissions.
    Details,
}

/// Whether Kabar may show notifications (Android); `None` where the question
/// doesn't apply.
pub fn notifications_enabled() -> Option<bool> {
    #[cfg(target_os = "android")]
    {
        android::notifications_enabled().map_err(|e| crate::diag::report!("notification check: {e}")).ok()
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Opens Kabar's page in the system settings (Android only).
pub fn open_app_page(page: AppPage) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        android::open_app_page(page)
    }
    #[cfg(not(target_os = "android"))]
    {
        Err(format!("{page:?}: no system settings page on this platform"))
    }
}

/// Downloaded episodes play in a native media service (Android), so they
/// keep playing with the screen off. Elsewhere the WebView's `<audio>` does.
pub const BACKGROUND_AUDIO: bool = cfg!(target_os = "android");

/// The native player's calls; each is an error where [`BACKGROUND_AUDIO`]
/// is false (callers check it first).
pub mod track {
    #[cfg(target_os = "android")]
    pub use super::android::{
        track_pause as pause, track_play as play, track_rate as rate, track_resume as resume,
        track_seek as seek, track_state as state, track_stop as stop,
    };

    #[cfg(not(target_os = "android"))]
    mod none {
        const NONE: &str = "no native player on this platform";
        pub fn play(_path: &str, _title: &str, _show: &str, _at: f64, _rate: f32) -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn pause() -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn resume() -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn seek(_at: f64) -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn rate(_rate: f32) -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn stop() -> Result<(), String> {
            Err(NONE.into())
        }
        pub fn state() -> Result<(f64, bool, bool), String> {
            Err(NONE.into())
        }
    }
    #[cfg(not(target_os = "android"))]
    pub use none::*;
}

/// Kak Nur's reminders reach this device as local notifications: Android,
/// and the bundled Mac app (UNUserNotificationCenter). Push through FCM is
/// issue #187.
pub const LOCAL_REMINDERS: bool = cfg!(any(target_os = "android", target_os = "macos"));

/// Local reminder scheduling; no-ops where [`LOCAL_REMINDERS`] is false.
pub mod reminders {
    /// Replaces the scheduled reminders with `json` (`[{at, title, text}]`).
    pub fn schedule(json: &str) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            super::android::schedule_reminders(json)
        }
        #[cfg(target_os = "macos")]
        {
            super::macos::reminders::schedule(json)
        }
        #[cfg(not(any(target_os = "android", target_os = "macos")))]
        {
            let _ = json;
            Ok(())
        }
    }

    /// Whether notifications can work here: Android, or the bundled Mac app
    /// (not `cargo run`, which has no bundle).
    pub fn available() -> bool {
        #[cfg(target_os = "macos")]
        {
            super::macos::reminders::bundled()
        }
        #[cfg(not(target_os = "macos"))]
        {
            super::LOCAL_REMINDERS
        }
    }

    /// The reminders shown while the app was closed (unix ms), once each.
    pub fn take_delivered() -> Vec<u64> {
        #[cfg(target_os = "android")]
        {
            super::android::take_delivered_reminders()
                .map_err(|e| crate::diag::report!("delivered reminders: {e}"))
                .unwrap_or_default()
        }
        #[cfg(not(target_os = "android"))]
        {
            Vec::new()
        }
    }

    /// `Some(false)` when Android may put Kabar to sleep and stop its
    /// alarms (Samsung's "sleeping apps"); `None` where that can't happen.
    pub fn battery_unrestricted() -> Option<bool> {
        #[cfg(target_os = "android")]
        {
            super::android::battery_unrestricted()
                .map_err(|e| crate::diag::report!("battery check: {e}"))
                .ok()
        }
        #[cfg(not(target_os = "android"))]
        {
            None
        }
    }

    /// Asks for the notification permission where the system needs one.
    pub fn request() {
        #[cfg(target_os = "android")]
        if let Err(e) = super::android::request_notifications() {
            crate::diag::report!("notification permission: {e}");
        }
        #[cfg(target_os = "macos")]
        super::macos::reminders::request();
    }
}

/// The "new version" notification ([`crate::update::Notice`]) and, on
/// Android, the periodic background check that sends it while Kabar is
/// closed (`app.kabar.UpdateCheckJob`). No-ops elsewhere.
pub mod update_notice {
    use crate::update::Notice;

    /// Shows `n` now; true once it's shown. Android skips it while Kabar
    /// is in front (the banner says it) or notifications are off; the Mac
    /// app shows it only when bundled, and macOS keeps it quiet while Kabar
    /// is the active app.
    pub fn post(n: &Notice) -> bool {
        #[cfg(target_os = "android")]
        {
            super::android::post_update_notice(n)
                .map_err(|e| crate::diag::report!("update notification: {e}"))
                .unwrap_or(false)
        }
        #[cfg(target_os = "macos")]
        {
            super::macos::reminders::post_update(n)
        }
        #[cfg(not(any(target_os = "android", target_os = "macos")))]
        {
            let _ = n;
            false
        }
    }

    /// Removes a shown notification (the offer was closed or installed).
    pub fn clear() {
        #[cfg(target_os = "android")]
        if let Err(e) = super::android::clear_update_notice() {
            crate::diag::report!("update notification: {e}");
        }
        #[cfg(target_os = "macos")]
        super::macos::reminders::clear_update();
    }

    /// Android: starts (or, with `false`, stops) the background check.
    pub fn schedule(on: bool) {
        #[cfg(target_os = "android")]
        if let Err(e) = super::android::schedule_update_checks(on) {
            crate::diag::report!("background update check: {e}");
        }
        #[cfg(not(target_os = "android"))]
        let _ = on;
    }

    /// What a tap on the notification asked for, once: `open`, `download`
    /// or `install`.
    pub fn take_tap() -> Option<String> {
        #[cfg(target_os = "android")]
        {
            super::android::take_update_tap()
                .map_err(|e| crate::diag::report!("update notification: {e}"))
                .ok()
                .flatten()
        }
        #[cfg(not(target_os = "android"))]
        {
            None
        }
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;

    #[test]
    fn off_android_speech_goes_to_the_webview() {
        assert_eq!(speak("Selamat pagi!", "id-ID", false), Err(SpeakError::UseWebView));
        assert_eq!(tts_status(), TtsStatus::WebView);
        assert!(speech_rate(true) < speech_rate(false));
    }
}
