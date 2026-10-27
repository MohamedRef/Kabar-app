//! Bake-off runner tests against recorded provider replies in
//! `fixtures/bakeoff/`. No live calls.

use std::cell::RefCell;
use std::collections::VecDeque;

use kabar_protocol::ai::EngineId;
use serde_json::json;

use super::engines::{CallError, Engine, Http};
use super::report::{self, Row};
use super::*;
use crate::golden;

macro_rules! fixture {
    ($name:literal) => {
        serde_json::from_str::<Value>(include_str!(concat!("../../../../fixtures/bakeoff/", $name))).unwrap()
    };
}

/// Replies in order; records what was sent.
#[derive(Default)]
struct Scripted {
    replies: RefCell<VecDeque<Result<(u16, Value)>>>,
    sent: RefCell<Vec<(String, Value)>>,
}

impl Scripted {
    fn new(replies: Vec<(u16, Value)>) -> Self {
        Self { replies: RefCell::new(replies.into_iter().map(Ok).collect()), ..Self::default() }
    }
}

impl Http for Scripted {
    fn post_json(&self, url: &str, _: &[(&str, String)], body: &Value) -> Result<(u16, Value)> {
        self.sent.borrow_mut().push((url.to_string(), body.clone()));
        self.replies.borrow_mut().pop_front().unwrap_or_else(|| panic!("unscripted call to {url}"))
    }
}

fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

const KEYS: &[(&str, &str)] = &[
    ("CLOUDFLARE_ACCOUNT_ID", "acc"),
    ("CLOUDFLARE_API_TOKEN", "cf-token"),
    ("GEMINI_API_KEY", "g-key"),
    ("ANTHROPIC_API_KEY", "a-key"),
];

fn local() -> Engine {
    Engine::from_spec("local=qwen-sea-lion-v4-4b", env(KEYS)).unwrap()
}

/// A scratch run folder under the target dir, removed first.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-bakeoff").join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn set(task: Task, items: Vec<GoldenItem>) -> GoldenSet {
    GoldenSet { task: task.as_str().into(), version: 1, rubric: "r".into(), items }
}

fn item(id: &str, input: Value) -> GoldenItem {
    GoldenItem {
        id: id.into(),
        lang: None,
        input,
        expect: Value::Null,
        criteria: vec![],
        avoid: vec![],
        tags: vec![],
        image: None,
        verified: false,
    }
}

/// docs/SPEC.md §24-style grading item.
fn grade_input() -> Value {
    json!({
        "exercise": "Translate: I am reading a book.",
        "references": ["Saya sedang membaca buku."],
        "answer": "Saya sedang baca buku."
    })
}

fn root() -> Root {
    Root::new(None)
}

#[test]
fn engines_need_their_keys_and_name_local_models() {
    let err = Engine::from_spec("gemini_free", env(&[])).unwrap_err().to_string();
    assert!(err.contains("GEMINI_API_KEY"), "{err}");
    assert!(Engine::from_spec("local", env(KEYS)).is_err(), "local needs a model");
    assert!(Engine::from_spec("openai", env(KEYS)).is_err());
    assert_eq!(local().label(), "local-qwen-sea-lion-v4-4b");
    assert_eq!(Engine::from_spec("workers_ai", env(KEYS)).unwrap().label(), "workers_ai");
}

#[test]
fn requests_match_what_kabar_cloud_sends() {
    let call = EngineCall {
        cache_key: CacheKey {
            task: Task::GradeAnswer,
            input_hash: "h".into(),
            engine: EngineId::WorkersAi,
            prompt_version: 1,
            profile_hash: String::new(),
        },
        system: "sys".into(),
        user: "Sek tak lapor pusat".into(),
        image_sha256: None,
        is_repair: false,
        json: true,
        max_tokens: Some(512),
        personal: false,
    };
    let wai = Engine::from_spec("workers_ai", env(KEYS)).unwrap();
    let (url, headers, body) = wai.request(&call, None);
    assert_eq!(
        url,
        format!("{}/accounts/acc/ai/run/@cf/aisingapore/gemma-sea-lion-v4-27b-it", engines::CLOUDFLARE_API)
    );
    assert_eq!(headers, [("authorization", "Bearer cf-token".to_string())]);
    assert_eq!(body, kabar_server::ai::workers_ai_input(&call, None));

    // An image switches Workers AI to its vision model.
    let img = Image { mime: "image/png".into(), base64: "iVBO".into() };
    let (url, _, _) = wai.request(&call, Some(&img));
    assert!(url.ends_with("/ai/run/@cf/meta/llama-4-scout-17b-16e-instruct"), "{url}");

    let (url, _, body) = local().request(&call, Some(&img));
    assert_eq!(url, "http://localhost:11434/api/chat");
    assert_eq!(body["format"], "json");
    assert_eq!(body["options"]["num_predict"], 512);
    assert_eq!(body["messages"][0], json!({"role": "system", "content": "sys"}));
    assert_eq!(body["messages"][1]["images"], json!(["iVBO"]));
}

