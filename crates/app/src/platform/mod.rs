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
