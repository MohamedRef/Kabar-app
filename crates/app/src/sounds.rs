//! Lesson sounds (SPEC §9), synthesized by `scripts/assets/synth_sounds.py`:
//! a gong when a lesson ends and a short chime for a right answer, each
//! picked in Settings › Learning (gong 3 and ding 1 unless changed). WAV, so
//! every web view plays them; silent when sounds are off. Phones also give a
//! light tap of vibration with each ([`crate::platform::vibrate`]).

use std::collections::HashMap;
use std::sync::LazyLock;

use base64::Engine as _;
use dioxus::prelude::*;
use kabar_core::settings::{CorrectSound, EndSound};

use crate::Ui;
use crate::platform::Haptic;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// A lesson is done.
    LessonEnd,
    /// A right (or nearly right) answer.
    Correct,
}

/// Every variation, by file stem, so Settings can preview any of them.
const FILES: [(&str, &[u8]); 9] = [
    ("gong-1", include_bytes!("../../../assets/sounds/gong-1.wav")),
    ("gong-2", include_bytes!("../../../assets/sounds/gong-2.wav")),
    ("gong-3", include_bytes!("../../../assets/sounds/gong-3.wav")),
    ("ding-1", include_bytes!("../../../assets/sounds/ding-1.wav")),
    ("ding-2", include_bytes!("../../../assets/sounds/ding-2.wav")),
    ("ding-3", include_bytes!("../../../assets/sounds/ding-3.wav")),
    ("kendang-1", include_bytes!("../../../assets/sounds/kendang-1.wav")),
    ("kendang-2", include_bytes!("../../../assets/sounds/kendang-2.wav")),
    ("kendang-3", include_bytes!("../../../assets/sounds/kendang-3.wav")),
];

static URLS: LazyLock<HashMap<&'static str, String>> = LazyLock::new(|| {
    FILES
        .iter()
        .map(|&(id, bytes)| {
            (id, format!("data:audio/wav;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
        })
        .collect()
});

fn url(id: &str) -> Option<&'static str> {
    URLS.get(id).map(String::as_str)
}

fn eval_play(id: &str, volume: f32) {
    let Some(url) = url(id) else { return };
    document::eval(&format!(
        "try {{ const a = new Audio({url:?}); a.volume = {volume}; a.play().catch(() => {{}}); }} catch (e) {{}}"
    ));
}

const END_VOLUME: f32 = 0.7;
const CORRECT_VOLUME: f32 = 0.6;

/// Plays the learner's chosen sound, and the matching vibration.
pub fn play(ui: Ui, sound: Sound) {
    let settings = ui.settings.peek().clone();
    if !settings.haptics_off {
        crate::platform::vibrate(match sound {
            Sound::LessonEnd => Haptic::LessonEnd,
            Sound::Correct => Haptic::Correct,
        });
    }
    if settings.sounds_off {
        return;
    }
    match sound {
        Sound::LessonEnd => eval_play(settings.end_sound.id(), END_VOLUME),
        Sound::Correct => eval_play(settings.correct_sound.id(), CORRECT_VOLUME),
    }
}

/// A gentle tap of vibration for a wrong answer; no sound, nothing to scold.
pub fn missed(ui: Ui) {
    if !ui.settings.peek().haptics_off {
        crate::platform::vibrate(Haptic::Missed);
    }
}

/// Settings: let the learner hear a variation before (or while) choosing it.
pub fn preview_correct(choice: CorrectSound) {
    eval_play(choice.id(), CORRECT_VOLUME);
}

pub fn preview_end(choice: EndSound) {
    eval_play(choice.id(), END_VOLUME);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_has_a_wav() {
        let ids = CorrectSound::ALL.iter().map(|c| c.id()).chain(EndSound::ALL.iter().map(|e| e.id()));
        for id in ids {
            assert!(url(id).is_some_and(|u| u.starts_with("data:audio/wav;base64,UklGR")), "{id}");
        }
        assert_eq!(URLS.len(), CorrectSound::ALL.len() + EndSound::ALL.len());
    }
}
