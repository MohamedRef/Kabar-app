//! Kabar: one Dioxus codebase for the Mac, the S24 Ultra and the Tab S11.
//!
//! The same code is the web app (`web/`, docs/WEB.md): built for wasm32, the
//! browser stands in for the OS (`platform::web`).

// The web build leaves out the Mac/Android-only paths (the sync thread,
// updates over the air, files), so some of their helpers go unused there.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports, unused_variables, unused_mut))]

mod account;
mod ai;
mod annotated;
mod app_hooks;
mod audio;
mod back;
mod chat;
mod cloud;
mod coach;
mod cost;
mod data_export;
mod dead_ends;
mod diag;
mod embedded;
mod focus;
mod fonts;
mod i18n;
mod icons;
mod journey;
mod lesson;
mod listen;
mod map_art;
mod misc_art;
mod more;
mod packs;
mod penlab;
mod persistence;
mod platform;
mod play;
mod portraits;
mod pron;
mod reader;
mod review;
mod schedule;
mod screen;
#[cfg(test)]
mod screen_kit;
mod screens;
mod settings;
mod sheet;
mod shell;
mod sounds;
mod souvenir_art;
mod state;
mod stats;
mod store;
mod studio;
mod teach;
#[cfg(test)]
mod test_dom;
#[cfg(test)]
mod test_screens;
#[cfg(test)]
mod testkit;
mod theme;
mod ui;
mod update;
mod version;
mod views;

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;

use crate::account::Cloud;
use crate::cloud::{CloudStatus, Command, Event};
use crate::coach::Coach;
use crate::embedded::{Css, css};
use crate::i18n::Msg;
use crate::store::Store;

pub use screen::Screen;
pub use state::{Db, Ui};

use app_hooks::*;
use shell::Shell;

const MAIN_CSS: Css = css!("assets/main.css");
/// Shared components and the Glossy layer (docs/DESIGN.md § Components).
const COMPONENTS_CSS: Css = css!("assets/components.css");
const LIQUID_GLASS_CSS: Css = css!("assets/liquid-glass.css");

fn main() {
    diag::install_panic_hook();
    // Automatic updates on the Mac: a verified download from the last
    // session goes in before the window opens (and Kabar reopens).
    #[cfg(target_os = "macos")]
    update::apply_staged_on_launch();
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
        // The Mac gets its menu bar (Kabar › Settings… ⌘,); the Linux dev
        // window stays bare so previews match the Mac's content area.
        #[cfg(target_os = "macos")]
        let menu = Some(settings::menu::menu_bar());
        #[cfg(not(target_os = "macos"))]
        let menu = None;
        dioxus::LaunchBuilder::desktop()
            .with_cfg(Config::new().with_window(window).with_menu(menu).with_disable_context_menu(true))
            .launch(App);
    }
    // The web: the kept database is read before the first screen.
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async {
        platform::web::boot().await;
        dioxus::launch(App);
    });
    #[cfg(not(any(all(feature = "desktop", not(target_os = "android")), target_arch = "wasm32")))]
    dioxus::launch(App);
}

/// The web: the database the browser kept, in memory (`platform::web`). A
/// snapshot this build can't read stays in the browser untouched; this
/// session then runs on a fresh store and doesn't save over it.
#[cfg(target_arch = "wasm32")]
fn open_store() -> Result<(Store, std::path::PathBuf), String> {
    let store = Store::from_snapshot(&platform::web::take_snapshot()).or_else(|e| {
        crate::diag::report!("the browser's copy didn't open: {e}");
        platform::web::stop_saving(platform::web::Trouble::Failed);
        Store::open_in_memory()
    });
    Ok((store.map_err(|e| e.to_string())?, std::path::PathBuf::from("kabar.db")))
}

#[cfg(not(target_arch = "wasm32"))]
fn open_store() -> Result<(Store, std::path::PathBuf), String> {
    let dir = platform::data_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("kabar.db");
    // A content bundle from the update feed replaces the built-in content
    // before anything reads it (update/content.rs).
    update::content::use_installed(&dir);
    packs::init(&dir);
    update::init(&dir);
    let marker = platform::install_marker();
    Ok((Store::open_marked(&path, marker.as_deref()).map_err(|e| e.to_string())?, path))
}