#[test]
fn quota_and_rate_limits_stop_the_engine() {
    let wai = Engine::from_spec("workers_ai", env(KEYS)).unwrap();
    let gem = Engine::from_spec("gemini_free", env(KEYS)).unwrap();
    assert!(matches!(
        wai.parse(200, &fixture!("workers_ai_rest.neurons_exhausted.json"), false),
        Err(CallError::Exhausted(_))
    ));
    assert!(matches!(
        gem.parse(429, &fixture!("gemini.resource_exhausted.json"), false),
        Err(CallError::Exhausted(_))
    ));
    assert!(matches!(
        wai.parse(500, &json!({"errors": [{"message": "boom"}]}), false),
        Err(CallError::Failed(_))
    ));
    let r = wai.parse(200, &fixture!("workers_ai_rest.grade_answer.ok.json"), false).unwrap();
    assert_eq!((r.tokens_in, r.tokens_out), (Some(598), Some(91)));
}

#[test]
fn runs_retry_once_like_the_router_and_resume() {
    let http = Scripted::new(vec![
        (200, fixture!("ollama.grade_answer.ok.json")),
        (200, fixture!("ollama.grade_answer.inconsistent.json")),
        (200, fixture!("ollama.grade_answer.almost.json")),
    ]);
    let out = scratch("resume");
    let mut runner = Runner::new(&http, out.clone());
    let s = set(Task::GradeAnswer, vec![item("g-1", grade_input()), item("g-2", grade_input())]);
    let t = runner.run(Path::new("."), Task::GradeAnswer, &s, &local()).unwrap();
    assert_eq!((t.answered, t.already, t.stopped), (2, 0, None));

    let recs = read_records(&out.join("local-qwen-sea-lion-v4-4b/grade_answer.jsonl")).unwrap();
    assert!(recs[0].first_ok && recs[0].ok);
    assert_eq!((recs[0].tokens_in, recs[0].tokens_out), (Some(612), Some(88)));
    // The second needed the repair retry: valid, but not first time.
    assert!(!recs[1].first_ok && recs[1].ok);
    assert_eq!(recs[1].output.as_ref().unwrap()["verdict"], "almost");
    assert_eq!(recs[1].tokens_out, Some(160));
    let repair = &http.sent.borrow()[2].1["messages"][1]["content"];
    assert!(repair.as_str().unwrap().contains("does not match verdict"), "{repair}");

    // Same prompt and input: nothing is sent again.
    let t = runner.run(Path::new("."), Task::GradeAnswer, &s, &local()).unwrap();
    assert_eq!((t.answered, t.already), (0, 2));
    assert_eq!(http.sent.borrow().len(), 3);
}

#[test]
fn an_exhausted_engine_records_nothing_and_resumes_later() {
    let http = Scripted::new(vec![(429, fixture!("gemini.resource_exhausted.json"))]);
    let out = scratch("exhausted");
    let mut runner = Runner::new(&http, out.clone());
    runner.sleep = |_| {};
    let gem = Engine::from_spec("gemini_free", env(KEYS)).unwrap();
    let s = set(Task::GradeAnswer, vec![item("g-1", grade_input())]);
    let t = runner.run(Path::new("."), Task::GradeAnswer, &s, &gem).unwrap();
    assert!(t.stopped.unwrap().contains("rate limited or out of quota"));
    assert!(read_records(&out.join("gemini_free/grade_answer.jsonl")).unwrap().is_empty());
}

#[test]
fn personal_tasks_never_reach_gemini_and_captures_wait() {
    let http = Scripted::new(vec![]);
    let mut runner = Runner::new(&http, scratch("privacy"));
    let gem = Engine::from_spec("gemini_free", env(KEYS)).unwrap();
    let plan = set(Task::DailyPlan, vec![item("p-1", json!({"goal_minutes": 5}))]);
    assert_eq!(runner.run(Path::new("."), Task::DailyPlan, &plan, &gem).unwrap(), Tally::default());

    let mut sketch = item("s-1", json!({"target_word": "rumah"}));
    sketch.tags = vec![NEEDS_CAPTURE.into()];
    let t = runner
        .run(Path::new("."), Task::GuessSketch, &set(Task::GuessSketch, vec![sketch]), &local())
        .unwrap();
    assert_eq!(t.needs_capture, 1);
    assert!(http.sent.borrow().is_empty());
}

