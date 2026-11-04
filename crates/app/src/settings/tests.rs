//! Settings as the learner meets it: each section rendered in Arabic and
//! English, and taps that change a preference, checked in the stored
//! settings and on screen.

use super::*;
use crate::cloud::{CloudStatus, Command, Problem, SyncState};
use crate::screen_kit::Harness;
use crate::{Cloud, Db, Screen};
use kabar_core::locale::UiLanguage;
use kabar_core::settings::Settings;
use kabar_protocol::cloud::DeviceSummary;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

#[derive(Clone)]
struct Case {
    width: u32,
    tablet: bool,
    section: Option<Section>,
    settings: Settings,
    cloud: Option<CloudStatus>,
    devices: Option<Result<Vec<DeviceSummary>, Problem>>,
    signing_in: bool,
    profile: Option<&'static str>,
    /// Render this instead of the sheet (sections this platform hides).
    body: Option<fn() -> Element>,
}

impl Case {
    fn new(lang: UiLanguage, section: Option<Section>) -> Self {
        Self {
            width: 1280,
            tablet: false,
            section,
            settings: Settings { ui_language: lang, ..Settings::default() },
            cloud: None,
            devices: None,
            signing_in: false,
            profile: None,
            body: None,
        }
    }

    fn body(mut self, body: fn() -> Element) -> Self {
        self.body = Some(body);
        self
    }

    fn phone(mut self) -> Self {
        self.width = 400;
        self
    }
}

thread_local! {
    static CASE: RefCell<Option<Case>> = const { RefCell::new(None) };
    static SENT: RefCell<Option<UnboundedReceiver<Command>>> = const { RefCell::new(None) };
}

fn fixture() -> Element {
    let case = CASE.with(|c| c.borrow().clone().unwrap());
    crate::persistence::use_provide_saves();
    let db =
        use_context_provider(|| Db(Rc::new(RefCell::new(crate::store::Store::open_in_memory().unwrap()))));
    let ui = use_context_provider(|| Ui {
        settings: Signal::new(case.settings.clone()),
        width: Signal::new(WidthClass::from_width(case.width.into())),
        tablet: Signal::new(case.tablet),
        screen: Signal::new(Screen::Today),
        lesson: Signal::new(None),
        lesson_from: Signal::new(Screen::Journey),
        focus: Signal::new(false),
    });
    use_context_provider(|| device::DevicePrefs::load(&db));
    let coach = use_context_provider(|| crate::coach::Coach::load(&db));
    if let Some(name) = case.profile {
        use_hook(move || {
            let mut profile = coach.profile;
            let mut p = profile.peek().clone().unwrap_or_default();
            p.name = name.to_string();
            profile.set(Some(p));
        });
    }
    if let Some(status) = case.cloud.clone() {
        use_context_provider(move || {
            let (tx, rx) = unbounded_channel();
            SENT.with(|s| *s.borrow_mut() = Some(rx));
            Cloud {
                status: Signal::new(status),
                devices: Signal::new(case.devices.clone()),
                usage: Signal::new(None),
                signing_in: Signal::new(case.signing_in),
                login: Signal::new(Default::default()),
                revision: Signal::new(0),
                commands: Signal::new(Some(tx)),
                push: Signal::new(false),
                push_sent: Signal::new(None),
            }
        });
    }
    use_context_provider(|| SettingsNav {
        open: Signal::new(true),
        section: Signal::new(case.section),
        query: Signal::new(String::new()),
    });
    rsx! {
        div { class: "app", lang: ui.lang().tag(), dir: ui.lang().dir().as_str(),
            if let Some(body) = case.body {
                {body()}
            } else {
                SettingsSheet {}
            }
        }
    }
}

fn open(case: Case) -> Harness {
    CASE.with(|c| *c.borrow_mut() = Some(case));
    Harness::new(fixture)
}

