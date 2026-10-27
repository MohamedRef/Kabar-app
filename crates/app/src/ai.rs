//! The app's AI router: the bundled routes and prompts, Kabar Cloud's
//! engines behind the AI proxy, and a SQLite answer cache.

use kabar_core::ai::{
    Cache, Engine, EngineFuture, ForgetFuture, LearnerProfile, PromptSet, RouteTable, Router,
};
use kabar_core::coach::plan;
use kabar_core::review::Deck;
use kabar_core::srs::Scheduler;
use kabar_core::stats as core_stats;
use kabar_protocol::ai::{CacheKey, EngineCall, EngineId};
use kabar_protocol::coach::{ActivityEvent, CoachProfile, LocalDate, Motivation};

use crate::Db;
use crate::cloud::link::CloudLink;
use crate::coach::WEAK_SPOT_WINDOW_DAYS;
use crate::stats::{Logs, level_label, load_logs};

/// Weak topics the prompts mention, worst first.
const STRUGGLES_IN_PROMPT: usize = 3;

/// Engines Kabar Cloud's `/v1/ai` proxy runs. Kabar Home (`local`) comes
/// with its own client.
const CLOUD_ENGINES: [EngineId; 3] = [EngineId::WorkersAi, EngineId::GeminiFree, EngineId::Claude];

pub fn router(db: Db, link: CloudLink) -> Router {
    let mut router = Router::new(RouteTable::builtin(), PromptSet::builtin(), Box::new(StoreCache(db)));
    for id in CLOUD_ENGINES {
        router = router.with_engine(Box::new(CloudEngine { id, link: link.clone() }));
    }
    router
}

/// What the prompts say about the learner: levels from the Journey, words
/// from the review deck, goals from onboarding and struggles from the last
/// two weeks of practice. Goals and struggles are personal; the router keeps
/// them away from engines that train on inputs.
pub fn learner_profile(
    db: &Db,
    profile: Option<&CoachProfile>,
    events: &[ActivityEvent],
    today: LocalDate,
) -> LearnerProfile {
    let (id, jv) = crate::journey::learner_levels(db, profile);
    let Logs { cards, log, .. } = load_logs(db);
    let deck = Deck::new(cards, log, &Scheduler::default());
    let known_words =
        core_stats::word_counts(deck.cards(), |id| deck.state(id)).iter().map(|w| w.known + w.learning).sum();
    let mut goals: Vec<String> =
        profile.map(|p| p.motivations.iter().map(|m| goal(*m).to_string()).collect()).unwrap_or_default();
    if let Some(note) = profile.and_then(|p| p.why_note.as_deref()).map(str::trim).filter(|n| !n.is_empty()) {
        goals.push(note.to_string());
    }
    let recent_struggles = plan::weak_spots(events, today, WEAK_SPOT_WINDOW_DAYS)
        .into_iter()
        .take(STRUGGLES_IN_PROMPT)
        .map(|f| f.topic)
        .collect();
    LearnerProfile {
        indonesian_level: level_label(&id),
        javanese_level: jv.map_or_else(|| "not started".into(), |l| level_label(&l)),
        known_words,
        goals,
        recent_struggles,
        ..LearnerProfile::default()
    }
}

fn goal(m: Motivation) -> &'static str {
    match m {
        Motivation::Travel => "travel in Indonesia",
        Motivation::Work => "work",
        Motivation::FamilyFriends => "talk with family and friends",
        Motivation::Media => "films, music and social media",
        Motivation::Faith => "faith: Islam in Indonesia and Java",
        Motivation::Curiosity => "curiosity",
    }
}

/// One engine run by Kabar Cloud. The router decides which may be used (free
/// only while the $0 hard stop is on, never `gemini_free` for personal input);
/// the proxy enforces the same rules again.
struct CloudEngine {
    id: EngineId,
    link: CloudLink,
}

impl Engine for CloudEngine {
    fn id(&self) -> EngineId {
        self.id
    }

    /// Workers AI and Gemini run on free allowances. Claude is free only
    /// through plan credits, which the learner turns on with "allow paid".
    fn is_free(&self) -> bool {
        self.id != EngineId::Claude
    }

    fn call<'a>(&'a self, call: &'a EngineCall) -> EngineFuture<'a> {
        let mut call = call.clone();
        call.cache_key.engine = self.id;
        Box::pin(self.link.ai(call))
    }

