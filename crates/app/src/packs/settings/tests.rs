use std::cell::RefCell;
use std::collections::BTreeMap;

use kabar_protocol::pack::PACK_FORMAT;

use super::super::tests::{one_at_a_time, pack, pack_v, release, serve};
use super::*;
use crate::Screen;
use crate::test_screens::{self as td, Setup};

const EN: UiLanguage = UiLanguage::En;

thread_local! {
    static CARD: RefCell<Option<(&'static PackStore, Option<String>)>> = const { RefCell::new(None) };
}

fn view(screen: Screen) -> Option<Element> {
    (screen == Screen::Settings).then(|| {
        let (store, base) = CARD.with(|c| c.borrow().clone()).unwrap();
        rsx! { PacksCard { store, base } }
    })
}

/// A pack store in its own folder, for the life of the test binary.
fn new_store() -> &'static PackStore {
    let dir = Box::leak(Box::new(tempfile::tempdir().unwrap()));
    Box::leak(Box::new(PackStore::new(dir.path())))
}

fn mount(store: &'static PackStore, base: Option<String>) -> crate::test_dom::TestDom {
    CARD.with(|c| *c.borrow_mut() = Some((store, base)));
    td::mount(Setup::new(crate::store::Store::open_in_memory().unwrap(), Screen::Settings, view))
}

/// The row of `unit` on the page.
fn row<'a>(html: &'a str, unit: &str) -> &'a str {
    let needle = crate::journey::unit_title(EN, unit).map_or_else(|| unit.to_owned(), |t| td::esc(&t));
    let at = html.find(&needle).unwrap_or_else(|| panic!("no row for {unit}: {html}"));
    let start = html[..at].rfind("<li class=\"packs-row\"").unwrap();
    let end = html[at..].find("</li>").map_or(html.len(), |e| at + e);
    &html[start..end]
}

#[test]
fn without_a_packs_address_lessons_use_the_device_voice() {
    let _turn = one_at_a_time();
    let store = new_store();
    let dom = mount(store, None);
    let html = td::html(&dom);
    assert!(td::shows(&html, pt(EN, P::Title)), "{html}");
    assert!(td::shows(&html, pt(EN, P::NotConfigured)), "{html}");
    assert!(!td::shows(&html, pt(EN, P::Check)), "nothing to check: {html}");
    assert!(!td::shows(&html, &format!(">{}<", pt(EN, P::Download))), "no download buttons: {html}");
    assert!(html.contains(">0 KB<"), "nothing stored yet: {html}");
    // Every unit of the built-in index is listed, by level.
    let index = super::super::embedded_index();
    assert_eq!(html.matches("class=\"packs-row\"").count(), index.packs.len());
    assert!(html.contains(">BIPA 1<") && html.contains(">Jawa J1<"), "{html}");
    assert!(!html.contains("open"), "nothing installed or queued, every level folded: {html}");
}

#[test]
fn a_unit_downloads_shows_its_size_and_can_be_removed() {
    let _turn = one_at_a_time();
    let store = new_store();
    let (_, _, row1, tar) = pack();
    let not_ready = PackSummary {
        id: "id/bipa1.u02".into(),
        unit: "bipa1.u02".into(),
        file_sha256: None,
        missing: 3,
        ..row1.clone()
    };
    let (url, log) = release(vec![row1.clone(), not_ready], &[(&row1.file, &tar)]);
    let mut dom = mount(store, Some(url.clone()));
    // Check for new packs: the release's index replaces the built-in one.
    td::click(&mut dom, pt(EN, P::Check));
    assert!(td::shows(&td::html(&dom), pt(EN, P::Checking)), "says it's checking");
    td::settle(&mut dom, 800);
    let html = td::html(&dom);
    assert_eq!(html.matches("class=\"packs-row\"").count(), 2, "{html}");
    assert!(td::shows(row(&html, "bipa1.u02"), pt(EN, P::NotReady)), "{html}");
    let first = row(&html, "bipa1.u01");
    assert!(first.contains(&format!("{} {}", row1.clips, pt(EN, P::Clips))), "{first}");
    assert!(first.contains(&format!(">{}<", size_text_plain(row1.file_size.unwrap()))), "{first}");

    // Download: queued, then installed by the background queue.
    td::click_where(&mut dom, |l| l.text == pt(EN, P::Download));
    td::settle(&mut dom, 1500);
    let html = td::html(&dom);
    let first = row(&html, "bipa1.u01");
    assert!(td::shows(first, pt(EN, P::Downloaded)), "{html}");
    assert!(log.lock().unwrap().contains(&row1.file));
    assert!(store.installed_versions().contains_key(&row1.id));
    assert!(html.contains("<details class=\"packs-level\" open"), "its level opens: {html}");
    assert!(!html.contains(">0 KB<"), "storage counts the clips: {html}");

    // Remove: the clips go and the row offers the download again.
    td::click_where(&mut dom, |l| l.text == pt(EN, P::Remove));
    td::settle(&mut dom, 800);
    let html = td::html(&dom);
    assert!(store.installed().is_empty());
    assert!(td::shows(row(&html, "bipa1.u01"), pt(EN, P::Download)), "{html}");
    assert!(html.contains(">0 KB<"), "{html}");
}

