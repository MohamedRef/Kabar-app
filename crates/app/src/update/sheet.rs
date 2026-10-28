//! The "what's new" sheet: what an update brings before it's installed,
//! what it brought right after (once, by itself), and the whole changelog
//! from Settings › About › Updates. One sheet for all three, laid out the
//! way Morphe Manager's update dialog is (ideas only, none of its code):
//!
//! - a head with the state ("A new Kabar is out", "Kabar was updated") and
//!   `Kabar · v0.1.0 · build 412`;
//! - a summary card: the installed build → the new one, how many builds
//!   that is, and a pill per kind (new, improved, fixed);
//! - a timeline: a rail with a dot per build (newest open), each build a
//!   row with its badge, date and counts, folding open into one card per
//!   kind that shows five lines and "Show all (n)" beyond six;
//! - "Earlier builds" under it, a few at a time;
//! - GitHub (the channel's releases page) and the download, install or
//!   restart button pinned at the foot, so the whole update happens here.
//!
//! On a phone it covers the whole screen, with Kabar's icon heading it; on
//! wider windows it is a centred dialog. Morphe's filled pills: blue for
//! new, amber for fixes, green for improvements.

use dioxus::prelude::*;
use kabar_core::locale::{UiLanguage, eastern_arabic_digits};
use kabar_protocol::update::{BuildChanges, ChangeKind};

use super::changelog::{self, between, civil, day_of, label, line, of_kind, tally};
use super::text::{U, ut};
use super::view::{UPDATE_CSS, megabytes, primary_action, use_update_status};
use super::{Phase, Status};
use crate::Ui;

const SHEET_CSS: &str = r#"
/* Phones: a full page over everything, as Morphe's update screen is; from
   600px a centred dialog. */
