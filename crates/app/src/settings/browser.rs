//! Browser fixtures render production Settings and shared primitives.
//! Rust callbacks are not hydrated; browser assertions cover CSS and the real
//! keyboard controller, while Rust interaction tests remain separate.

use super::*;
use crate::{Db, Screen};
use kabar_core::settings::{Settings, Theme};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy)]
struct Case {
    width: u32,
    lang: kabar_core::locale::UiLanguage,
    theme: Theme,
    appearance: device::Appearance,
    size: device::TextSize,
}

thread_local! { static CASE: RefCell<Option<Case>> = const { RefCell::new(None) }; }

fn fixture() -> Element {
    let case = CASE.with(|c| c.borrow().unwrap());
    crate::persistence::use_provide_saves();
    let db =
        use_context_provider(|| Db(Rc::new(RefCell::new(crate::store::Store::open_in_memory().unwrap()))));
    let ui = use_context_provider(|| Ui {
        settings: Signal::new(Settings { ui_language: case.lang, theme: case.theme, ..Settings::default() }),
        width: Signal::new(WidthClass::from_width(case.width.into())),
        tablet: Signal::new(false),
        screen: Signal::new(Screen::Today),
        lesson: Signal::new(None),
        lesson_from: Signal::new(Screen::Journey),
        focus: Signal::new(false),
    });
    use_context_provider(|| {
        let mut prefs = device::DevicePrefs::load(&db);
        prefs.text_size.set(case.size);
        prefs.appearance.set(case.appearance);
        prefs
    });
    // The production section index reads the coach context. Without it
    // the medium/wide SSR fixture silently omits that subtree.
    use_context_provider(|| crate::coach::Coach::load(&db));
    use_context_provider(|| SettingsNav {
        open: Signal::new(true),
        section: Signal::new(Some(Section::Look)),
        query: Signal::new(String::new()),
    });
    rsx! {
        style { {crate::MAIN_CSS.text()} }
        style { {crate::COMPONENTS_CSS.text()} }
        style { {crate::LIQUID_GLASS_CSS.text()} }
        style { {kabar_core::theme::stylesheet()} }
        style { {crate::fonts::css()} }
        div { class: "app", lang: ui.lang().tag(), dir: ui.lang().dir().as_str(),
            "data-theme": case.theme.as_str(), "data-width": ui.width.read().as_str(),
            "data-text-size": case.size.as_str(), "data-appearance": case.appearance.as_str(),
            main { id: "background", button { id: "opener", r#type: "button", "Settings" } }
            SettingsSheet {}
        }
    }
}

/// Every fixture the browser job checks: each width, direction, theme,
/// interface style and text size.
fn all_cases() -> impl Iterator<Item = Case> {
    use kabar_core::locale::UiLanguage;
    [320, 384, 600, 840, 1280].into_iter().flat_map(|width| {
        [UiLanguage::ArEg, UiLanguage::En].into_iter().flat_map(move |lang| {
            Theme::ALL.into_iter().flat_map(move |theme| {
                device::Appearance::ALL.iter().copied().flat_map(move |appearance| {
                    [device::TextSize::Normal, device::TextSize::Larger].into_iter().map(move |size| Case {
                        width,
                        lang,
                        theme,
                        appearance,
                        size,
                    })
                })
            })
        })
    })
}

/// Renders each case into `dir` with a `manifest.json` naming them.
fn export(dir: &std::path::Path, cases: impl Iterator<Item = Case>) -> Vec<serde_json::Value> {
    std::fs::create_dir_all(dir).unwrap();
    let mut manifest = Vec::new();
    for case in cases {
        let Case { width, lang, theme, appearance, size } = case;
        CASE.with(|c| *c.borrow_mut() = Some(case));
        let mut dom = VirtualDom::new(fixture);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert!(html.contains("settings-window") && html.contains("text-sample"));
        if width >= 600 {
            assert!(html.contains("settings-index"), "wide Settings must render its section index");
        }
        let file = format!(
            "{width}-{}-{}-{}-{}.html",
            lang.tag(),
            theme.as_str(),
            appearance.as_str(),
            size.as_str()
        );
        std::fs::write(dir.join(&file), format!("<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"></head><body>{html}</body></html>")).unwrap();
        manifest.push(serde_json::json!({"file":file,"width":width,"dir":lang.dir().as_str(),"theme":theme.as_str(),"appearance":appearance.as_str(),"size":size.as_str()}));
    }
    std::fs::write(dir.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    manifest
}

#[test]
#[ignore = "exports production HTML for the dedicated browser CI job"]
fn export_browser_fixtures() {
    let dir =
        std::path::PathBuf::from(std::env::var_os("APP_BROWSER_FIXTURES").expect("APP_BROWSER_FIXTURES"));
    export(&dir, all_cases());
}

/// A sample of the browser job's fixtures, rendered on every test run so a
/// broken fixture shows up before the browser job.
#[test]
fn browser_fixtures_render_each_direction_and_width() {
    assert_eq!(all_cases().count(), 5 * 2 * Theme::ALL.len() * device::Appearance::ALL.len() * 2);
    let dir = tempfile::tempdir().unwrap();
    let manifest = export(dir.path(), all_cases().step_by(23));
    assert!(manifest.len() >= 10);
    let listed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("manifest.json")).unwrap()).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), manifest.len());
    for entry in &manifest {
        let html = std::fs::read_to_string(dir.path().join(entry["file"].as_str().unwrap())).unwrap();
        let dir_attr = format!("dir=\"{}\"", entry["dir"].as_str().unwrap());
        assert!(html.starts_with("<!doctype html>") && html.contains(&dir_attr), "{entry}");
        assert!(html.contains(&format!("data-text-size=\"{}\"", entry["size"].as_str().unwrap())));
        assert!(html.contains(&format!("data-theme=\"{}\"", entry["theme"].as_str().unwrap())));
    }
    assert!(manifest.iter().any(|e| e["dir"] == "rtl") && manifest.iter().any(|e| e["dir"] == "ltr"));
}
