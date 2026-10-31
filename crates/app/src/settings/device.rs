//! Preferences that belong to one device and never sync: the phone, the
//! tablet and the Mac each want their own text size, hover peek and sidebar.
//! Kept in the local `meta` table.

use dioxus::prelude::*;

use crate::Db;

const TEXT_SIZE_KEY: &str = "pref.text_size";
const PEN_HOVER_KEY: &str = "pref.pen_hover_peek";
const MOUSE_HOVER_KEY: &str = "pref.mouse_hover_peek";
const NAV_STYLE_KEY: &str = "pref.nav_style";
const REMIND_HERE_KEY: &str = "pref.remind_here";
const READ_SIZE_KEY: &str = "pref.read_size";
const READ_SPACING_KEY: &str = "pref.read_spacing";
const READ_WIDTH_KEY: &str = "pref.read_width";
const READ_FOCUS_KEY: &str = "pref.read_focus";
const APPEARANCE_KEY: &str = "pref.appearance";

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

/// A device preference with a few named values: the stored string doubles as
/// the `data-*` value the CSS keys on, and unknown strings fall back to the
/// default.
macro_rules! choice {
    ($(#[$doc:meta])* $name:ident { $($variant:ident = $s:literal),+ $(,)? } default $def:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant,)+
        }

        impl Default for $name {
            fn default() -> Self {
                Self::$def
            }
        }

        impl $name {
            pub const ALL: &[Self] = &[$(Self::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $s,)+
                }
            }

            fn parse(s: &str) -> Self {
                Self::ALL.iter().copied().find(|t| t.as_str() == s).unwrap_or_default()
            }
        }
    };
}

choice! {
    /// Interface style (redesign 2026-10-29): Calm (solid surfaces, the
    /// default), Glossy · Material You, or iOS-inspired Liquid Glass.
    /// Independent of the colour theme; `data-appearance` on the app root.
    /// A device without the pref, or with an unknown value, gets Calm.
    Appearance { Calm = "calm", Glossy = "glossy", LiquidGlass = "liquid-glass" } default Calm
}

choice! {
    /// The Reader's text size, on top of the app's text size.
    ReadSize { Smaller = "s", Normal = "m", Larger = "l", Largest = "xl" } default Normal
}

choice! {
    /// The Reader's line spacing.
    ReadSpacing { Snug = "snug", Normal = "normal", Airy = "airy" } default Normal
}

choice! {
    /// The Reader's line width on tablet and Mac: one narrow column with the
    /// meanings under each line, the book-width two columns, or the window.
    ReadWidth { Narrow = "narrow", Book = "book", Wide = "wide" } default Book
}

/// How the Reader lays its text out on this device (issue #180).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub size: ReadSize,
    pub spacing: ReadSpacing,
    pub width: ReadWidth,
    /// Tablet and Mac: the sidebar and the Reader's bar step aside.
    pub focus: bool,
}

