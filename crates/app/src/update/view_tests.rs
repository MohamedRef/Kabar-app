//! Settings › Updates, the Today banner and the "what's new" sheet, rendered
//! for each state the updater can be in (Dioxus SSR), driven through the
//! shared test kit ([`TestDom`], `testkit::provide`).

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_protocol::update::{
    BuildChanges, Change, ChangeKind, Channel, UpdateAsset, UpdateNotes, UpdatePlatform,
};

use super::content::Refresh;
use super::sheet::{SheetFor, SheetMode, UpdateSheetNav};
use super::text::{U, ut};
use super::view::{UpdateBannerFor, UpdatesCardFor, summary};
use super::{Failure, Mode, Off, Offer, Phase, Prefs, Status};
use crate::test_dom::TestDom;
use crate::{Screen, Ui};

const EN: UiLanguage = UiLanguage::En;
const EG: UiLanguage = UiLanguage::ArEg;

fn change(kind: ChangeKind, en: &str) -> Change {
    Change { kind, text: UpdateNotes { en: en.into(), ar_eg: format!("مصري: {en}"), ar: String::new() } }
}

fn build(b: u64, changes: Vec<Change>) -> BuildChanges {
    BuildChanges {
        version: "1.0.0-alpha.1".into(),
        build: b,
        published_at: "2026-10-30T09:00:00Z".into(),
        changes,
        content_only: false,
    }
}

fn offer(b: u64, changes: Vec<BuildChanges>) -> Offer {
    Offer {
        version: "1.0.0-alpha.1".into(),
        build: b,
        commit: None,
        notes: UpdateNotes {
            en: "Faster lessons".into(), ar_eg: "دروس أسرع".into(), ar: String::new()
        },
        asset: UpdateAsset {
            platform: UpdatePlatform::AndroidArm64,
            file: "kabar.apk".into(),
            size: 3 * 1024 * 1024 + 300 * 1024,
            sha256: "00".repeat(32),
            version_code: None,
            build: None,
        },
        changes,
    }
}

fn status(phase: Phase) -> Status {
    Status {
        off: None,
        phase,
        prefs: Prefs::default(),
        channel: Channel::Alpha,
        preview: false,
        version: "1.0.0-alpha.1".into(),
        build: 100,
        content_build: 0,
        content_since_ms: None,
        content_waiting: None,
        content_check: None,
        packs_refreshed: 0,
        checking_all: false,
        history: Vec::new(),
    }
}

#[derive(Clone, PartialEq)]
enum View {
    Card,
    /// The card in a shell without the sheet (no "what's new?" button).
    CardAlone,
    Banner,
    Sheet(SheetMode),
}

#[derive(Clone, PartialEq)]
struct Case {
    s: Status,
    lang: UiLanguage,
    view: View,
    /// Also show which sheet the shell would open (click tests).
    probe: bool,
}

/// The shared test kit's contexts, on Settings in `lang`.
fn provide_ui(lang: UiLanguage) -> Ui {
    crate::testkit::provide(|| crate::testkit::Setup { lang, screen: Screen::Settings, ..Default::default() })
        .0
}

fn app(c: Case) -> Element {
    provide_ui(c.lang);
    let open = match c.view {
        View::Sheet(m) => Some(m),
        _ => None,
    };
    let nav = (c.view != View::CardAlone).then(|| use_context_provider(|| UpdateSheetNav(Signal::new(open))));
    let s = c.s;
    // Which sheet the shell would show, so a click that opens or closes one
    // can be seen on the page.
    let probe = nav.filter(|_| c.probe).map(|n| format!("[sheet: {:?}]", *n.0.read()));
    rsx! {
        match c.view {
            View::Card | View::CardAlone => rsx! { UpdatesCardFor { s } },
            View::Banner => rsx! { UpdateBannerFor { s } },
            View::Sheet(m) => rsx! { SheetFor { s, m } },
        }
        if let Some(p) = probe {
            p { "{p}" }
        }
    }
}

/// The page as text a reader would get, entities decoded.
fn render(s: Status, lang: UiLanguage, view: View) -> String {
    let dom = TestDom::unsettled(VirtualDom::new_with_props(app, Case { s, lang, view, probe: false }));
    decoded(dom.ssr())
}

