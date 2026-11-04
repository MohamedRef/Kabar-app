//! Settings → Offline & storage: one click writes everything local to
//! `kabar-export-YYYY-MM-DD.json` (spec §7). The document comes from
//! [`Store::export`]; this module names, writes and reports the file.

use std::io::Write;
use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use kabar_protocol::coach::LocalDate;

use crate::coach::Coach;
use crate::i18n::Msg;
use crate::store::Store;
use crate::{Db, Ui};

/// `kabar-export-2026-10-26.json`, or `…-2.json`, `…-3.json` when one exists.
pub fn file_path(dir: &Path, date: LocalDate) -> PathBuf {
    let base = format!("kabar-export-{date}");
    let first = dir.join(format!("{base}.json"));
    if !first.exists() {
        return first;
    }
    (2..).map(|n| dir.join(format!("{base}-{n}.json"))).find(|p| !p.exists()).unwrap_or(first)
}

/// Writes the export into `dir` (created if missing); returns the path and
/// the number of rows. Written to a temporary file first, so a failure never
/// leaves half a file behind.
pub fn save(store: &Store, dir: &Path, now_ms: u64, utc_offset_min: i16) -> Result<(PathBuf, u64), String> {
    let export = store.export(env!("CARGO_PKG_VERSION"), now_ms).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = file_path(dir, LocalDate::from_unix_ms(now_ms, utc_offset_min));
    let tmp = path.with_extension("json.part");
    let write = || -> std::io::Result<()> {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        serde_json::to_writer_pretty(&mut out, &export.document)?;
        out.write_all(b"\n")?;
        out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        std::fs::rename(&tmp, &path)
    };
    write().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", path.display())
    })?;
    Ok((path, export.rows))
}

/// The web: the same export as a browser download (to the browser's
/// Downloads); returns its name and the number of rows.
#[cfg(target_arch = "wasm32")]
fn download(store: &Store, now_ms: u64, utc_offset_min: i16) -> Result<(PathBuf, u64), String> {
    let export = store.export(env!("CARGO_PKG_VERSION"), now_ms).map_err(|e| e.to_string())?;
    let mut text = serde_json::to_vec_pretty(&export.document).map_err(|e| e.to_string())?;
    text.push(b'\n');
    let name = format!("kabar-export-{}.json", LocalDate::from_unix_ms(now_ms, utc_offset_min));
    crate::platform::web::download(&name, "application/json", &text).map(|n| (PathBuf::from(n), export.rows))
}

#[derive(Clone, Debug, PartialEq)]
enum Outcome {
    Saved {
        path: String,
        rows: u64,
    },
    /// Android: written, and the system "Save to" dialog is showing.
    Choosing,
    /// Android: copied to where the learner chose, under this name.
    Placed {
        name: Option<String>,
        rows: u64,
    },
    Failed(String),
}

#[component]
pub fn ExportCard() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut outcome = use_signal(|| None::<Outcome>);
    let mut busy = use_signal(|| false);

    let run = move |_| {
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        let result = download(&db.0.borrow(), coach.now(), coach.offset());
        #[cfg(not(target_arch = "wasm32"))]
        let result = platform_dir().and_then(|dir| save(&db.0.borrow(), &dir, coach.now(), coach.offset()));
        if crate::platform::SAVE_DIALOG
            && let Ok((path, rows)) = &result
        {
            let (path, rows) = (path.clone(), *rows);
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if crate::platform::save_document(&path, &name).is_ok() {
                outcome.set(Some(Outcome::Choosing));
                spawn(async move {
                    use crate::platform::SaveState;
                    let state = loop {
                        crate::platform::time::sleep(std::time::Duration::from_millis(300)).await;
                        match crate::platform::save_state() {
                            SaveState::Open => {}
                            other => break other,
                        }
                    };
                    outcome.set(Some(match state {
                        SaveState::Saved(name) => Outcome::Placed { name, rows },
                        // Cancelled or failed: the copy in Kabar's folder is still there.
                        _ => Outcome::Saved { path: path.display().to_string(), rows },
                    }));
                    busy.set(false);
                });
                return;
            }
        }
        outcome.set(Some(match result {
            Ok((path, rows)) => Outcome::Saved { path: path.display().to_string(), rows },
            Err(e) => {
                crate::diag::report!("export failed: {e}");
                Outcome::Failed(e)
            }
        }));
        busy.set(false);
    };

    rsx! {
        crate::ui::DivCard { class: "export-card",
            h2 { "{ui.t(Msg::ExportTitle)}" }
            div { class: "row",
                button { class: "primary", disabled: busy(), onclick: run,
                    if busy() { "{ui.t(Msg::ExportSaving)}" } else { "{ui.t(Msg::ExportButton)}" }
                }
            }
            if let Some(outcome) = outcome() {
                ExportResult { outcome }
            }
        }
    }
}

