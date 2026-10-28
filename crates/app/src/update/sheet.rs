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
//! - the download, install or restart button in the foot, so the whole
//!   update happens here.

use dioxus::prelude::*;
use kabar_core::locale::{UiLanguage, eastern_arabic_digits};
use kabar_protocol::update::{BuildChanges, ChangeKind};

use super::changelog::{self, between, civil, day_of, label, line, of_kind, tally};
use super::text::{U, ut};
use super::view::{UPDATE_CSS, megabytes, primary_action, use_update_status};
use super::{Phase, Status};
use crate::Ui;

const SHEET_CSS: &str = r#"
.us-backdrop { position: fixed; inset: 0; z-index: 40; background: rgb(0 0 0 / 0.35); }
.us-sheet { position: fixed; z-index: 41; inset-inline: 0; inset-block-end: 0; max-block-size: 90dvh;
  display: flex; flex-direction: column; background: var(--surface); box-shadow: var(--shadow-raised, var(--shadow));
  border-start-start-radius: var(--radius-lg); border-start-end-radius: var(--radius-lg); }
.app:not([data-width="compact"]) .us-sheet { inset: 0; margin: auto; inline-size: min(36rem, 92vw);
  block-size: fit-content; max-block-size: 86dvh; border-radius: var(--radius-lg); }
.us-head { display: grid; grid-template-columns: auto 1fr auto; gap: var(--sp-3); align-items: center;
  padding: var(--sp-4) var(--sp-4) var(--sp-2); }
.us-logo { inline-size: 2.6rem; block-size: 2.6rem; border-radius: 12px; display: grid; place-items: center;
  background: color-mix(in srgb, var(--sawah) 14%, var(--surface)); color: var(--sawah); }
.us-logo svg { inline-size: 1.5rem; block-size: 1.5rem; }
.us-head h2 { margin: 0; font-size: var(--fs-lg); line-height: var(--lh-tight); }
.us-head p { margin: 0; font-size: var(--fs-sm); color: var(--muted); }
.us-body { overflow-y: auto; padding: var(--sp-2) var(--sp-4) var(--sp-4); overscroll-behavior: contain; }
.us-foot { display: flex; flex-direction: column; gap: var(--sp-2);
  padding: var(--sp-3) var(--sp-4); padding-block-end: max(var(--sp-3), env(safe-area-inset-bottom)); }
.us-foot button { inline-size: 100%; min-block-size: 48px; border-radius: var(--radius-pill); font-weight: 600; }
.us-summary { display: grid; justify-items: center; gap: var(--sp-3); text-align: center;
  padding: var(--sp-5) var(--sp-4); margin-block-end: var(--sp-5); border-radius: var(--radius-lg);
  background: var(--surface-2); border: 1px solid var(--line); }
.us-jump { display: flex; align-items: center; gap: var(--sp-2); flex-wrap: wrap; justify-content: center; }
.us-ver-badge { padding: 0.2rem 0.65rem; border-radius: var(--radius-sm); font-size: var(--fs-sm);
  background: color-mix(in srgb, var(--muted) 22%, var(--surface)); }
.us-ver-badge.to { background: var(--indigo); color: var(--surface); font-weight: 600; }
.us-arrow { color: var(--muted); }
[dir="rtl"] .us-arrow, .app[dir="rtl"] .us-arrow { transform: scaleX(-1); }
.us-count { margin: 0; font-weight: 700; font-size: var(--fs-md); }
.us-pills { display: flex; flex-wrap: wrap; gap: var(--sp-2); justify-content: center; }
.us-pill { display: inline-flex; align-items: center; gap: 0.4rem; padding: 0.45rem 1rem;
  border-radius: var(--radius-pill); font-size: var(--fs-sm); font-weight: 600; }
.us-pill svg, .us-kind-icon svg { inline-size: 1rem; block-size: 1rem; }
[data-kind="new"] { --tone: var(--indigo); }
[data-kind="improved"] { --tone: var(--sawah); }
[data-kind="fixed"] { --tone: var(--dawn); }
.us-pill { background: color-mix(in srgb, var(--tone) 22%, var(--surface)); color: var(--tone); }
.us-summary progress { inline-size: 100%; accent-color: var(--sawah); }
.us-timeline { list-style: none; margin: 0; padding: 0; }
.us-release { position: relative; padding-inline-start: 1.6rem; padding-block-end: var(--sp-4); }
.us-release::before { content: ""; position: absolute; inset-inline-start: 0.45rem; inset-block: 1.1rem 0;
  inline-size: 2px; background: var(--line); }
.us-release:last-child::before { display: none; }
/* Nodes: a dashed ring for builds not installed yet, a green ring for the
   installed one, a small dot for the ones before it. */
.us-release::after { content: ""; position: absolute; inset-inline-start: 0.2rem; inset-block-start: 0.6rem;
  inline-size: 0.55rem; block-size: 0.55rem; border-radius: 50%; background: var(--muted); }
