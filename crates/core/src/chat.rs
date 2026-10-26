//! Persona chats (SPEC §15, PLAN §16 C): the scenes a persona offers, the
//! level band and style sheet a `roleplay_turn` request carries, and a chat
//! as a [`Program`] for "replay as dialogue".

use kabar_protocol::chat::{ChatMessage, ChatRole};
use kabar_protocol::cloud::Voice;
use kabar_protocol::lesson::Localized;
use kabar_protocol::story::{LevelBand, Persona, voice_named};
use serde_json::{Value, json};

use crate::ai::AiRequest;
use crate::learn::text::scrub;
use crate::story::{Cue, Heading, Program};

/// Turns of history sent with a request: enough for the scene, small enough
/// for a free engine's context.
pub const MAX_HISTORY: usize = 12;
/// The `speaker` of learner lines in the personas' example dialogues.
pub const LEARNER: &str = "learner";

/// A scene to start a chat in: one of the persona's example dialogues, or a
/// free chat opened with one of their openers.
#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    /// The example dialogue's id (`d01`); `None` for a free chat.
    pub id: Option<String>,
    pub level: String,
    pub situation: Localized,
    /// The persona's first line.
    pub opener: String,
    pub opener_meaning: Localized,
}

/// The persona's scenes, easiest first: each example dialogue, then a free
/// chat per opener level.
pub fn scenarios(persona: &Persona) -> Vec<Scenario> {
    let mut out: Vec<Scenario> = persona
        .example_dialogues
        .iter()
        .filter_map(|d| {
            let first = d.lines.iter().find(|l| l.speaker != LEARNER)?;
            Some(Scenario {
                id: Some(d.id.clone()),
                level: d.level.clone(),
                situation: d.situation.clone(),
                opener: first.text.clone(),
                opener_meaning: first.meaning.clone(),
            })
        })
        .collect();
    let mut levels: Vec<&str> = persona.openers.iter().filter_map(|o| o.level.as_deref()).collect();
    levels.sort_by_key(|l| level_key(l));
    levels.dedup();
    for level in levels {
        if let Some(o) = persona.openers.iter().find(|o| o.level.as_deref() == Some(level)) {
            out.push(Scenario {
                id: None,
                level: level.to_string(),
                situation: persona.role.clone(),
                opener: o.text.clone(),
                opener_meaning: o.meaning.clone(),
            });
        }
    }
    out.sort_by_key(|s| (level_key(&s.level), s.id.is_none()));
    out
}

/// `bipa1`…`bipa7` sort before `j1`…`j6`; anything else last.
fn level_key(level: &str) -> (u8, u8) {
    let l = level.trim().to_lowercase();
    let num = |s: &str| s.parse::<u8>().ok();
    if let Some(n) = l.strip_prefix("bipa").and_then(num) {
        (0, n)
    } else if let Some(n) = l.strip_prefix('j').and_then(num) {
        (1, n)
    } else {
        (2, 0)
    }
}

/// The band of the persona's style guide for a learner at `level`: the
/// highest band at or below it in the same journey, else the lowest of that
/// journey, else the persona's first band.
pub fn level_band<'a>(persona: &'a Persona, level: &str) -> Option<&'a LevelBand> {
    let (family, n) = level_key(level);
    let bands = &persona.style.levels;
    let same = || bands.iter().filter(move |b| level_key(&b.level).0 == family);
    same()
        .filter(|b| level_key(&b.level).1 <= n)
        .max_by_key(|b| level_key(&b.level))
        .or_else(|| same().min_by_key(|b| level_key(&b.level)))
        .or_else(|| bands.first())
}

/// The level a chat with `persona` is pitched at. Javanese personas use the
/// Javanese level once the learner has started that journey, else the
/// Indonesian one (they then speak Indonesian with a Javanese word a turn).
pub fn chat_level(persona: &Persona, indonesian: &str, javanese: Option<&str>) -> String {
    match javanese {
        Some(j) if is_javanese(persona) => j.to_string(),
        _ => indonesian.to_string(),
    }
}

