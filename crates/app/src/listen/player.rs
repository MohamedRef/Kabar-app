//! Plays a [`Program`] (a Sinetron episode or a Kabar Radio show) line by
//! line with a transcript that follows the audio. A downloaded episode is one
//! joined track, played offline; otherwise each line comes from the audio
//! cache, Kabar Cloud's neural voices, or (Indonesian) the device's voice.

use std::collections::BTreeSet;

use dioxus::document::Eval;
use dioxus::prelude::*;
use kabar_core::lesson::pick;
use kabar_core::settings::TranslationMode;
use kabar_core::story::{Heading, Program, cue_at};
use kabar_protocol::cloud::TtsRequest;
use kabar_protocol::story::SegmentKind;
use serde::Deserialize;

use super::text::{T, tt};
use crate::account::Cloud;
use crate::annotated::Annotated;
use crate::audio::{self, AudioCache, Track};
use crate::cloud::SyncState;
use crate::cloud::link::Unreachable;
use crate::lesson::{Rich, target_tag};
use crate::portraits::{self, Face, Mood};
use crate::{Ui, platform};

/// One `<audio>` element for the whole player, driven by messages. Every
/// event carries the `seq` of the load it belongs to, so a late `ended` from
/// a previous line never skips ahead.
const ENGINE_JS: &str = r#"
const a = new Audio();
let seq = 0, timer = null;
const send = (ev, extra) => dioxus.send(Object.assign({ ev, seq }, extra || {}));
a.addEventListener("ended", () => send("ended"));
a.addEventListener("error", () => { if (a.getAttribute("src")) send("error"); });
a.addEventListener("timeupdate", () => send("time", { t: a.currentTime }));
for (;;) {
  const c = await dioxus.recv();
  if (timer) { clearTimeout(timer); timer = null; }
  if (c.op === "stop") { a.pause(); a.removeAttribute("src"); break; }
  if (typeof c.seq === "number") seq = c.seq;
  if (c.op === "load") {
    a.pause();
    a.src = c.src;
    a.playbackRate = c.rate;
    if (c.at) a.addEventListener("loadedmetadata", () => { a.currentTime = c.at; }, { once: true });
    a.play().catch(() => send("error"));
  } else if (c.op === "seek") {
    a.currentTime = c.at;
    a.playbackRate = c.rate;
    a.play().catch(() => send("error"));
  } else if (c.op === "pause") {
    a.pause();
  } else if (c.op === "resume") {
    a.play().catch(() => send("error"));
  } else if (c.op === "rate") {
    a.playbackRate = c.rate;
  } else if (c.op === "wait") {
    a.pause();
    timer = setTimeout(() => send("ended"), c.ms);
  }
}
"#;

#[derive(Deserialize)]
struct EngineEvent {
    ev: String,
    seq: u64,
    #[serde(default)]
    t: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DownloadState {
    Idle,
    Running(usize, usize),
    Failed(T),
}

/// The TTS request for every line, in order (what a download fetches).
pub fn requests(program: &Program) -> Vec<TtsRequest> {
    program.cues.iter().map(|c| audio::line(c.voice, &c.text, c.pitch_percent, 0)).collect()
}

/// What to tell the learner when Kabar Cloud couldn't be asked.
pub fn notice_for(e: &Unreachable) -> T {
    match e {
        Unreachable::NotConfigured | Unreachable::SignedOut => T::NeedCloud,
        Unreachable::Offline => T::Offline,
        Unreachable::QuotaExhausted => T::Quota,
        Unreachable::Server(_) => T::ServerTrouble,
    }
}

/// Reading pace for lines the device's voice speaks (it doesn't say when
/// it's done): about 75 ms a character plus a breath.
fn spoken_ms(text: &str, rate: f64) -> u64 {
    let ms = 600.0 + 75.0 * text.chars().count() as f64;
    (ms / rate) as u64
}

/// Everything the player's handlers share. All fields are `Copy` handles.
#[derive(Clone, Copy)]
struct Deck {
    program: CopyValue<Program>,
    /// Shown on the lock screen while the native player has the track.
    title: CopyValue<String>,
    /// The downloaded track is loaded in the native background player.
    native: Signal<bool>,
    cache: CopyValue<Option<AudioCache>>,
    cloud: Option<Cloud>,
    engine: Signal<Option<Eval>>,
    seq: Signal<u64>,
    current: Signal<usize>,
    playing: Signal<bool>,
    track: Signal<Option<Track>>,
    track_loaded: Signal<bool>,
    rate: Signal<f64>,
    notice: Signal<Option<T>>,
    finished: Signal<bool>,
    missing: Signal<bool>,
}

impl Deck {
    fn send(&self, msg: serde_json::Value) {
        if let Some(e) = self.engine.peek().as_ref() {
            e.send(msg).ok();
        }
    }

