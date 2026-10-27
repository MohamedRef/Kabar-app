//! Kabar: one Dioxus codebase for the Mac, the S24 Ultra and the Tab S11.

mod account;
mod ai;
mod annotated;
mod audio;
mod back;
mod chat;
mod cloud;
mod coach;
mod cost;
mod data_export;
mod diag;
mod embedded;
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
mod platform;
mod play;
mod pron;
mod reader;
mod review;
mod schedule;
mod screens;
mod settings;
mod sounds;
mod souvenir_art;
mod stats;
mod store;
mod studio;
mod teach;
mod theme;
mod update;

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;
use kabar_core::locale::{UiLanguage, format_number, format_percent, localize_digits, uses_eastern_digits};
use kabar_core::settings::Settings;

use crate::account::Cloud;
use crate::cloud::{CloudStatus, Command, Event};
use crate::coach::Coach;
use crate::i18n::{Msg, t};
use crate::store::Store;

const MAIN_CSS: &str = include_str!("../assets/main.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Today,
    Journey,
    Reader,
    Review,
    Coach,
    Play,
    Studio,
    Settings,
    PenLab,
    Devices,
    /// The Lesson Player, full screen; the lesson is `Ui::lesson`.
    Lesson,
    /// Teach Kabar a word (spec §26).
    Teach,
    /// The learning journal: the timeline and reflections (spec §6).
    Journal,
    /// Cost & limits (spec §19), from Settings.
    Cost,
    /// Stats (spec §16): from the Review home and Today.
    Stats,
    /// The monthly level check (spec §6).
    LevelCheck,
    /// Sinetron and Kabar Radio.
    Listen,
    /// Persona chats (spec §15).
    Chat,
}

impl Screen {
    pub fn label(self) -> Msg {
        match self {
            Self::Today => Msg::NavToday,
            Self::Journey => Msg::NavJourney,
            Self::Reader => Msg::NavReader,
            Self::Review => Msg::NavReview,
            Self::Coach => Msg::NavCoach,
            Self::Play => Msg::NavPlay,
            Self::Studio => Msg::NavStudio,
            Self::Settings => Msg::NavSettings,
            Self::PenLab => Msg::NavPenLab,
            Self::Devices => Msg::NavDevices,
            Self::Lesson => Msg::NavJourney,
            Self::Teach => Msg::NavTeach,
            Self::Journal => Msg::NavJournal,
            Self::Cost => Msg::CostTitle,
            Self::Stats => Msg::NavReview,
            Self::LevelCheck => Msg::NavCoach,
            Self::Listen => Msg::NavListen,
            Self::Chat => Msg::NavChat,
        }
    }

    /// Hidden from the phone's bottom bar, which keeps five items within
    /// thumb reach. Kak Nur, the journal, Watch & listen, Chat and Teach
    /// Kabar are linked from Today there (the journal from Kak Nur too); Settings is the gear
    /// in the phone's top bar.
    /// Pen tools: shown on a tablet only (`Ui::tablet`).
    pub fn pen_only(self) -> bool {
        matches!(self, Self::Studio | Self::PenLab)
    }

    fn secondary(self) -> bool {
        matches!(
            self,
            Self::Play
                | Self::Coach
                | Self::Journal
                | Self::Settings
                | Self::Studio
                | Self::PenLab
                | Self::Devices
                | Self::Teach
                | Self::Cost
                | Self::Listen
                | Self::Chat
        )
    }
}

/// App-wide UI state, shared through context.
#[derive(Clone, Copy)]
pub struct Ui {
    pub settings: Signal<Settings>,
    pub width: Signal<WidthClass>,
    /// An Android tablet (smallest screen side ≥ 600 dp): the only place
    /// the pen tools (Studio, Pen Lab) apply while phone and Mac pen work
    /// is deferred (SPEC §23). Fixed per device, unlike `width`.
    pub tablet: Signal<bool>,
    pub screen: Signal<Screen>,
    /// The lesson the player shows (curriculum lesson id).
    pub lesson: Signal<Option<String>>,
    /// The screen the player was opened from; closing it goes back there.
    pub lesson_from: Signal<Screen>,
}

/// The local database, shared through context.
#[derive(Clone)]
pub struct Db(pub Rc<RefCell<Store>>);

