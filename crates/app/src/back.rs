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

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;

use crate::{Screen, Ui};

const BACK_JS: &str = r#"
window.__kabarBackSend = (m) => dioxus.send(m);
if (!window.__kabarBackInit) {
  window.__kabarBackInit = true;
  let trapped = false;
  window.__kabarTrap = () => {
    if (!trapped) {
      history.pushState({ kabar: true }, "");
      trapped = true;
    }
  };
  window.addEventListener("popstate", () => {
    trapped = false;
    const targets = document.querySelectorAll('[data-back="true"]');
    const top = targets[targets.length - 1];
    if (top) {
      top.click();
      window.__kabarTrap();
    } else {
      window.__kabarBackSend(true);
    }
  });
}
await new Promise(() => {});
"#;

/// How many screens Back remembers.
const DEPTH: usize = 30;

/// Screens Back never returns to: full-screen players are left with their
/// own close button, and Settings is a sheet.
fn remembered(screen: Screen) -> bool {
    !matches!(screen, Screen::Lesson | Screen::LevelCheck | Screen::Settings)
}

/// The screens visited, newest last.
#[derive(Default)]
struct Trail {
    stack: Vec<Screen>,
    last: Option<Screen>,
    /// The next screen change comes from Back: don't remember it.
    popping: bool,
}

impl Trail {
    /// Records a move to `now`; returns whether Back has somewhere to go.
    fn visit(&mut self, now: Screen) -> bool {
        if let Some(last) = self.last
            && last != now
            && !std::mem::take(&mut self.popping)
            && remembered(last)
            && self.stack.last() != Some(&last)
        {
            self.stack.push(last);
            if self.stack.len() > DEPTH {
                self.stack.remove(0);
            }
        }
        self.last = Some(now);
        self.stack.retain(|s| *s != now);
        !self.stack.is_empty() || now != Screen::Today
    }

    /// Where Back goes from `now`, if anywhere.
    fn back(&mut self, now: Screen) -> Option<Screen> {
        let to = self.stack.pop().or((now != Screen::Today).then_some(Screen::Today))?;
        self.popping = true;
        Some(to)
    }
}

/// Wires Back for the Shell. `overlay_open`: a sheet that Back should close
/// is showing (it carries its own `data-back` control).
pub fn use_back(ui: Ui, current: Screen, overlay_open: bool) {
    let trail = use_hook(|| Rc::new(RefCell::new(Trail::default())));
    let armed = use_hook(|| Rc::new(Cell::new(None::<(Screen, bool)>)));

    // Re-arm the trap whenever the screen or an overlay changes.
    let can_back = trail.borrow_mut().visit(current) || overlay_open;
    if armed.replace(Some((current, overlay_open))) != Some((current, overlay_open)) && can_back {
        document::eval("window.__kabarTrap && window.__kabarTrap();");
    }

    use_future({
        let trail = trail.clone();
        move || {
            let trail = trail.clone();
            async move {
                let mut eval = document::eval(BACK_JS);
                // The first screen may already have somewhere to go.
                document::eval("window.__kabarTrap && window.__kabarTrap();");
                while eval.recv::<bool>().await.is_ok() {
                    let now = *ui.screen.peek();
                    let to = trail.borrow_mut().back(now);
                    if let Some(to) = to {
                        let mut screen = ui.screen;
                        screen.set(to);
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_walks_the_visited_screens_then_today() {
        let mut t = Trail::default();
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
        let mut t = Trail::default();
        t.visit(Screen::Journey);
        t.visit(Screen::Lesson);
        t.visit(Screen::Journey); // the player closed itself
        assert_eq!(t.back(Screen::Journey), Some(Screen::Today));
    }

    #[test]
    fn a_screen_is_remembered_once() {
        let mut t = Trail::default();
        for s in [Screen::Today, Screen::Review, Screen::Today, Screen::Review] {
            t.visit(s);
        }
        assert_eq!(t.back(Screen::Review), Some(Screen::Today));
        t.visit(Screen::Today);
        assert_eq!(t.back(Screen::Today), None);
    }
}