    /// Drops the rejected answer from Cloud's shared cache too, so the
    /// regenerate (and every other device) gets a fresh one.
    fn forget<'a>(&'a self, key: &'a CacheKey) -> ForgetFuture<'a> {
        let mut key = key.clone();
        key.engine = self.id;
        Box::pin(async move {
            if let Err(e) = self.link.ai_reject(key).await {
                crate::diag::report!("AI reject: {e}");
            }
        })
    }
}

/// Validated answers in the local `ai_cache` table. A busy store reads as a
/// miss rather than a panic.
struct StoreCache(Db);

impl Cache for StoreCache {
    fn get(&self, key: &CacheKey) -> Option<String> {
        self.0.0.try_borrow().ok()?.ai_cache_get(&key.id()).ok().flatten()
    }

    fn put(&self, key: &CacheKey, json: &str) {
        if let Ok(store) = self.0.0.try_borrow()
            && let Err(e) = store.ai_cache_put(&key.id(), json)
        {
            crate::diag::report!("AI cache: {e}");
        }
    }

    fn remove(&self, key: &CacheKey) {
        if let Ok(store) = self.0.0.try_borrow() {
            store.ai_cache_remove(&key.id()).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use kabar_core::ai::tasks::GradeAnswer;
    use kabar_core::ai::{AiError, AiRequest, AttemptError};
    use kabar_protocol::ai::{EngineError, Task};

    use super::*;
    use crate::store::Store;

    fn db() -> Db {
        Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())))
    }

    #[test]
    fn the_profile_follows_onboarding_and_the_journey() {
        let db = db();
        let today = LocalDate::from_ymd(2026, 10, 27).unwrap();
        let fresh = learner_profile(&db, None, &[], today);
        assert_eq!(fresh, LearnerProfile::default());

        let coach = CoachProfile {
            motivations: vec![Motivation::Travel, Motivation::Faith],
            why_note: Some("  To read Kompas every morning  ".into()),
            start: kabar_protocol::coach::StartingPoint::CanChat,
            ..CoachProfile::default()
        };
        let p = learner_profile(&db, Some(&coach), &[], today);
        assert_eq!(p.indonesian_level, "BIPA 3", "the onboarding answer places the learner");
        assert_eq!(p.javanese_level, "not started");
        assert_eq!(p.known_words, 0);
        assert_eq!(
            p.goals,
            ["travel in Indonesia", "faith: Islam in Indonesia and Java", "To read Kompas every morning"]
        );
        assert!(p.recent_struggles.is_empty());
        assert!(p.render(EngineId::WorkersAi).contains("Indonesian level: BIPA 3."));
    }

    #[test]
    fn cloud_engines_are_planned_free_first() {
        let router = router(db(), CloudLink::default());
        let plan = router.plan(Task::RoleplayTurn, false).unwrap();
        assert_eq!(plan, [EngineId::WorkersAi, EngineId::GeminiFree], "claude waits for allow-paid");
        let personal = router.plan(Task::RoleplayTurn, true).unwrap();
        assert!(!personal.contains(&EngineId::GeminiFree));
    }

    #[test]
    fn without_kabar_cloud_every_engine_says_why() {
        let router = router(db(), CloudLink::default());
        let req = AiRequest::new(serde_json::json!({ "question": "Apa kabar?", "answer": "Baik" }));
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let Err(AiError::AllFailed { attempts, .. }) = runtime.block_on(router.run::<GradeAnswer>(&req))
        else {
            panic!("expected every engine to fail");
        };
        assert!(!attempts.is_empty());
        for a in attempts {
            assert!(
                matches!(&a.error, AttemptError::Engine(EngineError::Unavailable { reason }) if reason.contains("isn't set up")),
                "{a:?}"
            );
        }
    }

    #[test]
    fn answers_are_cached_in_the_store() {
        let db = db();
        let cache = StoreCache(db.clone());
        let key = CacheKey {
            task: Task::RoleplayTurn,
            input_hash: "abc".into(),
            engine: EngineId::WorkersAi,
            prompt_version: 1,
            profile_hash: String::new(),
        };
        assert_eq!(cache.get(&key), None);
        cache.put(&key, "{}");
        assert_eq!(cache.get(&key).as_deref(), Some("{}"));
        // While the store is borrowed elsewhere the cache misses, never panics.
        let held = db.0.borrow_mut();
        assert_eq!(cache.get(&key), None);
        drop(held);
        cache.remove(&key);
        assert_eq!(cache.get(&key), None);
    }
}
