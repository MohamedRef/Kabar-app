//! Sinetron episodes and Kabar Radio as playable programs (PLAN §16 B): who
//! speaks with which voice, which episodes are open, and which line is on
//! air at a point in a joined track.

use kabar_protocol::cloud::Voice;
use kabar_protocol::lesson::{LessonContent, Localized};
use kabar_protocol::story::{NARRATOR, Persona, RadioEpisode, SegmentKind, Story, voice_named};

use crate::journey::{Curriculum, ParseError, Progress};

pub fn parse_story(yaml: &str) -> Result<Story, ParseError> {
    serde_yaml_ng::from_str(yaml).map_err(|e| ParseError(e.to_string()))
}

pub fn parse_radio(yaml: &str) -> Result<RadioEpisode, ParseError> {
    serde_yaml_ng::from_str(yaml).map_err(|e| ParseError(e.to_string()))
}

pub fn parse_persona(yaml: &str) -> Result<Persona, ParseError> {
    serde_yaml_ng::from_str(yaml).map_err(|e| ParseError(e.to_string()))
}

// --- voices ------------------------------------------------------------------

/// Azure has two neural voices per language, so the Sinetron cast shares
/// them and pitch tells the characters apart: (speaker, male, pitch %).
const SINETRON_CAST: &[(&str, bool, i8)] = &[
    (NARRATOR, true, 0),
    ("Pak Budi", true, -12),
    ("Mas Joko", true, 4),
    ("Bayu", true, 12),
    ("Karim", true, -5),
    ("Petugas", true, -8),
    ("Pak RT", true, -6),
    ("Pak Hendra", true, -2),
    ("Pak Mahmoud", true, -15),
    // Javanese season (e71+).
    ("Mas Heru", true, 8),
    ("Mas Yudha", true, 14),
    ("Mas Dimas", true, 2),
    ("Pak Harsono", true, -10),
    ("Pak Slamet", true, -13),
    ("Pak Harjo", true, -18),
    ("Sari", false, 0),
    ("Bu Rahayu", false, -10),
    ("Nadia", false, 10),
    ("Hagga Fatma", false, -18),
    ("Dokter Wulan", false, -5),
    ("Bu Sri", false, -14),
    ("Bu Tini", false, -3),
    ("Bu Ani", false, -8),
    ("Mbak Dewi", false, 6),
    ("Bu Samia", false, -12),
    ("Bu Darmi", false, -4),
    ("Bu Siti", false, 2),
    ("Bu Harjo", false, -9),
    ("Bu Harsono", false, -6),
    ("Mbak Ayu", false, 8),
    ("Mbah Putri", false, -16),
];

/// A speaker's voice and pitch in a Sinetron episode. A character this
/// build doesn't know yet is guessed from the honorific, at natural pitch.
pub fn sinetron_voice(speaker: &str, language: &str) -> (Voice, i8) {
    let (male, pitch) = SINETRON_CAST.iter().find(|(name, ..)| *name == speaker).map_or_else(
        || {
            let first = speaker.split_whitespace().next().unwrap_or_default();
            (["Pak", "Bapak", "Mas", "Om", "Kang", "Kakek", "Mbah"].contains(&first), 0)
        },
        |&(_, male, pitch)| (male, pitch),
    );
    (gendered(language, male), pitch)
}

fn gendered(language: &str, male: bool) -> Voice {
    match (language, male) {
        ("jv", true) => Voice::JvDimas,
        ("jv", false) => Voice::JvSiti,
        (_, true) => Voice::IdArdi,
        (_, false) => Voice::IdGadis,
    }
}

// --- programs ------------------------------------------------------------------

/// What heads a group of lines: a Sinetron scene or a Radio segment.
#[derive(Clone, Debug, PartialEq)]
pub enum Heading {
    Scene(Localized),
    Segment { kind: SegmentKind, title: Option<String> },
}

/// One line on air.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    /// Index into [`Program::groups`].
    pub group: usize,
    pub speaker: String,
    pub text: String,
    pub meaning: Localized,
    pub note: Option<Localized>,
    pub voice: Voice,
    pub pitch_percent: i8,
}

/// An episode flattened into the lines to play, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub id: String,
    pub language: String,
    pub groups: Vec<Heading>,
    pub cues: Vec<Cue>,
}