fn sent() -> Vec<String> {
    SENT.with(|s| {
        let mut out = Vec::new();
        if let Some(rx) = s.borrow_mut().as_mut() {
            while let Ok(c) = rx.try_recv() {
                out.push(format!("{c:?}"));
            }
        }
        out
    })
}

fn settings(h: &Harness) -> Settings {
    h.read(|| consume_context::<Ui>().settings.read().clone())
}

fn stored(h: &Harness) -> Settings {
    h.read(|| consume_context::<Db>().0.borrow().settings().unwrap())
}

fn prefs(h: &Harness) -> device::DevicePrefs {
    h.read(consume_context::<device::DevicePrefs>)
}

fn local(h: &Harness, key: &str) -> Option<String> {
    h.read(|| consume_context::<Db>().0.borrow().local_value(key).unwrap())
}

fn nav(h: &Harness) -> (bool, Option<Section>) {
    h.read(|| {
        let n = consume_context::<SettingsNav>();
        (*n.open.peek(), *n.section.peek())
    })
}

/// Text as SSR escapes it.
fn esc(text: &str) -> String {
    text.replace('&', "&#38;").replace('\'', "&#39;").replace('"', "&#34;")
}

fn signed_in(sync: SyncState) -> CloudStatus {
    CloudStatus {
        configured: true,
        email: Some("learner@example.com".into()),
        sync,
        pending: 3,
        last_sync_ms: Some(crate::store::now_ms() - 5 * 60_000),
        ..CloudStatus::not_configured(kabar_protocol::DeviceId("dev-this".into()))
    }
}

#[test]
fn every_settings_string_reads_right_in_every_language() {
    crate::i18n::checks::catalog(text::S::ALL, st, |s| {
        matches!(s, text::S::QuickLookup | text::S::AppearanceGlossy | text::S::AppearanceLiquidGlass)
    });
}

#[test]
fn the_wide_sheet_lists_grouped_sections_beside_learning() {
    let h = open(Case::new(UiLanguage::ArEg, None));
    let html = h.html();
    assert!(html.contains("dir=\"rtl\"") && html.contains("data-drilled=\"false\""), "{html}");
    for s in [text::S::GroupStudy, text::S::GroupLook, text::S::GroupDevice, text::S::GroupData] {
        assert!(html.contains(st(UiLanguage::ArEg, s)), "missing group {s:?}");
    }
    // Wide windows open on Learning, marked as the current row.
    assert!(
        html.contains(&format!("id=\"settings-section-title\">{}<", st(UiLanguage::ArEg, text::S::Learning)))
    );
    assert!(html.contains(&format!(
        "{} {}",
        st(UiLanguage::ArEg, text::S::Version),
        env!("CARGO_PKG_VERSION")
    )));
    assert!(!html.contains(st(UiLanguage::ArEg, text::S::Tablet)), "no tablet section off Android");
}

#[test]
fn the_phone_drills_into_a_section_and_back_out() {
    let mut h = open(Case::new(UiLanguage::En, None).phone());
    let html = h.html();
    assert!(
        html.contains("data-drilled=\"false\"") && !html.contains("class=\"settings-pane\""),
        "phone starts on the list"
    );
    h.click("Look & language Language, theme, text size, digits");
    assert_eq!(nav(&h), (true, Some(Section::Look)));
    let html = h.html();
    assert!(
        html.contains("data-drilled=\"true\"") && html.contains("class=\"icon-button settings-back\""),
        "{html}"
    );
    assert!(html.contains("<h2 id=\"settings-title\">Look &#38; language</h2>"), "{html}");
    h.click("Back");
    assert_eq!(nav(&h), (true, None));
    h.click("Close");
    assert_eq!(nav(&h), (false, None));
    assert_eq!(
        h.html(),
        "<div class=\"app\" lang=\"en\" dir=\"ltr\"></div>",
        "a closed sheet renders nothing"
    );
}

fn learning() -> Harness {
    open(Case::new(UiLanguage::En, Some(Section::Learning)))
}