fn size_text_plain(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1048576.0)
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

#[test]
fn index_checks_tell_404_newer_format_and_network_apart() {
    let _turn = one_at_a_time();
    // Network unreachable.
    let store = new_store();
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    let mut dom = mount(store, Some(closed));
    td::click(&mut dom, pt(EN, P::Check));
    td::settle(&mut dom, 800);
    let html = td::html(&dom);
    assert!(td::shows(&html, pt(EN, P::Failed)), "{html}");
    assert!(td::shows(&html, pt(EN, P::Check)), "the button is back: {html}");

    // Release not published yet (404).
    let (empty_url, _) = serve(BTreeMap::new());
    let mut dom = mount(new_store(), Some(empty_url));
    td::click(&mut dom, pt(EN, P::Check));
    td::settle(&mut dom, 800);
    assert!(td::shows(&td::html(&dom), pt(EN, P::NotPublished)));

    // Newer index format.
    let newer = PackIndex { format: PACK_FORMAT + 1, packs: vec![pack().2] };
    let mut files = BTreeMap::new();
    files.insert("index.json".to_owned(), serde_json::to_vec(&newer).unwrap());
    let (newer_url, _) = serve(files);
    let mut dom = mount(new_store(), Some(newer_url));
    td::click(&mut dom, pt(EN, P::Check));
    td::settle(&mut dom, 800);
    assert!(td::shows(&td::html(&dom), pt(EN, P::TooNew)));
}

#[test]
fn an_older_installed_pack_offers_the_update_and_dropped_units_stay_removable() {
    let _turn = one_at_a_time();
    let store = new_store();
    let (m1, _, row1, tar) = pack();
    store.install(&row1, &tar).unwrap();
    let (_, _, row2, _) = pack_v(2);
    store.save_index(&PackIndex { format: PACK_FORMAT, packs: vec![row2] }).unwrap();
    let dom = mount(store, Some("http://127.0.0.1:9".into()));
    let html = td::html(&dom);
    let r = row(&html, "bipa1.u01");
    assert!(td::shows(r, pt(EN, P::Update)) && td::shows(r, pt(EN, P::Remove)), "{r}");
    // Without a packs address there's nothing to update from.
    let dom = mount(store, None);
    let html = td::html(&dom);
    let r = row(&html, "bipa1.u01");
    assert!(!td::shows(r, pt(EN, P::Update)) && td::shows(r, pt(EN, P::Downloaded)), "{r}");

    // An installed unit omitted from a newer index still appears so it can be removed.
    let mut retired = m1;
    retired.id = "id/bipa1.u99".into();
    retired.unit = "bipa1.u99".into();
    let rows = all_rows(PackIndex { format: PACK_FORMAT, packs: vec![row1] }, &[retired]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].unit, "bipa1.u99");
}

#[test]
fn auto_download_is_a_switch_remembered_on_the_device() {
    let _turn = one_at_a_time();
    let store = new_store();
    let mut dom = mount(store, None);
    let switch = td::find(&dom, "change", |l| l.class() == "switch");
    assert_eq!(switch.attr("checked"), Some("true"), "on by default");
    td::change(&mut dom, |l| l.class() == "switch", "false");
    assert!(!store.auto());
    let switch = td::find(&dom, "change", |l| l.class() == "switch");
    assert_eq!(switch.attr("checked"), None, "off: a boolean attribute goes away");
}

/// A row's state: (installed version, busy, job, configured).
type RowState = (Option<u32>, bool, Option<Job>, bool);

thread_local! {
    static ROW: RefCell<Option<RowState>> = const { RefCell::new(None) };
}

fn row_view(screen: Screen) -> Option<Element> {
    (screen == Screen::Settings).then(|| {
        let (installed, busy, job, configured) = ROW.with(|r| r.borrow().clone()).unwrap();
        let row = pack().2;
        rsx! { PackRow { row, installed, busy, job, configured, on_download: |_| {}, on_remove: |_| {} } }
    })
}

