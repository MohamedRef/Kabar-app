//! Preferences that belong to one device and never sync: the phone, the
//! tablet and the Mac each keep their own theme and text size. A small
//! `key=value` file in the app's data folder, written whole on every change.
// adapted from kabar-app main: crates/app/src/settings/device.rs

use std::collections::BTreeMap;
use std::path::PathBuf;

use batna_core::theme::Theme;

const FILE: &str = "device-prefs.txt";
const THEME: &str = "theme";
const TEXT_SIZE: &str = "text_size";

/// The app's own data folder: the local database and device files live here.
pub fn data_dir() -> Option<PathBuf> {
    if cfg!(target_os = "android") {
        // The app's private files directory (Context.getFilesDir for the
        // device owner). A JNI lookup can replace this once platform/ has one.
        return Some(PathBuf::from("/data/data/app.batna/files"));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/app.batna"));
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".local/share")))
        .map(|d| d.join("batna"))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextSize {
    Small,
    #[default]
    Normal,
    Large,
    Larger,
}

impl TextSize {
    pub const ALL: [Self; 4] = [Self::Small, Self::Normal, Self::Large, Self::Larger];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Normal => "normal",
            Self::Large => "large",
            Self::Larger => "larger",
        }
    }

    fn parse(s: &str) -> Self {
        Self::ALL.into_iter().find(|t| t.as_str() == s).unwrap_or_default()
    }

    /// The root font size: every rem in the app scales with it.
    pub fn percent(self) -> u32 {
        match self {
            Self::Small => 90,
            Self::Normal => 100,
            Self::Large => 112,
            Self::Larger => 125,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DevicePrefs {
    pub theme: Theme,
    pub text_size: TextSize,
}

impl DevicePrefs {
    pub fn load() -> Self {
        let text = data_dir().and_then(|d| std::fs::read_to_string(d.join(FILE)).ok()).unwrap_or_default();
        Self::parse(&text)
    }

    /// Best effort: a folder that can't be written keeps the choice for this run.
    pub fn save(&self) {
        let Some(dir) = data_dir() else { return };
        let _ = std::fs::create_dir_all(&dir);
        let tmp = dir.join(format!("{FILE}.tmp"));
        if std::fs::write(&tmp, self.render()).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join(FILE));
        }
    }

    fn parse(text: &str) -> Self {
        let map: BTreeMap<&str, &str> =
            text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
        Self {
            theme: map.get(THEME).map_or_else(Theme::default, |v| Theme::parse(v)),
            text_size: map.get(TEXT_SIZE).map_or_else(TextSize::default, |v| TextSize::parse(v)),
        }
    }

    fn render(&self) -> String {
        format!("{THEME}={}\n{TEXT_SIZE}={}\n", self.theme.as_str(), self.text_size.as_str())
    }

    /// The page-wide CSS for these choices: colour tokens and root text size.
    pub fn css(&self) -> String {
        format!("{}html{{font-size:{}%;}}", self.theme.css(), self.text_size.percent())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_ignores_junk() {
        let p = DevicePrefs { theme: Theme::NightShift, text_size: TextSize::Larger };
        assert_eq!(DevicePrefs::parse(&p.render()), p);
        assert_eq!(DevicePrefs::parse("theme=neon\nnonsense\ntext_size=huge"), DevicePrefs::default());
    }

    #[test]
    fn css_carries_theme_and_size() {
        let p = DevicePrefs { theme: Theme::Paper, text_size: TextSize::Large };
        let css = p.css();
        assert!(css.contains("--bg:#f6f7f4") && css.ends_with("html{font-size:112%;}"), "{css}");
    }
}
