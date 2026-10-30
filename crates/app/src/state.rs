//! App-wide reactive state and shared update paths.

use crate::i18n::{Msg, t};
use crate::{Screen, store::Store};
use dioxus::prelude::*;
use kabar_core::layout::WidthClass;
use kabar_core::locale::{UiLanguage, format_number, format_percent, localize_digits, uses_eastern_digits};
use kabar_core::settings::Settings;
use std::cell::RefCell;
use std::rc::Rc;

/// App-wide UI state, shared through context.
#[derive(Clone, Copy)]
pub struct Ui {
    pub settings: Signal<Settings>,
    pub width: Signal<WidthClass>,
    /// An Android tablet (smallest screen side ≥ 600 dp): the only place
    /// the pen tools (Studio, Pen Lab) apply while phone and Mac pen work
    /// is deferred (SPEC §23). Fixed per device, unlike `width`.
    pub tablet: Signal<bool>,
    pub screen: Signal<Screen>,
    /// The lesson the player shows (curriculum lesson id).
    pub lesson: Signal<Option<String>>,
    /// The screen the player was opened from; closing it goes back there.
    pub lesson_from: Signal<Screen>,
    /// A lesson or review session holds the screen (`focus::use_focus_mode`).
    pub focus: Signal<bool>,
}

/// The local database, shared through context.
#[derive(Clone)]
pub struct Db(pub Rc<RefCell<Store>>);

impl Ui {
    pub fn t(&self, msg: Msg) -> &'static str {
        t(self.settings.read().ui_language, msg)
    }

    pub fn lang(&self) -> UiLanguage {
        self.settings.read().ui_language
    }

    /// Whether numbers use Eastern Arabic digits right now.
    pub fn eastern(&self) -> bool {
        let s = self.settings.read();
        uses_eastern_digits(s.ui_language, s.eastern_digits)
    }

    /// A number in the UI's digits.
    pub fn num(&self, n: impl Into<u64>) -> String {
        let s = self.settings.read();
        format_number(n.into(), s.ui_language, s.eastern_digits)
    }

    /// Text with its digits in the UI's digits ("3:05", "1.4 MB").
    pub fn digits(&self, text: &str) -> String {
        let s = self.settings.read();
        localize_digits(text, s.ui_language, s.eastern_digits)
    }

    /// A share (0.0–1.0) as a percentage in the UI's digits: "87%", "٨٧٪".
    pub fn percent(&self, share: f32) -> String {
        self.whole_percent((share.clamp(0.0, 1.0) * 100.0).round() as u64)
    }

    /// A whole percentage (87) in the UI's digits: "87%", "٨٧٪".
    pub fn whole_percent(&self, n: impl Into<u64>) -> String {
        let s = self.settings.read();
        format_percent(n.into(), s.ui_language, s.eastern_digits)
    }

    /// Saves changed settings and updates the UI.
    pub fn update_settings(mut self, db: &Db, f: impl FnOnce(&mut Settings)) {
        let old = self.settings.read().clone();
        let mut new = old.clone();
        f(&mut new);
        if let Err(e) = db.0.borrow_mut().save_settings(&old, &new) {
            crate::diag::report!("saving settings failed: {e}");
        }
        self.settings.set(new);
    }

    /// Opens the Lesson Player on a lesson (from the Journey Map, Today or Lanjut).
    pub fn open_lesson(mut self, lesson_id: impl Into<String>) {
        let from = *self.screen.peek();
        if from != Screen::Lesson {
            self.lesson_from.set(from);
        }
        self.lesson.set(Some(lesson_id.into()));
        self.screen.set(Screen::Lesson);
    }

    /// Leaves the Lesson Player back to the screen it was opened from.
    pub fn close_lesson(mut self) {
        let back = *self.lesson_from.peek();
        self.lesson.set(None);
        self.screen.set(back);
    }
}

impl PartialEq for Db {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