/// What the last export did: where the file went, or why it didn't.
#[component]
fn ExportResult(outcome: Outcome) -> Element {
    let ui = use_context::<Ui>();
    match outcome {
        Outcome::Saved { path, rows } => rsx! {
            div { class: "export-result", role: "status", aria_live: "polite",
                p { {ui.fill(Msg::ExportSaved, &[("rows", &ui.num(rows))])} }
                code { class: "export-path", dir: "ltr", bdi { "{path}" } }
                if cfg!(target_os = "android") {
                    p { class: "muted", "{ui.t(Msg::ExportAndroidHint)}" }
                }
            }
        },
        Outcome::Choosing => rsx! {
            p { class: "muted", role: "status", aria_live: "polite", "{placed_text(ui.lang(), true)}" }
        },
        Outcome::Placed { name, rows } => rsx! {
            div { class: "export-result", role: "status", aria_live: "polite",
                p { {crate::i18n::fill(ui.lang(), placed_text(ui.lang(), false), &[("rows", &ui.num(rows))])} }
                if let Some(name) = name {
                    code { class: "export-path", dir: "ltr", bdi { "{name}" } }
                }
            }
        },
        Outcome::Failed(e) => rsx! {
            div { class: "notice", role: "status",
                p { "{ui.t(Msg::ExportFailed)}" }
                p { class: "muted mono", dir: "ltr", bdi { "{e}" } }
            }
        },
    }
}

/// Android's "Save to" step: waiting on the dialog, or done.
fn placed_text(lang: kabar_core::locale::UiLanguage, waiting: bool) -> &'static str {
    use kabar_core::locale::UiLanguage::*;
    match (waiting, lang) {
        (true, ArEg) => "اختار تحفظه فين (التنزيلات أو Drive مثلًا)…",
        (true, Ar) => "اختر مكان الحفظ (التنزيلات أو Drive مثلًا)…",
        (true, En) => "Choose where to save it (Downloads or Drive, say)…",
        (false, ArEg) => "اتحفظ {rows|صف|صفين|صفوف} في المكان اللي اخترته، وتقدر تفتحه من تطبيق الملفات:",
        (false, Ar) => "حُفظ {rows|صفّ|صفّان|صفوف|صفًّا} في المكان الذي اخترته، ويمكنك فتحه من تطبيق الملفات:",
        (false, En) => "Saved {rows|row|rows} where you chose; open it from the Files app:",
    }
}

