//! Versioned prompt templates (`prompts/<task>@v<N>.md`) and the learner
//! profile they carry.
//!
//! A template has a system part and a user part, split by the marker lines
//! `<!-- system -->` and `<!-- user -->`. Placeholders: `{{profile}}` and
//! `{{input}}` (pretty JSON of the request input).

use std::collections::BTreeMap;

use kabar_protocol::ai::{EngineId, Task};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SYSTEM_MARK: &str = "<!-- system -->";
const USER_MARK: &str = "<!-- user -->";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptTemplate {
    pub task: Task,
    pub version: u32,
    system: String,
    user: String,
}

impl PromptTemplate {
    /// `file_name` is `analyze_text@v1.md`.
    pub fn parse(file_name: &str, text: &str) -> Result<Self, String> {
        let stem = file_name.strip_suffix(".md").ok_or("prompt files end in .md")?;
        let (task, version) = stem.split_once("@v").ok_or("prompt files are named <task>@v<N>.md")?;
        let task = Task::parse(task).ok_or_else(|| format!("unknown task `{task}`"))?;
        let version: u32 = version.parse().map_err(|_| format!("bad version in `{file_name}`"))?;

        let s = text.find(SYSTEM_MARK).ok_or_else(|| format!("`{file_name}` has no {SYSTEM_MARK}"))?;
        let u = text.find(USER_MARK).ok_or_else(|| format!("`{file_name}` has no {USER_MARK}"))?;
        if u < s {
            return Err(format!("`{file_name}`: system part must come first"));
        }
        let system = text[s + SYSTEM_MARK.len()..u].trim().to_string();
        let user = text[u + USER_MARK.len()..].trim().to_string();
        if !user.contains("{{input}}") {
            return Err(format!("`{file_name}`: user part must contain {{{{input}}}}"));
        }
        Ok(Self { task, version, system, user })
    }

    /// (system, user) for one engine. The profile is trimmed for engines that
    /// train on inputs.
    pub fn render(&self, profile: &LearnerProfile, input: &Value, engine: EngineId) -> (String, String) {
        let profile = profile.render(engine);
        let input = serde_json::to_string_pretty(input).unwrap_or_default();
        let fill = |s: &str| s.replace("{{profile}}", &profile).replace("{{input}}", &input);
        (fill(&self.system), fill(&self.user))
    }
}

/// All bundled templates; `latest` picks the highest version per task.
#[derive(Clone, Debug, Default)]
pub struct PromptSet {
    templates: BTreeMap<(Task, u32), PromptTemplate>,
}

impl PromptSet {
    pub fn from_files<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<Self, String> {
        let mut set = Self::default();
        for (name, text) in files {
            let t = PromptTemplate::parse(name, text)?;
            set.templates.insert((t.task, t.version), t);
        }
        Ok(set)
    }

    /// The prompts compiled into the app.
    pub fn builtin() -> Self {
        Self::from_files(BUILTIN.iter().copied()).expect("bundled prompts are valid")
    }

    pub fn get(&self, task: Task, version: Option<u32>) -> Option<&PromptTemplate> {
        match version {
            Some(v) => self.templates.get(&(task, v)),
            None => self.templates.range((task, 0)..=(task, u32::MAX)).next_back().map(|(_, t)| t),
        }
    }
}

macro_rules! bundled {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("../../../../prompts/", $name)))),*]
    };
}

const BUILTIN: &[(&str, &str)] = bundled![
    "analyze_text@v1.md",
    "explain_sentence@v1.md",
    "translate_text@v1.md",
    "grade_answer@v1.md",
    "roleplay_turn@v1.md",
    "roleplay_turn@v2.md",
    "learn_entry@v1.md",
    "structure_screenshot@v1.md",
    "read_handwriting@v1.md",
    "guess_sketch@v1.md",
    "check_strokes@v1.md",
    "mind_map@v1.md",
    "story_episode@v1.md",
    "radio_script@v1.md",
    "placement_test@v1.md",
    "level_check@v1.md",
    "daily_plan@v1.md",
    "daily_recap@v1.md",
    "weekly_report@v1.md",
    "coach_message@v1.md",
];

/// Who the prompts are written for (spec §19).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LearnerProfile {
    pub native_language: String,
    pub reads: String,
    /// e.g. "BIPA 2".
    pub indonesian_level: String,
    /// e.g. "J1", or "not started".
    pub javanese_level: String,
    pub known_words: u32,
    /// Personal: never sent to engines that train on inputs.
    pub goals: Vec<String>,
    /// From coach memory. Personal: never sent to engines that train on inputs.
    pub recent_struggles: Vec<String>,
}

impl Default for LearnerProfile {
    fn default() -> Self {
        Self {
            native_language: "Egyptian Arabic".into(),
            reads: "English".into(),
            indonesian_level: "BIPA 1".into(),
            javanese_level: "not started".into(),
            known_words: 0,
            goals: Vec::new(),
            recent_struggles: Vec::new(),
        }
    }
}

