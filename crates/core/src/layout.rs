//! Layout follows window width, never the device name. Breakpoints are
//! Android's window size classes (verified Oct 2026).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WidthClass {
    /// < 600 dp: phone portrait.
    Compact,
    /// 600–840 dp: tablet portrait, split screen.
    Medium,
    /// 840–1200 dp: tablet landscape, small desktop windows.
    Expanded,
    /// 1200–1600 dp: DeX, Mac.
    Large,
    /// ≥ 1600 dp.
    ExtraLarge,
}

impl WidthClass {
    /// `width` is in CSS pixels, which equal dp in the Android WebView.
    pub fn from_width(width: f64) -> Self {
        match width {
            w if w < 600.0 => Self::Compact,
            w if w < 840.0 => Self::Medium,
            w if w < 1200.0 => Self::Expanded,
            w if w < 1600.0 => Self::Large,
            _ => Self::ExtraLarge,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Medium => "medium",
            Self::Expanded => "expanded",
            Self::Large => "large",
            Self::ExtraLarge => "extra-large",
        }
    }

    /// How the main navigation is shown at this width.
    pub fn nav(self) -> NavStyle {
        match self {
            Self::Compact => NavStyle::BottomBar,
            Self::Medium => NavStyle::Rail,
            _ => NavStyle::Sidebar,
        }
    }

    /// The Study Desk shows three panes from expanded upwards.
    pub fn panes(self) -> u8 {
        match self {
            Self::Compact => 1,
            Self::Medium => 2,
            _ => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavStyle {
    BottomBar,
    Rail,
    Sidebar,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_match_android_size_classes() {
        assert_eq!(WidthClass::from_width(412.0), WidthClass::Compact); // S24 Ultra portrait
        assert_eq!(WidthClass::from_width(599.9), WidthClass::Compact);
        assert_eq!(WidthClass::from_width(600.0), WidthClass::Medium);
        assert_eq!(WidthClass::from_width(840.0), WidthClass::Expanded);
        assert_eq!(WidthClass::from_width(1200.0), WidthClass::Large);
        assert_eq!(WidthClass::from_width(1600.0), WidthClass::ExtraLarge);
    }

    #[test]
    fn nav_and_panes_follow_width() {
        assert_eq!(WidthClass::Compact.nav(), NavStyle::BottomBar);
        assert_eq!(WidthClass::Medium.nav(), NavStyle::Rail);
        assert_eq!(WidthClass::Large.nav(), NavStyle::Sidebar);
        assert_eq!(WidthClass::Expanded.panes(), 3);
    }
}
