//! The app's AI router: the bundled routes and prompts, Kabar Cloud's
//! engines behind the AI proxy, and a SQLite answer cache.

use kabar_core::ai::{Cache, Engine, EngineFuture, PromptSet, RouteTable, Router};
use kabar_protocol::ai::{CacheKey, EngineCall, EngineId};

use crate::Db;
use crate::cloud::link::CloudLink;

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
