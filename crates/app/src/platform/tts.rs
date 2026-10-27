//! Speech in English for oral mode: the system TextToSpeech engine on
//! Android (its WebView has no `speechSynthesis`), the WebView's
//! `speechSynthesis` on the Mac and other desktops, and nothing at all when
//! neither has a voice (the text is always on screen, so the session goes on).
//!
//! Adapted from kabar-app main: crates/app/src/platform/mod.rs (`speak`) and
//! crates/app/src/lesson.rs (the WebView path). Kabar fires and forgets; here
//! [`say`] waits until the voice has finished, so the think timer starts
//! when the question ends.

use std::time::Duration;

#[cfg(target_os = "android")]
use super::tts_android;

/// Why the device's voice didn't speak.
#[allow(dead_code)] // each platform builds only some variants
#[derive(Clone, Debug, PartialEq)]
pub enum SpeakError {
    /// No text-to-speech engine answered (none installed, or disabled).
    NoEngine,
    /// The engine knows English but its voice data isn't downloaded.
    MissingVoice,
    /// The engine has no English voice at all.
    Unsupported,
    /// A JNI, WebView or engine error; logged, not shown.
    Failed(String),
}

/// Speaking rates the learner can pick (Android's scale; 1.0 is normal).
pub const RATES: [f32; 3] = [0.8, 0.95, 1.1];

/// Speaks `text` in English and returns once it has been said. `Err` means
/// nothing was said and the caller shows the text instead.
pub async fn say(text: &str, rate: f32) -> Result<(), SpeakError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    #[cfg(target_os = "android")]
    {
        tts_android::speak(text, rate)?;
        wait_android(text).await;
        Ok(())
    }
    #[cfg(not(target_os = "android"))]
    {
        say_webview(text, rate).await
    }
}

/// Stops whatever is being said (leaving the screen, skipping ahead).
pub fn stop() {
    #[cfg(target_os = "android")]
    if let Err(e) = tts_android::stop() {
        eprintln!("TextToSpeech.stop: {e}");
    }
    #[cfg(not(target_os = "android"))]
    {
        dioxus::document::eval("window.speechSynthesis && window.speechSynthesis.cancel();");
    }
}

/// A generous upper bound for saying `text`, in case the engine never
/// reports that it finished: about 2.5 words a second, plus start-up.
pub fn longest(text: &str) -> Duration {
    let words = text.split_whitespace().count() as u64;
    Duration::from_millis(4_000 + words * 400)
}

/// Android: `isSpeaking()` turns true shortly after `speak()` (longer while
/// the engine is still binding), then false at the end.
#[cfg(target_os = "android")]
async fn wait_android(text: &str) {
    const POLL: Duration = Duration::from_millis(150);
    let started = std::time::Instant::now();
    let limit = longest(text) + tts_android::CONNECT_TIMEOUT;
    let mut heard = false;
    // When the engine was up and nothing started within this, it never will.
    let mut silent_since: Option<std::time::Instant> = None;
    while started.elapsed() < limit {
        tokio::time::sleep(POLL).await;
        match tts_android::is_speaking() {
            Ok(true) => heard = true,
            Ok(false) if heard => return,
            Ok(false) if tts_android::idle() => {
                let since = *silent_since.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() > Duration::from_millis(2_500) {
                    return;
                }
            }
            Ok(false) => {}
            Err(_) => return,
        }
    }
}

#[cfg(not(target_os = "android"))]
async fn say_webview(text: &str, rate: f32) -> Result<(), SpeakError> {
    let js = format!(
        r#"
        const s = window.speechSynthesis;
        if (!s || typeof SpeechSynthesisUtterance === "undefined") return "none";
        s.cancel();
        let voices = s.getVoices();
        if (!voices.length) {{
            await new Promise(r => {{ s.onvoiceschanged = r; setTimeout(r, 1000); }});
            voices = s.getVoices();
        }}
        const en = voices.filter(v => v.lang.replace("_", "-").toLowerCase().startsWith("en"));
        const v = en.find(v => /en-(us|gb)/i.test(v.lang.replace("_", "-")) && v.localService) || en[0];
        if (!v && voices.length) return "missing";
        const u = new SpeechSynthesisUtterance({text});
        u.lang = v ? v.lang : "en-US";
        if (v) u.voice = v;
        u.rate = {rate};
        return await new Promise(resolve => {{
            u.onend = () => resolve("ok");
            u.onerror = e => resolve(e.error === "interrupted" || e.error === "canceled" ? "ok" : "error: " + e.error);
            s.speak(u);
        }});
        "#,
        text = js_string(text),
        rate = rate,
    );
    let limit = longest(text) + Duration::from_secs(2);
    let result = tokio::time::timeout(limit, dioxus::document::eval(&js).join::<String>()).await;
    match result {
        Ok(Ok(r)) if r == "ok" => Ok(()),
        Ok(Ok(r)) if r == "none" => Err(SpeakError::NoEngine),
        Ok(Ok(r)) if r == "missing" => Err(SpeakError::Unsupported),
        Ok(Ok(r)) => Err(SpeakError::Failed(r)),
        Ok(Err(e)) => Err(SpeakError::Failed(format!("{e:?}"))),
        // Spoke but never said it finished (some WebKit builds): carry on.
        Err(_) => Ok(()),
    }
}

/// `text` as a JavaScript string literal.
#[cfg(not(target_os = "android"))]
fn js_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_strings_are_escaped() {
        assert_eq!(js_string("a \"b\"\n\\"), r#""a \"b\"\n\\""#);
        assert_eq!(js_string("</script>"), r#""</script>""#);
    }

    #[test]
    fn long_text_gets_more_time() {
        assert!(longest("one two three") < longest(&"word ".repeat(100)));
    }
}