.us-backdrop { position: fixed; inset: 0; z-index: 60; background: rgb(0 0 0 / 0.45); }
.us-sheet { position: fixed; z-index: 61; inset: 0; display: flex; flex-direction: column;
  background: var(--bg); color: var(--text);
  /* Morphe's tones: blue for new, amber for fixes, green for improvements.
     Filled at 60% over the page and inked at 30% over the text, so each
     theme (light, dark, true black) gets a strong pill with readable text. */
  --us-new: #4a78e8; --us-fixed: #c08a00; --us-improved: #2f9a3a; --us-installed: #2f9a3a; }
.app:not([data-width="compact"]) .us-sheet { inset: 0; margin: auto; inline-size: min(34rem, 92vw);
  block-size: min(46rem, 88dvh); border-radius: 1.5rem; border: 1px solid var(--line);
  box-shadow: 0 12px 40px rgb(0 0 0 / 0.3); overflow: hidden; }
.us-sheet [data-kind="new"], .us-sheet .us-ver.to { --hue: var(--us-new); }
.us-sheet [data-kind="improved"] { --hue: var(--us-improved); }
.us-sheet [data-kind="fixed"], .us-badge[data-badge="latest"], .us-badge[data-badge="new"] { --hue: var(--us-fixed); }
.us-badge[data-badge="installed"] { --hue: var(--us-installed); }
.us-head { display: flex; gap: 0.875rem; align-items: center;
  padding: max(1rem, env(safe-area-inset-top)) 1rem 0.75rem; }
.us-logo { flex: none; inline-size: 3rem; block-size: 3rem; border-radius: 50%; overflow: hidden; }
.us-logo svg, .us-logo img { display: block; inline-size: 100%; block-size: 100%; }
.us-head-text { flex: 1; min-inline-size: 0; }
.us-head h2 { margin: 0; font-size: 1.25rem; font-weight: 700; line-height: 1.3; }
.us-head p { margin: 0.1rem 0 0; font-size: 0.875rem; color: var(--muted); }
.us-head .icon-button { flex: none; }
.us-body { flex: 1; overflow-y: auto; overscroll-behavior: contain; padding: 0.25rem 1rem 1rem; }
.us-lead { margin: 0 0 0.75rem; font-size: 0.875rem; color: var(--muted); }
.us-summary { display: grid; justify-items: center; gap: 0.75rem; text-align: center;
  padding: 1.25rem 1rem; margin-block-end: 1.5rem; border-radius: 1.5rem;
  background: color-mix(in srgb, var(--text) 3%, var(--bg)); border: 1px solid var(--line); }
.us-jump { display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap; justify-content: center; }
.us-ver { padding: 0.3rem 0.75rem; border-radius: 999px; font-size: 0.8125rem; font-weight: 600;
  background: color-mix(in srgb, var(--muted) 30%, var(--bg)); color: var(--text); }
.us-ver.to { background: color-mix(in srgb, var(--hue) 60%, var(--bg)); color: color-mix(in srgb, var(--hue) 22%, var(--text)); }
.us-arrow { display: inline-grid; color: var(--muted); }
.us-arrow svg { inline-size: 1.25rem; block-size: 1.25rem; }
[dir="rtl"] .us-arrow svg { transform: scaleX(-1); }
.us-count { margin: 0; font-weight: 700; font-size: 1.0625rem; }
.us-pills { display: flex; flex-wrap: wrap; gap: 0.5rem; justify-content: center; }
.us-pill { display: inline-flex; align-items: center; gap: 0.45rem; padding: 0.55rem 1.1rem;
  border-radius: 999px; font-size: 0.9375rem; font-weight: 600;
  background: color-mix(in srgb, var(--hue) 60%, var(--bg)); color: color-mix(in srgb, var(--hue) 22%, var(--text)); }
.us-pill svg { inline-size: 1.1rem; block-size: 1.1rem; }
.us-summary progress { inline-size: 100%; accent-color: var(--us-new); }
.us-summary .us-mb { margin: 0; font-size: 0.8125rem; color: var(--muted); }
/* The timeline: a rail on the start side, a node per build on it. */
/* The Settings sheet's own buttons hide under this one: one ✕ at a time. */
.app:has(.us-sheet) .settings-head > .icon-button { visibility: hidden; }
.us-failed { margin: 0; font-size: var(--fs-sm); color: var(--text); }
.us-timeline { list-style: none; margin: 0; padding: 0; }
.us-release { position: relative; padding-inline-start: 2rem; padding-block-end: 0.5rem; }
.us-release::before { content: ""; position: absolute; inset-inline-start: calc(0.625rem - 1px);
  inset-block: 1.9rem -0.6rem; inline-size: 2px; background: var(--line); }
.us-timeline > li:last-child::before { display: none; }
.us-release::after { content: ""; position: absolute; box-sizing: border-box;
  inset-inline-start: calc(0.625rem - 0.3rem); inset-block-start: 0.85rem;
  inline-size: 0.6rem; block-size: 0.6rem; border-radius: 50%; background: var(--muted); }
.us-release[data-ahead="true"]::after, .us-release[data-badge="installed"]::after {
  inset-inline-start: calc(0.625rem - 0.5rem); inset-block-start: 0.65rem;
  inline-size: 1rem; block-size: 1rem; background: var(--bg); border: 2px dashed var(--muted); }
.us-release[data-badge="installed"]::after { border: 3px solid var(--us-installed); }
.us-rel-head { inline-size: 100%; display: grid; grid-template-columns: 1fr auto; column-gap: 0.5rem;
  text-align: start; border: 0; background: transparent; padding: 0.45rem 0; color: inherit;
  font: inherit; cursor: pointer; }
.us-rel-title { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; font-size: 1rem; font-weight: 700; }
.us-rel-meta { grid-column: 1; font-size: 0.8125rem; color: var(--muted); margin-block-start: 0.1rem; }
.us-chevron { grid-row: 1 / span 2; grid-column: 2; align-self: center; display: inline-grid; color: var(--muted);
  transition: transform var(--dur) var(--ease); }
.us-chevron svg { inline-size: 1.4rem; block-size: 1.4rem; }
.us-rel-head[aria-expanded="true"] .us-chevron { transform: rotate(180deg); }
.us-badge { font-size: 0.75rem; font-weight: 600; padding: 0.15rem 0.6rem; border-radius: 999px;
  background: color-mix(in srgb, var(--hue) 45%, var(--bg)); color: color-mix(in srgb, var(--hue) 30%, var(--text)); }
.us-kinds { display: grid; gap: 0.625rem; margin-block: 0.4rem 0.6rem; }
.us-kind { border: 1px solid var(--line); border-radius: 1.125rem; overflow: hidden;
  background: color-mix(in srgb, var(--text) 3%, var(--bg)); }
.us-kind-head { display: flex; align-items: center; gap: 0.75rem; padding: 0.6rem 0.875rem;
  background: color-mix(in srgb, var(--text) 7%, var(--bg)); }
.us-kind-icon { inline-size: 2rem; block-size: 2rem; border-radius: 0.6rem; display: grid; place-items: center;
  background: color-mix(in srgb, var(--hue) 45%, var(--bg)); color: color-mix(in srgb, var(--hue) 30%, var(--text)); }
.us-kind-icon svg { inline-size: 1.05rem; block-size: 1.05rem; }
.us-kind-head h3 { margin: 0; font-size: 0.9375rem; font-weight: 600; }
.us-chip { margin-inline-start: auto; min-inline-size: 1.6rem; block-size: 1.6rem; padding-inline: 0.4rem;
  display: inline-grid; place-items: center; border-radius: 999px; font-size: 0.8125rem; font-weight: 600;
  background: color-mix(in srgb, var(--muted) 30%, var(--bg)); }
.us-items { list-style: none; margin: 0; padding: 0; display: grid; }
.us-items li { position: relative; padding: 0.65rem 0.875rem; padding-inline-start: 2.1rem;
  font-size: 0.875rem; line-height: 1.6; }
.us-items li + li { border-block-start: 1px solid var(--line); }
.us-items li::before { content: ""; position: absolute; inset-inline-start: 1rem; inset-block-start: calc(0.65rem + 0.55em);
  inline-size: 0.4rem; block-size: 0.4rem; border-radius: 50%; background: color-mix(in srgb, var(--hue) 85%, var(--text)); }
.us-more { border: 0; background: transparent; color: var(--indigo); padding: 0.4rem 0.875rem 0.6rem;
  font: inherit; font-size: 0.875rem; font-weight: 600; cursor: pointer; }
.us-divider { position: relative; padding-inline-start: 2rem; padding-block: 0.6rem 0.5rem;
  font-size: 0.9375rem; font-weight: 600; }
.us-divider::before { content: ""; position: absolute; inset-inline-start: calc(0.625rem - 1px);
  inset-block: 0 -0.2rem; inline-size: 2px; background: var(--line); }
/* The buttons stay at the bottom; the list fades under them. */
.us-foot { position: relative; display: grid; gap: 0.625rem; padding: 0.5rem 1rem;
  padding-block-end: max(0.875rem, env(safe-area-inset-bottom)); background: var(--bg); }
.us-foot::before { content: ""; position: absolute; inset-inline: 0; inset-block-end: 100%; block-size: 1.5rem;
  background: linear-gradient(to top, var(--bg), transparent); pointer-events: none; }
.us-btn { display: inline-flex; align-items: center; justify-content: center; gap: 0.55rem; inline-size: 100%;
  min-block-size: 3.25rem; border-radius: 999px; font: inherit; font-size: 1rem; font-weight: 600;
  border: 1px solid var(--line); background: color-mix(in srgb, var(--text) 5%, var(--bg)); color: var(--text);
  cursor: pointer; }
.us-btn svg { inline-size: 1.2rem; block-size: 1.2rem; }
.us-btn.us-go { border-color: color-mix(in srgb, var(--us-new) 55%, var(--bg));
  background: color-mix(in srgb, var(--us-new) 16%, var(--bg)); }
.us-btn:active { transform: scale(0.98); }
@media (prefers-reduced-motion: reduce) { .us-chevron { transition: none; } }
"#;

/// Which sheet is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetMode {
    /// What the offered update brings (banner, card, notification).
    Offer,
    /// What the update just installed brought: opens once by itself.
    Updated,
    /// Every build so far (Settings › About › Updates).
    History,
}