/// The radio (or toggle) in a group, by its text and which of the
/// same-named buttons it is (pronunciation and translation both say "On tap").
fn pressed(h: &Harness, text: &str, nth: usize) -> String {
    let t = &h.all("click", text)[nth];
    t.attr("aria-checked").or(t.attr("aria-pressed")).unwrap_or_default().to_string()
}

#[test]
fn learning_choices_sync_and_show_as_checked() {
    use kabar_core::settings::{PronMode, TranslationMode};
    let mut h = learning();
    assert_eq!(pressed(&h, "Off", 0), "false");
    let off = h.all("click", "Off")[0].clone();
    h.fire_click(&off);
    assert_eq!(settings(&h).pronunciation, PronMode::Off);
    assert_eq!(stored(&h).pronunciation, PronMode::Off, "the choice is saved, so it syncs");
    assert_eq!(pressed(&h, "Off", 0), "true");

    // The second "On tap" is the translation row.
    let tap = h.all("click", "On tap")[1].clone();
    h.fire_click(&tap);
    assert_eq!(stored(&h).translation, TranslationMode::Tap);
    h.click("Hidden");
    assert_eq!(stored(&h).translation, TranslationMode::Off);
    assert_eq!(pressed(&h, "Hidden", 0), "true");

    h.click("Firm · 95%");
    assert_eq!(stored(&h).desired_retention.0, 95);
    h.click("Light · 85%");
    assert_eq!(settings(&h).desired_retention.0, 85);
}

#[test]
fn learning_switches_flip_their_settings() {
    let mut h = learning();
    let changes: Vec<_> = h.targets().into_iter().filter(|t| t.event == "change").collect();
    // Word colours, grammar hints, sounds (no vibration motor off Android).
    assert_eq!(changes.len(), 3, "{changes:#?}");
    assert!(changes.iter().all(|t| t.attr("checked") == Some("true")), "all on by default");
    h.form(&changes[0], "false");
    assert!(stored(&h).word_colours_off);
    h.form(&changes[1], "false");
    assert!(stored(&h).grammar_hints_off);
    assert!(h.html().contains("Right-answer sound"));
    h.form(&changes[2], "false");
    assert!(stored(&h).sounds_off);
    let html = h.html();
    assert!(
        !html.contains("Right-answer sound") && !html.contains("End-of-lesson sound"),
        "sound pickers hide with sounds off"
    );
    let again: Vec<_> = h.targets().into_iter().filter(|t| t.event == "change").collect();
    h.form(&again[0], "true");
    assert!(!stored(&h).word_colours_off);
}

#[test]
fn sound_choices_preview_and_save() {
    use kabar_core::settings::{CorrectSound, EndSound};
    let mut h = learning();
    for (label, choice) in
        [("Javanese saron", CorrectSound::Ding3), ("Tight kendang", CorrectSound::Kendang3)]
    {
        h.click(label);
        assert_eq!(stored(&h).correct_sound, choice);
        assert_eq!(pressed(&h, label, 0), "true");
    }
    h.click("Deep gong");
    assert_eq!(stored(&h).end_sound, EndSound::Gong2);
    for label in ["Soft ding", "Bright bell", "Round kendang", "Low kendang", "Warm gong", "Small gong"] {
        assert_eq!(h.all("click", label).len(), 1, "{label} is offered once");
    }
}

#[test]
fn retention_note_explains_how_timing_is_tuned() {
    let h = learning();
    let min = kabar_core::srs::MIN_REVIEWS_TO_OPTIMIZE.to_string();
    assert!(h.html().contains(&min), "until enough reviews, the note says how many are needed");
    let mut case = Case::new(UiLanguage::En, Some(Section::Learning));
    case.settings.fsrs =
        Some(kabar_core::srs::FsrsParams { weights: Vec::new(), reviews: 412, trained_at_ms: 0 });
    let html = open(case).html();
    assert!(html.contains("Timing is tuned to your own memory, from 412\u{a0}reviews."), "{html}");
}