fn row_html(installed: Option<u32>, busy: bool, job: Option<Job>, configured: bool) -> String {
    ROW.with(|r| *r.borrow_mut() = Some((installed, busy, job, configured)));
    let dom =
        td::mount(Setup::new(crate::store::Store::open_in_memory().unwrap(), Screen::Settings, row_view));
    td::html(&dom)
}

#[test]
fn each_row_says_where_its_download_stands() {
    let _turn = one_at_a_time();
    assert!(td::shows(&row_html(None, true, None, true), pt(EN, P::Removing)));
    assert!(td::shows(&row_html(None, false, Some(Job::Downloading), true), pt(EN, P::Downloading)));
    assert!(td::shows(&row_html(None, false, Some(Job::Queued), true), pt(EN, P::Queued)));
    let failed = row_html(None, false, Some(Job::Failed), true);
    assert!(td::shows(&failed, pt(EN, P::RowFailed)) && td::shows(&failed, pt(EN, P::Retry)), "{failed}");
    // A failed update on an installed pack still shows Downloaded and Remove.
    let failed_installed = row_html(Some(1), false, Some(Job::Failed), true);
    assert!(
        td::shows(&failed_installed, pt(EN, P::Downloaded))
            && td::shows(&failed_installed, pt(EN, P::RowFailed))
            && td::shows(&failed_installed, pt(EN, P::Retry))
            && td::shows(&failed_installed, pt(EN, P::Remove)),
        "{failed_installed}"
    );
    // A failure with no address to retry from just offers nothing new.
    let failed = row_html(None, false, Some(Job::Failed), false);
    assert!(!td::shows(&failed, pt(EN, P::Retry)), "{failed}");
    assert!(td::shows(&row_html(Some(1), false, None, true), pt(EN, P::Downloaded)));
    assert!(td::shows(&row_html(None, false, None, true), pt(EN, P::Download)));
}

#[test]
fn no_card_before_the_pack_store_opens() {
    let _turn = one_at_a_time();
    fn view(screen: Screen) -> Option<Element> {
        (screen == Screen::Settings).then(|| rsx! { div { id: "packs", OfflinePacks {} } })
    }
    let dom = td::mount(Setup::new(crate::store::Store::open_in_memory().unwrap(), Screen::Settings, view));
    assert!(td::html(&dom).contains("<div id=\"packs\"></div>"), "{}", td::html(&dom));
}

#[test]
fn sizes_and_level_names_read_naturally() {
    assert_eq!(level_of("bipa2.u03"), "bipa2");
    assert_eq!(level_name("bipa2"), "BIPA 2");
    assert_eq!(level_name("j3"), "Jawa J3");
    assert_eq!(level_name("misc"), "misc");
    // Every line in every language.
    use P::*;
    for p in [
        Title,
        Auto,
        Storage,
        Download,
        Update,
        Remove,
        Removing,
        Downloaded,
        Downloading,
        NotReady,
        Check,
        Checking,
        Failed,
        NotConfigured,
        NotPublished,
        TooNew,
        Clips,
        Queued,
        RowFailed,
        Retry,
    ] {
        for lang in [UiLanguage::En, UiLanguage::ArEg, UiLanguage::Ar] {
            assert!(!pt(lang, p).trim().is_empty(), "{p:?} {lang:?}");
        }
        assert_ne!(pt(UiLanguage::En, p), pt(UiLanguage::ArEg, p), "{p:?} is translated");
    }
    fn app() -> Element {
        let ui = use_context_provider(|| Ui {
            settings: Signal::new(kabar_core::settings::Settings {
                ui_language: UiLanguage::ArEg,
                eastern_digits: true,
                ..Default::default()
            }),
            width: Signal::new(kabar_core::layout::WidthClass::Compact),
            tablet: Signal::new(false),
            screen: Signal::new(Screen::Settings),
            lesson: Signal::new(None),
            lesson_from: Signal::new(Screen::Settings),
            focus: Signal::new(false),
        });
        rsx! { p { "{size_text(&ui, 1)}|{size_text(&ui, 1536)}|{size_text(&ui, 3 * 1024 * 1024 / 2)}" } }
    }
    let dom = crate::test_dom::TestDom::unsettled(VirtualDom::new(app));
    assert_eq!(td::html(&dom), "<p>١ KB|٢ KB|١.٥ MB</p>");
}
