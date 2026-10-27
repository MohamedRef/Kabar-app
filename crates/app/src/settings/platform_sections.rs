//! Settings sections that depend on the device (Android, Tablet, Mac), plus
//! storage, account and about.

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;

use super::device::{DevicePrefs, NavStyle};
use super::text::{S, st};
use super::{Scope, SettingsNav};
use crate::i18n::Msg;
use crate::platform::{self, AppPage};
use crate::{Db, Screen, Ui};

#[component]
pub fn AndroidSection() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let prefs = use_context::<DevicePrefs>();
    let lang = ui.lang();
    let mut allowed = use_signal(platform::notifications_enabled);
    let mut failed = use_signal(|| false);
    // The switch lives in Android Settings: re-check while this page is open,
    // so coming back from there shows the new state.
    use_future(move || async move {
        if !cfg!(target_os = "android") {
            return;
        }
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            let now = platform::notifications_enabled();
            if *allowed.peek() != now {
                allowed.set(now);
            }
        }
    });
    let mut open = move |page: AppPage| {
        let ok = platform::open_app_page(page).map_err(|e| eprintln!("kabar: {e}")).is_ok();
        failed.set(!ok);
    };
    let pen = *prefs.pen_hover.read();

    rsx! {
        div { class: "card",
            div { class: "field",
                span { class: "label", "{st(lang, S::Notifications)}" Scope { device_only: true } }
                p { class: "muted settings-hint", "{st(lang, S::NotificationsBody)}" }
                div { class: "row",
                    match allowed() {
                        Some(true) => rsx! { span { class: "pill", "✓ {st(lang, S::NotificationsOn)}" } },
                        Some(false) => rsx! { span { class: "pill", "{st(lang, S::NotificationsOff)}" } },
                        None => rsx! {},
                    }
                    button {
                        class: if allowed() == Some(false) { "primary" } else { "" },
                        onclick: move |_| open(AppPage::Notifications),
                        if allowed() == Some(false) { "{st(lang, S::NotificationsAllow)}" } else { "{st(lang, S::NotificationsOpen)}" }
                    }
                }
            }

            div { class: "field",
                span { class: "label", "{st(lang, S::BackgroundAudio)}" }
                p { class: "muted settings-hint", "{st(lang, S::BackgroundAudioBody)}" }
            }

            div { class: "field",
                label {
                    input {
                        r#type: "checkbox",
                        checked: pen,
                        onchange: {
                            let db = db.clone();
                            move |e: FormEvent| prefs.set_pen_hover(&db, e.checked())
                        },
                    }
                    " {st(lang, S::PenHover)}"
                    Scope { device_only: true }
                }
                p { class: "muted settings-hint", "{st(lang, S::PenHoverBody)}" }
            }

            div { class: "field",
                span { class: "label", "{st(lang, S::AppInfo)}" }
                p { class: "muted settings-hint", "{st(lang, S::AppInfoBody)}" }
                button { onclick: move |_| open(AppPage::Details), "{st(lang, S::AppInfoOpen)}" }
            }

            if failed() {
                p { class: "notice", role: "status", "{st(lang, S::OpenFailed)}" }
            }
        }
    }
}

#[component]
pub fn TabletSection() -> Element {
    rsx! {
        div { class: "card",
            NavStyleField {}
            LayoutNow {}
        }
    }
}