.us-release[data-ahead="true"]::after { inset-inline-start: 0.05rem; inset-block-start: 0.45rem;
  inline-size: 0.85rem; block-size: 0.85rem; box-sizing: border-box; background: var(--surface);
  border: 2px dashed var(--muted); }
.us-release[data-badge="installed"]::after { inset-inline-start: 0.05rem; inset-block-start: 0.45rem;
  inline-size: 0.85rem; block-size: 0.85rem; box-sizing: border-box; background: var(--surface);
  border: 3px solid var(--sawah); }
.us-release[data-badge="latest"], .us-release[data-badge="new"] { --node: var(--dawn); }
.us-release[data-badge="installed"] { --node: var(--sawah); }
.us-rel-head { inline-size: 100%; display: grid; grid-template-columns: 1fr auto; gap: 0.15rem var(--sp-2);
  text-align: start; border: 0; background: transparent; padding: 0.2rem 0; color: inherit; }
.us-rel-title { display: flex; flex-wrap: wrap; align-items: center; gap: var(--sp-2); font-weight: 700; }
.us-rel-meta { grid-column: 1; font-size: var(--fs-sm); color: var(--muted); }
.us-chevron { grid-row: 1 / span 2; grid-column: 2; align-self: center; color: var(--muted);
  transition: transform var(--dur) var(--ease); }
.us-rel-head[aria-expanded="true"] .us-chevron { transform: rotate(180deg); }
.us-badge { font-size: var(--fs-xs); font-weight: 600; padding: 0.1rem 0.55rem; border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--node, var(--muted)) 22%, var(--surface)); color: var(--node, var(--muted)); }
.us-kinds { display: grid; gap: var(--sp-2); margin-block-start: var(--sp-2); }
.us-kind { border: 1px solid var(--line); border-radius: var(--radius-lg); padding: var(--sp-3) var(--sp-4);
  background: var(--surface-2); }
.us-kind-head { display: flex; align-items: center; gap: var(--sp-3); margin-block-end: var(--sp-2); }
.us-kind-icon { inline-size: 1.9rem; block-size: 1.9rem; border-radius: var(--radius-sm); display: grid; place-items: center;
  background: color-mix(in srgb, var(--tone) 22%, var(--surface)); color: var(--tone); }
.us-kind-head h3 { margin: 0; font-size: var(--fs-md); font-weight: 600; }
.us-chip { margin-inline-start: auto; min-inline-size: 1.6rem; padding: 0.05rem 0.45rem; border-radius: var(--radius-sm);
  text-align: center; font-size: var(--fs-sm); background: color-mix(in srgb, var(--muted) 18%, var(--surface)); }
.us-items { list-style: none; margin: 0; padding: 0; display: grid; }
.us-items li { position: relative; padding-inline-start: 1.25rem; padding-block: 0.55rem; line-height: var(--lh-ui); }
.us-items li + li { border-block-start: 1px solid var(--line-soft); }
.us-items li::before { content: ""; position: absolute; inset-inline-start: 0.2rem; inset-block-start: calc(0.55rem + 0.6em);
  inline-size: 0.35rem; block-size: 0.35rem; border-radius: 50%; background: var(--tone); }
.us-more { border: 0; background: transparent; color: var(--indigo); padding: var(--sp-1) 0; font-weight: 600; }
.us-divider { padding-inline-start: 1.6rem; padding-block: 0 var(--sp-4); font-weight: 600; position: relative; }
.us-divider::before { content: ""; position: absolute; inset-inline-start: 0.45rem; inset-block: 0;
  inline-size: 2px; background: var(--line); }
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
                span { class: "us-logo", aria_hidden: "true", {kind_icon(ChangeKind::New)} }
                div {
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
                    p { class: "muted", "{sub}" }
                }
                if m != SheetMode::History && (from_label.is_some() || !pills.is_empty() || progress.is_some()) {
                    div { class: "us-summary",
                        if let (Some(f), Some(t)) = (from_label.clone(), to_label.clone()) {
                            div { class: "us-jump",
                                span { class: "us-ver-badge", bdi { dir: "ltr", "v{f}" } }
                                span { class: "us-arrow", aria_hidden: "true", "→" }
                                span { class: "us-ver-badge to", bdi { dir: "ltr", "v{t}" } }
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
                            p { class: "muted", "{megabytes(&ui, done)} / {megabytes(&ui, total)}" }
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
                                            if let Some((_, u)) = badge {
                                                span { class: "us-badge", "{ut(lang, u)}" }
                                            }
                                        }
                                        span { class: "us-rel-meta", "{meta}" }
                                        span { class: "us-chevron", aria_hidden: "true", "⌄" }
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
                if action.is_some() {
                    button { onclick: close, "{ut(lang, U::Later)}" }
                }
                match action {
                    Some((label, run)) => rsx! {
                        button { class: "primary", onclick: move |_| run(lang), "{ut(lang, label)}" }
                    },
                    None => rsx! {
                        button { class: "primary", onclick: close,
                            {ut(lang, if m == SheetMode::Updated { U::GotIt } else { U::Close })}
                        }
                    },
                }
            }
        }
    }
}
