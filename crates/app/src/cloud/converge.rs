//! Property tests for sync on the app's real store: a phone and a tablet
//! write, sync now and then through a hub that delivers in any order (with
//! repeats), and a third device restores from the hub's log at the end. All
//! three must show the learner the same data, and an undone event never
//! comes back.

use std::collections::BTreeMap;

use kabar_core::settings::{Settings, Theme};
use kabar_protocol::coach::{ActivityKind, LocalDate};
use kabar_protocol::reader::WordStatus;
use kabar_protocol::sync::{OpKind, SeqOp};
use proptest::prelude::*;
use serde_json::Value;

use crate::store::Store;

const WORDS: [&str; 3] = ["id:rumah", "id:makan", "jv:omah"];
const THEMES: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];
const LESSONS: [&str; 2] = ["bipa1-u01-l01", "bipa1-u01-l02"];

#[derive(Clone, Debug)]
enum Act {
    Word(usize, WordStatus),
    ForgetWord(usize),
    Theme(usize),
    /// A plan item ticked: an event in the activity log.
    Tick,
    /// Undo the device's n-th tick it knows of (its own or synced).
    Untick(usize),
    Lesson(usize),
    /// Push the outbox to the hub and pull what the hub has.
    Sync,
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        3 => (0..WORDS.len(), proptest::sample::select(WordStatus::ALL.to_vec())).prop_map(|(w, s)| Act::Word(w, s)),
        1 => (0..WORDS.len()).prop_map(Act::ForgetWord),
        2 => (0..THEMES.len()).prop_map(Act::Theme),
        2 => Just(Act::Tick),
        1 => (0..4usize).prop_map(Act::Untick),
        1 => (0..LESSONS.len()).prop_map(Act::Lesson),
        3 => Just(Act::Sync),
    ]
}

/// The hub's log, and a delivery order source for each pull.
struct Hub {
    log: Vec<SeqOp>,
}

impl Hub {
    fn push(&mut self, store: &mut Store) {
        let ops = store.outbox(1_000).unwrap();
        let ids: Vec<String> = ops.iter().map(|o| o.id.clone()).collect();
        for op in ops {
            let seq = self.log.len() as u64 + 1;
            self.log.push(SeqOp { seq, op });
        }
        store.remove_from_outbox(ids.iter().map(String::as_str)).unwrap();
    }

    /// Everything after the device's `last_seq`, reordered by `shuffle`, with
    /// the first op of the page sent twice (a reconnect replays a page).
    fn pull(&self, store: &mut Store, shuffle: &[usize]) {
        let from = store.last_seq().unwrap() as usize;
        let mut page: Vec<SeqOp> = self.log[from..].to_vec();
        for (i, &j) in shuffle.iter().enumerate() {
            let len = page.len();
            if len > 1 {
                page.swap(i % len, j % len);
            }
        }
        if let Some(first) = page.first().cloned() {
            page.push(first);
        }
        store.apply_remote(&page, self.log.len() as u64).unwrap();
    }
}

/// What the learner sees on a device.
#[derive(Debug, PartialEq)]
struct View {
    settings: Settings,
    words: BTreeMap<String, WordStatus>,
    ticks: Vec<String>,
    lessons: Vec<Vec<String>>,
    word_rows: BTreeMap<String, Value>,
}

fn view(store: &Store) -> View {
    let words = store.word_statuses(WORDS).unwrap().into_iter().collect();
    let mut ticks: Vec<String> = store.activity(None).unwrap().into_iter().map(|e| e.id).collect();
    ticks.sort();
    let lessons = LESSONS
        .iter()
        .map(|l| {
            let mut ids: Vec<String> = store.lesson_events(l).unwrap().into_iter().map(|e| e.id).collect();
            ids.sort();
            ids
        })
        .collect();
    let word_rows = store.synced_rows("word_status").unwrap().into_iter().collect();
    View { settings: store.settings().unwrap(), words, ticks, lessons, word_rows }
}

fn run(script: &[(usize, Act)], shuffles: &[Vec<usize>]) -> (Vec<Store>, Hub, Vec<String>) {
    let mut devices = vec![Store::open_in_memory().unwrap(), Store::open_in_memory().unwrap()];
    let mut hub = Hub { log: Vec::new() };
    let mut retracted = Vec::new();
    let mut shuffle = shuffles.iter().cycle();
    for (who, a) in script {
        let store = &mut devices[*who];
        match a {
            Act::Word(w, s) => store.set_word_status(WORDS[*w], *s).unwrap(),
            Act::ForgetWord(w) => {
                drop(store.write_synced("word_status", WORDS[*w], OpKind::Delete, None).unwrap())
            }
            Act::Theme(t) => {
                let old = store.settings().unwrap();
                let new = Settings { theme: THEMES[*t], ..old.clone() };
                store.save_settings(&old, &new).unwrap();
            }
            Act::Tick => {
                let kind = ActivityKind::PlanItemDone { item_id: "review".into() };
                store.add_activity(kind, LocalDate(20_400), 60).unwrap();
            }
            Act::Untick(n) => {
                let events = store.activity(None).unwrap();
                if let Some(e) = events.get(n % events.len().max(1)) {
                    store.retract_activity(&e.id).unwrap();
                    retracted.push(e.id.clone());
                }
            }
            Act::Lesson(l) => drop(store.record_lesson_event(LESSONS[*l], "completed", &()).unwrap()),
            Act::Sync => {
                hub.push(store);
                hub.pull(store, shuffle.next().unwrap());
            }
        }
    }
    (devices, hub, retracted)
}

fn script() -> impl Strategy<Value = (Vec<(usize, Act)>, Vec<Vec<usize>>)> {
    (
        proptest::collection::vec((0..2usize, act()), 1..30),
        proptest::collection::vec(proptest::collection::vec(0..64usize, 0..8), 1..6),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn phone_tablet_and_a_new_device_converge((script, shuffles) in script()) {
        let (mut devices, mut hub, _) = run(&script, &shuffles);
        // Everyone pushes, then everyone pulls.
        for d in devices.iter_mut() {
            hub.push(d);
        }
        for d in devices.iter_mut() {
            hub.pull(d, &shuffles[0]);
        }
        // A new install gets the whole log, newest first.
        let mut laptop = Store::open_in_memory().unwrap();
        let mut log = hub.log.clone();
        log.reverse();
        laptop.apply_remote(&log, hub.log.len() as u64).unwrap();

        let phone = view(&devices[0]);
        prop_assert_eq!(&phone, &view(&devices[1]));
        prop_assert_eq!(&phone, &view(&laptop));
        for d in &devices {
            prop_assert_eq!(d.outbox_len().unwrap(), 0);
        }
    }

    #[test]
    fn an_undone_tick_never_comes_back((script, shuffles) in script()) {
        let (mut devices, mut hub, retracted) = run(&script, &shuffles);
        for d in devices.iter_mut() {
            hub.push(d);
        }
        // Replay the whole log again in another order: old appends of the
        // same events must not revive them.
        let mut laptop = Store::open_in_memory().unwrap();
        for d in devices.iter_mut().chain(std::iter::once(&mut laptop)) {
            d.set_last_seq(0).unwrap();
            hub.pull(d, &shuffles[shuffles.len() - 1]);
            let shown: Vec<String> = d.activity(None).unwrap().into_iter().map(|e| e.id).collect();
            for id in &retracted {
                prop_assert!(!shown.contains(id), "undone tick {id} is back");
            }
        }
    }
}
