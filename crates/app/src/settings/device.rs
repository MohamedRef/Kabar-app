//! Preferences that belong to one device and never sync: the phone, the
//! tablet and the Mac each want their own text size, hover peek and sidebar.
//! Kept in the local `meta` table.

use dioxus::prelude::*;

use crate::Db;

const TEXT_SIZE_KEY: &str = "pref.text_size";
const PEN_HOVER_KEY: &str = "pref.pen_hover_peek";
const MOUSE_HOVER_KEY: &str = "pref.mouse_hover_peek";
const NAV_STYLE_KEY: &str = "pref.nav_style";

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

    /// Stored value and `data-text-size` on the app root.
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
}

/// The sidebar on wide windows (tablet landscape, the Mac).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavStyle {
    #[default]
    Labels,
    /// The narrow icon rail, as on medium widths.
    Slim,
}

impl NavStyle {
    pub const ALL: [Self; 2] = [Self::Labels, Self::Slim];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Labels => "labels",
            Self::Slim => "slim",
        }
    }

    fn parse(s: &str) -> Self {
        Self::ALL.into_iter().find(|t| t.as_str() == s).unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
pub struct DevicePrefs {
    pub text_size: Signal<TextSize>,
    /// The Reader's peek card on S Pen hover.
    pub pen_hover: Signal<bool>,
    /// The Reader's peek card when the mouse rests on a word.
    pub mouse_hover: Signal<bool>,
    pub nav_style: Signal<NavStyle>,
}

impl DevicePrefs {
    pub fn load(db: &Db) -> Self {
        let store = db.0.borrow();
        let get = |key| store.local_value(key).ok().flatten();
        let on = |key| get(key).as_deref() != Some("off");
        Self {
            text_size: Signal::new(
                get(TEXT_SIZE_KEY).map_or_else(TextSize::default, |v| TextSize::parse(&v)),
            ),
            pen_hover: Signal::new(on(PEN_HOVER_KEY)),
            mouse_hover: Signal::new(on(MOUSE_HOVER_KEY)),
            nav_style: Signal::new(
                get(NAV_STYLE_KEY).map_or_else(NavStyle::default, |v| NavStyle::parse(&v)),
            ),
        }
    }

    pub fn set_text_size(mut self, db: &Db, size: TextSize) {
        save(db, TEXT_SIZE_KEY, size.as_str());
        self.text_size.set(size);
    }

    pub fn set_pen_hover(mut self, db: &Db, on: bool) {
        save(db, PEN_HOVER_KEY, if on { "on" } else { "off" });
        self.pen_hover.set(on);
    }

    pub fn set_mouse_hover(mut self, db: &Db, on: bool) {
        save(db, MOUSE_HOVER_KEY, if on { "on" } else { "off" });
        self.mouse_hover.set(on);
    }

    pub fn set_nav_style(mut self, db: &Db, style: NavStyle) {
        save(db, NAV_STYLE_KEY, style.as_str());
        self.nav_style.set(style);
    }
}

fn save(db: &Db, key: &str, value: &str) {
    if let Err(e) = db.0.borrow_mut().set_local_value(key, value) {
        eprintln!("kabar: saving {key} failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_values_fall_back_to_defaults() {
        assert_eq!(TextSize::parse("huge"), TextSize::Normal);
        assert_eq!(NavStyle::parse(""), NavStyle::Labels);
        for t in TextSize::ALL {
            assert_eq!(TextSize::parse(t.as_str()), t);
        }
        for n in NavStyle::ALL {
            assert_eq!(NavStyle::parse(n.as_str()), n);
        }
    }
}
