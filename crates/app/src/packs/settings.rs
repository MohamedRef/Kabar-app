//! Settings → Offline packs: every unit with its audio size, download and
//! remove buttons, auto-download, and the storage the clips take.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_protocol::pack::{PackIndex, PackSummary};

use super::{Job, NOT_CONFIGURED, base_url, enqueue, jobs, refresh_index, store};
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
    Intro,
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
        (Intro, ArEg) => "كل وحدة ليها باكة صوت بأصوات طبيعية. نزّلها مرة وتشتغل من غير نت.",
        (Intro, Ar) => "لكل وحدة حزمة صوت بأصوات طبيعية. نزّلها مرة واحدة لتعمل دون اتصال.",
        (Intro, En) => {
            "Each unit has a pack of natural-voice audio. Download it once and it works with no network."
        }
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
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let Some(store) = store() else { return rsx! {} };
    let mut index = use_signal(|| store.index());
    let mut installed = use_signal(|| store.installed_versions());
    let mut storage = use_signal(|| store.storage_bytes());
    let mut auto = use_signal(|| store.auto());
    let mut busy = use_signal(BTreeMap::<String, bool>::new);
    // Downloads run in the background queue; follow it while this shows.
    let mut queue = use_signal(jobs);
    let mut checking = use_signal(|| false);
    let mut notice = use_signal(|| None::<Notice>);
    let configured = base_url().is_some();

    let mut reload = move || {
        installed.set(store.installed_versions());
        storage.set(store.storage_bytes());
        index.set(store.index());
    };

    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
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
        enqueue(&id);
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
            let _ = std::thread::Builder::new().name("kabar-packs".into()).spawn(move || {
                let _ = tx.send(refresh_index(store).map(drop));
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
        section { class: "card", aria_labelledby: "packs-title",
            h2 { id: "packs-title", "{pt(lang, P::Title)}" }
            p { class: "muted", "{pt(lang, P::Intro)}" }
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
