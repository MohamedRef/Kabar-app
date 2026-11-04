//! Settings → Offline packs: every unit with its audio size, download and
//! remove buttons, auto-download, and the storage the clips take.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_protocol::pack::{PackIndex, PackSummary};

use super::{Job, NOT_CONFIGURED, PackStore, base_url, enqueue_in, jobs, refresh_index_from, store};
use crate::Ui;

const PACKS_CSS: &str = r#"
.packs-level { margin-block: 0.5rem; }
.packs-storage { display: flex; justify-content: space-between; gap: 0.5rem; margin-block: 0.5rem; }
.packs-auto { display: flex; align-items: center; gap: 0.75rem; }
.packs-level { border-block-start: 1px solid var(--line-soft); }
.packs-level > summary { cursor: pointer; font-weight: 600; min-block-size: 48px; display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; list-style: none; }
.packs-level > summary::-webkit-details-marker { display: none; }
.packs-level[open] > summary .chev { transform: rotate(135deg); }
.packs-list { list-style: none; margin: 0; padding: 0; display: grid; gap: 0.35rem; }
.packs-row { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; justify-content: space-between;
  padding-block: 0.35rem; padding-inline: 0.5rem; border-radius: 10px; background: var(--surface-2); }
.packs-name { display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: baseline; min-inline-size: 0; }
.packs-actions { display: flex; gap: 0.4rem; align-items: center; margin-inline-start: auto; }
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum P {
    Title,
    Auto,
    Storage,
    Download,
    Update,
    Remove,
    Downloaded,
    Downloading,
    NotReady,
    Check,
    Checking,
    Failed,
    NotConfigured,
    Clips,
    Queued,
    RowFailed,
    Retry,
}

fn pt(lang: UiLanguage, p: P) -> &'static str {
    use P::*;
    use UiLanguage::*;
    match (p, lang) {
        (Title, ArEg) => "الباكات للاستخدام من غير نت",
        (Title, Ar) => "الحزم دون اتصال",
        (Title, En) => "Offline packs",
        (Auto, ArEg) => "نزّل الوحدة الحالية واللي بعدها لوحدهم على الواي فاي",
        (Auto, Ar) => "نزّل الوحدة الحالية والتالية تلقائيًا عبر الواي فاي",
        (Auto, En) => "Download the current and next unit automatically on Wi-Fi",
        (Storage, ArEg) => "المساحة المستخدمة",
        (Storage, Ar) => "المساحة المستخدمة",
        (Storage, En) => "Storage used",
        (Download, ArEg) => "نزّل",
        (Download, Ar) => "تنزيل",
        (Download, En) => "Download",
        (Update, ArEg) => "حدّث",
        (Update, Ar) => "تحديث",
        (Update, En) => "Update",
        (Remove, ArEg) => "امسح",
        (Remove, Ar) => "إزالة",
        (Remove, En) => "Remove",
        (Downloaded, ArEg) => "متنزّلة",
        (Downloaded, Ar) => "منزّلة",
        (Downloaded, En) => "Downloaded",
        (Downloading, ArEg) => "بتنزل…",
        (Downloading, Ar) => "جارٍ التنزيل…",
        (Downloading, En) => "Downloading…",
        (NotReady, ArEg) => "الصوت لسه بيتجهّز",
        (NotReady, Ar) => "الصوت قيد التجهيز",
        (NotReady, En) => "Audio not ready yet",
        (Check, ArEg) => "شوف لو فيه جديد",
        (Check, Ar) => "تحقّق من الجديد",
        (Check, En) => "Check for new packs",
        (Checking, ArEg) => "بندوّر…",
        (Checking, Ar) => "جارٍ التحقق…",
        (Checking, En) => "Checking…",
        (Failed, ArEg) => "ماعرفناش نكمّل دلوقتي. الدرس هيشتغل بصوت الجهاز، وجرّب تاني لما النت يبقى أحسن.",
        (Failed, Ar) => "تعذّر الإكمال الآن. سيعمل الدرس بصوت الجهاز، وحاول مجددًا عندما يتحسن الاتصال.",
        (Failed, En) => {
            "Couldn't finish just now. Lessons use the device's voice meanwhile; try again when the connection is better."
        }
        (NotConfigured, ArEg) => "النسخة دي لسه مش متوصلة بمكان الباكات. الدروس شغالة بصوت الجهاز.",
        (NotConfigured, Ar) => "هذا الإصدار غير مرتبط بمصدر الحزم بعد. تعمل الدروس بصوت الجهاز.",
        (NotConfigured, En) => {
            "This build doesn't know where the packs live yet. Lessons use the device's voice."
        }
        (Queued, ArEg) => "مستنية دورها",
        (Queued, Ar) => "في الانتظار",
        (Queued, En) => "Queued",
        (RowFailed, ArEg) => "ماعرفناش ننزّلها",
        (RowFailed, Ar) => "تعذّر التنزيل",
        (RowFailed, En) => "Couldn't download",
        (Retry, ArEg) => "جرّب تاني",
        (Retry, Ar) => "أعد المحاولة",
        (Retry, En) => "Try again",
        (Clips, ArEg) => "مقطع",
        (Clips, Ar) => "مقطع",
        (Clips, En) => "clips",
    }
}

