//! Settings › Account: the changes Kabar Cloud refused, or this build
//! couldn't read, with a way to send them again or let them go
//! (engineering audit F12; `store/held.rs`).

use dioxus::prelude::*;
use kabar_core::locale::format_number;

use super::Cloud;
use super::msg::AccountMsg;
use crate::Ui;
use crate::cloud::Command;

#[component]
pub fn HeldCard(held: u64) -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let settings = ui.settings.read().clone();
    let count = format_number(held, settings.ui_language, settings.eastern_digits);
    let mut asking = use_signal(|| false);
    rsx! {
        crate::ui::DivCard { class: "held-ops", role: "status",
            h2 { "{ui.t(AccountMsg::HeldTitle)}" }
            p { class: "muted", "{count} {ui.t(AccountMsg::HeldCount)}" }
            p { "{ui.t(AccountMsg::HeldBody)}" }
            if *asking.read() {
                p { class: "notice", "{ui.t(AccountMsg::HeldForgetSure)}" }
                div { class: "row",
                    button {
                        onclick: move |_| {
                            asking.set(false);
                            cloud.send(Command::ForgetHeld);
                        },
                        "{ui.t(AccountMsg::HeldForgetYes)}"
                    }
                    button { class: "primary", onclick: move |_| asking.set(false), "{ui.t(AccountMsg::HeldForgetNo)}" }
                }
            } else {
                div { class: "row",
                    button { class: "primary", onclick: move |_| cloud.send(Command::SendHeldAgain), "{ui.t(AccountMsg::HeldSendAgain)}" }
                    button { onclick: move |_| asking.set(true), "{ui.t(AccountMsg::HeldForget)}" }
                }
            }
        }
    }
}