pub fn is_javanese(persona: &Persona) -> bool {
    persona.language == "jv"
}

/// What the register switch offers for this persona, its default first.
pub fn registers(persona: &Persona) -> [&'static str; 2] {
    match (is_javanese(persona), persona.id.as_str()) {
        // A market seller is a stranger: polite until the learner relaxes.
        (true, "bu-siti") => ["krama", "ngoko"],
        (true, _) => ["ngoko", "krama"],
        (false, "mbak-gadis") => ["casual", "standard"],
        (false, _) => ["standard", "casual"],
    }
}

/// The persona's voice (`content/personas/*.yaml` names it).
pub fn persona_voice(persona: &Persona) -> Voice {
    voice_named(&persona.voice).unwrap_or(if is_javanese(persona) { Voice::JvDimas } else { Voice::IdArdi })
}

/// The learner's lines in a replay use the language's other voice.
pub fn learner_voice(persona: &Persona) -> Voice {
    match persona_voice(persona) {
        Voice::JvDimas => Voice::JvSiti,
        Voice::JvSiti => Voice::JvDimas,
        Voice::IdArdi => Voice::IdGadis,
        _ => Voice::IdArdi,
    }
}

/// The style sheet the prompt reads, for this level.
fn guide(persona: &Persona, level: &str) -> Value {
    let band = level_band(persona, level);
    json!({
        "who": format!("{} ({}): {} {}", persona.name, persona.home, persona.role.en, persona.personality.en),
        "register": persona.style.register.en,
        "do": persona.style.dos,
        "dont": persona.style.dont,
        "corrections": persona.style.corrections.en,
        "level_guide": band.map(|b| b.guide.as_str()),
        "vocabulary": band.and_then(|b| b.vocabulary.as_deref()),
        "topics": persona.topics,
    })
}

/// One example dialogue at or near `level` (the scene's own when there is
/// one), as `{speaker, text}` lines.
fn example(persona: &Persona, level: &str, scenario: Option<&str>) -> Value {
    let target = level_key(level);
    let d = scenario.and_then(|id| persona.example_dialogues.iter().find(|d| d.id == id)).or_else(|| {
        persona.example_dialogues.iter().min_by_key(|d| {
            let k = level_key(&d.level);
            (k.0 != target.0, k.1.abs_diff(target.1))
        })
    });
    match d {
        Some(d) => d.lines.iter().map(|l| json!({ "speaker": l.speaker, "text": l.text })).collect(),
        None => Value::Array(vec![]),
    }
}

/// The `roleplay_turn` request for the learner's newest line. What the
/// learner types is learning text (PLAN §16 C), so it may reach
/// `gemini_free`, but e-mail addresses and phone numbers are scrubbed first.
pub fn request(
    persona: &Persona,
    level: &str,
    scenario: &Scenario,
    register: &str,
    history: &[ChatMessage],
    learner: &str,
) -> AiRequest {
    let start = history.len().saturating_sub(MAX_HISTORY);
    let history: Vec<Value> = history[start..]
        .iter()
        .map(|m| {
            let role = match m.role {
                ChatRole::Persona => "persona",
                ChatRole::Learner => "learner",
            };
            json!({ "role": role, "text": scrub(&m.text) })
        })
        .collect();
    AiRequest::new(json!({
        // The prompt and golden sets write persona ids with underscores.
        "persona": persona.id.replace('-', "_"),
        "level": level,
        "scenario": scenario.situation.en,
        "register_hint": register,
        "guide": guide(persona, level),
        "example": example(persona, level, scenario.id.as_deref()),
        "history": history,
        "learner": scrub(learner),
    }))
}