fn platform_dir() -> Result<PathBuf, String> {
    // Tests export into a temporary folder, never the real Downloads.
    #[cfg(test)]
    if let Some(dir) = tests::EXPORT_DIR.with(|d| d.borrow().clone()) {
        return dir;
    }
    crate::platform::export_dir()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        /// Where `ExportCard` writes in this test thread.
        pub static EXPORT_DIR: RefCell<Option<Result<PathBuf, String>>> = const { RefCell::new(None) };
    }

    #[test]
    fn saves_one_dated_file_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory().unwrap();
        let now = 1_793_011_856_000; // 2026-10-26T10:50:56Z
        // Late evening in Jakarta is already the 27th there.
        let (first, rows) = save(&store, dir.path(), now, 120).unwrap();
        assert_eq!(first.file_name().unwrap(), "kabar-export-2026-10-26.json");
        assert!(rows >= 1, "the meta row with the device id is always there");
        let (second, _) = save(&store, dir.path(), now, 120).unwrap();
        assert_eq!(second.file_name().unwrap(), "kabar-export-2026-10-26-2.json");
        let (jakarta, _) = save(&store, dir.path(), now + 10 * 3_600_000, 420).unwrap();
        assert_eq!(jakarta.file_name().unwrap(), "kabar-export-2026-10-27.json");

        let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(&first).unwrap()).unwrap();
        assert_eq!(doc["device_id"], store.device_id().0);
        assert_eq!(doc["app_version"], env!("CARGO_PKG_VERSION"));
        let leftovers = std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| e.as_ref().unwrap().path().to_string_lossy().ends_with(".part"))
            .count();
        assert_eq!(leftovers, 0);
    }

    fn ui(lang: kabar_core::locale::UiLanguage) -> Ui {
        use kabar_core::layout::WidthClass;
        Ui {
            settings: Signal::new(kabar_core::settings::Settings { ui_language: lang, ..Default::default() }),
            width: Signal::new(WidthClass::Compact),
            tablet: Signal::new(false),
            screen: Signal::new(crate::Screen::Today),
            lesson: Signal::new(None),
            lesson_from: Signal::new(crate::Screen::Today),
            focus: Signal::new(false),
        }
    }

    fn card() -> Element {
        use std::{cell::RefCell, rc::Rc};
        let db = use_context_provider(|| Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap()))));
        use_context_provider(|| ui(kabar_core::locale::UiLanguage::En));
        use_context_provider(|| Coach::load(&db));
        rsx! { ExportCard {} }
    }

    #[test]
    fn the_export_button_saves_a_file_and_says_where() {
        let dir = tempfile::tempdir().unwrap();
        EXPORT_DIR.with(|d| *d.borrow_mut() = Some(Ok(dir.path().join("Downloads"))));
        let mut h = crate::screen_kit::Harness::new(card);
        assert!(!h.html().contains("role=\"status\""), "nothing to report before the first export");
        h.click("Export everything");
        let html = h.html();
        let file = std::fs::read_dir(dir.path().join("Downloads")).unwrap().next().unwrap().unwrap().path();
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("kabar-export-") && name.ends_with(".json"), "{name}");
        assert!(html.contains(&name) && html.contains("Saved "), "{html}");
        assert!(!html.contains("disabled"), "the button is ready again");
        let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(doc["app_version"], env!("CARGO_PKG_VERSION"));
        EXPORT_DIR.with(|d| *d.borrow_mut() = None);
    }

    #[test]
    fn a_failed_export_says_so_and_changes_nothing() {
        EXPORT_DIR.with(|d| *d.borrow_mut() = Some(Err("no Downloads folder".into())));
        let mut h = crate::screen_kit::Harness::new(card);
        h.click("Export everything");
        let html = h.html();
        assert!(
            html.contains("Couldn&#39;t save the file. Nothing was changed; want to try again?"),
            "{html}"
        );
        assert!(html.contains("no Downloads folder"));
        EXPORT_DIR.with(|d| *d.borrow_mut() = None);
    }

    #[test]
    fn a_read_only_folder_leaves_no_half_file() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "x").unwrap();
        let store = Store::open_in_memory().unwrap();
        // A file where the folder should be: creating the folder fails.
        let err = save(&store, &blocker.join("sub"), 1_793_011_856_000, 0).unwrap_err();
        assert!(err.contains("sub"), "the error names the folder: {err}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    /// Android's Save-to steps, in each language.
    #[test]
    fn android_save_steps_read_right() {
        use crate::i18n::checks::{LANGS, problems};
        for lang in LANGS {
            for waiting in [true, false] {
                let text = placed_text(lang, waiting);
                assert!(problems(lang, text, true).is_empty(), "{lang:?} {waiting}: {text}");
            }
        }
        fn shown(lang: kabar_core::locale::UiLanguage, outcome: Outcome) -> String {
            let mut dom = VirtualDom::new_with_props(
                |(lang, outcome): (kabar_core::locale::UiLanguage, Outcome)| {
                    use_context_provider(|| ui(lang));
                    rsx! { ExportResult { outcome } }
                },
                (lang, outcome),
            );
            dom.rebuild_in_place();
            dioxus::ssr::render(&dom)
        }
        use kabar_core::locale::UiLanguage::*;
        assert!(shown(En, Outcome::Choosing).contains("Choose where to save it"));
        let placed =
            shown(ArEg, Outcome::Placed { name: Some("kabar-export-2026-11-04.json".into()), rows: 2 });
        assert!(
            placed.contains("اتحفظ صفين في المكان اللي اخترته")
                && placed.contains("kabar-export-2026-11-04.json")
        );
        let unnamed = shown(Ar, Outcome::Placed { name: None, rows: 12 });
        assert!(unnamed.contains("حُفظ 12\u{a0}صفًّا") && !unnamed.contains("export-path"), "{unnamed}");
        let saved = shown(En, Outcome::Saved { path: "/tmp/x.json".into(), rows: 1 });
        assert!(saved.contains("Saved 1\u{a0}row to:") && saved.contains("/tmp/x.json"));
    }

    #[test]
    fn a_failed_write_reports_the_file_and_keeps_no_part() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory().unwrap();
        let now = 1_793_011_856_000;
        // A folder squats where the temporary file goes, so writing fails.
        std::fs::create_dir(dir.path().join("kabar-export-2026-10-26.json.part")).unwrap();
        let err = save(&store, dir.path(), now, 0).unwrap_err();
        assert!(err.contains("kabar-export-2026-10-26.json"), "{err}");
        assert!(!dir.path().join("kabar-export-2026-10-26.json").exists(), "no half-written export");
    }
}