#[derive(Clone, Copy)]
pub struct DevicePrefs {
    pub text_size: Signal<TextSize>,
    /// The Reader's peek card on S Pen hover.
    pub pen_hover: Signal<bool>,
    /// The Reader's peek card when the mouse rests on a word.
    pub mouse_hover: Signal<bool>,
    pub nav_style: Signal<NavStyle>,
    /// Kak Nur's local reminders ring here. `None` until the learner picks:
    /// then only the phone rings (see [`Self::reminds_here`]), so the phone
    /// and the tablet never both go off.
    pub remind_here: Signal<Option<bool>>,
    pub reading: Signal<Reading>,
    pub appearance: Signal<Appearance>,
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
            remind_here: Signal::new(get(REMIND_HERE_KEY).map(|v| v == "on")),
            reading: Signal::new(Reading {
                size: get(READ_SIZE_KEY).map_or_else(ReadSize::default, |v| ReadSize::parse(&v)),
                spacing: get(READ_SPACING_KEY).map_or_else(ReadSpacing::default, |v| ReadSpacing::parse(&v)),
                width: get(READ_WIDTH_KEY).map_or_else(ReadWidth::default, |v| ReadWidth::parse(&v)),
                focus: get(READ_FOCUS_KEY).as_deref() == Some("on"),
            }),
            appearance: Signal::new(stored_appearance(&store)),
        }
    }

    /// Whether this device rings: the learner's choice, else only an
    /// Android device that isn't a tablet (smallest side < 600 dp, never by
    /// device name). The Mac and the tablet ring once the learner asks.
    pub fn reminds_here(&self, tablet: bool) -> bool {
        remind_default(*self.remind_here.read(), cfg!(target_os = "android") && !tablet)
    }

    pub fn set_remind_here(mut self, db: &Db, on: bool) {
        save(db, REMIND_HERE_KEY, if on { "on" } else { "off" });
        self.remind_here.set(Some(on));
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

    pub fn set_reading(mut self, db: &Db, r: Reading) {
        let old = *self.reading.peek();
        if r.size != old.size {
            save(db, READ_SIZE_KEY, r.size.as_str());
        }
        if r.spacing != old.spacing {
            save(db, READ_SPACING_KEY, r.spacing.as_str());
        }
        if r.width != old.width {
            save(db, READ_WIDTH_KEY, r.width.as_str());
        }
        if r.focus != old.focus {
            save(db, READ_FOCUS_KEY, if r.focus { "on" } else { "off" });
        }
        self.reading.set(r);
    }

    /// Only flips the root's `data-appearance`: every screen keeps its state.
    pub fn set_appearance(mut self, db: &Db, appearance: Appearance) {
        save(db, APPEARANCE_KEY, appearance.as_str());
        self.appearance.set(appearance);
    }

    pub fn set_nav_style(mut self, db: &Db, style: NavStyle) {
        save(db, NAV_STYLE_KEY, style.as_str());
        self.nav_style.set(style);
    }
}

/// The saved interface style; Calm when the row is missing (older installs)
/// or unreadable.
fn stored_appearance(store: &crate::store::Store) -> Appearance {
    store
        .local_value(APPEARANCE_KEY)
        .ok()
        .flatten()
        .map_or_else(Appearance::default, |v| Appearance::parse(&v))
}

fn remind_default(choice: Option<bool>, phone: bool) -> bool {
    choice.unwrap_or(phone)
}

fn save(db: &Db, key: &str, value: &str) {
    let db = db.clone();
    let key = key.to_owned();
    let value = value.to_owned();
    crate::persistence::save(format!("local.{key}"), move || {
        db.0.borrow_mut().set_local_value(&key, &value).map_err(|e| e.to_string())
    });
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
        assert_eq!(ReadWidth::parse("huge"), ReadWidth::Book);
        assert_eq!(ReadSize::parse(""), ReadSize::Normal);
        for &w in ReadWidth::ALL {
            assert_eq!(ReadWidth::parse(w.as_str()), w);
        }
        for &s in ReadSpacing::ALL {
            assert_eq!(ReadSpacing::parse(s.as_str()), s);
        }
    }

    #[test]
    fn appearance_defaults_to_calm_and_round_trips() {
        // Older installs have no pref.appearance row: they stay Calm.
        assert_eq!(Appearance::default(), Appearance::Calm);
        assert_eq!(Appearance::parse(""), Appearance::Calm);
        assert_eq!(Appearance::parse("shiny"), Appearance::Calm);
        for &a in Appearance::ALL {
            assert_eq!(Appearance::parse(a.as_str()), a);
        }
        assert_eq!(Appearance::Glossy.as_str(), "glossy");
    }

    #[test]
    fn appearance_survives_a_restart() {
        use crate::store::Store;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");
        let mut store = Store::open(&path).unwrap();
        assert_eq!(stored_appearance(&store), Appearance::Calm, "a profile without the pref is Calm");
        store.set_local_value(APPEARANCE_KEY, Appearance::Glossy.as_str()).unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(stored_appearance(&store), Appearance::Glossy);
    }

    #[test]
    fn only_the_phone_rings_until_the_learner_picks() {
        assert!(remind_default(None, true), "phone");
        assert!(!remind_default(None, false), "tablet or Mac");
        assert!(remind_default(Some(true), false));
        assert!(!remind_default(Some(false), true));
    }
}
