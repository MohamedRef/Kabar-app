//! Platform glue: where data lives and what device we're on.

#[cfg(target_os = "android")]
pub mod android;

use std::path::PathBuf;

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