/// Opens and closes the sheet; provided once in the shell.
#[derive(Clone, Copy)]
pub struct UpdateSheetNav(pub Signal<Option<SheetMode>>);

impl UpdateSheetNav {
    pub fn new() -> Self {
        Self(Signal::new(None))
    }

    pub fn open(mut self, mode: SheetMode) {
        self.0.set(Some(mode));
    }

    /// What the offer brings, or the changelog when nothing is offered
    /// right now (a notification tapped after Kabar restarted).
    pub fn open_offer(self) {
        let offered = super::get().is_some_and(|u| {
            matches!(
                u.status().phase,
                Phase::Available(_)
                    | Phase::Downloading { .. }
                    | Phase::Ready(_)
                    | Phase::Installing(_)
                    | Phase::NeedsPermission(_)
                    | Phase::Installed(_)
            )
        });
        self.open(if offered { SheetMode::Offer } else { SheetMode::History });
    }
}

/// Lines a kind card shows before "Show all"; it folds only past `FOLD_AT`.
const PREVIEW: usize = 5;
const FOLD_AT: usize = 6;
/// Earlier builds shown at a time.
const EARLIER_STEP: usize = 4;
/// Builds the changelog lists before "Earlier builds".
const HISTORY_FIRST: usize = 6;

fn kind_label(k: ChangeKind) -> U {
    match k {
        ChangeKind::New => U::KindNew,
        ChangeKind::Improved => U::KindImproved,
        ChangeKind::Fixed => U::KindFixed,
    }
}

