//! Collected Journey souvenirs and suitcase presentation.

use super::*;

#[component]
pub(super) fn Suitcase(completed: Rc<BTreeSet<String>>, approved: Rc<BTreeSet<String>>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let routes = ROUTES.as_ref().expect("checked by JourneyScreen");
    let cards: Vec<_> = [(&routes.id, "id"), (&routes.jv, "jv")]
        .into_iter()
        .flat_map(|(c, tl)| {
            souvenirs(c, &Progress::new(c, &completed).with_approved(&approved))
                .into_iter()
                .map(move |s| (s, tl))
        })
        .collect();
    let focus = use_hook(|| SUITCASE_AT.write().take());
    use_effect({
        let focus = focus.clone();
        move || {
            if let Some(unit) = &focus {
                document::eval(&format!(
                    r#"document.getElementById("souvenir-{unit}")?.scrollIntoView({{block: "center"}});"#
                ));
            }
        }
    });
    rsx! {
        section { class: "suitcase", aria_label: "{ui.t(Msg::JourneySuitcase)}",
            if cards.is_empty() {
                crate::ui::DivCard { class: "suitcase-empty",
                    crate::misc_art::Picture { art: crate::misc_art::Art::EmptySuitcase, class: "empty-art" }
                    p { "{ui.t(Msg::JourneySuitcaseEmpty)}" }
                }
            }
            ul { class: "souvenirs",
                // A souvenir opens the lesson that gave it.
                for (s, tl) in cards {
                    li { key: "{s.unit_id}", id: "souvenir-{s.unit_id}",
                    crate::ui::CardButton {
                        class: if focus.as_deref() == Some(s.unit_id.as_str()) { "souvenir tap found" } else { "souvenir tap" },
                        onclick: {
                            let lesson = souvenir_lesson(&s.unit_id);
                            move |_| if let Some(id) = &lesson { ui.open_lesson(id.clone()) }
                        },
                        crate::souvenir_art::SouvenirArt { unit_id: s.unit_id.clone(), size: crate::souvenir_art::Size::Thumb }
                        div { class: "souvenir-stamp", aria_hidden: "true", bdi { "{s.level_code}" } }
                        h3 { bdi { lang: tl, dir: "ltr", "{local_souvenir(&s.unit_id, lang).map(|(t, _)| t).unwrap_or(s.name.clone())}" } }
                        p { class: "muted souvenir-where",
                            bdi { lang: tl, dir: "ltr", "{s.stop}" }
                            if s.unverified {
                                " "
                                span { class: "badge-unverified", "{ui.t(Msg::JourneyUnverified)}" }
                            }
                        }
                        if let Some((_, note)) = local_souvenir(&s.unit_id, lang) {
                            p { class: "souvenir-note", crate::lesson::Rich { text: note, target: tl } }
                        } else {
                            p { class: "souvenir-note", lang: "en", dir: "ltr", "{s.note}" }
                        }
                    }
                    }
                }
            }
        }
    }
}