    fn next_seq(mut self) -> u64 {
        let s = *self.seq.peek() + 1;
        self.seq.set(s);
        s
    }

    fn live(&self) -> bool {
        self.cloud.is_some_and(|c| matches!(c.status.peek().sync, SyncState::Live | SyncState::CatchingUp))
    }

    fn offline_notice(&self) -> T {
        match self.cloud.map(|c| c.status.peek().sync) {
            None | Some(SyncState::SignedOut | SyncState::Stopped) => T::NeedCloud,
            _ => T::Offline,
        }
    }

    fn scroll_to(&self, i: usize) {
        document::eval(&format!(
            r#"setTimeout(() => document.querySelector('[data-cue="{i}"]')?.scrollIntoView({{
                block: "center",
                behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
            }}), 30);"#
        ));
    }

    /// Plays line `i` (and, while playing, the ones after it).
    fn play(mut self, i: usize) {
        let Some(cue) = self.program.peek().cues.get(i).cloned() else { return };
        let seq = self.next_seq();
        self.current.set(i);
        self.playing.set(true);
        self.finished.set(false);
        self.scroll_to(i);
        let rate = *self.rate.peek();
        let track = self.track.peek().clone();
        if let Some(track) = track {
            let at = track.starts.get(i).copied().unwrap_or_default();
            if platform::BACKGROUND_AUDIO {
                let ok = if *self.native.peek() {
                    platform::track::seek(at)
                } else {
                    platform::track::play(
                        &track.path.to_string_lossy(),
                        &self.title.peek(),
                        "Kabar",
                        at,
                        rate as f32,
                    )
                };
                match ok {
                    Ok(()) => return self.native.set(true),
                    Err(e) => crate::diag::report!("background playback failed, playing in the app: {e}"),
                }
            }
            if *self.track_loaded.peek() {
                self.send(serde_json::json!({ "op": "seek", "at": at, "rate": rate, "seq": seq }));
            } else {
                match audio::data_url(&track.path) {
                    Ok(src) => {
                        self.track_loaded.set(true);
                        self.send(serde_json::json!({ "op": "load", "src": src, "at": at, "rate": rate, "seq": seq }));
                    }
                    Err(e) => {
                        crate::diag::report!("reading the downloaded episode failed: {e}");
                        self.track.set(None);
                        self.play(i);
                    }
                }
            }
            return;
        }
        let Some(cache) = self.cache.peek().clone() else { return self.fallback(i, seq, None) };
        let req = audio::line(cue.voice, &cue.text, cue.pitch_percent, 0);
        if cache.cached(&req).is_none() && !self.live() {
            return self.fallback(i, seq, Some(self.offline_notice()));
        }
        let next =
            self.program.peek().cues.get(i + 1).map(|c| audio::line(c.voice, &c.text, c.pitch_percent, 0));
        spawn(async move {
            let got = cache.fetch(&req).await;
            if *self.seq.peek() != seq {
                return; // Another line was asked for meanwhile.
            }
            match got.map(|p| audio::data_url(&p)) {
                Ok(Ok(src)) => {
                    let mut notice = self.notice;
                    if notice.peek().is_some_and(|n| n != T::Quota) {
                        notice.set(None);
                    }
                    self.send(serde_json::json!({ "op": "load", "src": src, "rate": rate, "seq": seq }));
                    // The next line is fetched while this one plays.
                    if let Some(next) = next
                        && self.live()
                    {
                        cache.fetch(&next).await.ok();
                    }
                }
                Ok(Err(e)) => {
                    crate::diag::report!("reading cached audio failed: {e}");
                    self.fallback(i, seq, Some(T::ServerTrouble));
                }
                Err(e) => self.fallback(i, seq, Some(notice_for(&e))),
            }
        });
    }

    /// No neural audio for this line: Indonesian is read by the device's
    /// voice at reading pace; Javanese waits for its voice (plan §16).
    fn fallback(mut self, i: usize, seq: u64, why: Option<T>) {
        let program = self.program.peek();
        let Some(cue) = program.cues.get(i) else { return };
        let rate = *self.rate.peek();
        if program.language == "jv" {
            self.notice.set(Some(why.filter(|w| *w == T::Quota).unwrap_or(T::JavaneseLater)));
            self.playing.set(false);
            return;
        }
        self.notice.set(Some(if why == Some(T::Quota) { T::Quota } else { T::SystemVoice }));
        crate::lesson::speak_with(&cue.text, &program.language, rate < 1.0, None, self.missing);
        self.send(serde_json::json!({ "op": "wait", "ms": spoken_ms(&cue.text, rate), "seq": seq }));
    }

    fn pause(mut self) {
        self.playing.set(false);
        if *self.native.peek() {
            platform::track::pause().ok();
        }
        self.send(serde_json::json!({ "op": "pause" }));
    }

    fn resume(self) {
        if self.track.peek().is_some() && *self.native.peek() && !*self.finished.peek() {
            let mut playing = self.playing;
            playing.set(true);
            if let Err(e) = platform::track::resume() {
                crate::diag::report!("resuming the background player failed: {e}");
            }
        } else if self.track.peek().is_some() && *self.track_loaded.peek() {
            let mut playing = self.playing;
            playing.set(true);
            self.send(serde_json::json!({ "op": "resume" }));
        } else {
            let at = if *self.finished.peek() { 0 } else { *self.current.peek() };
            self.play(at);
        }
    }

    /// The native player's state, polled: moves the transcript along, and
    /// follows a pause or play from the lock screen.
    fn native_tick(mut self, t: f64, playing: bool, ended: bool) {
        let seq = *self.seq.peek();
        if ended {
            if *self.playing.peek() {
                self.on_event(EngineEvent { ev: "ended".into(), seq, t });
            }
            return;
        }
        if playing != *self.playing.peek() {
            self.playing.set(playing);
        }
        if t >= 0.0 {
            self.on_event(EngineEvent { ev: "time".into(), seq, t });
        }
    }

    /// Hands the track back from the native player (download removed, or leaving).
    fn stop_native(mut self) {
        if *self.native.peek() {
            self.native.set(false);
            platform::track::stop().ok();
        }
    }

    fn on_event(mut self, e: EngineEvent) {
        if e.seq != *self.seq.peek() {
            return;
        }
        let n = self.program.peek().cues.len();
        let tracked = self.track.peek().clone();
        match (e.ev.as_str(), tracked) {
            ("time", Some(track)) => {
                let at = cue_at(&track.starts, e.t);
                if at != *self.current.peek() {
                    self.current.set(at);
                    if *self.playing.peek() {
                        self.scroll_to(at);
                    }
                }
            }
            ("ended", Some(_)) => {
                self.playing.set(false);
                self.finished.set(true);
            }
            ("ended", None) => {
                let i = *self.current.peek();
                if i + 1 < n && *self.playing.peek() {
                    self.play(i + 1);
                } else {
                    self.playing.set(false);
                    self.finished.set(i + 1 >= n);
                }
            }
            ("error", Some(_)) => {
                // A broken download: drop it and stream instead.
                crate::diag::report!("the downloaded episode didn't play");
                self.track.set(None);
                self.track_loaded.set(false);
                let i = *self.current.peek();
                self.play(i);
            }
            ("error", None) => {
                let i = *self.current.peek();
                self.fallback(i, e.seq, Some(T::ServerTrouble));
            }
            _ => {}
        }
    }
}