fn kind_id(k: ChangeKind) -> &'static str {
    match k {
        ChangeKind::New => "new",
        ChangeKind::Improved => "improved",
        ChangeKind::Fixed => "fixed",
    }
}

/// Our own glyphs: a four-point spark, a rising line, a bug.
fn kind_icon(k: ChangeKind) -> Element {
    let path = match k {
        ChangeKind::New => "M12 3l1.9 5.6L19.5 10l-5.6 1.9L12 17.5l-1.9-5.6L4.5 10l5.6-1.4z",
        ChangeKind::Improved => "M4 16l5-5 4 4 7-7M15 8h5v5",
        ChangeKind::Fixed => {
            "M8 10h8v4a4 4 0 0 1-8 0zM9.5 10a2.5 2.5 0 0 1 5 0M12 10v8M4 13h4M16 13h4M5 8l3 2M19 8l-3 2M5 19l3-2M19 19l-3-2"
        }
    };
    let fill = if k == ChangeKind::New { "currentColor" } else { "none" };
    rsx! {
        svg { view_box: "0 0 24 24", fill: "{fill}", stroke: "currentColor", stroke_width: "2.2",
            stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
            path { d: "{path}" }
        }
    }
}

fn digits(ui: &Ui, s: String) -> String {
    if ui.eastern() { eastern_arabic_digits(&s) } else { s }
}

fn with_n(ui: &Ui, template: &str, n: usize) -> String {
    digits(ui, template.replace("{n}", &n.to_string()))
}

/// "3 new builds", with Arabic's dual and plural.
fn builds_count(ui: &Ui, n: usize) -> String {
    let s = match ui.lang() {
        UiLanguage::ArEg => match n {
            1 => "نسخة جديدة".to_owned(),
            2 => "نسختين جداد".to_owned(),
            3..=10 => format!("{n} نسخ جديدة"),
            _ => format!("{n} نسخة جديدة"),
        },
        UiLanguage::Ar => match n {
            1 => "إصدار جديد".to_owned(),
            2 => "إصداران جديدان".to_owned(),
            3..=10 => format!("{n} إصدارات جديدة"),
            _ => format!("{n} إصدارًا جديدًا"),
        },
        UiLanguage::En if n == 1 => "1 new build".to_owned(),
        UiLanguage::En => format!("{n} new builds"),
    };
    digits(ui, s)
}

