//! Indonesian speech through the WebView's Web Speech API (`speechSynthesis`,
//! `id-ID`, a learner-friendly rate). macOS ships Indonesian voices; Android
//! WebViews usually have no `speechSynthesis` at all, so callers check
//! [`use_voice`] first and keep the text usable without audio. Kabar Cloud's
//! Azure voices (spec §20) replace this once the app has a Cloud client.

use dioxus::prelude::*;

/// Slower than normal, so every syllable is heard.
pub const RATE: f32 = 0.85;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    Checking,
    Ready,
    /// No `speechSynthesis` or no Indonesian voice on this device.
    Missing,
}

/// Picks an Indonesian voice; `id`/`in` (old Android code) both count.
const FIND_VOICE: &str = r#"
const kabarIdVoice = () => {
  if (!("speechSynthesis" in window)) return null;
  return speechSynthesis.getVoices().find(v => {
    const l = (v.lang || "").toLowerCase().replace("_", "-");
    return l.startsWith("id") || l.startsWith("in-");
  }) || null;
};
"#;

/// Whether this device can speak Indonesian. Voices load asynchronously, so
/// it waits briefly for `voiceschanged`.
pub fn use_voice() -> Signal<Voice> {
    let mut voice = use_signal(|| Voice::Checking);
    use_future(move || async move {
        let mut eval = document::eval(&format!(
            r#"{FIND_VOICE}
            if (!("speechSynthesis" in window)) {{ dioxus.send(false); return; }}
            if (kabarIdVoice()) {{ dioxus.send(true); return; }}
            await new Promise(r => {{
              speechSynthesis.addEventListener("voiceschanged", r, {{ once: true }});
              setTimeout(r, 1500);
            }});
            dioxus.send(!!kabarIdVoice());
            "#
        ));
        let ok = eval.recv::<bool>().await.unwrap_or(false);
        voice.set(if ok { Voice::Ready } else { Voice::Missing });
    });
    voice
}

/// The script that speaks `texts` in order. It sends the index of each
/// sentence as it starts (offset by `first`) and `-1` when done or stopped.
fn speak_script(texts: &[String], first: usize) -> String {
    let json = serde_json::to_string(texts).unwrap_or_else(|_| "[]".into());
    format!(
        r#"{FIND_VOICE}
        const texts = {json};
        if (!("speechSynthesis" in window)) {{ dioxus.send(-1); return; }}
        window.kabarSpeakToken = (window.kabarSpeakToken || 0) + 1;
        const token = window.kabarSpeakToken;
        speechSynthesis.cancel();
        const voice = kabarIdVoice();
        for (let i = 0; i < texts.length; i++) {{
          if (window.kabarSpeakToken !== token) break;
          dioxus.send({first} + i);
          await new Promise(done => {{
            const u = new SpeechSynthesisUtterance(texts[i]);
            u.lang = "id-ID";
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

/// Speaks `texts`; `on_sentence` gets the index being spoken, then `None`.
pub fn speak(texts: Vec<String>, first: usize, mut on_sentence: impl FnMut(Option<usize>) + 'static) {
    spawn(async move {
        let mut eval = document::eval(&speak_script(&texts, first));
        while let Ok(i) = eval.recv::<i64>().await {
            match usize::try_from(i) {
                Ok(i) => on_sentence(Some(i)),
                Err(_) => break,
            }
        }
        on_sentence(None);
    });
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
        let s = speak_script(&["Apa \"kabar\"?".to_string(), "</script>".to_string()], 2);
        assert!(s.contains(r#"const texts = ["Apa \"kabar\"?","</script>"];"#));
        assert!(s.contains("dioxus.send(2 + i)"));
        assert!(s.contains("u.lang = \"id-ID\""));
        assert!(s.contains("u.rate = 0.85"));
    }
}