/// A chat as a dialogue to play back: the persona in their voice, the
/// learner's lines (as corrected, when the persona fixed them) in the other,
/// labelled `you`.
pub fn replay(
    persona: &Persona,
    conversation_id: &str,
    situation: &Localized,
    messages: &[ChatMessage],
    you: &str,
) -> Program {
    let (pv, lv) = (persona_voice(persona), learner_voice(persona));
    let cues = messages
        .iter()
        .map(|m| {
            let (speaker, text, voice) = match m.role {
                ChatRole::Persona => (persona.name.clone(), m.text.clone(), pv),
                ChatRole::Learner => {
                    let mut text = m.text.clone();
                    for c in &m.corrections {
                        text = text.replacen(&c.text, &c.fix, 1);
                    }
                    (you.to_string(), text, lv)
                }
            };
            Cue {
                group: 0,
                speaker,
                text,
                meaning: Localized::default(),
                note: None,
                voice,
                pitch_percent: 0,
            }
        })
        .collect();
    Program {
        id: format!("chat.{conversation_id}"),
        language: persona.language.clone(),
        groups: vec![Heading::Scene(situation.clone())],
        cues,
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::chat::ChatCorrection;

    use super::*;
    use crate::ai::AiOutput;
    use crate::ai::tasks::RoleplayTurn;
    use crate::story::parse_persona;

    const DIMAS: &str = include_str!("../../../content/personas/mas-dimas.yaml");
    const SITI: &str = include_str!("../../../content/personas/bu-siti.yaml");
    const GADIS: &str = include_str!("../../../content/personas/mbak-gadis.yaml");
    const ARDI: &str = include_str!("../../../content/personas/mas-ardi.yaml");

    fn all() -> Vec<Persona> {
        [DIMAS, SITI, GADIS, ARDI].iter().map(|y| parse_persona(y).unwrap()).collect()
    }

    fn msg(role: ChatRole, text: &str) -> ChatMessage {
        ChatMessage {
            id: text.into(),
            doc: "c1".into(),
            role,
            text: text.into(),
            corrections: vec![],
            register: None,
            unverified: false,
            created_ms: 0,
        }
    }

    #[test]
    fn every_persona_offers_scenes_with_an_opener() {
        for p in all() {
            let s = scenarios(&p);
            assert!(s.len() >= 2, "{}", p.id);
            assert!(s.iter().all(|s| !s.opener.trim().is_empty() && !s.situation.en.is_empty()), "{}", p.id);
            let keys: Vec<_> = s.iter().map(|s| level_key(&s.level)).collect();
            assert!(keys.windows(2).all(|w| w[0] <= w[1]), "{}: easiest first", p.id);
            assert!(s.iter().any(|s| s.id.is_none()), "{}: a free chat", p.id);
        }
    }

    #[test]
    fn the_level_band_is_the_nearest_one_below() {
        let dimas = parse_persona(DIMAS).unwrap();
        let band = |l: &str| level_band(&dimas, l).map(|b| b.level.as_str());
        assert_eq!(band("j1"), Some("j1"));
        assert_eq!(band("j5"), Some("j3"));
        assert_eq!(band("bipa1"), Some("bipa2"), "below every bipa band: the lowest");
        assert_eq!(band("bipa6"), Some("bipa2"));
        let ardi = parse_persona(ARDI).unwrap();
        assert_eq!(level_band(&ardi, "bipa7").map(|b| b.level.as_str()), Some("bipa4"));
        assert_eq!(level_band(&ardi, "bipa1").map(|b| b.level.as_str()), Some("bipa1"));
    }

    #[test]
    fn javanese_personas_wait_for_the_javanese_journey() {
        let dimas = parse_persona(DIMAS).unwrap();
        let ardi = parse_persona(ARDI).unwrap();
        assert_eq!(chat_level(&dimas, "bipa2", None), "bipa2");
        assert_eq!(chat_level(&dimas, "bipa2", Some("j2")), "j2");
        assert_eq!(chat_level(&ardi, "bipa3", Some("j2")), "bipa3");
    }

    #[test]
    fn voices_and_registers_follow_the_persona() {
        for p in all() {
            let (pv, lv) = (persona_voice(&p), learner_voice(&p));
            assert_ne!(pv, lv, "{}", p.id);
            assert_eq!(voice_named(&p.voice), Some(pv), "{}", p.id);
            let r = registers(&p);
            if is_javanese(&p) {
                assert!(r.contains(&"ngoko") && r.contains(&"krama"));
            } else {
                assert!(r.contains(&"casual") && r.contains(&"standard"));
            }
        }
    }

    #[test]
    fn the_request_carries_the_guide_and_scrubs_the_learner() {
        let dimas = parse_persona(DIMAS).unwrap();
        let scene = scenarios(&dimas).into_iter().find(|s| s.id.as_deref() == Some("d01")).unwrap();
        let mut history: Vec<ChatMessage> =
            (0..20).map(|i| msg(ChatRole::Persona, &format!("Baris {i}"))).collect();
        history.push(msg(ChatRole::Learner, "Nomerku 0812-3456-7890"));
        let req = request(&dimas, "j1", &scene, "ngoko", &history, "Aku wis mangan. Mailku a@b.co");
        let i = &req.input;
        assert!(!req.personal, "roleplay text is learning text");
        assert_eq!(i["persona"], "mas_dimas");
        assert_eq!(i["level"], "j1");
        assert_eq!(i["register_hint"], "ngoko");
        assert_eq!(i["guide"]["level_guide"], level_band(&dimas, "j1").unwrap().guide.as_str());
        assert!(i["guide"]["do"].as_array().is_some_and(|d| !d.is_empty()));
        assert_eq!(i["example"][0]["text"], scene.opener.as_str());
        let h = i["history"].as_array().unwrap();
        assert_eq!(h.len(), MAX_HISTORY);
        assert!(!h.last().unwrap()["text"].as_str().unwrap().contains("3456"));
        assert!(!i["learner"].as_str().unwrap().contains("a@b.co"));
        assert!(RoleplayTurn::input_needs_review(i), "Javanese stays unverified");
    }

    #[test]
    fn replay_uses_two_voices_and_the_corrected_line() {
        let dimas = parse_persona(DIMAS).unwrap();
        let mut learner = msg(ChatRole::Learner, "Aku lapar banget.");
        learner.corrections.push(ChatCorrection {
            text: "lapar".into(),
            fix: "luwe".into(),
            why_en: "ngoko".into(),
            why_ar: "نغوكو".into(),
        });
        let p = replay(&dimas, "c1", &dimas.role, &[msg(ChatRole::Persona, "Wis mangan?"), learner], "You");
        assert_eq!(p.id, "chat.c1");
        assert_eq!(p.cues[0].voice, Voice::JvDimas);
        assert_eq!(p.cues[1].voice, Voice::JvSiti);
        assert_eq!(p.cues[1].text, "Aku luwe banget.");
        assert_eq!(p.cues[1].speaker, "You");
    }

    #[test]
    fn answers_are_checked_against_the_learner_and_persona() {
        let turn = |register: &str, quote: &str| -> RoleplayTurn {
            serde_json::from_value(json!({
                "reply": "Wah, durung mangan to?",
                "corrections": [{ "text": quote, "fix": "durung mangan", "why_en": "Javanese", "why_ar": "بالجاوي" }],
                "register": register,
            }))
            .unwrap()
        };
        let input = json!({ "persona": "mas_dimas", "level": "j1", "learner": "Aku  Belum makan, lapar." });
        assert_eq!(turn("ngoko", "belum makan").check(&input), Ok(()));
        assert!(turn("ngoko", "belum mangan").check(&input).unwrap_err().contains("copied"));
        assert!(turn("standard", "belum makan").check(&input).unwrap_err().contains("ngoko"));
        let bipa = json!({ "persona": "mas_dimas", "level": "bipa2", "learner": "belum makan" });
        assert_eq!(
            turn("standard", "belum makan").check(&bipa),
            Ok(()),
            "Indonesian before the Javanese journey"
        );
        let gadis = json!({ "persona": "mbak_gadis", "level": "bipa3", "learner": "belum makan" });
        assert!(turn("ngoko", "belum makan").check(&gadis).is_err());
        assert_eq!(turn("casual", "belum makan").check(&gadis), Ok(()));
    }
}