impl Program {
    pub fn sinetron(story: &Story) -> Self {
        let mut p =
            Self { id: story.id.clone(), language: story.language.clone(), groups: vec![], cues: vec![] };
        for scene in &story.scenes {
            let group = p.groups.len();
            p.groups.push(Heading::Scene(scene.setting.clone()));
            for line in &scene.lines {
                let (voice, pitch_percent) = sinetron_voice(&line.speaker, &story.language);
                p.cues.push(Cue {
                    group,
                    speaker: line.speaker.clone(),
                    text: line.text.clone(),
                    meaning: line.meaning.clone(),
                    note: line.note.clone(),
                    voice,
                    pitch_percent,
                });
            }
        }
        p
    }

    /// Hosts speak in the voice their file names; anyone else (a guest)
    /// falls back to the Sinetron casting.
    pub fn radio(ep: &RadioEpisode) -> Self {
        let mut p = Self { id: ep.id.clone(), language: ep.language.clone(), groups: vec![], cues: vec![] };
        for segment in &ep.segments {
            let group = p.groups.len();
            p.groups.push(Heading::Segment { kind: segment.kind, title: segment.title.clone() });
            for line in &segment.lines {
                let host = ep.hosts.iter().find(|h| h.name == line.speaker);
                let (voice, pitch_percent) = match host.and_then(|h| voice_named(&h.voice)) {
                    Some(v) => (v, 0),
                    None => sinetron_voice(&line.speaker, &ep.language),
                };
                p.cues.push(Cue {
                    group,
                    speaker: line.speaker.clone(),
                    text: line.text.clone(),
                    meaning: line.meaning.clone(),
                    note: line.note.clone(),
                    voice,
                    pitch_percent,
                });
            }
        }
        p
    }

    /// Characters of speech, the unit Azure F0's monthly quota counts.
    pub fn chars(&self) -> usize {
        self.cues.iter().map(|c| c.text.trim().chars().count()).sum()
    }
}

/// The cue on air at `t` seconds into a joined track whose cues start at
/// `starts` (ascending).
pub fn cue_at(starts: &[f64], t: f64) -> usize {
    starts.partition_point(|&s| s <= t).saturating_sub(1)
}

/// Seconds of a constant-bitrate MP3 of `bytes` at `kbps`.
pub fn mp3_seconds(bytes: u64, kbps: u32) -> f64 {
    bytes as f64 * 8.0 / (f64::from(kbps) * 1000.0)
}

/// The episode's questions and new words as a lesson the Lesson Player can
/// run (no slides, no dialogue): same exercises, same grading.
pub fn quiz(story: &Story) -> LessonContent {
    LessonContent {
        schema: kabar_protocol::lesson::LESSON_SCHEMA.into(),
        id: story.id.clone(),
        language: story.language.clone(),
        title: story.title.target.clone(),
        can_do: story.summary.clone(),
        slides: vec![],
        exercises: story.questions.clone(),
        dialogue: None,
        new_words: story.new_words.clone(),
        souvenir: None,
        game: None,
        review: story.review.clone(),
    }
}

// --- unlocks -------------------------------------------------------------------