impl Ui {
    pub fn t(&self, msg: Msg) -> &'static str {
        t(self.settings.read().ui_language, msg)
    }

    pub fn lang(&self) -> UiLanguage {
        self.settings.read().ui_language
    }

    /// Whether numbers use Eastern Arabic digits right now.
    pub fn eastern(&self) -> bool {
        let s = self.settings.read();
        uses_eastern_digits(s.ui_language, s.eastern_digits)
    }

    /// A number in the UI's digits.
    pub fn num(&self, n: impl Into<u64>) -> String {
        let s = self.settings.read();
        format_number(n.into(), s.ui_language, s.eastern_digits)
    }

    /// Text with its digits in the UI's digits ("3:05", "1.4 MB").
    pub fn digits(&self, text: &str) -> String {
        let s = self.settings.read();
        localize_digits(text, s.ui_language, s.eastern_digits)
    }

    /// A share (0.0–1.0) as a percentage in the UI's digits: "87%", "٨٧٪".
    pub fn percent(&self, share: f32) -> String {
        self.whole_percent((share.clamp(0.0, 1.0) * 100.0).round() as u64)
    }

    /// A whole percentage (87) in the UI's digits: "87%", "٨٧٪".
    pub fn whole_percent(&self, n: impl Into<u64>) -> String {
        let s = self.settings.read();
        format_percent(n.into(), s.ui_language, s.eastern_digits)
    }

    /// Saves changed settings and updates the UI.
    pub fn update_settings(mut self, db: &Db, f: impl FnOnce(&mut Settings)) {
        let old = self.settings.read().clone();
        let mut new = old.clone();
        f(&mut new);
        if let Err(e) = db.0.borrow_mut().save_settings(&old, &new) {
            crate::diag::report!("saving settings failed: {e}");
        }
        self.settings.set(new);
    }

    /// Opens the Lesson Player on a lesson (from the Journey Map, Today or Lanjut).
    pub fn open_lesson(mut self, lesson_id: impl Into<String>) {
        let from = *self.screen.peek();
        if from != Screen::Lesson {
            self.lesson_from.set(from);
        }
        self.lesson.set(Some(lesson_id.into()));
        self.screen.set(Screen::Lesson);
    }

    /// Leaves the Lesson Player back to the screen it was opened from.
    pub fn close_lesson(mut self) {
        let back = *self.lesson_from.peek();
        self.lesson.set(None);
        self.screen.set(back);
    }
}

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
    #[cfg(not(all(feature = "desktop", not(target_os = "android"))))]
    dioxus::launch(App);
}

fn open_store() -> Result<(Store, std::path::PathBuf), String> {
    let dir = platform::data_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("kabar.db");
    packs::init(&dir);
    update::init(&dir);
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
    // KABAR_OPEN_LESSON=bipa1.u01.l01 opens the Lesson Player at launch
    // (previews and screenshots on the Mac).
    let open_lesson = use_hook(|| std::env::var("KABAR_OPEN_LESSON").ok());
    // KABAR_OPEN_SCREEN=stats | level_check | journey | reader | review | play | listen | chat
    // | coach | journal opens those screens the same way (previews and screenshots).
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
    });
    use_context_provider(|| db.clone());
    let prefs = use_context_provider(|| settings::device::DevicePrefs::load(&db));
    use_context_provider(settings::SettingsNav::new);
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
    use_cloud(&db, path, ui, link);
    coach::use_clock(coach, db.clone());
    schedule::use_optimizer(db.clone(), ui, coach);
    use_learner_profile(&db, coach);
    coach::notify::use_local_reminders(&db, coach, ui, prefs);
    let inbox = use_context_provider(|| reader::Inbox(Signal::new(None)));
    use_context_provider(|| reader::ReaderState::new(ui.settings.peek().ui_language));
    use_share_inbox(ui, inbox);

    let settings = ui.settings.read();
    let lang = settings.ui_language;
    let theme = settings.theme.as_str();
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
    let on_off = |on: bool| if on { "on" } else { "off" };
    let pen_hover = on_off(*prefs.pen_hover.read());
    let mouse_hover = on_off(*prefs.mouse_hover.read());
    // One switch for word colours everywhere: main.css neutralizes the
    // .wc-* / .wp-* colours under data-word-colours="off".
    let word_colours = on_off(!settings.word_colours_off);

    rsx! {
        document::Title { "Kabar" }
        style { {MAIN_CSS} }
        style { {theme_css} }
        style { {pron_font} }
        style { {annotated::CSS} }
        style { {reader::grammar::CSS} }
        style { {ui_fonts} }
        style { {misc_art::paper_css()} }
        div {
            class: "app",
            lang: lang.tag(),
            dir: lang.dir().as_str(),
            "data-theme": theme,
            "data-width": width,
            "data-text-size": text_size,
            "data-nav": nav_style,
            "data-pen-hover": pen_hover,
            "data-mouse-hover": mouse_hover,
            "data-word-colours": word_colours,
            style: "{batik}",
            if !settings.welcomed {
                screens::Welcome {}
            } else if !onboarded {
                coach::onboarding::Onboarding {}
            } else {
                Shell {}
            }
        }
    }
}

