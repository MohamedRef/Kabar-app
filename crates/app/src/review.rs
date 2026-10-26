//! Review screens (SPEC §16): classic cards, Card Wall, quick bursts.

use dioxus::prelude::*;

use crate::Ui;
use crate::i18n::Msg;

#[component]
pub fn ReviewScreen() -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "card",
            h2 { "{ui.t(Msg::NavReview)}" }
        }
    }
}
