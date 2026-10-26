//! The Lesson Player (SPEC §14): slides → exercises → dialogue → summary → souvenir.

use dioxus::prelude::*;

use crate::Ui;

#[component]
pub fn LessonScreen() -> Element {
    let ui = use_context::<Ui>();
    let lesson = ui.lesson.read().clone().unwrap_or_default();
    rsx! {
        div { class: "card",
            h2 { lang: "id", bdi { "{lesson}" } }
            button { onclick: move |_| ui.close_lesson(), "←" }
        }
    }
}
