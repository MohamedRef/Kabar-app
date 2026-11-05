//! The app shell: the navigation chrome around the current screen.

use super::*;

#[component]
pub(super) fn Shell() -> Element {
    let ui = use_context::<Ui>();
    let current = *ui.screen.read();
    // A new screen starts at its top, not at the last screen's scroll
    // position. Sent during render, so it lands before a screen's own
    // scroll (the Journey Map centres the current lesson in an effect).
    let last = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(current)));
    if last.replace(current) != current {
        document::eval(r#"window.scrollTo({ top: 0, left: 0, behavior: "instant" });"#);
    }
    let tablet = *ui.tablet.read();
    let settings_nav = use_context::<settings::SettingsNav>();
    #[cfg(all(feature = "desktop", not(target_os = "android")))]
    settings::menu::use_menu_events(ui, settings_nav);
    let settings_open = *settings_nav.open.read();
    back::use_back(ui, current);
    update::view::use_android_install(ui);
    update::view::use_update_notices(ui, settings_nav);
    update::view::use_content_confirmation();
    // Full-screen players keep the whole screen; everywhere else on the
    // phone a slim top bar carries the gear.
    let top_bar =
        *ui.width.read() == WidthClass::Compact && !matches!(current, Screen::Lesson | Screen::LevelCheck);
    // Grouped like a Mac sidebar (Apple HIG, Material 3): what you learn,
    // where you practise, and your own corner. On the phone the groups melt
    // into the bottom bar; on the rail they are small gaps.
    let groups = [
        (Msg::NavGroupLearn, &[Screen::Today, Screen::Journey, Screen::Reader, Screen::Review][..]),
        (
            Msg::NavGroupPractise,
            &[Screen::Play, Screen::Listen, Screen::Chat, Screen::Teach, Screen::Studio, Screen::PenLab][..],
        ),
        (Msg::NavGroupMe, &[Screen::Stats, Screen::Coach, Screen::Journal][..]),
    ];
    // Re-tapping the tab you're on starts that section over (Games → the
    // hub, Chat → the list, Review → its home): the screen remounts.
    let mut generation = use_signal(|| 0u32);

    rsx! {
        div { class: "shell",
            nav { class: "nav", aria_label: "Kabar",
                div { class: "brand latin", "Kabar" }
                for (g, (title, screens)) in groups.into_iter().enumerate() {
                    div { key: "{g}", class: "nav-group", role: "group", aria_labelledby: "nav-group-{g}",
                        p { id: "nav-group-{g}", class: "nav-heading", "{ui.t(title)}" }
                        for item in screens.iter().copied().filter(|s| tablet || !s.pen_only()) {
                    button {
                        key: "{item:?}",
                        class: if item.secondary() { "nav-item secondary" } else { "nav-item" },
                        aria_current: if item == current { "page" } else { "false" },
                        title: "{ui.t(item.label())}",
                        onclick: move |_| {
                            if item == *ui.screen.peek() {
                                generation += 1;
                            }
                            if item == Screen::Play {
                                let play::PlayNav(mut nav) = consume_context::<play::PlayNav>();
                                nav.set(play::Route::Hub);
                            }
                            if item == Screen::Stats {
                                stats::open_from_nav(ui);
                            } else {
                                let mut screen = ui.screen;
                                screen.set(item);
                            }
                        },
                        span { class: "nav-icon", aria_hidden: "true", {icons::nav(item)} }
                        // One name everywhere; the narrow rail shows its short
                        // form (CSS picks the span, so a resize needs no render).
                        if let Some(short) = more::rail_label(ui.lang(), item) {
                            span { class: "nav-label nav-label-full", "{ui.t(item.label())}" }
                            span { class: "nav-label nav-label-short", "{short}" }
                        } else {
                            span { class: "nav-label", "{ui.t(item.label())}" }
                        }
                    }
                        }
                    }
                }
                more::MoreTab { current }
                // Pinned to the foot of the rail and sidebar (hidden in the
                // phone's bottom bar, which has the gear up top instead).
                button {
                    class: "nav-item secondary nav-settings",
                    aria_current: if settings_open { "page" } else { "false" },
                    aria_haspopup: "dialog",
                    onclick: move |_| settings_nav.show(None),
                    span { class: "nav-icon", aria_hidden: "true", {icons::gear()} }
                    span { "{ui.t(Msg::NavSettings)}" }
                }
            }
            if top_bar {
                more::TopBar { current }
            }
            main { class: "content", key: "{current:?}-{generation}",
                if !settings_open { persistence::SaveNotice {} }
                match current {
                    Screen::Today => rsx! { coach::today::Today {} },
                    Screen::Coach => rsx! { coach::coach_screen::CoachScreen {} },
                    Screen::Journal => rsx! { coach::journal::JournalScreen {} },
                    Screen::Play => rsx! { play::Play {} },
                    Screen::Journey => rsx! { journey::JourneyScreen {} },
                    Screen::Reader => rsx! { reader::Reader {} },
                    Screen::Review => rsx! { review::ReviewScreen {} },
                    Screen::Studio => rsx! { studio::StudioScreen {} },
                    // Settings is a sheet over the current screen, not a screen.
                    Screen::Settings => rsx! { coach::today::Today {} },
                    Screen::PenLab => rsx! { penlab::PenLab {} },
                    Screen::Devices => rsx! { account::DevicesScreen {} },
                    Screen::Teach => rsx! { teach::TeachScreen {} },
                    Screen::Cost => rsx! { cost::CostScreen {} },
                    Screen::Listen => rsx! { listen::ListenScreen {} },
                    Screen::Chat => rsx! { chat::ChatScreen {} },
                    Screen::Lesson => rsx! { lesson::LessonScreen {} },
                    Screen::Stats => rsx! { stats::StatsScreen {} },
                    Screen::LevelCheck => rsx! { stats::check::LevelCheckScreen {} },
                }
            }
            more::MoreSheet {}
            settings::SettingsSheet {}
            update::sheet::UpdateSheet {}
        }
    }
}