#[test]
fn look_changes_theme_language_digits_and_device_prefs() {
    use kabar_core::settings::Theme;
    let mut h = open(Case::new(UiLanguage::En, Some(Section::Look)));
    let html = h.html();
    assert!(
        html.contains("Apa kabar? Kabar baik.") && html.contains("· 2026"),
        "text sample and Western digits"
    );
    assert_eq!(
        html.matches("This device only").count(),
        2,
        "interface style and text size stay on this device"
    );
    for th in Theme::ALL {
        assert!(html.contains(crate::i18n::t(UiLanguage::En, look::theme_label(th))));
    }
    h.click("Batik indigo");
    assert_eq!(stored(&h).theme, Theme::Batik);
    assert_eq!(pressed(&h, "Batik indigo", 0), "true");

    h.click("Larger");
    assert_eq!(*prefs(&h).text_size.peek(), device::TextSize::Larger);
    assert_eq!(local(&h, "pref.text_size").as_deref(), Some("larger"));
    h.click("Liquid Glass · iOS");
    assert_eq!(*prefs(&h).appearance.peek(), device::Appearance::LiquidGlass);
    assert_eq!(local(&h, "pref.appearance").as_deref(), Some("liquid-glass"));

    let digits = h.find("change", |t| t.tag == "input");
    h.form(&digits, "true");
    assert!(stored(&h).eastern_digits);

    // Switching to Arabic turns the whole sheet right-to-left, digits included.
    h.click("مصري");
    assert_eq!(settings(&h).ui_language, UiLanguage::ArEg);
    let html = h.html();
    assert!(html.contains("dir=\"rtl\"") && html.contains("٢٠٢٦"), "{html}");
}

#[test]
fn every_theme_swatch_previews_its_own_colours() {
    use kabar_core::settings::Theme;
    for th in Theme::ALL {
        let css = look::swatch(th);
        assert!(css.starts_with("background: linear-gradient(135deg, "), "{th:?}: {css}");
        if th.follows_os().is_some() && th.palette().is_none() {
            assert!(!css.contains("border-color"), "a system theme shows day and night only");
        } else {
            assert!(css.contains("border-color"), "{th:?} shows its gold edge");
        }
    }
    assert_ne!(look::swatch(Theme::Batik), look::swatch(Theme::Sawah));
}

#[test]
fn search_narrows_the_list_and_clears() {
    let mut h = open(Case::new(UiLanguage::En, None));
    let search = |h: &Harness| h.find("input", |t| t.attr("class") == Some("settings-search"));
    let field = search(&h);
    h.form(&field, "font size");
    let html = h.html();
    assert!(html.contains("Look &#38; language"), "{html}");
    assert!(!html.contains("Offline &#38; storage"), "unrelated sections hide while searching");
    let field = search(&h);
    h.form(&field, "zzzz");
    let html = h.html();
    assert!(html.contains("No matching sections. Try font or reminders."));
    assert!(!html.contains("Study and reminders"), "an empty group's heading is left out too");
    h.click("Clear search");
    assert!(h.html().contains("Study and reminders"));
    assert!(!h.html().contains("No matching sections"));
}

#[test]
fn the_learner_heads_the_list_and_opens_their_plan() {
    let mut case = Case::new(UiLanguage::En, None).phone();
    case.profile = Some("  salma ");
    let mut h = open(case);
    let html = h.html();
    assert!(
        html.contains("settings-avatar\">S</span>"),
        "the avatar is the name's first letter, capitalised: {html}"
    );
    h.click("S salma Your study plan and times with Kak Nur");
    assert_eq!(nav(&h).1, Some(Section::Coach));
    // A blank name shows no row.
    let mut blank = Case::new(UiLanguage::En, None);
    blank.profile = Some("   ");
    assert!(!open(blank).html().contains("class=\"settings-avatar\""));
}