/// A Sinetron episode opens once its unit is finished (spec §14). Units of
/// levels before the one placement started the learner at count as known.
pub fn unlocked(
    curriculum: &Curriculum,
    progress: &Progress,
    start_level: Option<&str>,
    unit_id: &str,
) -> bool {
    let Some((level_at, unit)) = curriculum
        .levels
        .iter()
        .enumerate()
        .find_map(|(i, l)| l.units.iter().find(|u| u.id == unit_id).map(|u| (i, u)))
    else {
        return false;
    };
    let start_at = start_level.and_then(|s| curriculum.levels.iter().position(|l| l.id == s));
    progress.unit_done(unit)
        || progress.level_done(&curriculum.levels[level_at].id)
        || start_at.is_some_and(|s| level_at < s)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::journey::parse_curriculum;

    const E01: &str = include_str!("../../../content/stories/sinetron/sinetron.e01.yaml");
    const RADIO: &str = include_str!("../../../content/radio/id/radio.bipa1.u01.yaml");
    const DIMAS: &str = include_str!("../../../content/personas/mas-dimas.yaml");
    const CURRICULUM: &str = include_str!("../../../curriculum/indonesian.yaml");

    #[test]
    fn every_speaker_gets_a_voice_in_the_episode_language() {
        let story = parse_story(E01).unwrap();
        let p = Program::sinetron(&story);
        assert_eq!(p.groups.len(), story.scenes.len());
        assert_eq!(p.cues.len(), story.scenes.iter().map(|s| s.lines.len()).sum::<usize>());
        let budi = p.cues.iter().find(|c| c.speaker == "Pak Budi").unwrap();
        assert_eq!((budi.voice, budi.pitch_percent), (Voice::IdArdi, -12));
        let sari = p.cues.iter().find(|c| c.speaker == "Sari").unwrap();
        assert_eq!((sari.voice, sari.pitch_percent), (Voice::IdGadis, 0));
        assert_eq!(sinetron_voice("Pak Harto", "jv"), (Voice::JvDimas, 0));
        assert_eq!(sinetron_voice("Bu Wati", "id"), (Voice::IdGadis, 0));
        assert!(SINETRON_CAST.iter().all(|(_, _, pitch)| (-20..=20).contains(pitch)));
    }

    #[test]
    fn cast_members_sharing_a_voice_sound_different() {
        for male in [true, false] {
            let mut pitches: Vec<i8> =
                SINETRON_CAST.iter().filter(|(_, m, _)| *m == male).map(|(.., p)| *p).collect();
            pitches.sort();
            assert!(pitches.windows(2).all(|w| w[0] != w[1]), "{pitches:?}");
        }
    }

    #[test]
    fn radio_hosts_use_their_own_voices() {
        let ep = parse_radio(RADIO).unwrap();
        let p = Program::radio(&ep);
        assert_eq!(
            p.groups.first(),
            Some(&Heading::Segment { kind: SegmentKind::Intro, title: Some("Selamat pagi!".into()) })
        );
        assert!(p.cues.iter().all(|c| c.pitch_percent == 0));
        assert!(p.cues.iter().any(|c| c.voice == Voice::IdArdi));
        assert!(p.cues.iter().any(|c| c.voice == Voice::IdGadis));
        // A 5-minute episode fits Azure F0 about a hundred times a month.
        assert!((2_000..=8_000).contains(&p.chars()), "{}", p.chars());
    }

    #[test]
    fn personas_parse() {
        let p = parse_persona(DIMAS).unwrap();
        assert_eq!((p.id.as_str(), p.language.as_str()), ("mas-dimas", "jv"));
        assert!(p.example_dialogues[0].lines.iter().any(|l| l.correction.is_some()));
        assert!(!p.style.dos.is_empty());
    }

    #[test]
    fn the_cue_on_air_follows_the_clock() {
        let starts = [0.0, 1.5, 4.0];
        assert_eq!(cue_at(&starts, 0.0), 0);
        assert_eq!(cue_at(&starts, 1.49), 0);
        assert_eq!(cue_at(&starts, 1.5), 1);
        assert_eq!(cue_at(&starts, 99.0), 2);
        assert_eq!(cue_at(&[], 3.0), 0);
        assert!((mp3_seconds(6_000, 48) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_quiz_is_the_episode_questions() {
        let story = parse_story(E01).unwrap();
        let q = quiz(&story);
        assert!(q.slides.is_empty() && q.dialogue.is_none());
        assert_eq!(q.exercises.len(), story.questions.len());
        let player = crate::lesson::Player::new(q);
        assert!(matches!(player.step(), crate::lesson::Step::Exercise { index: 0, retry: false }));
    }

    #[test]
    fn episodes_open_after_their_unit() {
        let c = parse_curriculum(CURRICULUM).unwrap();
        let u01 = &c.levels[0].units[0];
        let none = Progress::new(&c, &BTreeSet::new());
        assert!(!unlocked(&c, &none, None, &u01.id));
        let done: BTreeSet<String> = u01.lessons.iter().map(|l| l.id.clone()).collect();
        assert!(unlocked(&c, &Progress::new(&c, &done), None, &u01.id));
        assert!(!unlocked(&c, &Progress::new(&c, &done), None, &c.levels[0].units[1].id));
        // Placed at BIPA 2: BIPA 1's episodes are open.
        assert!(unlocked(&c, &none, Some(&c.levels[1].id), &u01.id));
        assert!(!unlocked(&c, &none, Some(&c.levels[1].id), &c.levels[1].units[0].id));
        assert!(!unlocked(&c, &none, None, "nope.u01"));
    }
}
