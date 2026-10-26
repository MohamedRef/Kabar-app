//! Indonesian (and, where the device has one, Javanese) speech through the
//! WebView's Web Speech API (`speechSynthesis`, a learner-friendly rate).
//! macOS ships Indonesian voices; Android WebViews usually have no
//! `speechSynthesis` at all, so callers check [`use_voice`] first and keep
//! the text usable without audio. Kabar Cloud's Azure voices (spec §20)
//! replace this once the app has a Cloud client.
//!
//! Javanese has its own voice check ([`use_javanese_voice`]) and never falls
//! back to an Indonesian voice: Indonesian voices can't make the curled
//! Javanese dh/th, so Minimal Pairs would teach the wrong sound.

use dioxus::prelude::*;

/// Slower than normal, so every syllable is heard.
pub const RATE: f32 = 0.85;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    Checking,
    Ready,
    /// No `speechSynthesis` or no voice for the language on this device.
    Missing,
}

/// A language the device may have a voice for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeechLang {
    Indonesian,
    Javanese,
}

impl SpeechLang {
    /// The JS function (defined by [`FIND_VOICES`]) that finds this voice.
    fn finder(self) -> &'static str {
        match self {
            Self::Indonesian => "kabarIdVoice",
            Self::Javanese => "kabarJvVoice",
        }
    }

    fn bcp47(self) -> &'static str {
        match self {
            Self::Indonesian => "id-ID",
            Self::Javanese => "jv-ID",
        }
    }

    /// Whether to speak with the browser's default voice when no matching
    /// voice is listed. Never for Javanese (see the module docs).
    fn may_fall_back(self) -> bool {
        self == Self::Indonesian
    }
}

/// Picks voices: Indonesian is `id`/`in` (old Android code), Javanese is
/// `jv`/`jw` (old code).
const FIND_VOICES: &str = r#"
const kabarFindVoice = (codes) => {
  if (!("speechSynthesis" in window)) return null;
  return speechSynthesis.getVoices().find(v => {
    const l = (v.lang || "").toLowerCase().replace("_", "-");
    return codes.some(c => l === c || l.startsWith(c + "-"));
  }) || null;
};
const kabarIdVoice = () => kabarFindVoice(["id", "in"]);
const kabarJvVoice = () => kabarFindVoice(["jv", "jw"]);
"#;

fn probe_script(lang: SpeechLang) -> String {
    let find = lang.finder();
    format!(
        r#"{FIND_VOICES}
        if (!("speechSynthesis" in window)) {{ dioxus.send(false); return; }}
        if ({find}()) {{ dioxus.send(true); return; }}
        await new Promise(r => {{
          speechSynthesis.addEventListener("voiceschanged", r, {{ once: true }});
          setTimeout(r, 1500);
        }});
        dioxus.send(!!{find}());
        "#
    )
}

fn use_voice_for(lang: SpeechLang) -> Signal<Voice> {
    let mut voice = use_signal(|| Voice::Checking);
    use_future(move || async move {
        let mut eval = document::eval(&probe_script(lang));
        let ok = eval.recv::<bool>().await.unwrap_or(false);
        voice.set(if ok { Voice::Ready } else { Voice::Missing });
    });
    voice
}

/// Whether this device can speak Indonesian. Voices load asynchronously, so
/// it waits briefly for `voiceschanged`.
pub fn use_voice() -> Signal<Voice> {
    use_voice_for(SpeechLang::Indonesian)
}

/// Whether this device has a real Javanese (`jv`) voice. An Indonesian voice
/// doesn't count.
pub fn use_javanese_voice() -> Signal<Voice> {
    use_voice_for(SpeechLang::Javanese)
}

/// The script that speaks `texts` in order. It sends the index of each
/// sentence as it starts (offset by `first`) and `-1` when done or stopped.
fn speak_script(texts: &[String], first: usize, lang: SpeechLang) -> String {
    let json = serde_json::to_string(texts).unwrap_or_else(|_| "[]".into());
    let find = lang.finder();
    let tag = lang.bcp47();
    let fallback = lang.may_fall_back();
    format!(
        r#"{FIND_VOICES}
        const texts = {json};
        if (!("speechSynthesis" in window)) {{ dioxus.send(-1); return; }}
        window.kabarSpeakToken = (window.kabarSpeakToken || 0) + 1;
        const token = window.kabarSpeakToken;
        speechSynthesis.cancel();
        const voice = {find}();
        if (!voice && !{fallback}) {{ dioxus.send(-1); return; }}
        for (let i = 0; i < texts.length; i++) {{
          if (window.kabarSpeakToken !== token) break;
          dioxus.send({first} + i);
          await new Promise(done => {{
            const u = new SpeechSynthesisUtterance(texts[i]);
            u.lang = "{tag}";
            u.rate = {RATE};
            if (voice) u.voice = voice;
            u.onend = done;
            u.onerror = done;
            speechSynthesis.speak(u);
          }});
        }}
        dioxus.send(-1);
        "#
    )
}

fn speak_in(
    lang: SpeechLang,
    texts: Vec<String>,
    first: usize,
    mut on_sentence: impl FnMut(Option<usize>) + 'static,
) {
    spawn(async move {
        let mut eval = document::eval(&speak_script(&texts, first, lang));
        while let Ok(i) = eval.recv::<i64>().await {
            match usize::try_from(i) {
                Ok(i) => on_sentence(Some(i)),
                Err(_) => break,
            }
        }
        on_sentence(None);
    });
}

/// Speaks `texts`; `on_sentence` gets the index being spoken, then `None`.
pub fn speak(texts: Vec<String>, first: usize, on_sentence: impl FnMut(Option<usize>) + 'static) {
    speak_in(SpeechLang::Indonesian, texts, first, on_sentence);
}

/// Says one word with the device's Javanese voice, or nothing if it has none.
pub fn speak_javanese(text: String) {
    speak_in(SpeechLang::Javanese, vec![text], 0, |_| {});
}

/// Stops whatever is being spoken.
pub fn stop() {
    document::eval(
        r#"window.kabarSpeakToken = (window.kabarSpeakToken || 0) + 1;
        if ("speechSynthesis" in window) speechSynthesis.cancel();"#,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_embeds_text_safely() {
        let s =
            speak_script(&["Apa \"kabar\"?".to_string(), "</script>".to_string()], 2, SpeechLang::Indonesian);
        assert!(s.contains(r#"const texts = ["Apa \"kabar\"?","</script>"];"#));
        assert!(s.contains("dioxus.send(2 + i)"));
        assert!(s.contains("u.lang = \"id-ID\""));
        assert!(s.contains("u.rate = 0.85"));
        assert!(s.contains("const voice = kabarIdVoice();"));
        assert!(s.contains("if (!voice && !true)"));
    }

    #[test]
    fn javanese_never_falls_back_to_another_voice() {
        let s = speak_script(&["wedhi".to_string()], 0, SpeechLang::Javanese);
        assert!(s.contains("u.lang = \"jv-ID\""));
        assert!(s.contains("const voice = kabarJvVoice();"));
        assert!(s.contains("if (!voice && !false) { dioxus.send(-1); return; }"));
        let p = probe_script(SpeechLang::Javanese);
        assert!(p.contains("if (kabarJvVoice())"));
        assert!(!p.contains("if (kabarIdVoice())"));
        assert!(FIND_VOICES.contains(r#"kabarFindVoice(["jv", "jw"])"#));
    }
}