#[component]
fn App() -> Element {
    let store = use_hook(|| open_store().map(|(s, path)| (Db(Rc::new(RefCell::new(s))), path)));
    match store {
        Ok((db, path)) => rsx! { Loaded { db, path } },
        Err(e) => rsx! {
            div { class: "app", dir: "ltr",
                style { {MAIN_CSS.text()} }
                div { class: "content",
                    crate::ui::Card { element: crate::ui::CardElement::Div, class: "",
                        h2 { "Kabar couldn't open its local database" }
                        p { class: "mono", "{e}" }
                    }
                }
            }
        },
    }
}

#[component]
fn Loaded(db: Db, path: std::path::PathBuf) -> Element {
    let initial = use_hook(|| db.0.borrow().settings().unwrap_or_default());
    // KABAR_OPEN_LESSON=bipa1.u01.l01 opens the Lesson Player at launch
    // (previews and screenshots on the Mac).
    let open_lesson = use_hook(|| std::env::var("KABAR_OPEN_LESSON").ok());
    // KABAR_OPEN_SCREEN=stats | level_check | journey | reader | review | play | listen | chat
    // | coach | journal | studio | teach | cost | devices opens those screens the same way (previews and screenshots).
    let open_screen = use_hook(|| match std::env::var("KABAR_OPEN_SCREEN").as_deref() {
        Ok("stats") => Some(Screen::Stats),
        Ok("level_check") => Some(Screen::LevelCheck),
        Ok("journey") => Some(Screen::Journey),
        Ok("reader") => Some(Screen::Reader),
        Ok("review") => Some(Screen::Review),
        Ok("play") => Some(Screen::Play),
        Ok("listen") => Some(Screen::Listen),
        Ok("chat") => Some(Screen::Chat),
        Ok("coach") => Some(Screen::Coach),
        Ok("journal") => Some(Screen::Journal),
        Ok("studio") => Some(Screen::Studio),
        Ok("teach") => Some(Screen::Teach),
        Ok("cost") => Some(Screen::Cost),
        Ok("devices") => Some(Screen::Devices),
        _ => None,
    });
    let ui = use_context_provider(|| Ui {
        settings: Signal::new(initial),
        width: Signal::new(WidthClass::Expanded),
        tablet: Signal::new(false),
        screen: Signal::new(match (&open_lesson, open_screen) {
            (Some(_), _) => Screen::Lesson,
            (None, Some(s)) => s,
            (None, None) => Screen::Today,
        }),
        lesson: Signal::new(open_lesson),
        lesson_from: Signal::new(Screen::Journey),
        focus: Signal::new(false),
    });
    use_context_provider(|| db.clone());
    persistence::use_provide_saves();
    #[cfg(target_arch = "wasm32")]
    platform::web::use_saver(db.clone());
    let prefs = use_context_provider(|| settings::device::DevicePrefs::load(&db));
    use_context_provider(settings::SettingsNav::new);
    use_context_provider(update::sheet::UpdateSheetNav::new);
    use_context_provider(more::MoreNav::new);
    use_context_provider(play::PlayNav::new);
    use_context_provider(|| teach::TeachInbox(Signal::new(None)));
    let link = use_hook(cloud::link::CloudLink::default);
    use_context_provider(|| link.clone());
    use_context_provider(|| teach::Ai(Rc::new(ai::router(db.clone(), link.clone()))));
    let audio_dir = path.parent().map(|d| d.join("audio")).unwrap_or_else(|| "audio".into());
    use_context_provider(|| audio::AudioCache::new(audio_dir, link.clone()));
    let coach = use_context_provider(|| Coach::load(&db));
    annotated::use_words(&db);
    use_width_tracking(ui.width, ui.tablet);
    sheet::use_gestures();
    use_cloud(&db, path, ui, link);
    coach::use_clock(coach, db.clone());
    schedule::use_optimizer(db.clone(), ui, coach);
    use_learner_profile(&db, coach);
    coach::notify::use_local_reminders(&db, coach, ui, prefs);
    coach::notify::use_push_token(ui, prefs);
    // Android: a renderer reclaimed in the background ends Kabar quietly
    // instead of crashing it; earlier crashes show in the diagnostics.
    use_hook(platform::guard_webview);
    let inbox = use_context_provider(|| reader::Inbox(Signal::new(None)));
    use_context_provider(|| reader::ReaderState::new(ui.settings.peek().ui_language));
    use_share_inbox(ui, inbox);
    let device_dark = use_device_dark(ui);
    // Android's status and navigation bars take the theme's background, so
    // True black keeps the whole OLED screen dark.
    use_effect(move || {
        device_dark.read();
        let (day, night) = ui.settings.read().theme.day_and_night();
        platform::system_bars(day.bg, night.bg);
    });
    // The web page's own root says lang="ar" dir="rtl" for the boot screen;
    // follow the learner's language so English isn't laid out (or read
    // aloud) as Arabic.
    #[cfg(target_arch = "wasm32")]
    use_effect(move || {
        let lang = ui.settings.read().ui_language;
        document::eval(&format!(
            "document.documentElement.lang={:?};document.documentElement.dir={:?};",
            lang.tag(),
            lang.dir().as_str()
        ));
    });

    let settings = ui.settings.read();
    let lang = settings.ui_language;
    let theme = settings.theme.resolved(*device_dark.read()).as_str();
    let width = ui.width.read().as_str();
    let theme_css = use_hook(theme::stylesheet);
    let pron_font = use_hook(pron::font_css);
    let ui_fonts = use_hook(fonts::css);
    // The theme draws the kawung background in its own colour; a batik from
    // the Warung overrides it on this device.
    let batik = use_memo(move || {
        let chosen = coach.batik.read().as_deref().is_some_and(|id| !id.is_empty());
        if chosen {
            format!("--batik: {}", theme::kawung_tile(coach.batik_color(), 0.06, 72))
        } else {
            String::new()
        }
    });
    let onboarded = coach.profile.read().is_some();
    let text_size = prefs.text_size.read().as_str();
    let nav_style = prefs.nav_style.read().as_str();
    let appearance = prefs.appearance.read().as_str();
    let on_off = |on: bool| if on { "on" } else { "off" };
    let pen_hover = on_off(*prefs.pen_hover.read());
    let mouse_hover = on_off(*prefs.mouse_hover.read());
    // One switch for word colours everywhere: main.css neutralizes the
    // .wc-* / .wp-* colours under data-word-colours="off".
    let word_colours = on_off(!settings.word_colours_off);

    rsx! {
        document::Title { "Kabar" }
        style { {MAIN_CSS.text()} }
        style { {COMPONENTS_CSS.text()} }
        style { {theme_css} }
        style { {pron_font} }
        style { {annotated::CSS.text()} }
        style { {reader::grammar::CSS.text()} }
        style { {sheet::CSS.text()} }
        style { {LIQUID_GLASS_CSS.text()} }
        style { {ui_fonts} }
        style { {misc_art::paper_css()} }
        div {
            class: "app",
            lang: lang.tag(),
            dir: lang.dir().as_str(),
            "data-theme": theme,
            "data-appearance": appearance,
            "data-focus": if *ui.focus.read() { "true" } else { "false" },
            "data-width": width,
            "data-text-size": text_size,
            "data-nav": nav_style,
            "data-pen-hover": pen_hover,
            "data-mouse-hover": mouse_hover,
            "data-word-colours": word_colours,
            style: "{batik}",
            if cfg!(target_arch = "wasm32") {
                WebNotice {}
            }
            if !settings.welcomed {
                persistence::SaveNotice {}
                screens::Welcome {}
            } else if !onboarded {
                persistence::SaveNotice {}
                coach::onboarding::Onboarding {}
            } else {
                Shell {}
            }
        }
    }
}

/// The web: a line on top while this tab can't save (`platform::web`).
#[component]
fn WebNotice() -> Element {
    #[cfg(target_arch = "wasm32")]
    return rsx! { platform::web::StorageNotice {} };
    #[cfg(not(target_arch = "wasm32"))]
    rsx! {}
}