const MONTHS_AR: [&str; 12] = [
    "يناير",
    "فبراير",
    "مارس",
    "أبريل",
    "مايو",
    "يونيو",
    "يوليو",
    "أغسطس",
    "سبتمبر",
    "أكتوبر",
    "نوفمبر",
    "ديسمبر",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn glyph(view_box: &'static str, d: &'static str, filled: bool) -> Element {
    let (fill, stroke) = if filled { ("currentColor", "none") } else { ("none", "currentColor") };
    rsx! {
        svg { view_box: "{view_box}", fill: "{fill}", stroke: "{stroke}", stroke_width: "2",
            stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
            path { d: "{d}" }
        }
    }
}

fn chevron() -> Element {
    glyph("0 0 24 24", "M6 9l6 6 6-6", false)
}

fn arrow() -> Element {
    glyph("0 0 24 24", "M5 12h14M13 6l6 6-6 6", false)
}

fn download_icon() -> Element {
    glyph("0 0 24 24", "M12 4v11M7 10l5 5 5-5M5 20h14", false)
}

/// GitHub's mark, for the button that opens the releases page.
fn github_icon() -> Element {
    glyph(
        "0 0 16 16",
        "M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z",
        true,
    )
}

/// Kabar's icon (the web export), as the sheet's app icon.
fn app_mark() -> Element {
    static URL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        use base64::Engine as _;
        let png = include_bytes!("../../../../assets/export/web/icon-192.png");
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
    });
    rsx! {
        img { src: URL.as_str(), alt: "", draggable: "false" }
    }
}

/// The releases page of the channel this build follows.
fn releases_url() -> String {
    let base = super::FEEDS_URL.replace("/releases/download", "/releases/tag");
    match super::get() {
        Some(u) => format!("{base}/{}", u.installed().channel.release_tag()),
        None => base.trim_end_matches("/tag").to_owned(),
    }
}

/// The summary's two chips: `#165 → #190` when the version is the same
/// (preview builds), else `v0.1.0 (12) → v0.2.0 (30)`.
fn jump_labels(from: &str, to: &str) -> (String, String) {
    match (from.split_once(" ("), to.split_once(" (")) {
        (Some((fv, fb)), Some((tv, tb))) if fv == tv => {
            (format!("#{}", fb.trim_end_matches(')')), format!("#{}", tb.trim_end_matches(')')))
        }
        _ => (format!("v{from}"), format!("v{to}")),
    }
}

/// "Today", "2 days ago" within a week, then "28 October".
fn when(ui: &Ui, published: &str, today: i64) -> Option<String> {
    let day = day_of(published)?;
    let lang = ui.lang();
    let ago = today - day;
    Some(match ago {
        ..=0 => ut(lang, U::Today).to_owned(),
        1 => ut(lang, U::Yesterday).to_owned(),
        2 => ut(lang, U::TwoDaysAgo).to_owned(),
        3..=6 => with_n(ui, ut(lang, U::DaysAgo), ago as usize),
        _ => {
            let (y, m, d) = civil(day);
            let (ty, _, _) = civil(today);
            let month = match lang {
                UiLanguage::En => MONTHS_EN[m as usize - 1],
                _ => MONTHS_AR[m as usize - 1],
            };
            let s = if y == ty { format!("{d} {month}") } else { format!("{d} {month} {y}") };
            digits(ui, s)
        }
    })
}

fn today(s: &Status) -> i64 {
    let offset = s.prefs.notice.map_or(0, |n| i64::from(n.utc_offset_min)) * 60_000;
    (super::now_ms() as i64 + offset).div_euclid(86_400_000)
}