#[test]
fn the_account_row_shows_the_sync_state() {
    let mut case = Case::new(UiLanguage::En, None);
    case.cloud = Some(signed_in(SyncState::Live));
    let html = open(case).html();
    assert!(html.contains("In sync"), "Account's row says how sync is going: {html}");
    let mut out = Case::new(UiLanguage::En, None);
    out.cloud = Some(CloudStatus::not_configured(kabar_protocol::DeviceId("d".into())));
    assert!(open(out).html().contains("Kabar Cloud, devices, cost &#38; limits"));
}

#[test]
fn mac_lists_shortcuts_and_keeps_hover_on_this_device() {
    let mut h = open(Case::new(UiLanguage::En, Some(Section::Mac)));
    let html = h.html();
    for label in ["Keyboard shortcuts", "Close Settings", "In review: flip the card", "Quick Lookup", "Later"]
    {
        assert!(html.contains(label), "{label}");
    }
    assert!(html.contains("<kbd>⌘1</kbd><span> … </span><kbd>⌘7</kbd>"), "ranges show an ellipsis: {html}");
    let hover = h.find("change", |t| t.tag == "input");
    assert_eq!(hover.attr("checked"), Some("true"));
    h.form(&hover, "false");
    assert!(!*prefs(&h).mouse_hover.peek());
    assert_eq!(local(&h, "pref.mouse_hover_peek").as_deref(), Some("off"));
    h.click("Slim, more room for content");
    assert_eq!(*prefs(&h).nav_style.peek(), device::NavStyle::Slim);
    assert_eq!(local(&h, "pref.nav_style").as_deref(), Some("slim"));
    assert_eq!(pressed(&h, "Slim, more room for content", 0), "true");
}

#[test]
fn the_tablet_section_names_the_layout_in_use() {
    for (width, label) in
        [(400, "Compact"), (700, "Medium"), (1000, "Expanded"), (1300, "Large"), (1700, "Extra large")]
    {
        let mut case = Case::new(UiLanguage::En, None).body(|| rsx! { platform_sections::TabletSection {} });
        case.width = width;
        let html = open(case).html();
        assert!(html.contains(&format!("<span class=\"pill\">{label}</span>")), "{width}: {html}");
        assert!(html.contains("With labels"));
    }
}

#[test]
fn android_says_when_a_settings_page_wont_open() {
    let mut h = open(Case::new(UiLanguage::En, None).body(|| rsx! { platform_sections::AndroidSection {} }));
    let html = h.html();
    assert!(html.contains("Background audio") && html.contains("Peek at a word when the S Pen hovers"));
    assert!(!html.contains("Couldn't open that page"));
    h.click("Open app info");
    let html = h.html();
    assert!(
        html.contains("Couldn&#39;t open that page. Go to Android Settings › Apps › Kabar."),
        "{html}\n
        off Android the system page can't open, and the learner is told where to go"
    );
    let pen = h.find("change", |t| t.tag == "input");
    h.form(&pen, "false");
    assert_eq!(local(&h, "pref.pen_hover_peek").as_deref(), Some("off"));
    assert!(!*prefs(&h).pen_hover.peek());
}

#[test]
fn about_shows_the_version_device_and_replays_the_welcome() {
    let mut h = open(Case::new(UiLanguage::En, Some(Section::About)));
    let device = h.read(|| consume_context::<Db>().0.borrow().device_id().to_string());
    let html = h.html();
    assert!(html.contains(&device) && html.contains(env!("CARGO_PKG_VERSION")), "{html}");
    assert!(html.contains("<span class=\"pill\">Large</span>"), "1280 px is the Large layout");
    let mut case = settings(&h);
    case.welcomed = true;
    h.read(|| consume_context::<Ui>().settings).set(case);
    h.click("Show the welcome screen again");
    assert!(!stored(&h).welcomed);
    assert!(!nav(&h).0, "the sheet closes to show the welcome");
}

#[test]
fn storage_shows_where_the_data_lives() {
    let html = open(Case::new(UiLanguage::En, Some(Section::Storage))).html();
    assert!(html.contains("Where your data lives on this device") && html.contains("Export everything"));
    let dir = crate::platform::data_dir().unwrap().display().to_string();
    assert!(html.contains(&dir), "{html}");
}

