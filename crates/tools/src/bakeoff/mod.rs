//! The engine bake-off (`docs/BAKEOFF.md`): every golden item to every
//! candidate engine, with the app's own prompts and checks, then blind
//! judging and a report that proposes `prompts/routes.toml`.
//!
//! 1. `run` records each answer to `bakeoff/runs/<date>/<engine>/<task>.jsonl`.
//!    It is resumable: an item already answered with the same prompt version
//!    and input is skipped, and an engine that runs out of free quota (or
//!    can't be reached) stops without recording, so the next run picks up.
//! 2. `blind` writes `blind/<task>.jsonl` with engine names replaced by
//!    letters, for the judge; the letter → engine key goes to `key.jsonl`
//!    beside it, outside what the judge reads.
//! 3. The judge writes `scores/<task>.jsonl`: `{item, letter, score, note}`.
//! 4. `report` joins it all into `report.md` and a proposed `routes.toml`
//!    (with `--write`, into `docs/bakeoff-report.md` and `prompts/routes.toml`).

pub mod engines;
mod report;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use clap::{Args, Subcommand};
use kabar_core::ai::prompt::{LearnerProfile, PromptSet};
use kabar_core::ai::routes::RouteTable;
use kabar_core::ai::tasks::check_reply;
use kabar_core::ai::{json, repair_prompt};
use kabar_protocol::ai::{CacheKey, EngineCall, Task};
use kabar_protocol::coach::LocalDate;
use kabar_protocol::content::{GoldenItem, GoldenSet};
use kabar_server::blobs::Image;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use self::engines::{CallError, Engine, Http, Ureq};
use crate::golden::{self, NEEDS_CAPTURE};
use crate::paths::Root;

#[derive(Subcommand)]
pub enum Cmd {
    /// Send every golden item to each engine and record the answers.
    Run(RunArgs),
    /// Hide engine names for blind judging.
    Blind {
        /// The run folder, e.g. bakeoff/runs/2026-11-02.
        run: PathBuf,
    },
    /// Join answers and scores into a report and a proposed route table.
    Report {
        run: PathBuf,
        /// Also write docs/bakeoff-report.md and prompts/routes.toml.
        #[arg(long)]
        write: bool,
    },
}

#[derive(Args)]
pub struct RunArgs {
    /// Comma-separated: workers_ai, gemini_free, claude, local=<ollama model>.
    #[arg(long, value_delimiter = ',', required = true)]
    pub engines: Vec<String>,
    /// Only these tasks (default: all).
    #[arg(long, value_delimiter = ',')]
    pub tasks: Vec<String>,
    /// Only the first N items of each set (a quick smoke run; rerunning
    /// with the same N sends nothing new).
    #[arg(long)]
    pub limit: Option<usize>,
    /// Run folder (default: bakeoff/runs/<today>).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// One answer, as recorded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub item: String,
    pub task: String,
    /// `workers_ai`, `local-<model>`, …
    pub engine: String,
    pub model: String,
    pub prompt_version: u32,
    pub input_hash: String,
    /// The first reply passed the task's checks.
    pub first_ok: bool,
    /// It passed, possibly after the one repair retry (what the app would show).
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// First call only: what the learner waits for when it's right first time.
    pub latency_ms: u64,
    /// Both calls when a repair was needed.
    pub total_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    /// The last raw reply.
    pub reply: String,
    /// The parsed answer when `ok`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
}

/// What one task × engine pass did.
#[derive(Debug, Default, PartialEq)]
pub struct Tally {
    pub answered: usize,
    pub already: usize,
    pub needs_capture: usize,
    /// Why the engine stopped early, if it did.
    pub stopped: Option<String>,
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Run(args) => run_cmd(root, args),
        Cmd::Blind { run } => {
            let n = blind(root, &root.path().join(run))?;
            println!("{n} answers ready for blind judging");
            Ok(())
        }
        Cmd::Report { run, write } => report::report(root, &root.path().join(run), write),
    }
}