/// The sheet; mounted once in the shell. Opens by itself once after an
/// update that brought something to tell.
#[component]
pub fn UpdateSheet() -> Element {
    let ui = use_context::<Ui>();
    let UpdateSheetNav(mut mode) = use_context::<UpdateSheetNav>();
    let status = use_update_status();
    let mut opened_after_update = use_signal(|| false);
    let mut open_builds = use_signal(Vec::<String>::new);
    let mut open_kinds = use_signal(Vec::<String>::new);
    let mut earlier_shown = use_signal(|| EARLIER_STEP);

    use_effect(move || {
        let has = status.read().as_ref().is_some_and(|s| !s.whats_new().is_empty());
        if has && !*opened_after_update.peek() && mode.peek().is_none() {
            opened_after_update.set(true);
            mode.set(Some(SheetMode::Updated));
        }
    });

    let Some(m) = *mode.read() else { return rsx! {} };
    let Some(s) = status.read().clone() else { return rsx! {} };
    let lang = ui.lang();
    let close = move |_| {
        if *mode.peek() == Some(SheetMode::Updated)
            && let Some(u) = super::get()
        {
            u.close_whats_new();
        }
        mode.set(None);
        open_builds.set(Vec::new());
        open_kinds.set(Vec::new());
        earlier_shown.set(EARLIER_STEP);
    };

    let offer = match &s.phase {
        Phase::Available(o)
        | Phase::Ready(o)
        | Phase::Installing(o)
        | Phase::NeedsPermission(o)
        | Phase::Installed(o)
        | Phase::Downloading { offer: o, .. } => Some(o.clone()),
        Phase::Failed { offer, .. } => offer.clone(),
        _ => None,
    };
    let current = s.current();
    let this_label = label(&s.version, s.build);
    let latest_key = s.history.first().map(BuildChanges::key);

    // What the timeline shows first, the jump the summary names, and what
    // goes under "Earlier builds".
    let (title, sub, top, from_label, to_label, earlier): (
        String,
        Option<String>,
        Vec<BuildChanges>,
        _,
        _,
        Vec<BuildChanges>,
    ) = match m {
        SheetMode::Offer => {
            let head = match &s.phase {
                Phase::Downloading { .. } => U::Downloading,
                Phase::Ready(_) => U::Ready,
                Phase::Installing(_) => U::Installing,
                Phase::NeedsPermission(_) => U::NeedsPermission,
                Phase::Installed(_) => U::Installed,
                _ => U::Available,
            };
            let top = offer.as_ref().map(|o| o.changes.clone()).unwrap_or_default();
            let earlier = current
                .map(|c| between(&s.history, None, c).into_iter().cloned().collect())
                .unwrap_or_default();
            let to = offer.as_ref().map(|o| o.label());
            (ut(lang, head).to_owned(), None, top, Some(this_label.clone()), to, earlier)
        }
        SheetMode::Updated => {
            let top = s.whats_new();
            let from = s.prefs.whats_new_from.as_deref().map(|k| {
                let (v, b) = k.split_once('+').unwrap_or((k, "0"));
                label(v, b.parse().unwrap_or(0))
            });
            (
                ut(lang, U::UpdatedTitle).to_owned(),
                Some(ut(lang, U::UpdatedSub).to_owned()),
                top,
                from,
                Some(this_label.clone()),
                Vec::new(),
            )
        }
        SheetMode::History => {
            let (top, rest) = s.history.split_at(s.history.len().min(HISTORY_FIRST));
            (
                ut(lang, U::History).to_owned(),
                Some(ut(lang, U::HistoryHint).to_owned()),
                top.to_vec(),
                None,
                None,
                rest.to_vec(),
            )
        }
    };
    let earlier: Vec<BuildChanges> =
        earlier.into_iter().filter(|e| !top.iter().any(|t| t.key() == e.key())).collect();
    let progress = match &s.phase {
        Phase::Downloading { offer, done } if m == SheetMode::Offer => Some((*done, offer.asset.size.max(1))),
        _ => None,
    };
    let action = if m == SheetMode::Offer { primary_action(lang, &s.phase) } else { None };
    let today = today(&s);
    let badge_of = |b: &BuildChanges| -> Option<(&'static str, U)> {
        let o = changelog::order(&b.version, b.build);
        if Some(b.key()) == Some(format!("{}+{}", s.version, s.build)) {
            Some(("installed", U::BadgeInstalled))
        } else if latest_key.as_deref() == Some(b.key().as_str()) {
            Some(("latest", U::BadgeLatest))
        } else if o.is_some() && o > current {
            Some(("new", U::BadgeNew))
        } else {
            None
        }
    };
    // The newest build starts open; the learner's taps flip any build.
    let first_open = top.first().map(BuildChanges::key);
    let is_open = move |key: &str| {
        let flipped = open_builds.read().iter().any(|k| k == key);
        (first_open.as_deref() == Some(key)) != flipped
    };
    let summary_builds = if m == SheetMode::History { Vec::new() } else { top.clone() };
    let pills = tally(summary_builds.iter());
    let show_earlier = (*earlier_shown.read()).min(earlier.len());
    let shown: Vec<(bool, BuildChanges)> = top
        .iter()
        .cloned()
        .map(|b| (false, b))
        .chain(earlier.iter().take(show_earlier).cloned().map(|b| (true, b)))
        .collect();
    // As in Morphe, the installed build closes the main list and "Earlier
    // builds" starts after it.
    let this_key = format!("{}+{}", s.version, s.build);
    let first_earlier = shown.iter().position(|(e, b)| *e && b.key() != this_key);
    let more_earlier = earlier.len() > show_earlier;
    let empty = shown.is_empty();
    let fallback_notes = offer
        .as_ref()
        .filter(|o| m == SheetMode::Offer && o.changes.is_empty())
        .map(|o| super::notes_in(lang, &o.notes))
        .filter(|n| !n.is_empty());

    rsx! {
        style { {UPDATE_CSS} }
        style { {SHEET_CSS} }
        div { class: "us-backdrop", onclick: close }
        div { class: "us-sheet", role: "dialog", aria_modal: "true", aria_labelledby: "us-title",
            div { class: "us-head",
                span { class: "us-logo", {app_mark()} }
                div { class: "us-head-text",
                    h2 { id: "us-title", "{title}" }
                    p {
                        "Kabar · "
                        bdi { dir: "ltr", class: "latin",
                            "v{offer.as_ref().filter(|_| m == SheetMode::Offer).map_or(this_label.clone(), |o| o.label())}"
                        }
                    }
                }
                button { class: "icon-button", "data-back": "true", aria_label: "{ut(lang, U::Close)}", onclick: close,
                    span { aria_hidden: "true", "✕" }
                }
            }
            div { class: "us-body",
                if let Some(sub) = sub {
                    p { class: "us-lead", "{sub}" }
                }
                if m != SheetMode::History && (from_label.is_some() || !pills.is_empty() || progress.is_some()) {
                    div { class: "us-summary",
                        if let (Some(f), Some(t)) = (from_label.clone(), to_label.clone()) {
                            {
                                let (f, t) = jump_labels(&f, &t);
                                rsx! {
                                    div { class: "us-jump",
                                        span { class: "us-ver", bdi { dir: "ltr", class: "latin", "{f}" } }
                                        span { class: "us-arrow", {arrow()} }
                                        span { class: "us-ver to", bdi { dir: "ltr", class: "latin", "{t}" } }
                                    }
                                }
                            }
                        }
                        if !top.is_empty() {
                            p { class: "us-count", "{builds_count(&ui, top.len())}" }
                        }
                        if !pills.is_empty() {
                            div { class: "us-pills",
                                for (k, n) in pills {
                                    span { key: "{kind_id(k)}", class: "us-pill", "data-kind": "{kind_id(k)}",
                                        {kind_icon(k)}
                                        "{digits(&ui, n.to_string())} {ut(lang, kind_label(k))}"
                                    }
                                }
                            }
                        }
                        if let Some((done, total)) = progress {
                            progress { max: "{total}", value: "{done}" }
                            p { class: "us-mb", "{megabytes(&ui, done)} / {megabytes(&ui, total)}" }
                        }
                    }
                }
                if let Some(n) = fallback_notes {
                    p { class: "update-notes", "{n}" }
                }
                if empty {
                    p { class: "muted", {ut(lang, if m == SheetMode::History { U::HistoryEmpty } else { U::NoDetails })} }
                }
                ol { class: "us-timeline",
                    for (i, (is_earlier, b)) in shown.into_iter().enumerate() {
                        if Some(i) == first_earlier {
                            li { key: "{i}-earlier", class: "us-divider", role: "presentation", "{ut(lang, U::Earlier)}" }
                        }
                        {
                            let key = b.key();
                            let open = is_open(&key);
                            let badge = badge_of(&b);
                            let ahead = current.is_some() && changelog::order(&b.version, b.build) > current;
                            let counts = tally([&b]);
                            let mut meta: Vec<String> = when(&ui, &b.published_at, today).into_iter().collect();
                            meta.extend(counts.iter().map(|(k, n)| format!("{} {}", digits(&ui, n.to_string()), ut(lang, kind_label(*k)))));
                            let meta = meta.join(" · ");
                            let toggle_key = key.clone();
                            let _ = is_earlier;
                            rsx! {
                                li { key: "{key}", class: "us-release", "data-badge": badge.map_or("", |b| b.0), "data-ahead": "{ahead}",
                                    button {
                                        class: "us-rel-head",
                                        aria_expanded: if open { "true" } else { "false" },
                                        onclick: move |_| {
                                            let mut v = open_builds.write();
                                            if let Some(i) = v.iter().position(|k| *k == toggle_key) {
                                                v.remove(i);
                                            } else {
                                                v.push(toggle_key.clone());
                                            }
                                        },
                                        span { class: "us-rel-title",
                                            bdi { dir: "ltr", class: "latin", "v{label(&b.version, b.build)}" }
                                            if let Some((id, u)) = badge {
                                                span { class: "us-badge", "data-badge": "{id}", "{ut(lang, u)}" }
                                            }
                                        }
                                        span { class: "us-rel-meta", "{meta}" }
                                        span { class: "us-chevron", {chevron()} }
                                    }
                                    if open {
                                        div { class: "us-kinds",
                                            for k in ChangeKind::ALL {
                                                {
                                                    let items = of_kind(&b, k);
                                                    let kkey = format!("{key}:{}", kind_id(k));
                                                    let all = open_kinds.read().contains(&kkey);
                                                    let folds = items.len() > FOLD_AT;
                                                    let n = items.len();
                                                    let show: Vec<(String, bool)> = items
                                                        .into_iter()
                                                        .take(if folds && !all { PREVIEW } else { usize::MAX })
                                                        .map(|c| line(lang, c))
                                                        .collect();
                                                    rsx! {
                                                        if n > 0 {
                                                            section { key: "{kkey}", class: "us-kind", "data-kind": "{kind_id(k)}",
                                                                div { class: "us-kind-head",
                                                                    span { class: "us-kind-icon", {kind_icon(k)} }
                                                                    h3 { "{ut(lang, kind_label(k))}" }
                                                                    span { class: "us-chip", "{digits(&ui, n.to_string())}" }
                                                                }
                                                                ul { class: "us-items",
                                                                    for (j, (text, latin)) in show.into_iter().enumerate() {
                                                                        li { key: "{j}",
                                                                            if latin {
                                                                                bdi { dir: "ltr", class: "latin", "{text}" }
                                                                            } else {
                                                                                "{text}"
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                if folds {
                                                                    button {
                                                                        class: "us-more",
                                                                        onclick: move |_| {
                                                                            let mut v = open_kinds.write();
                                                                            if let Some(i) = v.iter().position(|x| *x == kkey) {
                                                                                v.remove(i);
                                                                            } else {
                                                                                v.push(kkey.clone());
                                                                            }
                                                                        },
                                                                        if all {
                                                                            "{ut(lang, U::ShowLess)}"
                                                                        } else {
                                                                            "{with_n(&ui, ut(lang, U::ShowAll), n)}"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if more_earlier {
                    button { class: "us-more", onclick: move |_| earlier_shown += EARLIER_STEP, "{ut(lang, U::ShowOlder)}" }
                }
            }
            div { class: "us-foot",
                if let (SheetMode::Offer, Phase::Failed { why, .. }) = (m, &s.phase) {
                    p { class: "us-failed", role: "alert", "{ut(lang, super::view::failure_text(why))}" }
                }
                button {
                    class: "us-btn",
                    onclick: move |_| {
                        let _ = crate::platform::open_url(&releases_url());
                    },
                    {github_icon()}
                    bdi { dir: "ltr", class: "latin", "GitHub" }
                }
                match action {
                    Some((label, run)) => rsx! {
                        button { class: "us-btn us-go", onclick: move |_| run(lang),
                            if matches!(s.phase, Phase::Available(_)) {
                                {download_icon()}
                            }
                            "{ut(lang, label)}"
                        }
                    },
                    None if m == SheetMode::Updated => rsx! {
                        button { class: "us-btn us-go", onclick: close, "{ut(lang, U::GotIt)}" }
                    },
                    None => rsx! {},
                }
            }
        }
    }
}