fn decoded(html: String) -> String {
    html.replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// A rendered page a test can click the way a learner would: by the words
/// on the button (all the text inside it, as it reads now).
struct Page {
    dom: TestDom,
}

impl Page {
    fn new(s: Status, lang: UiLanguage, view: View) -> Self {
        Self::of(VirtualDom::new_with_props(app, Case { s, lang, view, probe: true }))
    }

    fn of(dom: VirtualDom) -> Self {
        Page { dom: TestDom::unsettled(dom) }
    }

    /// Clicks the first button that reads `words`, and draws the result.
    fn click(&mut self, words: &str) -> &mut Self {
        let buttons: Vec<_> = self.dom.nodes().into_iter().filter(|n| n.listens("click")).collect();
        let b = buttons.iter().find(|n| n.text.contains(words)).unwrap_or_else(|| {
            panic!("no button reads {words:?}: {:?}", buttons.iter().map(|n| &n.text).collect::<Vec<_>>())
        });
        self.dom.dispatch(b.el, "click", Box::new(dioxus::html::SerializedMouseData::default()), true);
        self.dom.render();
        self
    }

    fn html(&self) -> String {
        decoded(self.dom.ssr())
    }
}

/// The Ready button: the Mac installs by restarting, so it says so.
fn install_label() -> U {
    if cfg!(target_os = "macos") { U::InstallRestart } else { U::InstallNow }
}

/// What waiting lessons offer: a restart button on the Mac, a hint elsewhere.
fn reopen_label() -> U {
    if cfg!(target_os = "macos") { U::RestartNow } else { U::ReopenHint }
}

fn card(s: Status) -> String {
    render(s, EN, View::Card)
}

fn has(html: &str, lang: UiLanguage, u: U) -> bool {
    html.contains(ut(lang, u))
}

#[test]
fn a_build_that_cannot_update_says_why_and_hides_the_controls() {
    for (off, why) in [
        (Off::DevBuild, U::OffDev),
        (Off::NoKey, U::OffKey),
        (Off::NoPlatform, U::OffPlatform),
        (Off::NotInBundle, U::OffBundle),
    ] {
        let html = card(Status { off: Some(off), ..status(Phase::Idle) });
        assert!(has(&html, EN, U::OffShort) && has(&html, EN, why), "{off:?}: {html}");
        assert!(!has(&html, EN, U::CheckNow), "no check button when updates are off");
        assert!(!has(&html, EN, U::HowItWorks));
        // What's on the device still shows, and opens the history.
        assert!(has(&html, EN, U::AppRow) && has(&html, EN, U::History));
    }
}

#[test]
fn before_and_after_a_check_the_card_says_where_things_stand() {
    let html = card(status(Phase::Idle));
    assert!(has(&html, EN, U::NeverChecked), "{html}");
    assert!(html.contains(&format!(r#"<button class="primary">{}"#, ut(EN, U::CheckNow))), "{html}");
    assert!(html.contains("build "), "the installed build number: {html}");

    let now = super::now_ms();
    let s = Status {
        prefs: Prefs { last_check_ms: Some(now), ..Prefs::default() },
        content_check: Some(Refresh::Current),
        packs_refreshed: 2,
        ..status(Phase::UpToDate)
    };
    let html = card(s.clone());
    assert!(html.contains(&format!("✓ {}", ut(EN, U::UpToDate))), "{html}");
    assert!(html.contains(&ut(EN, U::LastChecked).replace("{when}", "Today")), "{html}");
    assert!(html.contains(&format!("✓ {}", ut(EN, U::ContentSame))));
    assert!(html.contains(&format!("✓ {}", ut(EN, U::PacksRefreshed).replace("{n}", "2"))));
    assert!(summary_of(&s).starts_with(&ut(EN, U::LastChecked).replace("{when}", "Today")));

    // A check that failed for the lessons says so too.
    let html = card(Status { content_check: Some(Refresh::Failed), ..s });
    assert!(has(&html, EN, U::ContentFailed));
}

fn summary_of(s: &Status) -> String {
    #[derive(Clone, PartialEq)]
    struct Probe(Status);
    thread_local!(static OUT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) });
    fn probe(p: Probe) -> Element {
        let ui = provide_ui(EN);
        OUT.with(|o| *o.borrow_mut() = summary(&ui, &p.0));
        rsx! {}
    }
    TestDom::unsettled(VirtualDom::new_with_props(probe, Probe(s.clone())));
    OUT.with(|o| o.borrow().clone())
}

#[test]
fn the_settings_row_summarises_the_live_state() {
    assert_eq!(summary_of(&Status { off: Some(Off::NoKey), ..status(Phase::Idle) }), ut(EN, U::OffShort));
    assert_eq!(summary_of(&Status { checking_all: true, ..status(Phase::UpToDate) }), ut(EN, U::Checking));
    assert_eq!(
        summary_of(&status(Phase::Failed { offer: None, why: Failure::Network })),
        ut(EN, U::CheckFailedShort)
    );
    assert_eq!(
        summary_of(&Status { content_waiting: Some(120), ..status(Phase::UpToDate) }),
        ut(EN, U::ContentWaitingShort)
    );
    assert_eq!(summary_of(&status(Phase::Idle)), ut(EN, U::NeverChecked));
    assert_eq!(summary_of(&status(Phase::Ready(offer(120, vec![])))), ut(EN, U::Ready));
    assert_eq!(summary_of(&status(Phase::Available(offer(120, vec![])))), ut(EN, U::Available));
}

#[test]
fn an_offer_shows_its_size_its_changes_and_one_way_forward() {
    let changes = vec![build(
        120,
        vec![change(ChangeKind::New, "Javanese stories"), change(ChangeKind::Fixed, "Audio stutter")],
    )];
    let html = card(status(Phase::Available(offer(120, changes))));
    assert!(has(&html, EN, U::Available), "{html}");
    assert!(html.contains("1.0.0-alpha.1 (120)"), "the offered build");
    assert!(html.contains("3.3 MB"), "its size: {html}");
    assert!(html.contains("Javanese stories") && html.contains("Audio stutter"), "{html}");
    assert!(has(&html, EN, U::Download) && has(&html, EN, U::WhatsNewAsk));
    // With an offer in hand, "Check now" is no longer the primary button.
    assert!(html.contains(&format!(r#"<button class="">{}"#, ut(EN, U::CheckNow))), "{html}");

    // Without per-change lines, the feed's notes speak; without the sheet, no link to it.
    let html = render(status(Phase::Available(offer(120, vec![]))), EN, View::CardAlone);
    assert!(html.contains("Faster lessons"), "{html}");
    assert!(!has(&html, EN, U::WhatsNewAsk));
}

#[test]
fn each_step_of_an_update_has_its_own_headline_and_button() {
    let o = || offer(120, vec![]);
    let cases: Vec<(Phase, U, Option<U>)> = vec![
        (Phase::Checking, U::Checking, None),
        (Phase::NotForThisDevice, U::NotForDevice, None),
        (Phase::Ready(o()), U::Ready, Some(install_label())),
        (Phase::Installing(o()), U::Installing, None),
        (Phase::NeedsPermission(o()), U::NeedsPermission, Some(U::TryAgain)),
        (Phase::Installed(o()), U::Installed, Some(U::RestartNow)),
        (Phase::Failed { offer: Some(o()), why: Failure::Network }, U::FailedDownload, Some(U::TryAgain)),
        (Phase::Failed { offer: None, why: Failure::Network }, U::FailedNetwork, None),
        (
            Phase::Failed { offer: Some(o()), why: Failure::Install("refused".into()) },
            U::FailedInstall,
            Some(U::TryAgain),
        ),
        (Phase::Failed { offer: None, why: Failure::TooNew }, U::TooNew, None),
        (Phase::Failed { offer: None, why: Failure::KeyChanged }, U::KeyChanged, None),
    ];
    for (phase, headline, button) in cases {
        let html = card(status(phase.clone()));
        assert!(has(&html, EN, headline), "{phase:?}: {html}");
        if let Some(b) = button {
            assert!(html.contains(&format!(">{}</button>", ut(EN, b))), "{phase:?} needs {b:?}: {html}");
        }
    }
    // The installing permission gets a second button to Android's settings.
    assert!(has(&card(status(Phase::NeedsPermission(o()))), EN, U::OpenPermission));
    assert!(has(&card(status(Phase::Installed(o()))), EN, U::NextLaunch));
    // A verification failure shows its reason, isolated left to right.
    let html =
        card(status(Phase::Failed { offer: Some(o()), why: Failure::Verify("sha256 mismatch".into()) }));
    assert!(has(&html, EN, U::FailedVerify) && html.contains("sha256 mismatch"), "{html}");
    assert!(html.contains(r#"dir="ltr""#));
    // A download in progress shows how far it got, and the buttons wait.
    let html = card(status(Phase::Downloading { offer: o(), done: 1024 * 1024 }));
    assert!(html.contains(r#"value="1048576""#) && html.contains(r#"max="3452928""#), "{html}");
    assert!(html.contains("disabled"));
}

#[test]
fn a_check_by_hand_and_waiting_lessons_are_shown() {
    let html = card(Status { checking_all: true, ..status(Phase::UpToDate) });
    assert!(has(&html, EN, U::Checking) && !has(&html, EN, U::UpToDate), "{html}");
    let html = card(Status { content_waiting: Some(130), content_build: 125, ..status(Phase::UpToDate) });
    assert!(has(&html, EN, U::ContentWaiting) && has(&html, EN, reopen_label()), "{html}");
    assert!(html.contains(&format!("{} · {} 125", ut(EN, U::ContentUpdated), ut(EN, U::Build))), "{html}");
}

#[test]
fn mode_and_channel_show_the_learners_choice() {
    let s = Status {
        prefs: Prefs { mode: Mode::Notify, ..Prefs::default() },
        channel: Channel::Beta,
        ..status(Phase::Idle)
    };
    let html = card(s);
    let checked = |label: U| html.contains(&format!(r#"aria-checked="true">{}"#, ut(EN, label)));
    assert!(checked(U::ModeNotify) && checked(U::Beta), "{html}");
    assert!(!checked(U::ModeAuto) && !checked(U::Alpha));
    assert!(has(&html, EN, U::ModeNotifyHint) && has(&html, EN, U::NotifyToggle));

    let html = card(Status {
        prefs: Prefs { mode: Mode::Off, ..Prefs::default() },
        channel: Channel::Preview,
        ..status(Phase::Idle)
    });
    assert!(has(&html, EN, U::ModeOffHint) && !has(&html, EN, U::NotifyToggle), "no notifications when off");
    assert!(has(&html, EN, U::PreviewChannelNote));
}

#[test]
fn the_card_speaks_egyptian_arabic_with_isolated_latin() {
    let html = render(status(Phase::Available(offer(120, vec![]))), EG, View::Card);
    assert!(has(&html, EG, U::Available) && has(&html, EG, U::Download), "{html}");
    assert!(html.contains("دروس أسرع"), "the Egyptian notes");
    assert!(html.contains(r#"<bdi dir="ltr" class="latin">1.0.0-alpha.1 (120)</bdi>"#), "{html}");
    assert!(!html.contains("Faster lessons"));
}

#[test]
fn the_banner_appears_only_when_there_is_something_to_do() {
    for phase in [Phase::Idle, Phase::UpToDate, Phase::Checking] {
        assert_eq!(render(status(phase), EN, View::Banner), "");
    }
    // Automatic mode downloads by itself: no banner for an available build.
    assert_eq!(render(status(Phase::Available(offer(120, vec![]))), EN, View::Banner), "");
    let notify = Prefs { mode: Mode::Notify, ..Prefs::default() };
    let changes = vec![build(
        120,
        vec![
            change(ChangeKind::New, "One"),
            change(ChangeKind::New, "Two"),
            change(ChangeKind::Fixed, "Three"),
            change(ChangeKind::Improved, "Four"),
        ],
    )];
    let html =
        render(Status { prefs: notify, ..status(Phase::Available(offer(120, changes))) }, EN, View::Banner);
    assert!(
        has(&html, EN, U::Available) && has(&html, EN, U::Download) && has(&html, EN, U::Later),
        "{html}"
    );
    assert!(html.contains("One") && !html.contains("Four"), "two lines at most: {html}");
    assert!(html.contains(&super::text::more_changes(EN, 2)), "{html}");

    let html = render(status(Phase::Installed(offer(120, vec![]))), EN, View::Banner);
    assert!(has(&html, EN, U::Installed) && has(&html, EN, U::RestartNow));
    let html = render(status(Phase::Ready(offer(120, vec![]))), EN, View::Banner);
    assert!(has(&html, EN, U::Ready) && has(&html, EN, install_label()) && html.contains("Faster lessons"));

    // Closed for this offer: gone until the next one.
    let closed = Prefs { dismissed: Some("1.0.0-alpha.1+120".into()), ..Prefs::default() };
    assert_eq!(
        render(Status { prefs: closed, ..status(Phase::Ready(offer(120, vec![]))) }, EN, View::Banner),
        ""
    );

    // New lessons over the air, nothing to install: a quiet note.
    let html = render(Status { content_waiting: Some(130), ..status(Phase::UpToDate) }, EN, View::Banner);
    assert!(has(&html, EN, U::ContentWaitingShort) && has(&html, EN, U::GotIt), "{html}");
}

fn history(n: u64) -> Vec<BuildChanges> {
    (0..n)
        .map(|i| build(100 - i, vec![change(ChangeKind::Improved, &format!("Change in {}", 100 - i))]))
        .collect()
}

#[test]
fn the_offer_sheet_counts_builds_and_kinds_and_holds_the_button() {
    let changes = vec![
        build(122, vec![change(ChangeKind::New, "Story mode"), change(ChangeKind::Fixed, "Crash on resume")]),
        build(121, vec![change(ChangeKind::New, "Night batik")]),
    ];
    let s = Status { history: history(3), ..status(Phase::Available(offer(122, changes))) };
    let html = render(s, EN, View::Sheet(SheetMode::Offer));
    assert!(html.contains(r#"role="dialog""#), "{html}");
    assert!(html.contains("v1.0.0-alpha.1 (122)"), "head names the offer: {html}");
    assert!(html.contains("#100") && html.contains("#122"), "installed → offered: {html}");
    assert!(html.contains("2 new builds"), "{html}");
    assert!(
        html.contains(&format!("2 {}", ut(EN, U::KindNew)))
            && html.contains(&format!("1 {}", ut(EN, U::KindFixed))),
        "{html}"
    );
    // The newest build is open, the next one folded.
    assert!(html.contains("Story mode") && !html.contains("Night batik"), "{html}");
    assert!(
        html.contains(&format!(">{}</button>", ut(EN, U::Download))) || html.contains(ut(EN, U::Download))
    );
    assert!(html.contains("GitHub"));
    // The installed build sits by the offer, then "Earlier".
    assert!(has(&html, EN, U::BadgeInstalled) && has(&html, EN, U::Earlier), "{html}");

    // Downloading: how much of how much.
    let s = Status {
        history: history(1),
        ..status(Phase::Downloading { offer: offer(122, vec![]), done: 1024 * 1024 })
    };
    let html = render(s, EN, View::Sheet(SheetMode::Offer));
    assert!(html.contains("1.0 MB / 3.3 MB"), "{html}");
    assert!(html.contains("Faster lessons"), "notes when there are no change lines");

    // A failed download is named at the foot.
    let s = status(Phase::Failed { offer: Some(offer(122, vec![])), why: Failure::Network });
    let html = render(s, EN, View::Sheet(SheetMode::Offer));
    assert!(html.contains(r#"role="alert""#) && has(&html, EN, U::FailedDownload), "{html}");
}

#[test]
fn the_history_sheet_folds_long_lists_and_pages_older_builds() {
    // Seven fixes in one build fold after five.
    let mut builds =
        vec![build(110, (1..=7).map(|i| change(ChangeKind::Fixed, &format!("Fix {i}"))).collect())];
    builds.extend(history(6));
    let s = Status { history: builds, ..status(Phase::UpToDate) };
    let html = render(s, EN, View::Sheet(SheetMode::History));
    assert!(html.contains("Fix 5") && !html.contains("Fix 6"), "{html}");
    assert!(html.contains(&ut(EN, U::ShowAll).replace("{n}", "7")), "{html}");
    assert!(has(&html, EN, U::BadgeLatest) && has(&html, EN, U::Earlier));
    // Four earlier builds at a time, then "older".
    assert!(html.contains("v1.0.0-alpha.1 (97)") && !html.contains("v1.0.0-alpha.1 (96)"), "{html}");
    assert!(has(&html, EN, U::ShowOlder));
    assert!(html.contains(&format!(">{}</button>", ut(EN, U::Close))), "a plain close: {html}");

    let html = render(status(Phase::UpToDate), EN, View::Sheet(SheetMode::History));
    assert!(has(&html, EN, U::HistoryEmpty), "{html}");
}

#[test]
fn after_an_update_the_sheet_says_what_came_and_got_it_closes() {
    let s = Status {
        version: "1.0.0-alpha.1".into(),
        build: 102,
        history: vec![
            build(102, vec![change(ChangeKind::New, "Kak Nur's weekly report")]),
            build(101, vec![change(ChangeKind::Fixed, "Sync badge")]),
            build(100, vec![]),
        ],
        prefs: Prefs { whats_new_from: Some("1.0.0-alpha.1+100".into()), ..Prefs::default() },
        ..status(Phase::UpToDate)
    };
    assert_eq!(s.whats_new().len(), 2);
    let html = render(s.clone(), EN, View::Sheet(SheetMode::Updated));
    assert!(has(&html, EN, U::UpdatedTitle) && html.contains("Kak Nur's weekly report"), "{html}");
    assert!(html.contains("#100") && html.contains("#102"));
    assert!(html.contains(&format!(">{}</button>", ut(EN, U::GotIt))), "{html}");

    // In Egyptian Arabic, with the dual for two builds.
    let html = render(s, EG, View::Sheet(SheetMode::Updated));
    assert!(html.contains("نسختين جداد") && html.contains("مصري: Kak Nur's weekly report"), "{html}");
}

#[test]
fn every_update_string_exists_in_all_three_languages_with_the_same_slots() {
    use U::*;
    let all = [
        Build,
        OffDev,
        OffKey,
        OffPlatform,
        OffBundle,
        UpToDate,
        Checking,
        CheckNow,
        NotForDevice,
        Available,
        Download,
        Downloading,
        Ready,
        InstallNow,
        InstallRestart,
        Installing,
        InstallingAndroid,
        NeedsPermission,
        OpenPermission,
        TryAgain,
        Installed,
        RestartNow,
        NextLaunch,
        FailedNetwork,
        FailedDownload,
        FailedVerify,
        FailedInstall,
        TooNew,
        KeyChanged,
        Later,
        Mode,
        ModeAuto,
        ModeAutoHint,
        ModeNotify,
        ModeNotifyHint,
        ModeOff,
        ModeOffHint,
        Channel,
        Alpha,
        Beta,
        Stable,
        Preview,
        PreviewChannelNote,
        WhatsNew,
        DoneTitle,
        DoneBody,
        ReadyTitle,
        NoticeAvailable,
        NoticeReady,
        NoticeInstalled,
        NoticeTapAvailable,
        NoticeTapReady,
        NoticeTapInstalled,
        NoticeChannel,
        NotifyToggle,
        WhatsNewAsk,
        History,
        HistoryHint,
        HistoryEmpty,
        UpdatedTitle,
        KindNew,
        KindImproved,
        KindFixed,
        BadgeLatest,
        BadgeInstalled,
        BadgeNew,
        BadgeContent,
        ContentWaiting,
        Today,
        Yesterday,
        TwoDaysAgo,
        DaysAgo,
        ShowAll,
        ShowLess,
        Earlier,
        ShowOlder,
        Close,
        GotIt,
        NoDetails,
        AppRow,
        ContentRow,
        ContentUpdated,
        ContentWaitingShort,
        ContentSame,
        ContentFailed,
        PacksRefreshed,
        LastChecked,
        NeverChecked,
        CheckFailedShort,
        OffShort,
        ReopenHint,
        OnThisDevice,
        HowItWorks,
    ];
    fn slots(s: &str) -> Vec<&str> {
        let mut v: Vec<&str> =
            s.match_indices('{').filter_map(|(i, _)| s[i..].split_once('}').map(|(a, _)| a)).collect();
        v.sort();
        v
    }
    let arabic = |s: &str| s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
    for u in all {
        let (en, eg, ar) = (ut(EN, u), ut(EG, u), ut(UiLanguage::Ar, u));
        assert!(!en.trim().is_empty() && !eg.trim().is_empty() && !ar.trim().is_empty(), "{u:?}");
        assert!(!arabic(en), "{u:?} English is English: {en}");
        assert!(arabic(eg) && arabic(ar), "{u:?} is Arabic: {eg} / {ar}");
        assert_eq!(slots(en), slots(eg), "{u:?} slots");
        assert_eq!(slots(en), slots(ar), "{u:?} slots");
    }
}

#[test]
fn the_history_sheet_opens_folds_and_closes_under_the_learners_taps() {
    let mut builds =
        vec![build(110, (1..=7).map(|i| change(ChangeKind::Fixed, &format!("Fix {i}"))).collect())];
    builds.extend(history(6));
    let s = Status { history: builds, ..status(Phase::UpToDate) };
    let mut page = Page::new(s, EN, View::Sheet(SheetMode::History));
    let html = page.html();
    assert!(html.contains("[sheet: Some(History)]"), "{html}");
    assert!(!html.contains("Change in 99"), "older builds start folded: {html}");

    // The newest build's long list unfolds, then folds again.
    let show_all = ut(EN, U::ShowAll).replace("{n}", "7");
    let html = page.click(&show_all).html();
    assert!(html.contains("Fix 7") && has(&html, EN, U::ShowLess), "{html}");
    let html = page.click(ut(EN, U::ShowLess)).html();
    assert!(!html.contains("Fix 6"), "{html}");

    // An earlier build opens on a tap; the newest one closes on its own.
    let html = page.click("v1.0.0-alpha.1 (99)").html();
    assert!(html.contains("Change in 99"), "{html}");
    let html = page.click("v1.0.0-alpha.1 (110)").html();
    assert!(!html.contains("Fix 1"), "{html}");

    // "Older" pages in four more builds.
    assert!(!html.contains("(96)"));
    let html = page.click(ut(EN, U::ShowOlder)).html();
    assert!(html.contains("v1.0.0-alpha.1 (96)") && !has(&html, EN, U::ShowOlder), "{html}");

    // Closing hides the sheet in the shell.
    let html = page.click(ut(EN, U::Close)).html();
    assert!(html.contains("[sheet: None]"), "{html}");
}

#[test]
fn the_banner_opens_whats_new_and_its_lessons_note_can_be_put_away() {
    let o = offer(101, vec![build(101, vec![change(ChangeKind::New, "Night mode")])]);
    let notify = Prefs { mode: Mode::Notify, ..Prefs::default() };
    let s = Status { prefs: notify, ..status(Phase::Available(o)) };
    let mut page = Page::new(s, EN, View::Banner);
    let html = page.click(ut(EN, U::WhatsNewAsk)).html();
    assert!(html.contains("[sheet: Some(Offer)]"), "{html}");

    let s = Status { content_waiting: Some(12), ..status(Phase::UpToDate) };
    let mut page = Page::new(s, EN, View::Banner);
    assert!(has(&page.html(), EN, U::ContentWaitingShort));
    let html = page.click(ut(EN, U::GotIt)).html();
    assert!(!has(&html, EN, U::ContentWaitingShort), "{html}");
}

#[test]
fn the_cards_rows_open_the_sheet_they_name() {
    let o = offer(101, vec![build(101, vec![change(ChangeKind::New, "Night mode")])]);
    let mut page = Page::new(status(Phase::Available(o)), EN, View::Card);
    assert!(page.html().contains("[sheet: None]"));
    let html = page.click(ut(EN, U::WhatsNewAsk)).html();
    assert!(html.contains("[sheet: Some(Offer)]"), "{html}");
}

/// `days` days before today (UTC), as a feed stamps it.
fn days_ago(days: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    // Howard Hinnant's civil_from_days.
    let z = (now.as_secs() as i64).div_euclid(86_400) - days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T12:00:00Z")
}

#[test]
fn each_build_says_when_it_came_in_the_learners_words() {
    let dated = |b: u64, at: String| BuildChanges {
        published_at: at,
        ..build(b, vec![change(ChangeKind::Improved, &format!("Change in {b}"))])
    };
    let history = vec![
        dated(104, days_ago(0)),
        dated(103, days_ago(1)),
        dated(102, days_ago(2)),
        dated(101, days_ago(4)),
        dated(100, "2020-02-29T08:00:00Z".into()),
    ];
    let s = Status { history, ..status(Phase::UpToDate) };
    let html = render(s.clone(), EN, View::Sheet(SheetMode::History));
    for when in ["Today", "Yesterday", "2 days ago", "4 days ago", "29 February 2020"] {
        assert!(html.contains(when), "{when}: {html}");
    }
    let html = render(s.clone(), EG, View::Sheet(SheetMode::History));
    for when in ["النهارده", "امبارح", "من يومين", "من 4 أيام", "29 فبراير 2020"]
    {
        assert!(html.contains(when), "{when}: {html}");
    }
}

#[test]
fn the_offer_counts_its_builds_with_arabic_number_agreement() {
    let offered = |n: u64, lang| {
        let builds = (0..n).map(|i| build(200 - i, vec![change(ChangeKind::New, "Night mode")])).collect();
        let s = status(Phase::Available(offer(200, builds)));
        render(s, lang, View::Sheet(SheetMode::Offer))
    };
    let ar = UiLanguage::Ar;
    for (n, words) in [(1, "إصدار جديد"), (2, "إصداران جديدان"), (3, "إصدارات جديدة"), (11, "إصدارًا جديدًا")]
    {
        let html = offered(n, ar);
        assert!(html.contains(words), "{n}: {html}");
    }
    for (n, words) in [(1, "نسخة جديدة"), (2, "نسختين جداد"), (3, "نسخ جديدة"), (11, "نسخة جديدة")]
    {
        assert!(offered(n, EG).contains(words), "{n}");
    }
    assert!(offered(1, EN).contains("1 new build"));
    assert!(offered(3, EN).contains("3 new builds"));
}

thread_local! {
    static FOLLOWED: std::cell::Cell<Option<&'static super::Updater>> = const { std::cell::Cell::new(None) };
}

/// What a screen mounted in the shell sees of one updater.
fn follower() -> Element {
    let ui = provide_ui(EG);
    let u = FOLLOWED.with(std::cell::Cell::get);
    super::view::use_notice_context(ui, u);
    let status = super::view::use_status_of(u);
    let phase = status.read().as_ref().map(|s| format!("{:?}", s.phase)).unwrap_or_else(|| "none".into());
    rsx! { p { "phase: {phase}" } }
}

/// Lets the page's tasks and effects run, then draws what changed.
fn settle(dom: &mut TestDom) -> String {
    for _ in 0..5 {
        dom.run_until_quiet(std::time::Duration::from_millis(20), 1);
        dom.render();
    }
    dom.ssr()
}

#[test]
fn the_page_follows_the_updater_and_tells_it_the_learners_language() {
    let data = tempfile::tempdir().unwrap();
    let info = super::BuildInfo {
        version: super::Version::parse("1.0.0-alpha.1").unwrap(),
        build: 0,
        preview: false,
        key: None,
    };
    let u: &'static super::Updater = Box::leak(Box::new(super::Updater::new(
        data.path().join("updates"),
        info,
        Some(UpdatePlatform::AndroidArm64),
        None,
        None,
    )));
    FOLLOWED.with(|c| c.set(Some(u)));
    let mut dom = TestDom::unsettled(VirtualDom::new(follower));
    assert!(settle(&mut dom).contains("phase: Idle"));
    // The background job will word its notice in Egyptian Arabic, on the
    // coach's clock (no quiet hours before onboarding).
    let ctx = u.status().prefs.notice.expect("the updater was told");
    let offset = dom.peek_app(|| {
        let c = consume_context::<crate::coach::Coach>();
        (*c.device_offset.peek()).unwrap_or_else(|| c.offset())
    });
    assert_eq!((ctx.lang, ctx.quiet, ctx.utc_offset_min), (EG, None, offset));

    // A change on the updater's thread reaches the page.
    u.set_phase(Phase::Checking);
    assert!(settle(&mut dom).contains("phase: Checking"));
    u.set_phase(Phase::UpToDate);
    assert!(settle(&mut dom).contains("phase: UpToDate"));

    // Without an updater (a build that can't update), there is no status.
    FOLLOWED.with(|c| c.set(None));
    let mut dom = TestDom::unsettled(VirtualDom::new(follower));
    assert!(settle(&mut dom).contains("phase: none"));
}

/// An updater of its own for a test: signed builds, Android, and a feed
/// nobody answers on (a check fails at once, offline).
fn own_updater(build: u64) -> (&'static super::Updater, tempfile::TempDir) {
    let data = tempfile::tempdir().unwrap();
    let key = include_str!("../../../../fixtures/update/test-key.pub");
    let info = super::BuildInfo {
        version: super::Version::parse("1.0.0-alpha.1").unwrap(),
        build,
        preview: false,
        key: Some(super::parse_public_key(key).unwrap()),
    };
    let u = super::Updater::new(
        data.path().join("updates"),
        info,
        Some(UpdatePlatform::AndroidArm64),
        Some("http://127.0.0.1:9".into()),
        None,
    );
    (Box::leak(Box::new(u)), data)
}

#[derive(Clone, Copy, PartialEq)]
enum Mounted {
    Card,
    Banner,
    Sheet,
}

/// The update screens as the shell mounts them, on `FOLLOWED`'s updater.
fn shell(view: Mounted) -> Element {
    provide_ui(EN);
    let u = FOLLOWED.with(std::cell::Cell::get).expect("an updater");
    use_context_provider(|| super::view::UseUpdater(u));
    let nav = use_context_provider(UpdateSheetNav::new);
    let open = format!("[sheet: {:?}]", *nav.0.read());
    rsx! {
        match view {
            Mounted::Card => rsx! { super::view::UpdatesCard {} },
            Mounted::Banner => rsx! { super::view::UpdateBanner {} },
            Mounted::Sheet => rsx! { super::sheet::UpdateSheet {} },
        }
        p { "{open}" }
        p { "[releases: {super::sheet::releases_url()}]" }
        // What a tap on the update notification does in the shell.
        button { onclick: move |_| nav.open_offer(), "[notification tapped]" }
    }
}

fn mounted(u: &'static super::Updater, view: Mounted) -> Page {
    FOLLOWED.with(|c| c.set(Some(u)));
    let mut page = Page::of(VirtualDom::new_with_props(shell, view));
    // Its tasks start following the updater.
    page.settled();
    page
}

impl Page {
    fn settled(&mut self) -> String {
        decoded(settle(&mut self.dom))
    }
}

/// Waits (a little) for the updater's own thread.
fn eventually(what: &str, ok: impl Fn() -> bool) {
    for _ in 0..200 {
        if ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("never: {what}");
}

#[test]
fn the_learners_choices_on_the_card_reach_the_updater() {
    let (u, _data) = own_updater(0);
    let mut page = mounted(u, Mounted::Card);
    // The sheet's GitHub link goes to this channel's release page.
    assert!(page.html().contains("/releases/tag/update-alpha]"), "{}", page.html());
    assert_eq!(u.status().prefs.mode, Mode::Auto);
    let html = page.click(ut(EN, U::ModeNotify)).settled();
    assert_eq!(u.status().prefs.mode, Mode::Notify);
    assert!(has(&html, EN, U::ModeNotifyHint), "the card follows: {html}");
    page.click(ut(EN, U::ModeOff));
    assert_eq!(u.status().prefs.mode, Mode::Off);

    // Another channel looks again, off the UI thread.
    page.click(ut(EN, U::Beta));
    eventually("on beta", || u.status().channel == Channel::Beta);
    eventually("the look-again ended", || !matches!(u.status().phase, Phase::Idle | Phase::Checking));
    assert!(matches!(u.status().phase, Phase::Failed { why: Failure::Network, .. }), "offline here");
}

#[test]
fn later_puts_the_banner_away_until_the_next_version() {
    let (u, _data) = own_updater(0);
    u.set_mode(Mode::Notify);
    let o = offer(101, vec![build(101, vec![change(ChangeKind::New, "Night mode")])]);
    u.set_phase(Phase::Available(o.clone()));
    let mut page = mounted(u, Mounted::Banner);
    assert!(has(&page.html(), EN, U::Available));
    let html = page.click(ut(EN, U::Later)).settled();
    assert_eq!(u.status().prefs.dismissed, Some(o.key()));
    assert!(!has(&html, EN, U::Available), "{html}");
}

#[test]
fn after_an_update_the_sheet_opens_once_and_got_it_lets_it_go() {
    let (u, _data) = own_updater(2);
    let dated = |b| BuildChanges { ..build(b, vec![change(ChangeKind::New, &format!("Thing {b}"))]) };
    u.remember_history(&[dated(2), dated(1), dated(0)]);
    u.update_prefs(|p| p.whats_new_from = Some("1.0.0-alpha.1+0".into()));
    let mut page = mounted(u, Mounted::Sheet);
    let html = page.settled();
    assert!(html.contains("[sheet: Some(Updated)]") && has(&html, EN, U::UpdatedTitle), "{html}");
    assert!(html.contains("Thing 2") && html.contains("v1.0.0-alpha.1 (1)"), "{html}");

    let html = page.click(ut(EN, U::GotIt)).settled();
    assert!(html.contains("[sheet: None]") && !has(&html, EN, U::UpdatedTitle), "{html}");
    assert_eq!(u.status().prefs.whats_new_from, None, "seen");
    // It doesn't open again by itself.
    assert!(page.settled().contains("[sheet: None]"));
}

#[test]
fn a_tapped_notification_shows_the_offer_or_else_the_history() {
    let (u, _data) = own_updater(0);
    let mut page = mounted(u, Mounted::Banner);
    let html = page.click("[notification tapped]").settled();
    assert!(html.contains("[sheet: Some(History)]"), "nothing offered: {html}");

    let (u, _data) = own_updater(0);
    u.set_phase(Phase::Available(offer(101, Vec::new())));
    let mut page = mounted(u, Mounted::Banner);
    let html = page.click("[notification tapped]").settled();
    assert!(html.contains("[sheet: Some(Offer)]"), "{html}");
}

#[test]
fn the_banners_download_button_starts_the_download() {
    let (u, _data) = own_updater(0);
    u.set_mode(Mode::Notify);
    let o = offer(101, Vec::new());
    u.set_phase(Phase::Available(o.clone()));
    let mut page = mounted(u, Mounted::Banner);
    page.click(ut(EN, U::Download));
    // Offline here: the download fails and keeps the offer for a retry.
    eventually("the download ended", || matches!(u.status().phase, Phase::Failed { .. }));
    assert_eq!(u.status().phase, Phase::Failed { offer: Some(o), why: Failure::Network });
    // The Updates card says so and offers the retry.
    let html = mounted(u, Mounted::Card).html();
    assert!(has(&html, EN, U::FailedDownload) && has(&html, EN, U::TryAgain), "{html}");
}

#[test]
fn installing_from_the_banner_rechecks_the_download_first() {
    let (u, _data) = own_updater(0);
    u.set_mode(Mode::Notify);
    let o = offer(101, Vec::new());
    u.set_phase(Phase::Ready(o.clone()));
    let mut page = mounted(u, Mounted::Banner);
    page.click(ut(EN, install_label()));
    // The download isn't on disk any more: nothing is installed, and the
    // offer stays for a fresh download.
    eventually("the install was refused", || matches!(u.status().phase, Phase::Failed { .. }));
    let Phase::Failed { offer, why } = u.status().phase else { unreachable!() };
    assert_eq!(offer, Some(o));
    assert!(matches!(why, Failure::Verify(_) | Failure::Install(_)), "{why:?}");
}