fn run_cmd(root: &Root, args: RunArgs) -> Result<()> {
    let env = |k: &str| std::env::var(k).ok();
    let engines = args.engines.iter().map(|s| Engine::from_spec(s, env)).collect::<Result<Vec<_>>>()?;
    let tasks = if args.tasks.is_empty() {
        Task::ALL.to_vec()
    } else {
        args.tasks
            .iter()
            .map(|t| Task::parse(t).with_context(|| format!("unknown task `{t}`")))
            .collect::<Result<_>>()?
    };
    let out =
        root.path().join(args.out.unwrap_or_else(|| PathBuf::from(format!("bakeoff/runs/{}", today()))));
    let http = Ureq::new();
    let mut runner = Runner::new(&http, out.clone());
    runner.limit = args.limit;
    let mut stopped = BTreeSet::new();
    for task in tasks {
        let set = golden::load(&golden::dir(root).join(format!("{}.yaml", task.as_str())))?;
        for engine in &engines {
            if stopped.contains(&engine.label()) {
                continue;
            }
            let t = runner.run(root.path(), task, &set, engine)?;
            println!(
                "{:22} {:28} {} new, {} already, {} need capture{}",
                task.as_str(),
                engine.label(),
                t.answered,
                t.already,
                t.needs_capture,
                t.stopped.as_deref().map(|s| format!(" — stopped: {s}")).unwrap_or_default()
            );
            if t.stopped.is_some() {
                stopped.insert(engine.label());
            }
        }
    }
    println!("answers in {}", out.display());
    if !stopped.is_empty() {
        println!("stopped early: {stopped:?}. Run the same command again later to finish.");
    }
    Ok(())
}

fn today() -> String {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    LocalDate::from_unix_ms(ms, 0).to_string()
}

pub struct Runner<'a> {
    http: &'a dyn Http,
    out: PathBuf,
    prompts: PromptSet,
    routes: RouteTable,
    /// A synthetic learner: nothing personal goes into a bake-off.
    profile: LearnerProfile,
    pub limit: Option<usize>,
    /// Pacing, swapped out in tests.
    pub sleep: fn(Duration),
    last_call: BTreeMap<String, Instant>,
}

impl<'a> Runner<'a> {
    pub fn new(http: &'a dyn Http, out: PathBuf) -> Self {
        Self {
            http,
            out,
            prompts: PromptSet::builtin(),
            routes: RouteTable::builtin(),
            profile: LearnerProfile {
                indonesian_level: "BIPA 2".into(),
                known_words: 600,
                ..Default::default()
            },
            limit: None,
            sleep: std::thread::sleep,
            last_call: BTreeMap::new(),
        }
    }

    fn file(&self, engine: &Engine, task: Task) -> PathBuf {
        self.out.join(engine.label()).join(format!("{}.jsonl", task.as_str()))
    }

    /// Every item of one task through one engine.
    pub fn run(&mut self, root: &Path, task: Task, set: &GoldenSet, engine: &Engine) -> Result<Tally> {
        let mut tally = Tally::default();
        if task.is_personal() && engine.id.trains_on_inputs() {
            // Same rule as the router: personal tasks never reach this engine.
            return Ok(tally);
        }
        let template = self
            .prompts
            .get(task, self.routes.get(task).prompt_version)
            .with_context(|| format!("no prompt for {}", task.as_str()))?
            .clone();
        let version = template.version;
        let path = self.file(engine, task);
        let done: BTreeSet<(String, u32, String)> =
            read_records(&path)?.into_iter().map(|r| (r.item, r.prompt_version, r.input_hash)).collect();
        for item in set.items.iter().take(self.limit.unwrap_or(usize::MAX)) {
            let input_hash = json::input_hash(&item.input);
            if done.contains(&(item.id.clone(), version, input_hash.clone())) {
                tally.already += 1;
                continue;
            }
            let image = match load_image(root, item)? {
                Some(img) => Some(img),
                None if task.needs_vision() && item.tags.iter().any(|t| t == NEEDS_CAPTURE) => {
                    tally.needs_capture += 1;
                    continue;
                }
                None => None,
            };
            let (system, user) = template.render(&self.profile, &item.input, engine.id);
            let mut call = EngineCall {
                cache_key: CacheKey {
                    task,
                    input_hash: input_hash.clone(),
                    engine: engine.id,
                    prompt_version: version,
                },
                system,
                user,
                image_sha256: None,
                is_repair: false,
                json: true,
                max_tokens: self.routes.get(task).max_tokens,
                personal: task.is_personal(),
            };
            let record = match self.answer(task, item, engine, &mut call, image.as_ref()) {
                Ok(mut r) => {
                    r.input_hash = input_hash;
                    r.prompt_version = version;
                    r
                }
                Err(stop) => {
                    tally.stopped = Some(stop);
                    break;
                }
            };
            append(&path, &record)?;
            tally.answered += 1;
        }
        Ok(tally)
    }