/// Starts Kabar Cloud sync (when this build knows a server) and mirrors its
/// events into signals.
fn use_cloud(db: &Db, path: std::path::PathBuf, mut ui: Ui, link: cloud::link::CloudLink) {
    let device_id = db.0.borrow().device_id().clone();
    let cloud = use_context_provider(|| Cloud {
        status: Signal::new(CloudStatus::not_configured(device_id.clone())),
        devices: Signal::new(None),
        usage: Signal::new(None),
        signing_in: Signal::new(false),
        revision: Signal::new(0),
        commands: Signal::new(None),
    });
    let db = db.clone();
    let coach = try_use_context::<Coach>();
    let words = use_context::<annotated::Words>();
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
                crate::diag::report!("sync didn't start: {e}");
                return;
            }
        };
        let mut commands = cloud.commands;
        commands.set(Some(handle.commands.clone()));
        link.connect(handle.commands.clone());
        spawn(async move {
            let (mut status, mut devices, mut signing_in, mut revision) =
                (cloud.status, cloud.devices, cloud.signing_in, cloud.revision);
            let mut usage = cloud.usage;
            while let Some(event) = handle.events.recv().await {
                match event {
                    Event::Status(s) => {
                        if *status.peek() != s {
                            status.set(s);
                        }
                    }
                    Event::Devices(d) => devices.set(Some(d)),
                    Event::Usage(u) => usage.set(Some(u)),
                    Event::SigningIn(on) => signing_in.set(on),
                    Event::Changed(tables) => {
                        let store = db.0.borrow();
                        if tables.contains("settings")
                            && let Ok(s) = store.settings()
                        {
                            ui.settings.set(s);
                        }
                        // Kak Nur's rows: goal met on any device counts here too.
                        if let Some(mut coach) = coach
                            && ["goals", "daily_plans", "xp_events", "reminders", "reports"]
                                .iter()
                                .any(|t| tables.contains(*t))
                        {
                            coach.profile.set(store.coach_profile().ok().flatten());
                            coach.events.set(store.activity(None).unwrap_or_default());
                            coach.reminders.set(store.reminder_state().unwrap_or_default());
                            coach.plan_rev += 1;
                        }
                        if tables.contains("taught_entries") {
                            drop(store);
                            words.refresh(&db);
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
/// Keeps the AI prompts' learner profile current: it follows the activity
/// log (lessons, reviews, practice) and the onboarding answers. The minute
/// clock is only peeked, so the deck isn't re-read every minute.
fn use_learner_profile(db: &Db, coach: Coach) {
    let ai = use_context::<teach::Ai>();
    let db = db.clone();
    use_effect(move || {
        let events = coach.events.read();
        let profile = coach.profile.read();
        let offset = profile
            .as_ref()
            .map(|p| p.utc_offset_min)
            .or(*coach.device_offset.peek())
            .unwrap_or(kabar_protocol::coach::CoachProfile::default().utc_offset_min);
        let today = coach::clock::today(*coach.now_ms.peek(), offset);
        ai.0.set_profile(ai::learner_profile(&db, profile.as_ref(), &events, today));
    });
}

fn use_width_tracking(mut width: Signal<WidthClass>, mut tablet: Signal<bool>) {
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            const send = () => dioxus.send([window.innerWidth, Math.min(screen.width, screen.height)]);
            window.addEventListener("resize", send);
            send();
            await new Promise(() => {});
            "#,
        );
        while let Ok((w, smallest)) = eval.recv::<(f64, f64)>().await {
            let class = WidthClass::from_width(w);
            if *width.peek() != class {
                width.set(class);
            }
            let is_tablet = cfg!(target_os = "android") && smallest >= 600.0;
            if *tablet.peek() != is_tablet {
                tablet.set(is_tablet);
            }
        }
    });
}

/// Follows the window to the back and front: sync sleeps in the background,
/// and text or images shared to Kabar (Android share sheet) open the Reader
/// at start and whenever the window comes back.
fn use_share_inbox(ui: Ui, inbox: reader::Inbox) {
    let cloud = use_context::<Cloud>();
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            const send = () => dioxus.send(!document.hidden);
            document.addEventListener("visibilitychange", send);
            window.addEventListener("focus", send);
            send();
            await new Promise(() => {});
            "#,
        );
        let mut shown = true;
        while let Ok(visible) = eval.recv::<bool>().await {
            if visible != shown {
                shown = visible;
                cloud.send(Command::Background(!visible));
            }
            if !visible {
                continue;
            }
            if let Some(input) = platform::take_shared_input() {
                let (mut slot, mut screen) = (inbox.0, ui.screen);
                slot.set(Some(input));
                screen.set(Screen::Reader);
            }
        }
    });
}

#[component]
fn Shell() -> Element {
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
    back::use_back(ui, current, settings_open);
    update::view::use_android_install(ui);
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
        (Msg::NavGroupMe, &[Screen::Coach, Screen::Journal][..]),
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
                            let mut screen = ui.screen;
                            screen.set(item);
                        },
                        span { class: "nav-icon", aria_hidden: "true", {icons::nav(item)} }
                        span { class: "nav-label",
                            "{more::rail_label(ui.lang(), item).unwrap_or(ui.t(item.label()))}"
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
                    Screen::Devices => rsx! { account::AccountScreen {} },
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
        }
    }
}