#[component]
pub fn MacSection() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let prefs = use_context::<DevicePrefs>();
    let lang = ui.lang();
    let hover = *prefs.mouse_hover.read();
    let rows: [(S, &[&str]); 5] = [
        (S::ShortcutSettings, &["⌘", ","]),
        (S::ShortcutScreens, &["⌘1", "…", "⌘7"]),
        (S::ShortcutPick, &["1", "2", "3"]),
        (S::ShortcutCheck, &["Return"]),
        (S::ShortcutClose, &["Esc"]),
    ];

    rsx! {
        div { class: "card",
            p { class: "muted settings-hint", "{st(lang, S::MenuTip)}" }
        }
        div { class: "card",
            h2 { "{st(lang, S::Shortcuts)}" }
            table { class: "shortcut-table",
                tbody {
                    for (label, keys) in rows {
                        tr { key: "{label:?}",
                            td { "{st(lang, label)}" }
                            td {
                                span { class: "keys", dir: "ltr",
                                    for (i, k) in keys.iter().enumerate() {
                                        if *k == "…" {
                                            span { key: "{i}", " … " }
                                        } else {
                                            kbd { key: "{i}", "{k}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        div { class: "card",
            div { class: "field",
                label {
                    input {
                        r#type: "checkbox",
                        checked: hover,
                        onchange: {
                            let db = db.clone();
                            move |e: FormEvent| prefs.set_mouse_hover(&db, e.checked())
                        },
                    }
                    " {st(lang, S::MouseHover)}"
                    Scope { device_only: true }
                }
                p { class: "muted settings-hint", "{st(lang, S::MouseHoverBody)}" }
            }
            NavStyleField {}
            div { class: "field",
                span { class: "label",
                    bdi { class: "latin", "{st(lang, S::QuickLookup)}" }
                    span { class: "pill settings-scope", "{st(lang, S::Later)}" }
                }
                p { class: "muted settings-hint", "{st(lang, S::QuickLookupBody)}" }
            }
        }
    }
}

/// Full sidebar or slim rail on wide windows.
#[component]
fn NavStyleField() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let prefs = use_context::<DevicePrefs>();
    let lang = ui.lang();
    let style = *prefs.nav_style.read();
    rsx! {
        div { class: "field",
            span { class: "label", "{st(lang, S::NavStyle)}" Scope { device_only: true } }
            div { class: "row", role: "group",
                for n in NavStyle::ALL {
                    button {
                        key: "{n.as_str()}",
                        aria_pressed: if n == style { "true" } else { "false" },
                        onclick: {
                            let db = db.clone();
                            move |_| prefs.set_nav_style(&db, n)
                        },
                        {st(lang, if n == NavStyle::Labels { S::NavLabels } else { S::NavIcons })}
                    }
                }
            }
        }
    }
}

#[component]
fn LayoutNow() -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    rsx! {
        div { class: "field",
            span { class: "label", "{st(lang, S::LayoutNow)}" }
            span { class: "pill", "{ui.t(width_label(*ui.width.read()))}" }
            p { class: "muted settings-hint", "{st(lang, S::LayoutBody)}" }
        }
    }
}

#[component]
pub fn StorageSection() -> Element {
    let ui = use_context::<Ui>();
    let dir = platform::data_dir().map(|d| d.display().to_string()).unwrap_or_default();
    rsx! {
        crate::packs::settings::OfflinePacks {}
        crate::data_export::ExportCard {}
        div { class: "card",
            span { class: "label", "{st(ui.lang(), S::DataFolder)}" }
            p { code { dir: "ltr", bdi { "{dir}" } } }
        }
    }
}

#[component]
pub fn AccountSection() -> Element {
    let ui = use_context::<Ui>();
    let nav = use_context::<SettingsNav>();
    rsx! {
        crate::account::AccountScreen {}
        div { class: "card",
            div { class: "row",
                button {
                    onclick: move |_| {
                        let mut s = ui.screen;
                        s.set(Screen::Cost);
                        nav.close();
                    },
                    span { class: "nav-icon", aria_hidden: "true", {crate::icons::nav(Screen::Cost)} }
                    " {ui.t(Msg::CostTitle)}"
                }
            }
        }
    }
}

#[component]
pub fn AboutSection() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let nav = use_context::<SettingsNav>();
    let lang = ui.lang();
    let device = db.0.borrow().device_id().to_string();
    let model = platform::device_model().unwrap_or_default();
    rsx! {
        div { class: "card",
            p { class: "about-name latin", bdi { lang: "id", dir: "ltr", "Kabar" } }
            p { class: "muted", "{st(lang, S::PersonalBuild)}" }
            div { class: "field",
                span { class: "label", "{st(lang, S::Version)}" }
                code { dir: "ltr", bdi { "{env!(\"CARGO_PKG_VERSION\")}" } }
            }
            div { class: "field",
                span { class: "label", "{ui.t(Msg::SettingsDevice)}" }
                if !model.is_empty() {
                    p { class: "settings-hint", bdi { dir: "ltr", "{model}" } }
                }
                code { dir: "ltr", bdi { "{device}" } }
            }
            LayoutNow {}
            button {
                onclick: {
                    let db = db.clone();
                    move |_| {
                        nav.close();
                        ui.update_settings(&db, |s| s.welcomed = false)
                    }
                },
                "{ui.t(Msg::SettingsShowWelcome)}"
            }
        }
    }
}

fn width_label(w: WidthClass) -> Msg {
    match w {
        WidthClass::Compact => Msg::WidthCompact,
        WidthClass::Medium => Msg::WidthMedium,
        WidthClass::Expanded => Msg::WidthExpanded,
        WidthClass::Large => Msg::WidthLarge,
        WidthClass::ExtraLarge => Msg::WidthExtraLarge,
    }
}
