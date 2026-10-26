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

/// Where Settings → Export saves its file: `~/Downloads` on the Mac (and other
/// desktops); on Android the app's external files directory
/// (`Android/data/app.kabar/files`), which a computer can open over USB
/// without any permission. A share-sheet / MediaStore "Downloads" bridge is a
/// follow-up (it needs a FileProvider in `android/`).
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
                eprintln!("kabar: reading the shared item failed: {e}");
                None
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        None
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