    /// One item: a call, and the router's one repair retry if it fails the
    /// checks. `Err` means stop this engine (quota, unreachable).
    fn answer(
        &mut self,
        task: Task,
        item: &GoldenItem,
        engine: &Engine,
        call: &mut EngineCall,
        image: Option<&Image>,
    ) -> Result<Record, String> {
        let mut record = Record {
            item: item.id.clone(),
            task: task.as_str().into(),
            engine: engine.label(),
            model: engine.model.clone(),
            prompt_version: 0,
            input_hash: String::new(),
            first_ok: false,
            ok: false,
            error: None,
            latency_ms: 0,
            total_ms: 0,
            tokens_in: None,
            tokens_out: None,
            reply: String::new(),
            output: None,
        };
        for attempt in 0..2 {
            self.pace(engine);
            let started = Instant::now();
            let reply = engine.call(self.http, call, image);
            let ms = started.elapsed().as_millis() as u64;
            record.total_ms += ms;
            if attempt == 0 {
                record.latency_ms = ms;
            }
            let reply = match reply {
                Ok(r) => r,
                Err(CallError::Exhausted(why)) => return Err(why),
                Err(CallError::Failed(why)) => {
                    record.error = Some(why);
                    return Ok(record);
                }
            };
            record.model = reply.model;
            record.tokens_in = add(record.tokens_in, reply.tokens_in);
            record.tokens_out = add(record.tokens_out, reply.tokens_out);
            record.reply = reply.text;
            match check_reply(task, &record.reply, &item.input) {
                Ok(v) => {
                    record.first_ok = attempt == 0;
                    record.ok = true;
                    record.error = None;
                    record.output = Some(v);
                    return Ok(record);
                }
                Err(e) => {
                    call.is_repair = true;
                    call.user = repair_prompt(&call.user, &e, &record.reply);
                    record.error = Some(e);
                }
            }
        }
        Ok(record)
    }

    fn pace(&mut self, engine: &Engine) {
        let Some(n) = engine.per_minute() else { return };
        let gap = Duration::from_millis(60_000 / u64::from(n.max(1)) + 100);
        if let Some(last) = self.last_call.get(&engine.label()) {
            let since = last.elapsed();
            if since < gap {
                (self.sleep)(gap - since);
            }
        }
        self.last_call.insert(engine.label(), Instant::now());
    }
}

fn add(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (None, None) => None,
        _ => Some(a.unwrap_or(0) + b.unwrap_or(0)),
    }
}

fn load_image(root: &Path, item: &GoldenItem) -> Result<Option<Image>> {
    let Some(rel) = &item.image else { return Ok(None) };
    let path = root.join(rel);
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let mime = match path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => bail!("{}: only PNG and JPEG images", path.display()),
    };
    Ok(Some(Image { mime: mime.into(), base64: base64::engine::general_purpose::STANDARD.encode(bytes) }))
}

pub fn read_records(path: &Path) -> Result<Vec<Record>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    fs::read_to_string(path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).with_context(|| format!("{}: bad line", path.display())))
        .collect()
}

