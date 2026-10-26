//! The Journey Map (SPEC §10): BIPA islands and the Javanese train line.

use dioxus::prelude::*;

use crate::Ui;
use crate::i18n::Msg;

#[component]
pub fn JourneyScreen() -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "card",
            h2 { "{ui.t(Msg::NavJourney)}" }
        }
    }
}
