//! Kabar: one Dioxus codebase for the Mac, the S24 Ultra and the Tab S11.

mod account;
mod cloud;
mod i18n;
mod icons;
mod penlab;
mod platform;
mod screens;
mod store;
mod theme;

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;
use kabar_core::settings::Settings;

use crate::account::Cloud;
use crate::cloud::{CloudStatus, Command, Event};
use crate::i18n::{Msg, t};
use crate::store::Store;

const MAIN_CSS: &str = include_str!("../assets/main.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Today,
    Journey,
    Reader,
    Review,
    Studio,
    Settings,
    PenLab,
    Devices,
}

impl Screen {
    pub fn label(self) -> Msg {
        match self {
            Self::Today => Msg::NavToday,
            Self::Journey => Msg::NavJourney,
            Self::Reader => Msg::NavReader,
            Self::Review => Msg::NavReview,
            Self::Studio => Msg::NavStudio,
            Self::Settings => Msg::NavSettings,
            Self::PenLab => Msg::NavPenLab,
            Self::Devices => Msg::NavDevices,
        }
    }

    /// Hidden from the phone's bottom bar; reachable from Settings there.
    fn secondary(self) -> bool {
        matches!(self, Self::Studio | Self::PenLab | Self::Devices)
    }
}

/// App-wide UI state, shared through context.
#[derive(Clone, Copy)]
pub struct Ui {
    pub settings: Signal<Settings>,
    pub width: Signal<WidthClass>,
    pub screen: Signal<Screen>,
}

/// The local database, shared through context.
#[derive(Clone)]
pub struct Db(pub Rc<RefCell<Store>>);

impl Ui {
    pub fn t(&self, msg: Msg) -> &'static str {
        t(self.settings.read().ui_language, msg)
    }

    /// Saves changed settings and updates the UI.
    pub fn update_settings(mut self, db: &Db, f: impl FnOnce(&mut Settings)) {
        let old = self.settings.read().clone();
        let mut new = old.clone();
        f(&mut new);
        if let Err(e) = db.0.borrow_mut().save_settings(&old, &new) {
            eprintln!("kabar: saving settings failed: {e}");
        }
        self.settings.set(new);
    }
}