fn append(path: &Path, record: &Record) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", serde_json::to_string(record)?)?;
    Ok(())
}

/// Every record of a run, by task, from `<run>/<engine>/<task>.jsonl`. When
/// an item was answered more than once (a prompt changed), the last wins.
pub fn load_run(run: &Path) -> Result<BTreeMap<Task, Vec<Record>>> {
    let mut by_task: BTreeMap<Task, BTreeMap<(String, String), Record>> = BTreeMap::new();
    if !run.exists() {
        bail!("{} doesn't exist", run.display());
    }
    for dir in fs::read_dir(run)? {
        let dir = dir?.path();
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        if !dir.is_dir() || matches!(name.as_str(), "blind" | "scores") {
            continue;
        }
        for task in Task::ALL {
            for r in read_records(&dir.join(format!("{}.jsonl", task.as_str())))? {
                by_task.entry(task).or_default().insert((r.item.clone(), r.engine.clone()), r);
            }
        }
    }
    Ok(by_task.into_iter().map(|(t, m)| (t, m.into_values().collect())).collect())
}

/// A stable shuffle key, so re-running `blind` keeps the same letters.
fn mix(s: &str) -> u64 {
    // FNV-1a.
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyEntry {
    pub task: String,
    pub item: String,
    pub letter: String,
    pub engine: String,
}

/// Writes the judge's files; returns how many answers are in them. Answers
/// that failed the checks aren't judged: they count against the engine's
/// valid-JSON rate instead, and the app would never show them.
pub fn blind(root: &Root, run: &Path) -> Result<usize> {
    let out = run.join("blind");
    fs::create_dir_all(&out)?;
    let mut keys = Vec::new();
    let mut n = 0;
    for (task, records) in load_run(run)? {
        let set = golden::load(&golden::dir(root).join(format!("{}.yaml", task.as_str())))?;
        let mut by_item: BTreeMap<&str, Vec<&Record>> = BTreeMap::new();
        for r in records.iter().filter(|r| r.ok) {
            by_item.entry(r.item.as_str()).or_default().push(r);
        }
        let mut lines = String::new();
        for item in &set.items {
            let Some(rs) = by_item.get_mut(item.id.as_str()) else { continue };
            rs.sort_by_key(|r| mix(&format!("{}:{}", item.id, r.engine)));
            for (i, r) in rs.iter().enumerate() {
                let letter = char::from(b'A' + i as u8).to_string();
                let line = serde_json::json!({
                    "item": item.id,
                    "letter": letter,
                    "input": item.input,
                    "expect": item.expect,
                    "criteria": item.criteria,
                    "avoid": item.avoid,
                    "answer": r.output,
                });
                lines.push_str(&serde_json::to_string(&line)?);
                lines.push('\n');
                keys.push(KeyEntry {
                    task: task.as_str().into(),
                    item: item.id.clone(),
                    letter,
                    engine: r.engine.clone(),
                });
                n += 1;
            }
        }
        fs::write(out.join(format!("{}.jsonl", task.as_str())), lines)?;
        fs::write(
            out.join(format!("{}.rubric.md", task.as_str())),
            format!(
                "# {} — judging rubric\n\nScore each answer 0–3 (docs/BAKEOFF.md). 0 = wrong or harmful, \
                 1 = poor, 2 = acceptable, 3 = what a careful teacher would write.\n\n{}\n\nWrite one line per \
                 answer to `scores/{}.jsonl`: {{\"item\": …, \"letter\": …, \"score\": 0-3, \"note\": \"why\"}}\n",
                task.as_str(),
                set.rubric.trim(),
                task.as_str()
            ),
        )?;
    }
    let mut key = String::new();
    for k in &keys {
        key.push_str(&serde_json::to_string(k)?);
        key.push('\n');
    }
    fs::write(run.join("key.jsonl"), key)?;
    Ok(n)
}

#[cfg(test)]
mod tests;