/// Known-word counts are rounded down to these steps, in the prompt and in
/// the cache key, so a cached answer survives the next few words learned.
const KNOWN_WORD_BANDS: [u32; 14] = [0, 25, 50, 100, 200, 300, 500, 750, 1000, 1500, 2000, 3000, 5000, 8000];

fn known_word_band(n: u32) -> u32 {
    KNOWN_WORD_BANDS.iter().copied().take_while(|&b| b <= n).last().unwrap_or(0)
}

impl LearnerProfile {
    pub fn render(&self, engine: EngineId) -> String {
        let mut out = format!(
            "- Native language: {}. Reads explanations in {}.\n- Indonesian level: {}. Javanese level: {}.\n- Known words: about {}.",
            self.native_language,
            self.reads,
            self.indonesian_level,
            self.javanese_level,
            known_word_band(self.known_words)
        );
        if !engine.trains_on_inputs() {
            if !self.goals.is_empty() {
                out.push_str(&format!("\n- Goals: {}.", self.goals.join("; ")));
            }
            if !self.recent_struggles.is_empty() {
                out.push_str(&format!("\n- Recent struggles: {}.", self.recent_struggles.join("; ")));
            }
        }
        out
    }

    /// The cache key's profile part: a short hash of what the answer is
    /// pitched at (languages, levels, known-word band). Goals and recent
    /// struggles only colour an answer, so they don't split the cache.
    pub fn cache_tag(&self) -> String {
        let key = format!(
            "{}|{}|{}|{}|{}",
            self.native_language,
            self.reads,
            self.indonesian_level,
            self.javanese_level,
            known_word_band(self.known_words)
        );
        super::json::input_hash(&Value::String(key))[..12].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builtin_prompts_parse_and_pick_latest() {
        let set = PromptSet::builtin();
        let t = set.get(Task::AnalyzeText, None).unwrap();
        assert_eq!((t.task, t.version), (Task::AnalyzeText, 1));
        assert!(set.get(Task::AnalyzeText, Some(9)).is_none());
        for task in Task::ALL {
            assert!(set.get(task, None).is_some(), "no bundled prompt for {task:?}");
        }

        let set = PromptSet::from_files([
            ("mind_map@v1.md", "<!-- system -->\nA\n<!-- user -->\n{{input}}"),
            ("mind_map@v3.md", "<!-- system -->\nC\n<!-- user -->\n{{input}}"),
            ("mind_map@v2.md", "<!-- system -->\nB\n<!-- user -->\n{{input}}"),
        ])
        .unwrap();
        assert_eq!(set.get(Task::MindMap, None).unwrap().version, 3);
    }

    #[test]
    fn bad_files_are_rejected() {
        assert!(PromptTemplate::parse("nope@v1.md", "").unwrap_err().contains("unknown task"));
        assert!(PromptTemplate::parse("mind_map.md", "").is_err());
        assert!(PromptTemplate::parse("mind_map@v1.md", "<!-- user -->\nx").is_err());
        assert!(
            PromptTemplate::parse("mind_map@v1.md", "<!-- system -->\nx\n<!-- user -->\nno input").is_err()
        );
    }

    #[test]
    fn personal_profile_stays_off_training_engines() {
        let p = LearnerProfile {
            goals: vec!["chat with in-laws in Surabaya".into()],
            recent_struggles: vec!["krama vs ngoko".into()],
            ..LearnerProfile::default()
        };
        let t = PromptSet::builtin().get(Task::AnalyzeText, None).unwrap().clone();
        let input = json!({"text": "Sek tak lapor pusat"});

        let (sys, user) = t.render(&p, &input, EngineId::WorkersAi);
        assert!(
            sys.contains("Egyptian Arabic") && sys.contains("Surabaya") && sys.contains("krama vs ngoko")
        );
        assert!(user.contains("Sek tak lapor pusat"));

        let (sys, _) = t.render(&p, &input, EngineId::GeminiFree);
        assert!(sys.contains("Egyptian Arabic"));
        assert!(!sys.contains("Surabaya") && !sys.contains("krama vs ngoko"));
    }

    #[test]
    fn the_cache_tag_follows_the_level_not_every_new_word() {
        let bipa1 = LearnerProfile { known_words: 120, ..LearnerProfile::default() };
        let more_words = LearnerProfile { known_words: 180, ..bipa1.clone() };
        let bipa4 = LearnerProfile { indonesian_level: "BIPA 4".into(), ..bipa1.clone() };
        let struggling = LearnerProfile { recent_struggles: vec!["meN- verbs".into()], ..bipa1.clone() };
        assert_eq!(bipa1.cache_tag().len(), 12);
        assert_eq!(bipa1.cache_tag(), more_words.cache_tag(), "120 and 180 are both 'about 100'");
        assert_eq!(bipa1.cache_tag(), struggling.cache_tag());
        assert_ne!(bipa1.cache_tag(), bipa4.cache_tag());
        let past_band = LearnerProfile { known_words: 200, ..bipa1.clone() };
        assert_ne!(bipa1.cache_tag(), past_band.cache_tag());
        assert!(bipa1.render(EngineId::WorkersAi).contains("Known words: about 100."));
    }
}
