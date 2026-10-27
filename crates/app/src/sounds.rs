//! Lesson sounds (SPEC §9), synthesized by `scripts/assets/synth_sounds.py`:
//! a gong (variant 3) when a lesson ends and a short rising chime (ding 1)
//! for a right answer, both picked by ear by the learner. WAV, so
//! every web view plays them; silent when sounds are off in Settings.

use std::sync::LazyLock;

use base64::Engine as _;
use dioxus::prelude::*;

use crate::Ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Gong,
    Ding,
}

static GONG: LazyLock<String> =
    LazyLock::new(|| data_url(include_bytes!("../../../assets/sounds/gong-3.wav")));
static DING: LazyLock<String> =
    LazyLock::new(|| data_url(include_bytes!("../../../assets/sounds/ding-1.wav")));

fn data_url(bytes: &[u8]) -> String {
    format!("data:audio/wav;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn url(sound: Sound) -> &'static str {
    match sound {
        Sound::Gong => &GONG,
        Sound::Ding => &DING,
    }
}

pub fn play(ui: Ui, sound: Sound) {
    if ui.settings.peek().sounds_off {
        return;
    }
    let volume = if sound == Sound::Gong { 0.7 } else { 0.6 };
    document::eval(&format!(
        "try {{ const a = new Audio({:?}); a.volume = {volume}; a.play().catch(() => {{}}); }} catch (e) {{}}",
        url(sound)
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_wav() {
        for s in [Sound::Gong, Sound::Ding] {
            assert!(url(s).starts_with("data:audio/wav;base64,UklGR"), "{s:?}");
        }
    }
}