fn record(item: &str, engine: &str, ok: bool, ms: u64) -> Record {
    Record {
        item: item.into(),
        task: "grade_answer".into(),
        engine: engine.into(),
        model: "m".into(),
        prompt_version: 1,
        input_hash: "h".into(),
        first_ok: ok,
        ok,
        error: (!ok).then(|| "bad".into()),
        latency_ms: ms,
        total_ms: ms,
        tokens_in: None,
        tokens_out: None,
        reply: "{}".into(),
        output: ok.then(|| json!({"verdict": "correct"})),
    }
}

#[test]
fn blind_letters_hide_engines_and_the_report_applies_the_bar() {
    let run = scratch("report");
    let set = golden::load(&golden::dir(&root()).join("grade_answer.yaml")).unwrap();
    let ids: Vec<&str> = set.items.iter().map(|i| i.id.as_str()).collect();
    for (engine, ms) in [("workers_ai", 900), ("local-qwen", 2_500), ("gemini_free", 1_200)] {
        for (n, id) in ids.iter().enumerate() {
            // gemini_free had one reply that never validated.
            let ok = !(engine == "gemini_free" && n == 3);
            append(&run.join(engine).join("grade_answer.jsonl"), &record(id, engine, ok, ms)).unwrap();
        }
    }
    let n = ids.len();

    assert_eq!(blind(&root(), &run).unwrap(), 3 * n - 1);
    let sheet = fs::read_to_string(run.join("blind/grade_answer.jsonl")).unwrap();
    assert!(!sheet.contains("workers_ai") && !sheet.contains("local-qwen") && !sheet.contains("gemini"));
    assert!(run.join("blind/grade_answer.rubric.md").exists());
    assert!(!run.join("blind/key.jsonl").exists(), "the key stays out of the judge's folder");

    // The judge: workers_ai excellent, local good but slow, gemini one 0.
    let key: Vec<KeyEntry> = fs::read_to_string(run.join("key.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut scores = String::new();
    for k in &key {
        let score = match (k.engine.as_str(), k.item.as_str()) {
            ("workers_ai", _) => 3,
            ("local-qwen", _) => 3,
            ("gemini_free", "grade_answer-002") => 0,
            _ => 3,
        };
        scores.push_str(&format!("{}\n", json!({"item": k.item, "letter": k.letter, "score": score})));
    }
    fs::create_dir_all(run.join("scores")).unwrap();
    fs::write(run.join("scores/grade_answer.jsonl"), scores).unwrap();

    report::report(&root(), &run, false).unwrap();
    let md = fs::read_to_string(run.join("report.md")).unwrap();
    assert!(
        md.contains(&format!("| workers_ai | {n} | 100% | 100% | 0.9 s | {n} | 3.00 | 0 | pass |")),
        "{md}"
    );
    assert!(
        md.contains(&format!("| local-qwen | {n} | 100% | 100% | 2.5 s | {n} | 3.00 | 0 | pass |")),
        "{md}"
    );
    assert!(md.contains("% < 98%") && md.contains("1 answer(s) scored 0"), "{md}");
    assert!(md.contains("Route: `workers_ai`, `local-qwen` (vetted)"), "{md}");

    let routes = RouteTable::parse(&fs::read_to_string(run.join("routes.toml")).unwrap()).unwrap();
    let r = routes.get(Task::GradeAnswer);
    assert_eq!(r.engines, [EngineId::WorkersAi, EngineId::Local]);
    assert_eq!(r.status, kabar_core::ai::routes::RouteStatus::Vetted);
    // Tasks nobody ran keep their current route, provisional.
    assert_eq!(routes.get(Task::MindMap), RouteTable::builtin().get(Task::MindMap));
}

#[test]
fn the_bar_counts_unjudged_answers_and_latency() {
    let recs = vec![record("a", "workers_ai", true, 2_600), record("b", "workers_ai", true, 2_400)];
    let scores = BTreeMap::from([(("a".to_string(), "workers_ai".to_string()), 3)]);
    let rows: Vec<Row> = report::rows(Task::AnalyzeText, 3, &recs, &scores);
    assert_eq!(rows[0].misses, ["only 2 of 3 items run", "1 of 2 answers not judged yet", "p50 2.4 s > 2 s"]);
    assert!(!rows[0].passes());
}
