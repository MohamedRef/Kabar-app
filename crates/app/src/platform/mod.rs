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