#[test]
fn the_account_section_opens_cost_and_limits() {
    let mut case = Case::new(UiLanguage::En, Some(Section::Account));
    case.cloud = Some(CloudStatus {
        configured: true,
        ..CloudStatus::not_configured(kabar_protocol::DeviceId("d".into()))
    });
    let mut h = open(case);
    assert!(h.html().contains("Sign in with Google so your progress syncs"));
    h.click("Sign in with Google");
    assert_eq!(sent(), ["SignInGoogle"]);
    h.click("Cost & limits What it costs a month, and the free tiers' limits");
    assert_eq!(h.read(|| *consume_context::<Ui>().screen.peek()), Screen::Cost);
    assert!(!nav(&h).0);
}

#[test]
fn every_section_opens_from_its_row_in_both_languages() {
    for lang in [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En] {
        let mut case = Case::new(lang, None);
        case.cloud = Some(signed_in(SyncState::Offline));
        let mut h = open(case);
        for s in Section::ALL.into_iter().filter(|s| s.shown(false)) {
            let row = h.find("click", |t| t.text.starts_with(&format!("{} ", st(lang, s.title()))));
            h.fire_click(&row);
            assert_eq!(nav(&h).1, Some(s));
            let html = h.html();
            assert!(
                html.contains(&format!(
                    "id=\"settings-section-title\">{}<",
                    st(lang, s.title()).replace('&', "&#38;")
                )),
                "{lang:?} {s:?}"
            );
        }
    }
}

#[test]
fn the_gear_opens_settings_on_the_list() {
    fn gear() -> Element {
        rsx! { GearButton {} SettingsSheet {} }
    }
    let mut case = Case::new(UiLanguage::En, Some(Section::Mac)).body(gear);
    case.width = 400;
    let mut h = open(case);
    h.read(|| consume_context::<SettingsNav>().close());
    h.settle();
    assert!(!h.html().contains("settings-window"));
    let gear = h.find("click", |t| t.attr("class") == Some("icon-button gear-button"));
    assert_eq!(gear.attr("aria-label"), Some("Settings"));
    h.fire_click(&gear);
    assert_eq!(nav(&h), (true, None), "the gear opens the section list, not the last section");
}

#[test]
fn settings_nav_starts_closed_without_a_preview_section() {
    let mut dom = VirtualDom::new(|| {
        let nav = use_hook(SettingsNav::new);
        let open = *nav.open.peek();
        rsx! { "{open} {nav.section.peek().is_some()}" }
    });
    dom.rebuild_in_place();
    // KABAR_OPEN_SETTINGS is only set by hand for previews.
    assert_eq!(dioxus::ssr::render(&dom), "false false");
}

#[cfg(all(feature = "desktop", not(target_os = "android")))]
mod menu_bar {
    use super::*;
    use crate::settings::menu::{Action, GO, action, apply};

    #[test]
    fn menu_items_open_settings_or_go_to_a_screen() {
        assert_eq!(action("kabar.settings"), Some(Action::Settings(None)));
        assert_eq!(action("kabar.shortcuts"), Some(Action::Settings(Some(Section::Mac))));
        assert_eq!(action("kabar.go.listen"), Some(Action::Go(Screen::Listen)));
        assert_eq!(action("copy"), None, "the standard Edit items are the system's");
        let screens: Vec<Screen> = GO.iter().map(|g| g.3).collect();
        assert_eq!(screens[..4], crate::more::TABS, "⌘1–⌘4 follow the four main tabs");
        for (i, (id, ..)) in GO.iter().enumerate() {
            assert_eq!(action(id), Some(Action::Go(screens[i])));
        }
    }

