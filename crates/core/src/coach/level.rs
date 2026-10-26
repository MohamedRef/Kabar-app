//! The monthly level check's decision (spec §6): up, down or stay, one level
//! at a time. This is the deterministic form of the `level_check` golden
//! set's rule (`content/golden/level_check.yaml`), so the decision never
//! depends on a model; AI only writes the evidence and next focus.
//!
//! Moving down was the learner's call (2026-10-26). It needs clear, sustained
//! evidence and is framed as consolidating the base, never as a loss: words,
//! reviews, Pelita, Koin and achievements are untouched.

use super::voice::Line;

/// A strong miss on the current level's test.
pub const DOWN_TEST_BELOW: f32 = 0.45;
/// Sustained struggle: review accuracy and retention both this low...
pub const DOWN_ACCURACY_BELOW: f32 = 0.55;
pub const DOWN_RETENTION_BELOW: f32 = 0.60;
/// ...over at least this many active days.
pub const MIN_ACTIVE_DAYS: u32 = 14;

/// The current level's test result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelTest {
    pub score: f32,
    /// From the curriculum: 0.75 for bipa1–2 and j1–j4, 0.70 above.
    pub pass_mark: f32,
}

/// What the check looks at. `None` stats never trigger a move.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CheckStats {
    pub level_test: Option<LevelTest>,
    pub review_accuracy: Option<f32>,
    pub retention: Option<f32>,
    pub active_days: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moved {
    Up,
    Down,
    Stay,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub level: String,
    pub moved: Moved,
}

impl Decision {
    /// What Kak Nur says about it; `None` for staying (the forecast line covers that).
    pub fn line(&self) -> Option<Line> {
        match self.moved {
            Moved::Up => Some(Line::LevelUp),
            Moved::Down => Some(Line::LevelRebuild),
            Moved::Stay => None,
        }
    }
}

const BIPA: &[&str] = &["bipa1", "bipa2", "bipa3", "bipa4", "bipa5", "bipa6", "bipa7"];
const JAVA: &[&str] = &["j1", "j2", "j3", "j4", "j5", "j6"];

/// The rule, in the golden set's order: up on a passed test; down on a clear
/// miss or sustained struggle; otherwise stay. Never past either end of the
/// track, never more than one level. An unknown level id stays.
pub fn decide(level: &str, stats: &CheckStats) -> Decision {
    let stay = Decision { level: level.to_string(), moved: Moved::Stay };
    let Some((track, i)) =
        [BIPA, JAVA].into_iter().find_map(|t| t.iter().position(|l| *l == level).map(|i| (t, i)))
    else {
        return stay;
    };

    if let Some(test) = stats.level_test
        && test.score >= test.pass_mark
    {
        return match track.get(i + 1) {
            Some(next) => Decision { level: next.to_string(), moved: Moved::Up },
            None => stay,
        };
    }

    let missed_test = stats.level_test.is_some_and(|t| t.score < DOWN_TEST_BELOW);
    let struggling = stats.active_days >= MIN_ACTIVE_DAYS
        && stats.review_accuracy.is_some_and(|a| a < DOWN_ACCURACY_BELOW)
        && stats.retention.is_some_and(|r| r < DOWN_RETENTION_BELOW);
    if (missed_test || struggling) && i > 0 {
        return Decision { level: track[i - 1].to_string(), moved: Moved::Down };
    }
    stay
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(score: f32, pass_mark: f32) -> Option<LevelTest> {
        Some(LevelTest { score, pass_mark })
    }

    fn d(level: &str, moved: Moved) -> Decision {
        Decision { level: level.into(), moved }
    }

    #[test]
    fn a_passed_test_moves_up_one_level() {
        let s = CheckStats { level_test: test(0.75, 0.75), ..Default::default() };
        assert_eq!(decide("bipa1", &s), d("bipa2", Moved::Up));
        assert_eq!(decide("j3", &s), d("j4", Moved::Up));
        assert_eq!(decide("bipa1", &s).line(), Some(Line::LevelUp));
        assert_eq!(decide("bipa7", &s), d("bipa7", Moved::Stay));
        assert_eq!(decide("j6", &s), d("j6", Moved::Stay));
    }

    #[test]
    fn a_clear_miss_moves_down_one_level() {
        let s = CheckStats { level_test: test(0.40, 0.70), ..Default::default() };
        assert_eq!(decide("bipa3", &s), d("bipa2", Moved::Down));
        assert_eq!(decide("bipa3", &s).line(), Some(Line::LevelRebuild));
        assert_eq!(decide("bipa1", &s), d("bipa1", Moved::Stay));
        assert_eq!(decide("j1", &s), d("j1", Moved::Stay));
    }

    #[test]
    fn sustained_struggle_moves_down_but_a_short_dip_does_not() {
        let s = CheckStats {
            review_accuracy: Some(0.50),
            retention: Some(0.55),
            active_days: 14,
            ..Default::default()
        };
        assert_eq!(decide("bipa2", &s), d("bipa1", Moved::Down));
        let comeback = CheckStats { active_days: 9, ..s };
        assert_eq!(decide("bipa2", &comeback), d("bipa2", Moved::Stay));
        let one_low = CheckStats { retention: Some(0.70), ..s };
        assert_eq!(decide("bipa2", &one_low), d("bipa2", Moved::Stay));
        let no_stats = CheckStats { active_days: 30, ..Default::default() };
        assert_eq!(decide("bipa2", &no_stats), d("bipa2", Moved::Stay));
    }

    #[test]
    fn a_near_miss_or_unknown_level_stays() {
        let near = CheckStats { level_test: test(0.70, 0.75), ..Default::default() };
        assert_eq!(decide("bipa2", &near), d("bipa2", Moved::Stay));
        assert_eq!(decide("bipa2", &near).line(), None);
        let miss = CheckStats { level_test: test(0.10, 0.75), ..Default::default() };
        assert_eq!(decide("bipa-2", &miss), d("bipa-2", Moved::Stay));
    }
}
