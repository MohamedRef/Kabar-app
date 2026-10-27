//! The oral session itself, without UI: which items can be asked aloud, how
//! each one is phrased, and the order they come in (Again comes back later in
//! the same session; an examiner's follow-up comes straight after a card).

use std::collections::VecDeque;

use batna_core::content::{Body, Item, Lecture, OutlineEntry, Status};
use batna_core::review::Grade;

/// How many items later an Again comes back.
pub const AGAIN_GAP: usize = 3;
/// At most one essay follow-up per this many cards.
const FOLLOW_UP_EVERY: u32 = 3;

/// One question as the examiner asks it.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    pub item_id: String,
    pub kind: &'static str,
    pub lecture: String,
    pub topic: Option<String>,
    /// Printed book page, to check the answer against.
    pub page: u32,
    /// What is read aloud and shown, in English. A case has its stem first.
    pub question: Vec<String>,
    /// The model answer, one line per point or heading.
    pub answer: Vec<String>,
    /// "Enumerate": how many points count as a pass, of how many.
    pub pass: Option<(usize, usize)>,
    /// Ask "And what else?" once before the answer (enumerate).
    pub what_else: bool,
}

impl Prompt {
    /// The oral version of an item, if it has a short answer that can be said
    /// aloud: cards, enumerations, essay outlines and a case's first step.
    pub fn from_item(lecture: &str, item: &Item) -> Option<Prompt> {
        if item.status == Status::Rejected {
            return None;
        }
        let (question, answer, pass, what_else) = match &item.body {
            Body::Card { front, back } => (vec![front.clone()], vec![back.clone()], None, false),
            Body::Enumerate { prompt, points, pass } => {
                (vec![prompt.clone()], points.clone(), Some((*pass, points.len())), points.len() > 1)
            }
            Body::Essay { prompt, outline } => (vec![prompt.clone()], outline_lines(outline), None, false),
            Body::Case { stem, steps } => {
                let step = steps.first()?;
                let mut question = vec![stem.clone()];
                question.extend(step.reveal.clone());
                question.push(step.question.clone());
                let mut answer: Vec<String> =
                    step.answer.and_then(|i| step.options.get(i)).cloned().into_iter().collect();
                answer.push(step.why.clone());
                (question, answer, None, false)
            }
            _ => return None,
        };
        if question.iter().all(|q| q.trim().is_empty()) || answer.is_empty() {
            return None;
        }
        Some(Prompt {
            item_id: item.id.clone(),
            kind: item.body.kind(),
            lecture: lecture.to_owned(),
            topic: item.topic.clone(),
            page: item.src.page,
            question,
            answer,
            pass,
            what_else,
        })
    }

    pub fn spoken_question(&self) -> String {
        speakable(&self.question.join(". "))
    }

    pub fn spoken_answer(&self) -> String {
        speakable(&self.answer.join(". "))
    }
}

/// Essay headings, each with its bullets after a colon.
fn outline_lines(outline: &[OutlineEntry]) -> Vec<String> {
    outline
        .iter()
        .flat_map(|entry| match entry {
            OutlineEntry::Heading(h) => vec![h.clone()],
            OutlineEntry::Section(map) => map
                .iter()
                .map(
                    |(h, bullets)| {
                        if bullets.is_empty() { h.clone() } else { format!("{h}: {}", bullets.join("; ")) }
                    },
                )
                .collect(),
        })
        .collect()
}

/// Symbols a voice would read badly, spelled out.
pub fn speakable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '→' | '⇒' => out.push_str(" then "),
            '≥' => out.push_str(" at least "),
            '≤' => out.push_str(" at most "),
            '>' => out.push_str(" more than "),
            '<' => out.push_str(" less than "),
            '±' => out.push_str(" with or without "),
            '~' | '≈' => out.push_str(" about "),
            '↑' => out.push_str(" raised "),
            '↓' => out.push_str(" low "),
            '%' => out.push_str(" percent"),
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Which items go into a session.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    /// One lecture (`hb.L01`), or every written lecture.
    pub lecture: Option<String>,
    /// Questions in the session (Agains come on top).
    pub size: usize,
    pub seed: u64,
}