fn heading_label(kind: SegmentKind) -> T {
    match kind {
        SegmentKind::Intro => T::Intro,
        SegmentKind::Dialogue => T::Dialogue,
        SegmentKind::WordSpotlight => T::WordSpotlight,
        SegmentKind::Recap => T::Recap,
        SegmentKind::Outro => T::Outro,
    }
}

/// The player: transcript, controls and download. `children` is the end
/// card (next-time teaser, questions, words); `on_end` runs once when the
/// last line has played.
#[component]
pub fn ProgramPlayer(
    program: Program,
    title: String,
    on_back: EventHandler,
    on_end: EventHandler,
    children: Element,
    /// False for a chat replay: nothing to keep offline.
    #[props(default = true)]
    downloadable: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let cache = try_use_context::<AudioCache>();
    let cloud = try_use_context::<Cloud>();
    let target = target_tag(&program.language);

    let deck = use_hook(|| Deck {
        program: CopyValue::new(program.clone()),
        title: CopyValue::new(title.clone()),
        native: Signal::new(false),
        track: Signal::new(cache.as_ref().and_then(|c| c.track(&program.id))),
        cache: CopyValue::new(cache.clone()),
        cloud,
        engine: Signal::new(None),
        seq: Signal::new(0),
        current: Signal::new(0),
        playing: Signal::new(false),
        track_loaded: Signal::new(false),
        rate: Signal::new(1.0),
        notice: Signal::new(None),
        finished: Signal::new(false),
        missing: Signal::new(false),
    });
    // The translation setting decides whether every line starts translated.
    let mut show_all = use_signal(|| ui.settings.peek().translation == TranslationMode::On);
    let mut revealed = use_signal(BTreeSet::<usize>::new);
    let mut notes = use_signal(BTreeSet::<usize>::new);
    let mut download = use_signal(|| DownloadState::Idle);
    let mut ended_once = use_signal(|| false);

    use_future(move || async move {
        let mut eval = document::eval(ENGINE_JS);
        let mut engine = deck.engine;
        engine.set(Some(eval));
        while let Ok(e) = eval.recv::<EngineEvent>().await {
            deck.on_event(e);
        }
    });
    // Downloaded tracks on Android: follow the native player.
    use_future(move || async move {
        if !platform::BACKGROUND_AUDIO {
            return;
        }
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            if *deck.native.peek() {
                match platform::track::state() {
                    Ok((t, playing, ended)) => deck.native_tick(t, playing, ended),
                    Err(e) => crate::diag::report!("reading the background player failed: {e}"),
                }
            }
        }
    });
    use_drop(move || {
        deck.stop_native();
        deck.send(serde_json::json!({ "op": "stop" }));
    });
    use_effect(move || {
        if *deck.finished.read() && !*ended_once.peek() {
            ended_once.set(true);
            on_end.call(());
        }
    });

    let id = program.id.clone();
    let start_download = move |_| {
        let Some(cache) = deck.cache.peek().clone() else { return };
        let reqs = requests(&deck.program.peek());
        let id = id.clone();
        spawn(async move {
            download.set(DownloadState::Running(0, reqs.len()));
            match cache.download_track(&id, &reqs, |d, t| download.set(DownloadState::Running(d, t))).await {
                Ok(t) => {
                    let (mut track, mut loaded) = (deck.track, deck.track_loaded);
                    track.set(Some(t));
                    loaded.set(false);
                    download.set(DownloadState::Idle);
                }
                Err(e) => download.set(DownloadState::Failed(notice_for(&e))),
            }
        });
    };
    let id = program.id.clone();
    let remove_download = move |_| {
        if let Some(cache) = deck.cache.peek().as_ref() {
            cache.remove_track(&id);
        }
        deck.pause();
        deck.stop_native();
        let (mut track, mut loaded) = (deck.track, deck.track_loaded);
        track.set(None);
        loaded.set(false);
    };

    let current = *deck.current.read();
    let playing = *deck.playing.read();
    let slow = *deck.rate.read() < 1.0;
    let downloaded = deck.track.read().is_some();
    let notice = *deck.notice.read();
    let dl = *download.read();
    let translated = program.cues.iter().any(|c| !c.meaning.en.is_empty());
    let first_speaker = program.cues.first().map(|c| c.speaker.clone()).unwrap_or_default();
    let mut scene_no = 0;

    rsx! {
        div { class: "lx-player",
            header { class: "lx-head",
                button { class: "quiet", onclick: move |_| on_back.call(()), "{tt(lang, T::Back)}" }
                h2 { bdi { lang: target, class: "latin", "{title}" } }
            }
            div { class: "row lx-download", display: if downloadable { "" } else { "none" },
                match dl {
                    DownloadState::Running(done, total) => rsx! {
                        span { class: "muted small", role: "status",
                            "{tt(lang, T::Downloading)} "
                            bdi { "{ui.num(done as u64)}/{ui.num(total as u64)}" }
                        }
                    },
                    _ if downloaded => rsx! {
                        span { class: "pill lx-offline", "✓ {tt(lang, T::Downloaded)}" }
                        button { class: "quiet small", onclick: remove_download, "{tt(lang, T::RemoveDownload)}" }
                    },
                    _ => rsx! {
                        button { onclick: start_download, "⤓ {tt(lang, T::Download)}" }
                    },
                }
            }
            if let DownloadState::Failed(why) = dl {
                p { class: "lx-notice", role: "status", "{tt(lang, why)}" }
            }
            p { class: "muted small", "{tt(lang, T::TapLine)}" }
            if let Some(n) = notice {
                p { class: "lx-notice", role: "status", "{tt(lang, n)}" }
            }
            ol { class: "lx-lines",
                for (g, heading) in program.groups.iter().enumerate() {
                    li { key: "g{g}", class: "lx-group",
                        match heading {
                            Heading::Scene(setting) => {
                                scene_no += 1;
                                rsx! {
                                    h3 { "{tt(lang, T::Scene)} {ui.num(scene_no as u64)}" }
                                    p { class: "muted", Rich { text: pick(setting, lang).to_string(), target } }
                                }
                            }
                            Heading::Segment { kind, title } => rsx! {
                                h3 {
                                    "{tt(lang, heading_label(*kind))}"
                                    if let Some(t) = title {
                                        " · "
                                        bdi { lang: target, class: "latin", "{t}" }
                                    }
                                }
                            },
                        }
                        ol { class: "lx-cues",
                            for (i, cue) in program.cues.iter().enumerate().filter(|(_, c)| c.group == g) {
                                li {
                                    key: "{i}",
                                    "data-cue": "{i}",
                                    class: format!(
                                        "lx-cue {} {}",
                                        if cue.speaker == first_speaker { "a" } else { "b" },
                                        if i == current && *deck.seq.read() > 0 { "on" } else { "" },
                                    ),
                                    button {
                                        class: "lx-line",
                                        aria_current: if i == current && playing { "true" } else { "false" },
                                        onclick: move |_| deck.play(i),
                                        span { class: "lx-speaker",
                                            if let Some(src) = portraits::of(&cue.speaker, Mood::Neutral) {
                                                Face { src, class: "lx-face" }
                                            }
                                            bdi { "{cue.speaker}" }
                                        }
                                        Annotated { text: cue.text.clone(), tag: target, class: "lx-text" }
                                    }
                                    if show_all() || revealed.read().contains(&i) {
                                        p { class: "lx-meaning muted", Rich { text: pick(&cue.meaning, lang).to_string(), target } }
                                    } else if !cue.meaning.en.is_empty() {
                                        button {
                                            class: "lx-reveal quiet",
                                            aria_label: "{tt(lang, T::Translations)}",
                                            onclick: move |_| { revealed.write().insert(i); },
                                            "…"
                                        }
                                    }
                                    if let Some(note) = &cue.note {
                                        button {
                                            class: "pill lx-note-chip",
                                            aria_expanded: if notes.read().contains(&i) { "true" } else { "false" },
                                            onclick: move |_| {
                                                let mut n = notes.write();
                                                if !n.remove(&i) {
                                                    n.insert(i);
                                                }
                                            },
                                            "ⓘ {tt(lang, T::Note)}"
                                        }
                                        if notes.read().contains(&i) {
                                            p { class: "lx-note", Rich { text: pick(note, lang).to_string(), target } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            section { class: "lx-end", {children} }
            div { class: "lx-controls",
                if playing {
                    button { class: "primary", onclick: move |_| deck.pause(), "⏸ {tt(lang, T::Pause)}" }
                } else {
                    button { class: "primary", onclick: move |_| deck.resume(), "▶ {tt(lang, T::Play)}" }
                }
                button {
                    aria_pressed: if slow { "true" } else { "false" },
                    onclick: move |_| {
                        let mut rate = deck.rate;
                        let r = if slow { 1.0 } else { 0.8 };
                        rate.set(r);
                        if *deck.native.peek() {
                            platform::track::rate(r as f32).ok();
                        }
                        deck.send(serde_json::json!({ "op": "rate", "rate": r }));
                    },
                    "{tt(lang, T::Slower)}"
                }
                if translated {
                    button {
                        aria_pressed: if show_all() { "true" } else { "false" },
                        onclick: move |_| show_all.toggle(),
                        "{tt(lang, T::Translations)}"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_voice_pace_follows_length_and_rate() {
        assert_eq!(spoken_ms("", 1.0), 600);
        assert!(spoken_ms("Selamat pagi, Pak!", 0.8) > spoken_ms("Selamat pagi, Pak!", 1.0));
        assert_eq!(notice_for(&Unreachable::QuotaExhausted), T::Quota);
        assert_eq!(notice_for(&Unreachable::SignedOut), T::NeedCloud);
    }
}
