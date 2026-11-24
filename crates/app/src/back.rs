//! System Back (the Android gesture or button) moves back inside Kabar.
//!
//! wry's Android activity only handles Back while the WebView can go back in
//! its history, and otherwise sends the app to the background. So while
//! there is somewhere to go back to, one "trap" history entry is kept; Back
//! pops it, and Kabar steps back in this order:
//! 1. the topmost on-screen control marked `data-back="true"` (the Settings sheet's
//!    back/close, a word card's ×, "Stay" in the leave-test confirm, the
//!    Lesson Player's ×, the level check's back), clicked as if tapped;
//! 2. the previous screen, then Today.
//!
//! On Today with nothing open no entry is kept, so Back leaves Kabar as
//! Android users expect.
//!
//! On Android the Back key itself goes to the page first (`BackKey.kt` in
//! android/, `__kabarNativeBack` in system-back.js), because WebView skips
//! history entries added without a tap and would then let Android close
//! Kabar from any screen. The history trap stays as a fallback.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use study_shell::BackTrail;

use crate::{Screen, Ui};

pub(crate) fn back_script() -> String {
    study_shell::system_back_script("kabar")
}

/// Screens Back never returns to: full-screen players are left with their
/// own close button, and Settings is a sheet.
fn remembered(screen: Screen) -> bool {
    !matches!(screen, Screen::Lesson | Screen::LevelCheck | Screen::Settings)
}

fn trail() -> BackTrail<Screen> {
    BackTrail::new(Screen::Today, remembered)
}

/// One navigation trail for top-level screens. Every visible control with
/// data-back="true" participates automatically, without feature-specific flags.
pub fn use_back(ui: Ui, current: Screen) {
    let trail = use_hook(|| Rc::new(RefCell::new(trail())));
    // What the script was last told: the Shell re-renders often, and only a
    // change needs a round trip to the WebView.
    let told = use_hook(|| Rc::new(Cell::new(None::<bool>)));
    let can_back = trail.borrow_mut().visit(current);
    if told.replace(Some(can_back)) != Some(can_back) {
        tell(can_back);
    }

    use_future({
        let trail = trail.clone();
        let told = told.clone();
        move || {
            let trail = trail.clone();
            let told = told.clone();
            async move {
                let mut eval = document::eval(&back_script());
                while let Ok(message) = eval.recv::<String>().await {
                    if message == "back" {
                        let now = *ui.screen.peek();
                        if let Some(to) = trail.borrow_mut().back(now) {
                            let mut screen = ui.screen;
                            screen.set(to);
                        }
                    }
                    if message == "ready" || message == "back" {
                        // Always answer: the script waits for this after a Back.
                        let can_back = trail.borrow_mut().visit(*ui.screen.peek());
                        told.set(Some(can_back));
                        tell(can_back);
                    }
                }
            }
        }
    });
}

/// Tells the script whether the screen trail has somewhere to go back to.
fn tell(can_back: bool) {
    document::eval(if can_back {
        "window.__kabarSetScreenBack && window.__kabarSetScreenBack(true);"
    } else {
        "window.__kabarSetScreenBack && window.__kabarSetScreenBack(false);"
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_walks_the_visited_screens_then_today() {
        let mut t = trail();
        assert!(!t.visit(Screen::Today), "nothing to go back to on Today");
        assert!(t.visit(Screen::Journey));
        assert!(t.visit(Screen::Reader));
        assert_eq!(t.back(Screen::Reader), Some(Screen::Journey));
        assert!(t.visit(Screen::Journey));
        assert_eq!(t.back(Screen::Journey), Some(Screen::Today));
        assert!(!t.visit(Screen::Today));
        assert_eq!(t.back(Screen::Today), None, "Back on Today leaves the app");
    }

    #[test]
    fn players_are_not_returned_to() {
        let mut t = trail();
        t.visit(Screen::Journey);
        t.visit(Screen::Lesson);
        t.visit(Screen::Journey); // the player closed itself
        assert_eq!(t.back(Screen::Journey), Some(Screen::Today));
    }

    #[test]
    fn going_round_in_circles_never_lengthens_the_way_back() {
        let mut t = trail();
        for screen in [
            Screen::Today,
            Screen::Journey,
            Screen::Reader,
            Screen::Journey,
            Screen::Reader,
            Screen::Journey,
            Screen::Reader,
        ] {
            t.visit(screen);
        }
        assert_eq!(t.back(Screen::Reader), Some(Screen::Journey));
        t.visit(Screen::Journey);
        assert_eq!(t.back(Screen::Journey), Some(Screen::Today));
        t.visit(Screen::Today);
        assert_eq!(t.back(Screen::Today), None);
    }

    #[test]
    fn back_on_today_leaves_once_today_is_reached_again() {
        let mut t = trail();
        for screen in [Screen::Today, Screen::Journey, Screen::Reader, Screen::Today] {
            t.visit(screen);
        }
        assert!(!t.visit(Screen::Today), "home has nowhere to go back to");
        assert_eq!(t.back(Screen::Today), None);
    }

    #[test]
    fn closing_a_transient_screen_does_not_create_a_ghost_step() {
        let mut t = trail();
        t.visit(Screen::Today);
        t.visit(Screen::Journey);
        t.visit(Screen::Reader);
        t.visit(Screen::Lesson);
        t.visit(Screen::Reader);
        assert_eq!(t.back(Screen::Reader), Some(Screen::Journey));
        t.visit(Screen::Journey);
        assert_eq!(t.back(Screen::Journey), Some(Screen::Today));
        t.visit(Screen::Today);
        assert_eq!(t.back(Screen::Today), None);
    }
}