/// Every askable item of the chosen lectures.
pub fn pool<'a>(lectures: impl IntoIterator<Item = &'a Lecture>, lecture: Option<&str>) -> Vec<Prompt> {
    lectures
        .into_iter()
        .filter(|l| lecture.is_none_or(|id| l.lecture == id))
        .flat_map(|l| l.items.iter().filter_map(|item| Prompt::from_item(&l.lecture, item)))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub prompt: Prompt,
    /// Asked as the examiner's follow-up to the previous card.
    pub follow_up: bool,
    /// Already asked once and rated Again.
    pub again: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tally {
    pub good: usize,
    pub again: usize,
    /// Heard without a rating (auto-continue while walking).
    pub heard: usize,
    /// Items rated Again at least once, in order.
    pub missed: Vec<Prompt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    queue: VecDeque<Entry>,
    /// Essays held back so a card can lead into them.
    essays: Vec<Prompt>,
    pub current: Option<Entry>,
    pub tally: Tally,
    pub total: usize,
    /// Counts every question asked, so the UI knows when a new one starts
    /// (even the same item again).
    pub asked: u64,
    cards_since_follow_up: u32,
}

impl Session {
    pub fn new(mut pool: Vec<Prompt>, pick: &Pick) -> Session {
        shuffle(&mut pool, pick.seed);
        pool.truncate(pick.size.max(1));
        let total = pool.len();
        let essays = pool.iter().filter(|p| p.kind == "essay").cloned().collect();
        let queue = pool.into_iter().map(|prompt| Entry { prompt, follow_up: false, again: false }).collect();
        Session {
            queue,
            essays,
            current: None,
            tally: Tally::default(),
            total,
            asked: 0,
            cards_since_follow_up: 0,
        }
    }

    /// Moves to the next question. `None` when the session is over.
    pub fn advance(&mut self) -> Option<&Entry> {
        self.current = self.queue.pop_front();
        self.asked += u64::from(self.current.is_some());
        self.current.as_ref()
    }

    /// Stops early; the summary shows what was done.
    pub fn end(&mut self) {
        self.current = None;
        self.queue.clear();
    }

    pub fn left(&self) -> usize {
        self.queue.len() + usize::from(self.current.is_some())
    }

    /// Rates the current question and lines up what comes after it.
    pub fn rate(&mut self, grade: Option<Grade>) {
        let Some(entry) = self.current.take() else { return };
        match grade {
            Some(Grade::Again) => {
                self.tally.again += 1;
                if !entry.again {
                    self.tally.missed.push(entry.prompt.clone());
                }
                let at = AGAIN_GAP.min(self.queue.len());
                self.queue.insert(at, Entry { again: true, follow_up: false, ..entry.clone() });
            }
            Some(_) => self.tally.good += 1,
            None => self.tally.heard += 1,
        }
        if entry.prompt.kind == "card" && !entry.again {
            self.cards_since_follow_up += 1;
            if self.cards_since_follow_up >= FOLLOW_UP_EVERY && self.follow_up(&entry.prompt) {
                self.cards_since_follow_up = 0;
            }
        }
    }

    /// Like an examiner: "Good. Now, how would you write about…?" Pulls the
    /// session's essay on the same topic to the front, once.
    fn follow_up(&mut self, card: &Prompt) -> bool {
        let Some(topic) = &card.topic else { return false };
        let Some(i) =
            self.essays.iter().position(|e| e.lecture == card.lecture && e.topic.as_ref() == Some(topic))
        else {
            return false;
        };
        let essay = self.essays.remove(i);
        let Some(at) = self.queue.iter().position(|e| e.prompt.item_id == essay.item_id && !e.again) else {
            return false;
        };
        let mut entry = self.queue.remove(at).expect("position is in range");
        entry.follow_up = true;
        self.queue.push_front(entry);
        true
    }
}

/// A small deterministic Fisher–Yates (xorshift64*), so a session needs no
/// random-number crate and tests can fix the order.
fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut state = seed | 1;
    for i in (1..items.len()).rev() {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let r = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        items.swap(i, (r % (i as u64 + 1)) as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Lecture {
        let text = include_str!("../../../../fixtures/content/sample.yaml");
        serde_yaml_ng::from_str(text).expect("sample parses")
    }

    fn pick(size: usize) -> Pick {
        Pick { lecture: None, size, seed: 7 }
    }

    #[test]
    fn only_short_answer_kinds_are_asked() {
        let lecture = sample();
        let kinds: Vec<_> = pool([&lecture], None).iter().map(|p| p.kind).collect();
        assert!(kinds.contains(&"card") && kinds.contains(&"enumerate") && kinds.contains(&"essay"));
        assert!(kinds.iter().all(|k| ["card", "enumerate", "essay", "case"].contains(k)));
        assert!(pool([&lecture], Some("hb.L02")).is_empty());
    }

    #[test]
    fn enumerate_asks_what_else_and_keeps_the_pass_mark() {
        let lecture = sample();
        let p = pool([&lecture], None).into_iter().find(|p| p.kind == "enumerate").unwrap();
        assert!(p.what_else);
        assert_eq!(p.pass, Some((5, 7)));
        assert!(p.spoken_answer().starts_with("Recurrent variceal bleeding. Recurrent HE"));
    }

    #[test]
    fn essay_answer_reads_headings_with_bullets() {
        let lecture = sample();
        let p = pool([&lecture], None).into_iter().find(|p| p.kind == "essay").unwrap();
        assert_eq!(
            p.answer[0],
            "Hyperdynamic circulatory syndrome: Features; Cause (systemic vasodilatation via PSS)"
        );
    }

    #[test]
    fn rejected_items_are_never_asked() {
        let mut lecture = sample();
        for item in &mut lecture.items {
            item.status = Status::Rejected;
        }
        assert!(pool([&lecture], None).is_empty());
    }

    #[test]
    fn symbols_are_spelled_out() {
        assert_eq!(speakable("MELD ≥15 → transplant"), "MELD at least 15 then transplant");
        assert_eq!(speakable("SAAG ≥ 1.1 g/dL, ~80%"), "SAAG at least 1.1 g/dL, about 80 percent");
    }

    fn card(id: &str, topic: &str) -> Prompt {
        Prompt {
            item_id: id.into(),
            kind: "card",
            lecture: "hb.L01".into(),
            topic: Some(topic.into()),
            page: 1,
            question: vec![format!("Q {id}")],
            answer: vec![format!("A {id}")],
            pass: None,
            what_else: false,
        }
    }

    #[test]
    fn again_comes_back_three_questions_later() {
        let pool: Vec<_> = (0..6).map(|i| card(&format!("c{i}"), "t")).collect();
        let mut s = Session::new(pool, &pick(6));
        let first = s.advance().unwrap().prompt.item_id.clone();
        s.rate(Some(Grade::Again));
        let mut order = Vec::new();
        while let Some(e) = s.advance() {
            order.push((e.prompt.item_id.clone(), e.again));
            s.rate(Some(Grade::Good));
        }
        assert_eq!(order.len(), 6);
        assert_eq!(order[AGAIN_GAP], (first, true));
        assert_eq!((s.tally.good, s.tally.again, s.tally.missed.len()), (6, 1, 1));
    }

    #[test]
    fn again_near_the_end_still_comes_back() {
        let mut s = Session::new(vec![card("only", "t")], &pick(5));
        s.advance();
        s.rate(Some(Grade::Again));
        assert_eq!(s.advance().map(|e| e.again), Some(true));
        s.rate(None);
        assert!(s.advance().is_none());
        assert_eq!(s.tally.heard, 1);
    }

    #[test]
    fn every_third_card_leads_into_its_topics_essay() {
        let mut essay = card("essay", "portal");
        essay.kind = "essay";
        let mut pool: Vec<_> = (0..5).map(|i| card(&format!("c{i}"), "portal")).collect();
        pool.push(essay);
        // Find a seed that puts the essay last, so only the follow-up can pull it forward.
        let seed = (0..200)
            .find(|&seed| {
                let mut s = Session::new(pool.clone(), &Pick { lecture: None, size: 6, seed });
                (0..5).all(|_| {
                    s.advance().is_some_and(|e| e.prompt.kind == "card") && {
                        s.current = None;
                        true
                    }
                })
            })
            .expect("some seed puts the essay last");
        let mut s = Session::new(pool, &Pick { lecture: None, size: 6, seed });
        for _ in 0..3 {
            assert_eq!(s.advance().unwrap().prompt.kind, "card");
            s.rate(Some(Grade::Good));
        }
        let next = s.advance().unwrap();
        assert_eq!((next.prompt.kind, next.follow_up), ("essay", true));
    }

    #[test]
    fn size_limits_the_session() {
        let pool: Vec<_> = (0..30).map(|i| card(&format!("c{i}"), "t")).collect();
        let s = Session::new(pool, &pick(10));
        assert_eq!((s.total, s.left()), (10, 10));
    }
}
