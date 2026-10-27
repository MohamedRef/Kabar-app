//! The Again / Hard / Good / Easy row that ends a table or order item, with
//! the suggested grade marked. The learner always has the last word.

use batna_core::review::Grade;
use dioxus::prelude::*;

use crate::i18n::table as tx;

const CSS: &str = include_str!("grades.css");

const GRADES: [(Grade, &str); 4] =
    [(Grade::Again, tx::AGAIN), (Grade::Hard, tx::HARD), (Grade::Good, tx::GOOD), (Grade::Easy, tx::EASY)];

#[component]
pub fn GradeRow(suggested: Grade, on_grade: EventHandler<Grade>) -> Element {
    let mut picked = use_signal(|| None::<Grade>);
    rsx! {
        style { {CSS} }
        div { class: "tb-grades", role: "group", aria_label: tx::RATE_YOURSELF,
            p { class: "muted small", {tx::RATE_YOURSELF} }
            div { class: "tb-grade-row",
                for (grade, label) in GRADES {
                    button {
                        key: "{grade:?}",
                        class: "tb-grade",
                        "data-grade": "{grade:?}",
                        "data-suggested": if grade == suggested { "yes" } else { "no" },
                        aria_pressed: if *picked.read() == Some(grade) { "true" } else { "false" },
                        disabled: picked.read().is_some(),
                        onclick: move |_| {
                            picked.set(Some(grade));
                            on_grade.call(grade);
                        },
                        span { {label} }
                        if grade == suggested {
                            span { class: "tb-suggested", {tx::SUGGESTED} }
                        }
                    }
                }
            }
        }
    }
}
