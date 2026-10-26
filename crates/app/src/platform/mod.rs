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
