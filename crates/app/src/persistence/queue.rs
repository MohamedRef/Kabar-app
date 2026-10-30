//! UI-independent latest-choice retry bookkeeping for preference writes.
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct Attempt {
    pub key: String,
    generation: u64,
    pub write: Rc<dyn Fn() -> Result<(), String>>,
}

#[derive(Default)]
pub(super) struct Queue {
    pending: BTreeMap<String, Attempt>,
    generation: u64,
}

impl Queue {
    pub fn pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn record(&mut self, key: String, succeeded: bool, write: Rc<dyn Fn() -> Result<(), String>>) {
        self.generation = self.generation.wrapping_add(1);
        if succeeded {
            self.pending.remove(&key);
        } else {
            self.pending.insert(key.clone(), Attempt { key, generation: self.generation, write });
        }
    }

    pub fn attempts(&self) -> Vec<Attempt> {
        self.pending.values().cloned().collect()
    }

    pub fn is_current(&self, attempt: &Attempt) -> bool {
        self.pending.get(&attempt.key).is_some_and(|now| now.generation == attempt.generation)
    }

    pub fn finish(&mut self, attempt: &Attempt, succeeded: bool) {
        if succeeded && self.is_current(attempt) {
            self.pending.remove(&attempt.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn unrelated_success_does_not_hide_a_failure() {
        let mut q = Queue::default();
        q.record("theme".into(), false, Rc::new(|| Ok(())));
        q.record("size".into(), true, Rc::new(|| Ok(())));
        assert_eq!(q.attempts()[0].key, "theme");
        assert!(q.pending());
    }

    #[test]
    fn newest_choice_replaces_the_retry_without_old_completion_clearing_it() {
        let mut q = Queue::default();
        q.record("size".into(), false, Rc::new(|| Ok(())));
        let old = q.attempts().remove(0);
        let latest = Rc::new(Cell::new(false));
        let observed = latest.clone();
        q.record(
            "size".into(),
            false,
            Rc::new(move || {
                observed.set(true);
                Ok(())
            }),
        );
        q.finish(&old, true);
        assert!(q.pending());
        let current = q.attempts().remove(0);
        (current.write)().unwrap();
        q.finish(&current, true);
        assert!(latest.get());
        assert!(!q.pending());
    }

    #[test]
    fn failed_retry_stays_pending_and_superseded_snapshot_is_skipped() {
        let mut q = Queue::default();
        q.record("theme".into(), false, Rc::new(|| Err("blocked".into())));
        let attempt = q.attempts().remove(0);
        q.finish(&attempt, false);
        assert!(q.is_current(&attempt));
        q.record("theme".into(), true, Rc::new(|| Ok(())));
        assert!(!q.is_current(&attempt));
        assert!(!q.pending());
    }
}