    #[test]
    fn going_somewhere_closes_settings() {
        let mut h = open(Case::new(UiLanguage::En, Some(Section::Look)));
        h.read(|| {
            apply(consume_context::<Ui>(), consume_context::<SettingsNav>(), Action::Go(Screen::Review))
        });
        h.settle();
        assert!(!nav(&h).0);
        assert_eq!(h.read(|| *consume_context::<Ui>().screen.peek()), Screen::Review);
        h.read(|| {
            apply(
                consume_context::<Ui>(),
                consume_context::<SettingsNav>(),
                Action::Settings(Some(Section::Mac)),
            )
        });
        h.settle();
        assert_eq!(nav(&h), (true, Some(Section::Mac)));
        assert!(h.html().contains("Keyboard shortcuts"));
    }

    #[test]
    fn the_menu_bar_has_the_mac_menus() {
        let menu = crate::settings::menu::menu_bar();
        let titles: Vec<String> =
            menu.items().iter().filter_map(|i| i.as_submenu().map(|s| s.text())).collect();
        assert_eq!(titles, ["Kabar", "Edit", "Go", "Window", "Help"]);
        let go = menu.items()[2].as_submenu().unwrap().items();
        assert_eq!(go.len(), 7);
        let first = go[0].as_menuitem().unwrap();
        assert_eq!((first.text(), first.id().0.as_str()), ("Today".to_string(), "kabar.go.today"));
    }
}

fn device(id: &str, name: &str) -> DeviceSummary {
    DeviceSummary {
        device_id: kabar_protocol::DeviceId(id.into()),
        name: name.into(),
        model: None,
        platform: kabar_protocol::cloud::Platform::Android,
        signed_in_at_ms: 0,
        last_seen_ms: 0,
        last_sync_ms: None,
        current: false,
        revoked: false,
    }
}

fn account(status: CloudStatus) -> Case {
    let mut case = Case::new(UiLanguage::En, Some(Section::Account));
    case.cloud = Some(status);
    case
}

#[test]
fn account_explains_a_build_without_kabar_cloud() {
    let h = open(account(CloudStatus::not_configured(kabar_protocol::DeviceId("d".into()))));
    let html = h.html();
    assert!(
        html.contains(&esc(crate::i18n::t(UiLanguage::En, crate::i18n::Msg::CloudNotConfigured))),
        "{html}"
    );
    assert!(!html.contains("Sign in with Google</button>") && !html.contains("Devices"));
}

#[test]
fn account_waits_for_the_browser_while_signing_in() {
    let mut case = account(CloudStatus {
        configured: true,
        ..CloudStatus::not_configured(kabar_protocol::DeviceId("d".into()))
    });
    case.signing_in = true;
    let html = open(case).html();
    assert!(html.contains("Finish signing in in your browser, then come back here."));
    assert!(!html.contains(">Sign in with Google<"), "no second sign-in while one is open");
}

#[test]
fn signed_in_account_shows_sync_and_signs_out() {
    let mut case = account(signed_in(SyncState::Live));
    case.devices = Some(Ok(Vec::new()));
    let mut h = open(case);
    h.settle();
    let html = h.html();
    assert!(html.contains("learner@example.com") && html.contains("class=\"pill ok\""), "{html}");
    assert!(html.contains("3 changes waiting to sync") && html.contains("Last sync: 5 min ago"), "{html}");
    assert!(sent().contains(&"LoadDevices".to_string()), "opening Account loads the device list");
    h.click("Sign out on this device");
    assert_eq!(sent(), ["SignOut"]);
}

#[test]
fn offline_shows_its_picture_and_problems_show_a_notice() {
    let mut status = signed_in(SyncState::Offline);
    status.problem = Some(Problem::Network);
    status.pending = 0;
    let html = open(account(status)).html();
    assert!(html.contains("empty-art small"), "the offline picture");
    assert!(html.contains("Can&#39;t reach Kabar Cloud right now. I&#39;ll keep trying."), "{html}");
    assert!(!html.contains("changes waiting to sync"), "nothing waiting, nothing said");
    assert!(!html.contains("class=\"pill ok\""));
}