fn main() {
    #[cfg(all(feature = "desktop", not(target_os = "android")))]
    {
        use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
        // KABAR_WINDOW_SIZE=412x900 previews the phone layout on the Mac.
        let (w, h) = std::env::var("KABAR_WINDOW_SIZE")
            .ok()
            .and_then(|v| {
                let (w, h) = v.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or((1100.0, 760.0));
        let window = WindowBuilder::new()
            .with_title("Kabar")
            .with_inner_size(LogicalSize::new(w, h))
            .with_min_inner_size(LogicalSize::new(360.0, 560.0));
        dioxus::LaunchBuilder::desktop()
            .with_cfg(Config::new().with_window(window).with_menu(None).with_disable_context_menu(true))
            .launch(App);
    }
    #[cfg(not(all(feature = "desktop", not(target_os = "android"))))]
    dioxus::launch(App);
}

fn open_store() -> Result<(Store, std::path::PathBuf), String> {
    let dir = platform::data_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("kabar.db");
    Ok((Store::open(&path).map_err(|e| e.to_string())?, path))
}

#[component]
fn App() -> Element {
    let store = use_hook(|| open_store().map(|(s, path)| (Db(Rc::new(RefCell::new(s))), path)));
    match store {
        Ok((db, path)) => rsx! { Loaded { db, path } },
        Err(e) => rsx! {
            div { class: "app", dir: "ltr",
                style { {MAIN_CSS} }
                div { class: "content",
                    div { class: "card",
                        h2 { "Kabar couldn't open its local database" }
                        p { class: "mono", "{e}" }
                    }
                }
            }
        },
    }
}

impl PartialEq for Db {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[component]
fn Loaded(db: Db, path: std::path::PathBuf) -> Element {
    let initial = use_hook(|| db.0.borrow().settings().unwrap_or_default());
    let ui = use_context_provider(|| Ui {
        settings: Signal::new(initial),
        width: Signal::new(WidthClass::Expanded),
        screen: Signal::new(Screen::Today),
    });
    use_context_provider(|| db.clone());
    use_width_tracking(ui.width);
    use_cloud(&db, path, ui);

    let settings = ui.settings.read();
    let lang = settings.ui_language;
    let theme = settings.theme.as_str();
    let width = ui.width.read().as_str();
    let batik = use_hook(|| theme::kawung_tile("2b3a70", 0.06, 72));

    rsx! {
        document::Title { "Kabar" }
        style { {MAIN_CSS} }
        div {
            class: "app",
            lang: lang.tag(),
            dir: lang.dir().as_str(),
            "data-theme": theme,
            "data-width": width,
            style: "--batik: {batik}",
            if settings.welcomed {
                Shell {}
            } else {
                screens::Welcome {}
            }
        }
    }
}

/// Starts Kabar Cloud sync (when this build knows a server) and mirrors its
/// events into signals.
fn use_cloud(db: &Db, path: std::path::PathBuf, mut ui: Ui) {
    let device_id = db.0.borrow().device_id().clone();
    let cloud = use_context_provider(|| Cloud {
        status: Signal::new(CloudStatus::not_configured(device_id.clone())),
        devices: Signal::new(None),
        signing_in: Signal::new(false),
        revision: Signal::new(0),
        commands: Signal::new(None),
    });
    let db = db.clone();
    use_hook(move || {
        let Some(config) = cloud::CloudConfig::from_env() else { return };
        let device = kabar_protocol::cloud::DeviceInfo {
            device_id,
            name: platform::device_name().unwrap_or_else(|| "Kabar".into()),
            model: platform::device_model(),
            platform: if cfg!(target_os = "android") {
                kabar_protocol::cloud::Platform::Android
            } else if cfg!(target_os = "macos") {
                kabar_protocol::cloud::Platform::Macos
            } else {
                kabar_protocol::cloud::Platform::Other
            },
        };
        let vault = Box::new(cloud::session::KeyringVault::new());
        let mut handle = match cloud::start(config, path, device, vault) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("kabar: sync didn't start: {e}");
                return;
            }
        };
        let mut commands = cloud.commands;
        commands.set(Some(handle.commands.clone()));
        spawn(async move {
            let (mut status, mut devices, mut signing_in, mut revision) =
                (cloud.status, cloud.devices, cloud.signing_in, cloud.revision);
            while let Some(event) = handle.events.recv().await {
                match event {
                    Event::Status(s) => {
                        if *status.peek() != s {
                            status.set(s);
                        }
                    }
                    Event::Devices(d) => devices.set(Some(d)),
                    Event::SigningIn(on) => signing_in.set(on),
                    Event::Changed(tables) => {
                        if tables.contains("settings")
                            && let Ok(s) = db.0.borrow().settings()
                        {
                            ui.settings.set(s);
                        }
                        revision += 1;
                    }
                }
            }
        });
    });
    // Local settings writes are in the outbox: push them.
    use_effect(move || {
        ui.settings.read();
        cloud.send(Command::Kick);
    });
}

/// Mirrors the window width into `width`, so layout follows the window,
/// not the device (split screen, DeX and Mac windows all just work).
fn use_width_tracking(mut width: Signal<WidthClass>) {
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            const send = () => dioxus.send(window.innerWidth);
            window.addEventListener("resize", send);
            send();
            await new Promise(() => {});
            "#,
        );
        while let Ok(w) = eval.recv::<f64>().await {
            let class = WidthClass::from_width(w);
            if *width.peek() != class {
                width.set(class);
            }
        }
    });
}

#[component]
fn Shell() -> Element {
    let ui = use_context::<Ui>();
    let current = *ui.screen.read();
    let items = [
        Screen::Today,
        Screen::Journey,
        Screen::Reader,
        Screen::Review,
        Screen::Studio,
        Screen::PenLab,
        Screen::Devices,
        Screen::Settings,
    ];

    rsx! {
        div { class: "shell",
            nav { class: "nav", aria_label: "Kabar",
                div { class: "brand latin", "Kabar" }
                for item in items {
                    button {
                        key: "{item:?}",
                        class: if item.secondary() { "nav-item secondary" } else { "nav-item" },
                        aria_current: if item == current { "page" } else { "false" },
                        onclick: move |_| {
                            let mut screen = ui.screen;
                            screen.set(item);
                        },
                        span { class: "nav-icon", aria_hidden: "true", {icons::nav(item)} }
                        span { "{ui.t(item.label())}" }
                    }
                }
            }
            main { class: "content",
                match current {
                    Screen::Today => rsx! { screens::Today {} },
                    Screen::Journey => rsx! { screens::ComingSoon { title: Msg::NavJourney, milestone: "M8" } },
                    Screen::Reader => rsx! { screens::ComingSoon { title: Msg::NavReader, milestone: "M5" } },
                    Screen::Review => rsx! { screens::ComingSoon { title: Msg::NavReview, milestone: "M6" } },
                    Screen::Studio => rsx! { screens::ComingSoon { title: Msg::NavStudio, milestone: "M9" } },
                    Screen::Settings => rsx! { screens::SettingsScreen {} },
                    Screen::PenLab => rsx! { penlab::PenLab {} },
                    Screen::Devices => rsx! { account::AccountScreen {} },
                }
            }
        }
    }
}
