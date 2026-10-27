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
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
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
        div { class: "card export-card",
            h2 { "{ui.t(Msg::ExportTitle)}" }
            p { class: "muted", "{ui.t(Msg::ExportBody)}" }
            div { class: "row",
                button { class: "primary", disabled: busy(), onclick: run,
                    if busy() { "{ui.t(Msg::ExportSaving)}" } else { "{ui.t(Msg::ExportButton)}" }
                }
            }
            match outcome() {
                None => rsx! {},
                Some(Outcome::Saved { path, rows }) => rsx! {
                    div { class: "export-result", role: "status", aria_live: "polite",
                        p { {ui.t(Msg::ExportSaved).replace("{rows}", &ui.num(rows))} }
                        code { class: "export-path", dir: "ltr", bdi { "{path}" } }
                        if cfg!(target_os = "android") {
                            p { class: "muted", "{ui.t(Msg::ExportAndroidHint)}" }
                        }
                    }
                },
                Some(Outcome::Choosing) => rsx! {
                    p { class: "muted", role: "status", aria_live: "polite", "{placed_text(ui.lang(), true)}" }
                },
                Some(Outcome::Placed { name, rows }) => rsx! {
                    div { class: "export-result", role: "status", aria_live: "polite",
                        p { {placed_text(ui.lang(), false).replace("{rows}", &ui.num(rows))} }
                        if let Some(name) = name {
                            code { class: "export-path", dir: "ltr", bdi { "{name}" } }
                        }
                    }
                },
                Some(Outcome::Failed(e)) => rsx! {
                    div { class: "notice", role: "status",
                        p { "{ui.t(Msg::ExportFailed)}" }
                        p { class: "muted mono", dir: "ltr", bdi { "{e}" } }
                    }
                },
            }
        }
    }
}

/// Android's "Save to" step: waiting on the dialog, or done.
fn placed_text(lang: kabar_core::locale::UiLanguage, waiting: bool) -> &'static str {
    use kabar_core::locale::UiLanguage::*;
    match (waiting, lang) {
        (true, ArEg) => "اختار تحفظه فين (التنزيلات أو Drive مثلًا)…",
        (true, Ar) => "اختر مكان الحفظ (التنزيلات أو Drive مثلًا)…",
        (true, En) => "Choose where to save it (Downloads or Drive, say)…",
        (false, ArEg) => "اتحفظ {rows} صف في المكان اللي اخترته، وتقدر تفتحه من تطبيق الملفات:",
        (false, Ar) => "حُفظ {rows} صفًّا في المكان الذي اخترته، ويمكنك فتحه من تطبيق الملفات:",
        (false, En) => "Saved {rows} rows where you chose; open it from the Files app:",
    }
}

fn platform_dir() -> Result<PathBuf, String> {
    crate::platform::export_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