#[test]
fn the_devices_list_marks_this_device_and_signs_others_out() {
    let now = crate::store::now_ms();
    let mut this = device("dev-this", "Galaxy S24");
    this.current = true;
    this.model = Some("SM-S928B".into());
    this.last_sync_ms = Some(now - 2 * 3_600_000);
    let mut gone = device("dev-old", "Old tablet");
    gone.revoked = true;
    gone.platform = kabar_protocol::cloud::Platform::Other;
    let mut mac = device("dev-mac", "MacBook");
    mac.platform = kabar_protocol::cloud::Platform::Macos;
    let mut case = account(signed_in(SyncState::Live));
    case.devices = Some(Ok(vec![this, gone, mac]));
    let mut h = open(case);
    let html = h.html();
    assert!(
        html.contains("This device") && html.contains("SM-S928B") && html.contains("Last sync: 2 h ago"),
        "{html}"
    );
    assert!(html.contains("class=\"device revoked\"") && html.contains("Signed out"));
    assert!(html.contains("Not synced yet"), "the Mac never synced");
    // Only the Mac can be signed out: not this device, not one already out.
    let outs = h.all("click", "Sign it out");
    assert_eq!(outs.len(), 1);
    h.settle();
    assert_eq!(sent(), ["LoadDevices", "LoadLogin"], "a live account loads its devices and login on open");
    h.fire_click(&outs[0]);
    assert_eq!(sent(), ["RevokeDevice(DeviceId(\"dev-mac\"))"]);
    h.click("Refresh");
    assert_eq!(sent(), ["LoadDevices"]);
}

#[test]
fn the_devices_list_loads_and_fails_gently() {
    let html = open(account(signed_in(SyncState::Live))).html();
    assert!(html.contains("<p class=\"muted\">…</p>"), "loading: {html}");
    let mut case = account(signed_in(SyncState::Live));
    case.devices = Some(Err(Problem::Server));
    let html = open(case).html();
    let server = esc(crate::i18n::t(UiLanguage::En, crate::i18n::Msg::ProblemServer));
    assert!(html.contains(&format!("<p class=\"notice\">{server}</p>")), "{html}");
}

#[test]
fn try_sync_saves_a_word_for_every_device() {
    let mut h = open(account(signed_in(SyncState::Live)));
    sent();
    let field = || |t: &crate::screen_kit::Target| t.attr("placeholder") == Some("rumah");
    h.type_into("input", field(), "  Rumah ");
    let form = h.find("submit", |_| true);
    h.form(&form, "");
    let rows = h.read(|| consume_context::<Db>().0.borrow().synced_rows("word_status").unwrap());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1["lexeme"], "rumah", "saved trimmed and lower-case");
    assert_eq!(sent(), ["Kick"], "sync is told a write is waiting");
    let html = h.html();
    assert!(html.contains("Saved words") && html.contains("<bdi>rumah</bdi>"), "{html}");
    // Blank and over-long words are not saved.
    for word in ["   ", &"a".repeat(65)] {
        h.type_into("input", field(), word);
        let form = h.find("submit", |_| true);
        h.form(&form, "");
    }
    assert_eq!(h.read(|| consume_context::<Db>().0.borrow().synced_rows("word_status").unwrap()).len(), 1);
}

#[test]
fn the_devices_screen_heads_the_same_account() {
    let mut case =
        account(signed_in(SyncState::CatchingUp)).body(|| rsx! { crate::account::DevicesScreen {} });
    case.devices = Some(Ok(vec![device("a", "Tab S11")]));
    let html = open(case).html();
    assert!(html.contains("<h1>Account &#38; devices</h1>") && html.contains("Tab S11"), "{html}");
}

#[test]
fn tapping_outside_the_sheet_closes_it() {
    let mut h = open(Case::new(UiLanguage::En, Some(Section::Look)));
    let backdrop = h.find("click", |t| t.attr("class") == Some("settings-backdrop"));
    h.fire_click(&backdrop);
    assert!(!nav(&h).0);
    assert!(!h.html().contains("settings-window"));
}