/// `1.4 MB`, `820 KB`.
fn size_text(ui: &Ui, bytes: u64) -> String {
    let s = if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    };
    ui.digits(&s)
}

fn level_of(unit: &str) -> &str {
    unit.split('.').next().unwrap_or(unit)
}

fn level_name(level: &str) -> String {
    if let Some(n) = level.strip_prefix("bipa") {
        format!("BIPA {n}")
    } else if let Some(n) = level.strip_prefix('j') {
        format!("Jawa J{n}")
    } else {
        level.to_owned()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Notice {
    Failed,
    NotConfigured,
}

#[component]
pub fn OfflinePacks() -> Element {
    let Some(store) = store() else { return rsx! {} };
    rsx! { PacksCard { store, base: base_url() } }
}

/// The card for `store`, downloading from the release at `base` (`None`:
/// this build doesn't know where the packs live).
#[component]
fn PacksCard(store: &'static PackStore, base: Option<String>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let base = use_signal(|| base);
    let mut index = use_signal(|| store.index());
    let mut installed = use_signal(|| store.installed_versions());
    let mut storage = use_signal(|| store.storage_bytes());
    let mut auto = use_signal(|| store.auto());
    let mut busy = use_signal(BTreeMap::<String, bool>::new);
    // Downloads run in the background queue; follow it while this shows.
    let mut queue = use_signal(jobs);
    let mut checking = use_signal(|| false);
    let mut notice = use_signal(|| None::<Notice>);
    let configured = base.peek().is_some();

    let mut reload = move || {
        installed.set(store.installed_versions());
        storage.set(store.storage_bytes());
        index.set(store.index());
    };

    use_future(move || async move {
        loop {
            crate::platform::time::sleep(std::time::Duration::from_millis(400)).await;
            let now = jobs();
            if *queue.peek() != now {
                // Something finished: show what's installed now.
                reload();
                queue.set(now);
            }
        }
    });

    let mut download = move |id: String| {
        notice.set(None);
        enqueue_in(store, base.peek().clone(), &id);
        queue.set(jobs());
    };

    let mut remove = move |id: String| {
        busy.write().insert(id.clone(), true);
        notice.set(None);
        spawn(async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            let job = id.clone();
            let _ = std::thread::Builder::new().name("kabar-packs".into()).spawn(move || {
                let unit = job.split_once('/').map_or(job.as_str(), |(_, u)| u).to_owned();
                let _ = tx.send(store.remove(&unit));
            });
            let result = rx.await.unwrap_or_else(|_| Err("stopped".into()));
            busy.write().remove(&id);
            if let Err(e) = result {
                crate::diag::report!("pack {id}: {e}");
                notice.set(Some(if e == NOT_CONFIGURED { Notice::NotConfigured } else { Notice::Failed }));
            }
            reload();
        });
    };

    let check = move |_| {
        checking.set(true);
        notice.set(None);
        spawn(async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            let base = base.peek().clone();
            let _ = std::thread::Builder::new().name("kabar-packs".into()).spawn(move || {
                let fresh = match base {
                    Some(base) => refresh_index_from(store, &base),
                    None => Err(NOT_CONFIGURED.into()),
                };
                let _ = tx.send(fresh.map(drop));
            });
            if let Ok(Err(e)) = rx.await {
                crate::diag::report!("pack index: {e}");
                notice.set(Some(Notice::Failed));
            }
            checking.set(false);
            reload();
        });
    };

    let idx: PackIndex = index.read().clone();
    let mut levels: Vec<(String, Vec<PackSummary>)> = Vec::new();
    for p in idx.packs {
        let level = level_of(&p.unit).to_owned();
        match levels.last_mut() {
            Some((l, rows)) if *l == level => rows.push(p),
            _ => levels.push((level, vec![p])),
        }
    }
    let installed_now = installed.read().clone();
    let queue_now = queue.read().clone();
    let levels: Vec<(String, Vec<PackSummary>, bool)> = levels
        .into_iter()
        .map(|(l, rows)| {
            let open =
                rows.iter().any(|r| installed_now.contains_key(&r.id) || queue_now.contains_key(&r.id));
            (l, rows, open)
        })
        .collect();
    let busy_now = busy.read().clone();
    let total = *storage.read();

    rsx! {
        style { {PACKS_CSS} }
        crate::ui::Card { class: "", aria_labelledby: "packs-title",
            h2 { id: "packs-title", "{pt(lang, P::Title)}" }
            div { class: "packs-storage",
                span { class: "label", "{pt(lang, P::Storage)}" }
                span { class: "ltr-count", "{size_text(&ui, total)}" }
            }
            div { class: "field",
                label { class: "packs-auto",
                    input {
                        class: "switch",
                        role: "switch",
                        r#type: "checkbox",
                        checked: *auto.read(),
                        onchange: move |e: FormEvent| {
                            let on = e.checked();
                            if store.set_auto(on).is_ok() {
                                auto.set(on);
                            }
                        },
                    }
                    span { "{pt(lang, P::Auto)}" }
                }
            }
            if !configured {
                p { class: "muted", role: "status", "{pt(lang, P::NotConfigured)}" }
            } else {
                div { class: "row",
                    button { class: "outline", disabled: *checking.read(), onclick: check,
                        if *checking.read() { "{pt(lang, P::Checking)}" } else { "{pt(lang, P::Check)}" }
                    }
                }
            }
            match *notice.read() {
                Some(Notice::Failed) => rsx! { p { class: "muted", role: "status", "{pt(lang, P::Failed)}" } },
                Some(Notice::NotConfigured) => rsx! { p { class: "muted", role: "status", "{pt(lang, P::NotConfigured)}" } },
                None => rsx! {},
            }
            for (level, rows, open) in levels {
                details {
                    key: "{level}",
                    class: "packs-level",
                    open,
                    summary {
                        bdi { class: "latin", "{level_name(&level)}" }
                        span { class: "chev" }
                    }
                    ul { class: "packs-list",
                        for row in rows {
                            PackRow {
                                key: "{row.id}",
                                row: row.clone(),
                                installed: installed_now.get(&row.id).copied(),
                                busy: busy_now.contains_key(&row.id),
                                job: queue_now.get(&row.id).cloned(),
                                configured,
                                on_download: move |id: String| download(id),
                                on_remove: move |id: String| remove(id),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PackRow(
    row: PackSummary,
    installed: Option<u32>,
    busy: bool,
    job: Option<Job>,
    configured: bool,
    on_download: EventHandler<String>,
    on_remove: EventHandler<String>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let size = row.file_size.unwrap_or(row.audio_bytes);
    let ready = row.downloadable();
    let outdated = installed.is_some_and(|v| v < row.version);
    let clips = ui.num(row.clips);
    let (id1, id2) = (row.id.clone(), row.id.clone());
    rsx! {
        li { class: "packs-row",
            span { class: "packs-name",
                if let Some(title) = crate::journey::unit_title(lang, &row.unit) {
                    span { "{title}" }
                } else {
                    bdi { class: "latin", "{row.unit}" }
                }
                if let Some(stop) = &row.stop {
                    span { class: "muted", bdi { class: "latin", "{stop}" } }
                }
                span { class: "muted lp-small",
                    if ready { span { class: "ltr-count", "{size_text(&ui, size)}" } " · " }
                    "{clips} {pt(lang, P::Clips)}"
                }
            }
            span { class: "packs-actions",
                if busy || job == Some(Job::Downloading) {
                    span { class: "status-chip accent", role: "status", "{pt(lang, P::Downloading)}" }
                } else if job == Some(Job::Queued) {
                    span { class: "status-chip neutral", role: "status", "{pt(lang, P::Queued)}" }
                } else if job == Some(Job::Failed) && configured {
                    span { class: "status-chip bad", role: "status", "{pt(lang, P::RowFailed)}" }
                    button { class: "tonal", onclick: move |_| on_download.call(id1.clone()), "{pt(lang, P::Retry)}" }
                } else if installed.is_some() {
                    if outdated && configured {
                        button { class: "tonal", onclick: move |_| on_download.call(id1.clone()), "{pt(lang, P::Update)}" }
                    } else {
                        span { class: "status-chip good", "✓ {pt(lang, P::Downloaded)}" }
                    }
                    button { class: "outline", onclick: move |_| on_remove.call(id2.clone()), "{pt(lang, P::Remove)}" }
                } else if !ready {
                    span { class: "muted", "{pt(lang, P::NotReady)}" }
                } else if configured {
                    button { class: "tonal", onclick: move |_| on_download.call(id1.clone()), "{pt(lang, P::Download)}" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use kabar_protocol::pack::PACK_FORMAT;

    use super::super::tests::{one_at_a_time, pack, pack_v, release};
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

    fn mount(store: &'static PackStore, base: Option<String>) -> VirtualDom {
        CARD.with(|c| *c.borrow_mut() = Some((store, base)));
        td::mount(Setup::new(crate::store::Store::open_in_memory().unwrap(), Screen::Settings, view))
    }

    /// The row of `unit` on the page.
    fn row<'a>(html: &'a str, unit: &str) -> &'a str {
        let title = crate::journey::unit_title(EN, unit).unwrap();
        let at = html.find(&td::esc(&title)).unwrap_or_else(|| panic!("no row for {unit}: {html}"));
        let start = html[..at].rfind("<li class=\"packs-row\"").unwrap();
        let end = html[at..].find("</li>").map_or(html.len(), |e| at + e);
        &html[start..end]
    }

    #[test]
    fn without_a_packs_address_lessons_use_the_device_voice() {
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
    fn a_failed_check_says_lessons_still_play() {
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
    }

    #[test]
    fn an_older_installed_pack_offers_the_update() {
        let store = new_store();
        let (_, _, row1, tar) = pack();
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
    }

    #[test]
    fn auto_download_is_a_switch_remembered_on_the_device() {
        let store = new_store();
        let mut dom = mount(store, None);
        let switch = td::find(&dom, "change", |l| l.class() == "switch");
        assert_eq!(switch.attr("checked"), Some("true"), "on by default");
        td::change(&mut dom, |l| l.class() == "switch", "false");
        assert!(!store.auto());
        let switch = td::find(&dom, "change", |l| l.class() == "switch");
        assert_eq!(switch.attr("checked"), Some("false"));
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
        assert!(td::shows(&row_html(None, true, None, true), pt(EN, P::Downloading)));
        assert!(td::shows(&row_html(None, false, Some(Job::Downloading), true), pt(EN, P::Downloading)));
        assert!(td::shows(&row_html(None, false, Some(Job::Queued), true), pt(EN, P::Queued)));
        let failed = row_html(None, false, Some(Job::Failed), true);
        assert!(td::shows(&failed, pt(EN, P::RowFailed)) && td::shows(&failed, pt(EN, P::Retry)), "{failed}");
        // A failure with no address to retry from just offers nothing new.
        let failed = row_html(None, false, Some(Job::Failed), false);
        assert!(!td::shows(&failed, pt(EN, P::Retry)), "{failed}");
        assert!(td::shows(&row_html(Some(1), false, None, true), pt(EN, P::Downloaded)));
        assert!(td::shows(&row_html(None, false, None, true), pt(EN, P::Download)));
    }

    #[test]
    fn no_card_before_the_pack_store_opens() {
        fn view(screen: Screen) -> Option<Element> {
            (screen == Screen::Settings).then(|| rsx! { div { id: "packs", OfflinePacks {} } })
        }
        let dom =
            td::mount(Setup::new(crate::store::Store::open_in_memory().unwrap(), Screen::Settings, view));
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
            Downloaded,
            Downloading,
            NotReady,
            Check,
            Checking,
            Failed,
            NotConfigured,
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
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        assert_eq!(td::html(&dom), "<p>١ KB|٢ KB|١.٥ MB</p>");
    }
}
