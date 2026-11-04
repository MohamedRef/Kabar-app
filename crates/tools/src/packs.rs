//! Offline packs per unit (SPEC §20, PLAN §7–8; `docs/RUN.md` §12).
//!
//! - `packs build` lists every line of every unit that gets audio and writes
//!   the manifests (`content/packs/<lang>/<unit>.json`, committed) and
//!   `content/packs/index.json`, with the TTS characters per unit.
//! - `packs synthesize` fills a local clip cache from Azure neural TTS (key
//!   from the environment, never committed) inside a monthly character
//!   budget, and records each clip's hash and size in the manifests.
//! - `packs bundle` writes `dist/packs/<unit>.tar` + `index.json` for a
//!   GitHub release (`scripts/packs-upload.sh`).
//!
//! The selection, keys and budget maths live in `kabar_core::packs`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Subcommand};
use kabar_core::journey::parse_curriculum;
use kabar_core::lesson::parse_lesson;
use kabar_core::packs::{
    self, Budget, DEFAULT_MONTHLY_BUDGET, FREE_MONTHLY_CHARS, PackInput, build_manifest, sha256_hex,
};
use kabar_protocol::lesson::LessonContent;
use kabar_protocol::pack::{
    AUDIO_EXT, AUDIO_OUTPUT_FORMAT, AudioEntry, PACK_FORMAT, PackIndex, PackManifest,
};
use serde::{Deserialize, Serialize};

use crate::paths::Root;

#[derive(Subcommand)]
pub enum Cmd {
    /// Write the unit manifests and the index from `content/lessons/**`, and
    /// print the TTS characters per unit.
    Build {
        /// Fail instead of writing when a manifest is out of date.
        #[arg(long)]
        check: bool,
    },
    /// Characters, clips and synthesized audio per unit (reads the manifests).
    Stats,
    /// Generate the missing clips with Azure TTS (`AZURE_SPEECH_KEY`,
    /// `AZURE_SPEECH_REGION`) into the clip cache, inside the monthly budget.
    Synthesize(SynthArgs),
    /// Write `dist/packs/<unit>.tar` for every fully synthesized unit, plus
    /// `index.json`, ready for `scripts/packs-upload.sh`.
    Bundle(BundleArgs),
}

#[derive(Args)]
pub struct SynthArgs {
    /// Units to synthesize, in order (e.g. `bipa1.u01,bipa1.u02`). Default: all.
    #[arg(long, value_delimiter = ',')]
    units: Vec<String>,
    /// Most characters this tool may spend per calendar month (UTC). Azure F0
    /// gives 500,000; keep some for Kabar Cloud's TTS proxy.
    #[arg(long, default_value_t = DEFAULT_MONTHLY_BUDGET)]
    budget: u64,
    /// Take whole units in order until the budget is used, instead of refusing.
    #[arg(long)]
    fit: bool,
    /// Only print what would be synthesized and what it costs.
    #[arg(long)]
    dry_run: bool,
    /// Clip cache (default `$KABAR_AUDIO_CACHE`, else `~/.kabar/audio`).
    #[arg(long)]
    cache: Option<PathBuf>,
}

#[derive(Args)]
pub struct BundleArgs {
    /// Units to bundle. Default: every fully synthesized unit.
    #[arg(long, value_delimiter = ',')]
    units: Vec<String>,
    /// Output directory.
    #[arg(long, default_value = "dist/packs")]
    out: PathBuf,
    /// Clip cache (default `$KABAR_AUDIO_CACHE`, else `~/.kabar/audio`).
    #[arg(long)]
    cache: Option<PathBuf>,
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Build { check } => build(root, check),
        Cmd::Stats => {
            let manifests = load_manifests(root)?;
            print_stats(&manifests);
            Ok(())
        }
        Cmd::Synthesize(args) => synthesize(root, args),
        Cmd::Bundle(args) => bundle(root, args),
    }
}

// --- paths -------------------------------------------------------------------

fn packs_dir(root: &Root) -> PathBuf {
    root.path().join("content/packs")
}

fn manifest_path(root: &Root, language: &str, unit: &str) -> PathBuf {
    packs_dir(root).join(language).join(format!("{unit}.json"))
}

fn index_path(root: &Root) -> PathBuf {
    packs_dir(root).join("index.json")
}

fn cache_dir(arg: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = arg.or_else(|| std::env::var_os("KABAR_AUDIO_CACHE").map(PathBuf::from)) {
        return Ok(p);
    }
    let home = std::env::var_os("HOME").context("set HOME or pass --cache")?;
    Ok(PathBuf::from(home).join(".kabar/audio"))
}

fn clip_path(cache: &Path, key: &str) -> PathBuf {
    cache.join(format!("{key}.{AUDIO_EXT}"))
}

fn to_json(v: &impl Serialize) -> Result<String> {
    Ok(serde_json::to_string_pretty(v)? + "\n")
}

// --- build -----------------------------------------------------------------------

/// Every lesson file under `content/lessons/`, by unit, in id order.
/// (language, unit) → its lessons as (id, YAML hash, content).
type Units = BTreeMap<(String, String), Vec<(String, String, LessonContent)>>;

fn lessons_by_unit(root: &Root) -> Result<Units> {
    let mut files = Vec::new();
    collect_yaml(&root.path().join("content/lessons"), &mut files)?;
    files.sort();
    let mut out: BTreeMap<(String, String), Vec<_>> = BTreeMap::new();
    for f in files {
        let yaml = fs::read_to_string(&f).with_context(|| f.display().to_string())?;
        let lesson = parse_lesson(&yaml).map_err(|e| anyhow::anyhow!("{}: {e}", f.display()))?;
        let unit = packs::unit_of(&lesson.id).to_owned();
        out.entry((lesson.language.clone(), unit)).or_default().push((
            lesson.id.clone(),
            sha256_hex(yaml.as_bytes()),
            lesson,
        ));
    }
    Ok(out)
}

fn collect_yaml(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| dir.display().to_string())? {
        let path = entry?.path();
        if path.is_dir() {
            collect_yaml(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "yaml") {
            out.push(path);
        }
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Option<PackManifest>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(serde_json::from_str(&s).with_context(|| path.display().to_string())?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| path.display().to_string()),
    }
}

fn read_index(root: &Root) -> Result<PackIndex> {
    match fs::read_to_string(index_path(root)) {
        Ok(s) => Ok(serde_json::from_str(&s)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(PackIndex { format: PACK_FORMAT, packs: vec![] })
        }
        Err(e) => Err(e.into()),
    }
}

/// Order of the index (and of "next unit"): Indonesian then Javanese, units
/// in curriculum order (`bipa1.u01` < `bipa1.u02` < `bipa2.u01`).
fn unit_order(a: &(String, String), b: &(String, String)) -> std::cmp::Ordering {
    let lang = |l: &str| if l == "id" { 0 } else { 1 };
    lang(&a.0).cmp(&lang(&b.0)).then_with(|| a.1.cmp(&b.1))
}

/// The manifests and index `content/lessons/**` gives, built on top of the
/// current ones (so synthesized hashes and versions carry over).
fn derive(root: &Root) -> Result<(Vec<(PathBuf, PackManifest)>, PackIndex)> {
    let units = lessons_by_unit(root)?;
    let mut keys: Vec<_> = units.keys().cloned().collect();
    keys.sort_by(unit_order);
    let old_index = read_index(root)?;
    let stops = unit_stops(root)?;
    let mut manifests = Vec::new();
    let mut index = PackIndex { format: PACK_FORMAT, packs: Vec::new() };
    for key in keys {
        let (language, unit) = &key;
        let lessons = &units[&key];
        let inputs: Vec<PackInput<'_>> = lessons
            .iter()
            .map(|(id, sha, lesson)| PackInput { id, yaml_sha256: sha.clone(), lesson })
            .collect();
        let path = manifest_path(root, language, unit);
        let previous = read_manifest(&path)?;
        let m = build_manifest(language, unit, &inputs, previous.as_ref());
        let mut row = packs::summary(&m, old_index.get(&m.id));
        row.stop = stops.get(unit.as_str()).cloned();
        index.packs.push(row);
        manifests.push((path, m));
    }
    Ok((manifests, index))
}

/// Unit id → its town or station, from `curriculum/*.yaml`.
fn unit_stops(root: &Root) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for file in ["indonesian.yaml", "javanese.yaml"] {
        let path = root.path().join("curriculum").join(file);
        let yaml = fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        let c = parse_curriculum(&yaml).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        for level in &c.levels {
            for unit in &level.units {
                out.insert(unit.id.clone(), unit.stop.clone());
            }
        }
    }
    Ok(out)
}

fn build(root: &Root, check: bool) -> Result<()> {
    let (manifests, index) = derive(root)?;
    let mut stale = Vec::new();
    for (path, m) in &manifests {
        let json = to_json(m)?;
        if fs::read_to_string(path).ok().as_deref() != Some(json.as_str()) {
            stale.push(path.clone());
            if !check {
                fs::create_dir_all(path.parent().expect("manifest dir"))?;
                fs::write(path, json)?;
            }
        }
    }
    let json = to_json(&index)?;
    if fs::read_to_string(index_path(root)).ok().as_deref() != Some(json.as_str()) {
        stale.push(index_path(root));
        if !check {
            fs::write(index_path(root), json)?;
        }
    }
    let all: Vec<PackManifest> = manifests.into_iter().map(|(_, m)| m).collect();
    print_stats(&all);
    if check {
        for p in &stale {
            println!("out of date: {}", p.display());
        }
        ensure!(stale.is_empty(), "{} pack file(s) out of date: run `kabar-tools packs build`", stale.len());
    } else {
        println!("{} file(s) written", stale.len());
    }
    Ok(())
}

fn load_manifests(root: &Root) -> Result<Vec<PackManifest>> {
    let index = read_index(root)?;
    let mut out = Vec::new();
    for p in &index.packs {
        let path = manifest_path(root, &p.language, &p.unit);
        out.push(
            read_manifest(&path)?
                .with_context(|| format!("{} missing: run `packs build`", path.display()))?,
        );
    }
    Ok(out)
}

fn print_stats(manifests: &[PackManifest]) {
    println!(
        "{:<12} {:>7} {:>6} {:>9} {:>8} {:>10}",
        "unit", "lessons", "clips", "chars", "missing", "audio KB"
    );
    let mut level: BTreeMap<String, (u64, u32)> = BTreeMap::new();
    let (mut chars, mut clips) = (0u64, 0u32);
    let mut seen = BTreeSet::new();
    let mut unique_chars = 0u64;
    for m in manifests {
        let t = &m.totals;
        println!(
            "{:<12} {:>7} {:>6} {:>9} {:>8} {:>10}",
            m.unit,
            t.lessons,
            t.clips,
            t.chars,
            t.missing,
            t.audio_bytes / 1024
        );
        let lv = level.entry(m.unit.split('.').next().unwrap_or_default().to_owned()).or_default();
        lv.0 += t.chars;
        lv.1 += t.clips;
        chars += t.chars;
        clips += t.clips;
        for a in &m.audio {
            if seen.insert(a.key.as_str()) {
                unique_chars += u64::from(a.chars);
            }
        }
    }
    println!();
    for (lv, (c, n)) in &level {
        println!("{lv:<12} {n:>6} clips {c:>9} chars");
    }
    println!(
        "total        {clips:>6} clips {chars:>9} chars ({unique_chars} after dedupe across units; Azure F0 = {FREE_MONTHLY_CHARS}/month)"
    );
    println!(
        "a first full run takes {} month(s) at the default --budget {DEFAULT_MONTHLY_BUDGET}; later runs only pay for changed lines",
        unique_chars.div_ceil(DEFAULT_MONTHLY_BUDGET).max(1)
    );
}

// --- synthesize --------------------------------------------------------------------

/// Characters this tool spent per month, kept next to the clips.
#[derive(Default, Serialize, Deserialize)]
struct Usage {
    months: BTreeMap<String, u64>,
}

fn usage_path(cache: &Path) -> PathBuf {
    cache.join("usage.json")
}

fn read_usage(cache: &Path) -> Result<Usage> {
    match fs::read_to_string(usage_path(cache)) {
        Ok(s) => Ok(serde_json::from_str(&s)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Usage::default()),
        Err(e) => Err(e.into()),
    }
}

fn this_month() -> String {
    packs::month_of(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default())
}

fn cached_keys(cache: &Path) -> BTreeSet<String> {
    let Ok(entries) = fs::read_dir(cache) else { return BTreeSet::new() };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let key = name.strip_suffix(&format!(".{AUDIO_EXT}"))?;
            packs::is_hex_key(key).then(|| key.to_owned())
        })
        .collect()
}

/// Records the hash and size of every cached clip in the manifests, then
/// rewrites them and the index.
fn record_cached(root: &Root, cache: &Path) -> Result<()> {
    let cached = cached_keys(cache);
    let old_index = read_index(root)?;
    let mut index = PackIndex { format: PACK_FORMAT, packs: Vec::new() };
    for mut m in load_manifests(root)? {
        for a in &mut m.audio {
            if a.sha256.is_none() && cached.contains(&a.key) {
                let bytes = fs::read(clip_path(cache, &a.key))?;
                a.sha256 = Some(sha256_hex(&bytes));
                a.size = Some(bytes.len() as u64);
            }
        }
        packs::refresh_totals(&mut m);
        fs::write(manifest_path(root, &m.language, &m.unit), to_json(&m)?)?;
        index.packs.push(packs::summary(&m, old_index.get(&m.id)));
    }
    fs::write(index_path(root), to_json(&index)?)?;
    Ok(())
}

fn select<'a>(manifests: &'a [PackManifest], units: &[String]) -> Result<Vec<&'a PackManifest>> {
    if units.is_empty() {
        return Ok(manifests.iter().collect());
    }
    units
        .iter()
        .map(|u| manifests.iter().find(|m| &m.unit == u).with_context(|| format!("no pack for unit {u}")))
        .collect()
}

/// One TTS request: (HTTP status, body) for a clip's SSML.
type Speak<'a> = Box<dyn FnMut(&AudioEntry) -> Result<(u16, Vec<u8>)> + 'a>;

/// How long to wait between requests, and after a 429 (times the try).
struct Pace {
    gap: Duration,
    backoff: Duration,
}

/// F0 allows 20 requests a minute: one every 3.1 s stays under it.
const AZURE_PACE: Pace = Pace { gap: Duration::from_millis(3_100), backoff: Duration::from_secs(15) };

fn synthesize(root: &Root, args: SynthArgs) -> Result<()> {
    synthesize_with(root, args, azure, AZURE_PACE)
}

/// Azure neural TTS, with the key and region from the environment.
fn azure() -> Result<Speak<'static>> {
    let var = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    let key =
        var("AZURE_SPEECH_KEY").context("set AZURE_SPEECH_KEY in .env (git-ignored; never commit it)")?;
    let region = var("AZURE_SPEECH_REGION").context("set AZURE_SPEECH_REGION in .env, e.g. southeastasia")?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .into();
    let url = format!("https://{region}.tts.speech.microsoft.com/cognitiveservices/v1");
    Ok(Box::new(move |entry: &AudioEntry| {
        let res = agent
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &key)
            .header("Content-Type", "application/ssml+xml")
            .header("X-Microsoft-OutputFormat", AUDIO_OUTPUT_FORMAT)
            .header("User-Agent", "kabar-tools")
            .send(packs::clip_ssml(entry.voice, &entry.text));
        let mut res = res.map_err(|e| anyhow::anyhow!("Azure TTS: {e}"))?;
        let status = res.status().as_u16();
        let mut buf = Vec::new();
        if status == 200 {
            res.body_mut().as_reader().read_to_end(&mut buf)?;
        } else {
            buf = res.body_mut().read_to_string().unwrap_or_default().into_bytes();
        }
        Ok((status, buf))
    }))
}

fn synthesize_with<'a>(
    root: &Root,
    args: SynthArgs,
    connect: impl FnOnce() -> Result<Speak<'a>>,
    pace: Pace,
) -> Result<()> {
    let cache = cache_dir(args.cache)?;
    let manifests = load_manifests(root)?;
    let mut chosen = select(&manifests, &args.units)?;
    let cached = cached_keys(&cache);
    let month = this_month();
    let usage = read_usage(&cache)?;
    let budget = Budget { used: usage.months.get(&month).copied().unwrap_or(0), limit: args.budget };
    ensure!(
        args.budget <= FREE_MONTHLY_CHARS,
        "--budget is above Azure F0's {FREE_MONTHLY_CHARS} characters a month"
    );

    let (_, mut chars) = packs::to_synthesize(chosen.iter().copied(), &cached);
    if budget.check(chars).is_err() && args.fit {
        // Whole units, in order, while they fit.
        let mut fit = Vec::new();
        for m in &chosen {
            let mut next = fit.clone();
            next.push(*m);
            if budget.check(packs::to_synthesize(next.iter().copied(), &cached).1).is_err() {
                break;
            }
            fit = next;
        }
        chosen = fit;
        chars = packs::to_synthesize(chosen.iter().copied(), &cached).1;
    }
    let (todo, _) = packs::to_synthesize(chosen.iter().copied(), &cached);
    println!(
        "{month}: {} used of {} → {} clip(s), {chars} characters to synthesize for {}",
        budget.used,
        budget.limit,
        todo.len(),
        chosen.iter().map(|m| m.unit.as_str()).collect::<Vec<_>>().join(", ")
    );
    if let Err(over) = budget.check(chars) {
        bail!(
            "that needs {} characters but only {} are left this month (budget {}). \
             Pick fewer --units, pass --fit, or wait for next month.",
            over.needed,
            over.left,
            budget.limit
        );
    }
    if args.dry_run || todo.is_empty() {
        if !args.dry_run {
            record_cached(root, &cache)?;
        }
        return Ok(());
    }

    let mut speak = connect()?;
    fs::create_dir_all(&cache)?;
    let mut usage = usage;
    let total = todo.len();
    let mut last = Instant::now().checked_sub(pace.gap).unwrap_or_else(Instant::now);
    for (i, entry) in todo.into_iter().enumerate() {
        let mut tries = 0;
        let bytes = loop {
            if let Some(wait) = pace.gap.checked_sub(last.elapsed()) {
                std::thread::sleep(wait);
            }
            last = Instant::now();
            let (status, body) = speak(entry)?;
            if status == 200 {
                break body;
            }
            tries += 1;
            if status == 429 && tries < 5 {
                std::thread::sleep(pace.backoff * tries);
                continue;
            }
            // Keep what's done: hashes of the clips so far go into the manifests.
            record_cached(root, &cache)?;
            let body = String::from_utf8_lossy(&body);
            bail!("Azure TTS answered {status}: {}", body.chars().take(200).collect::<String>());
        };
        ensure!(bytes.len() > 100, "Azure TTS returned an empty clip for {:?}", entry.text);
        let tmp = cache.join(format!("{}.part", entry.key));
        fs::write(&tmp, &bytes)?;
        fs::rename(&tmp, clip_path(&cache, &entry.key))?;
        *usage.months.entry(month.clone()).or_default() += u64::from(entry.chars);
        fs::write(usage_path(&cache), to_json(&usage)?)?;
        println!("[{}/{total}] {:?} {}", i + 1, entry.voice, entry.text);
    }
    record_cached(root, &cache)?;
    println!("done; this month: {} characters", usage.months.get(&month).copied().unwrap_or(0));
    Ok(())
}

// --- bundle --------------------------------------------------------------------------

/// A tar with fixed metadata, so the same pack always has the same hash.
fn tar_bytes(m: &PackManifest, cache: &Path) -> Result<Vec<u8>> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut add = |name: &str, data: &[u8]| -> Result<()> {
        let mut h = tar::Header::new_ustar();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_mtime(0);
        h.set_uid(0);
        h.set_gid(0);
        h.set_entry_type(tar::EntryType::Regular);
        builder.append_data(&mut h, name, data)?;
        Ok(())
    };
    add("manifest.json", to_json(m)?.as_bytes())?;
    for a in &m.audio {
        let bytes =
            fs::read(clip_path(cache, &a.key)).with_context(|| format!("clip {} not in the cache", a.key))?;
        ensure!(
            a.sha256.as_deref() == Some(sha256_hex(&bytes).as_str()),
            "clip {} doesn't match its manifest hash",
            a.key
        );
        add(&format!("audio/{}.{AUDIO_EXT}", a.key), &bytes)?;
    }
    Ok(builder.into_inner()?)
}

fn bundle(root: &Root, args: BundleArgs) -> Result<()> {
    let cache = cache_dir(args.cache)?;
    record_cached(root, &cache)?;
    let manifests = load_manifests(root)?;
    let chosen = select(&manifests, &args.units)?;
    let out = if args.out.is_absolute() { args.out } else { root.path().join(args.out) };
    fs::create_dir_all(&out)?;
    let mut index = read_index(root)?;
    let mut bundled = 0;
    for m in chosen {
        if m.totals.missing > 0 || m.audio.is_empty() {
            if !args.units.is_empty() {
                bail!("{}: {} clip(s) not synthesized yet", m.unit, m.totals.missing);
            }
            continue;
        }
        let bytes = tar_bytes(m, &cache)?;
        let row =
            index.packs.iter_mut().find(|p| p.id == m.id).context("index out of date: run `packs build`")?;
        fs::write(out.join(&row.file), &bytes)?;
        row.file_sha256 = Some(sha256_hex(&bytes));
        row.file_size = Some(bytes.len() as u64);
        bundled += 1;
        println!("{} → {} ({} KB)", m.unit, row.file, bytes.len() / 1024);
    }
    let json = to_json(&index)?;
    fs::write(index_path(root), &json)?;
    fs::write(out.join("index.json"), &json)?;
    println!(
        "{bundled} pack(s) in {}. Commit content/packs/, then upload with scripts/packs-upload.sh",
        out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testkit::Sandbox;
    use std::cell::Cell;

    /// The first unit of each track, with its lessons and the curriculum.
    fn tree() -> Sandbox {
        let s = Sandbox::with(&["curriculum"]);
        s.copy_matching("content/lessons/id", &["bipa1.u01."]);
        s.copy_matching("content/lessons/jv", &["j1.u01."]);
        s
    }

    fn manifest(s: &Sandbox, lang: &str, unit: &str) -> PackManifest {
        serde_json::from_str(&s.read(&format!("content/packs/{lang}/{unit}.json"))).unwrap()
    }

    fn index(s: &Sandbox) -> PackIndex {
        serde_json::from_str(&s.read("content/packs/index.json")).unwrap()
    }

    fn synth(cache: &Path, units: &[&str], budget: u64) -> SynthArgs {
        SynthArgs {
            units: units.iter().map(|u| u.to_string()).collect(),
            budget,
            fit: false,
            dry_run: false,
            cache: Some(cache.to_path_buf()),
        }
    }

    const NOW: Pace = Pace { gap: Duration::ZERO, backoff: Duration::ZERO };

    fn clip(entry: &AudioEntry) -> Vec<u8> {
        format!("{:<200}", entry.key).into_bytes()
    }

    #[test]
    fn build_writes_manifests_and_check_finds_them_stale() {
        let s = tree();
        let err = s.cli(&["packs", "build", "--check"]).unwrap_err();
        assert!(err.to_string().contains("3 pack file(s) out of date"), "{err}");
        assert!(!s.exists("content/packs/index.json"), "--check never writes");

        s.cli(&["packs", "build"]).unwrap();
        s.cli(&["packs", "build", "--check"]).unwrap();
        s.cli(&["packs", "stats"]).unwrap();
        let m = manifest(&s, "id", "bipa1.u01");
        assert_eq!((m.id.as_str(), m.version), ("id/bipa1.u01", 1));
        assert_eq!(m.totals.missing, m.totals.clips, "nothing synthesized yet");
        assert!(m.totals.clips > 10 && m.audio.iter().all(|a| a.sha256.is_none()));
        let index = index(&s);
        let units: Vec<&str> = index.packs.iter().map(|p| p.unit.as_str()).collect();
        assert_eq!(units, ["bipa1.u01", "j1.u01"], "Indonesian first, then Javanese");
        assert!(index.packs.iter().all(|p| p.stop.as_deref().is_some_and(|s| !s.is_empty())), "towns");

        // Edited lesson text bumps the version; rebuilding unchanged content doesn't.
        let lesson =
            fs::read_dir(s.path().join("content/lessons/id")).unwrap().next().unwrap().unwrap().path();
        let yaml = fs::read_to_string(&lesson).unwrap();
        fs::write(&lesson, format!("{yaml}\n# edited\n")).unwrap();
        s.cli(&["packs", "build"]).unwrap();
        assert_eq!(manifest(&s, "id", "bipa1.u01").version, 2);
        assert_eq!(manifest(&s, "jv", "j1.u01").version, 1);
    }

    #[test]
    fn stats_need_built_manifests() {
        let s = tree();
        s.write("content/packs/index.json", r#"{"format":1,"packs":[{"id":"id/bipa1.u01","language":"id","unit":"bipa1.u01","version":1,"content_hash":"x","lessons":1,"clips":1,"chars":1,"audio_bytes":0,"missing":1,"file":"x.tar"}]}"#);
        let err = s.cli(&["packs", "stats"]).unwrap_err();
        assert!(format!("{err:#}").contains("missing: run `packs build`"), "{err:#}");
        s.write("content/packs/index.json", "not json");
        assert!(s.cli(&["packs", "stats"]).is_err());
    }

    #[test]
    fn synthesize_stays_inside_the_budget() {
        let s = tree();
        s.cli(&["packs", "build"]).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let chars = manifest(&s, "id", "bipa1.u01").totals.chars;
        let never = || -> Result<Speak<'static>> { panic!("no request may be sent") };

        let err = synthesize_with(&s.root(), synth(cache.path(), &[], FREE_MONTHLY_CHARS + 1), never, NOW)
            .unwrap_err();
        assert!(err.to_string().contains("above Azure F0"), "{err}");
        let err = synthesize_with(&s.root(), synth(cache.path(), &["bipa1.u01"], chars - 1), never, NOW)
            .unwrap_err();
        assert!(err.to_string().contains("Pick fewer --units, pass --fit"), "{err}");
        let err =
            synthesize_with(&s.root(), synth(cache.path(), &["bipa9.u01"], chars), never, NOW).unwrap_err();
        assert!(err.to_string().contains("no pack for unit bipa9.u01"), "{err}");
        // --fit takes whole units while they fit: none here.
        let args = SynthArgs { fit: true, ..synth(cache.path(), &[], chars - 1) };
        synthesize_with(&s.root(), args, never, NOW).unwrap();
        // A dry run prices the work and sends nothing.
        let path = cache.path().display().to_string();
        s.cli(&["packs", "synthesize", "--dry-run", "--units", "bipa1.u01", "--cache", &path]).unwrap();
        assert!(fs::read_dir(cache.path()).unwrap().next().is_none(), "the cache stays empty");
    }

    #[test]
    fn synthesize_fills_the_cache_retries_429_and_records_hashes() {
        let s = tree();
        s.cli(&["packs", "build"]).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let m = manifest(&s, "id", "bipa1.u01");
        let calls = Cell::new(0);
        let speak = || -> Result<Speak<'_>> {
            Ok(Box::new(|e: &AudioEntry| {
                calls.set(calls.get() + 1);
                Ok(if calls.get() == 2 { (429, b"slow down".to_vec()) } else { (200, clip(e)) })
            }))
        };
        synthesize_with(&s.root(), synth(cache.path(), &["bipa1.u01"], m.totals.chars), speak, NOW).unwrap();
        assert_eq!(calls.get() as u32, m.totals.clips + 1, "one retry after the 429");
        let done = manifest(&s, "id", "bipa1.u01");
        assert_eq!(done.totals.missing, 0);
        for a in &done.audio {
            let bytes = fs::read(clip_path(cache.path(), &a.key)).unwrap();
            assert_eq!(a.sha256.as_deref(), Some(sha256_hex(&bytes).as_str()));
            assert_eq!(a.size, Some(bytes.len() as u64));
        }
        let usage = read_usage(cache.path()).unwrap();
        assert_eq!(usage.months.get(&this_month()), Some(&m.totals.chars), "spent characters are counted");
        assert_eq!(manifest(&s, "jv", "j1.u01").totals.missing, manifest(&s, "jv", "j1.u01").totals.clips);

        // Everything cached: a second run records and sends nothing.
        let never = || -> Result<Speak<'static>> { panic!("nothing left to synthesize") };
        synthesize_with(&s.root(), synth(cache.path(), &["bipa1.u01"], m.totals.chars), never, NOW).unwrap();
    }

    #[test]
    fn a_failed_request_keeps_the_clips_already_made() {
        let s = tree();
        s.cli(&["packs", "build"]).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let chars = manifest(&s, "id", "bipa1.u01").totals.chars;
        let calls = Cell::new(0);
        let speak = || -> Result<Speak<'_>> {
            Ok(Box::new(|e: &AudioEntry| {
                calls.set(calls.get() + 1);
                Ok(if calls.get() < 3 { (200, clip(e)) } else { (500, b"boom".to_vec()) })
            }))
        };
        let err =
            synthesize_with(&s.root(), synth(cache.path(), &["bipa1.u01"], chars), speak, NOW).unwrap_err();
        assert_eq!(err.to_string(), "Azure TTS answered 500: boom");
        let m = manifest(&s, "id", "bipa1.u01");
        assert_eq!(
            m.audio.iter().filter(|a| a.sha256.is_some()).count(),
            2,
            "the two clips made are recorded"
        );

        let short =
            || -> Result<Speak<'static>> { Ok(Box::new(|_: &AudioEntry| Ok((200, b"tiny".to_vec())))) };
        let err =
            synthesize_with(&s.root(), synth(cache.path(), &["bipa1.u01"], chars), short, NOW).unwrap_err();
        assert!(err.to_string().contains("empty clip"), "{err}");

        let offline = || -> Result<Speak<'static>> { anyhow::bail!("set AZURE_SPEECH_KEY") };
        let err =
            synthesize_with(&s.root(), synth(cache.path(), &["bipa1.u01"], chars), offline, NOW).unwrap_err();
        assert!(err.to_string().contains("AZURE_SPEECH_KEY"));
    }

    #[test]
    fn bundle_packs_only_fully_synthesized_units() {
        let s = tree();
        s.cli(&["packs", "build"]).unwrap();
        let cache = tempfile::tempdir().unwrap();
        // Clips for the Indonesian unit only, straight into the cache.
        for a in &manifest(&s, "id", "bipa1.u01").audio {
            fs::write(clip_path(cache.path(), &a.key), clip(a)).unwrap();
        }
        fs::write(cache.path().join("notes.txt"), "not a clip").unwrap();
        let out = tempfile::tempdir().unwrap();
        let (c, o) = (cache.path().display().to_string(), out.path().display().to_string());
        let err = s.cli(&["packs", "bundle", "--units", "j1.u01", "--cache", &c, "--out", &o]).unwrap_err();
        assert!(
            err.to_string().contains("j1.u01:") && err.to_string().contains("not synthesized yet"),
            "{err}"
        );

        s.cli(&["packs", "bundle", "--cache", &c, "--out", &o]).unwrap();
        let index = index(&s);
        let row = index.packs.iter().find(|p| p.unit == "bipa1.u01").unwrap();
        let tar = fs::read(out.path().join(&row.file)).unwrap();
        assert_eq!(row.file_sha256.as_deref(), Some(sha256_hex(&tar).as_str()));
        assert_eq!(row.file_size, Some(tar.len() as u64));
        let jv = index.packs.iter().find(|p| p.unit == "j1.u01").unwrap();
        assert!(jv.file_sha256.is_none() && !out.path().join(&jv.file).exists(), "skipped, not failed");
        assert_eq!(
            fs::read_to_string(out.path().join("index.json")).unwrap(),
            s.read("content/packs/index.json")
        );

        // A relative --out lands under the root.
        s.cli(&["packs", "bundle", "--units", "bipa1.u01", "--cache", &c, "--out", "dist/p"]).unwrap();
        assert!(s.path().join("dist/p").join(&row.file).exists());
    }

    #[test]
    fn the_cache_defaults_to_the_environment_then_home() {
        assert_eq!(cache_dir(Some(PathBuf::from("/x"))).unwrap(), PathBuf::from("/x"));
        let default = cache_dir(None).unwrap();
        assert!(
            std::env::var_os("KABAR_AUDIO_CACHE").is_some() || default.ends_with(".kabar/audio"),
            "{}",
            default.display()
        );
        assert_eq!(
            unit_order(&("jv".into(), "j1.u01".into()), &("id".into(), "bipa2.u01".into())),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn bundles_are_reproducible_and_checked() {
        let dir = std::env::temp_dir().join(format!("kabar-packs-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let root = Root::new(None);
        let units = lessons_by_unit(&root).unwrap();
        let ((language, unit), lessons) = units.iter().next().unwrap();
        let inputs: Vec<PackInput<'_>> = lessons
            .iter()
            .map(|(id, sha, l)| PackInput { id, yaml_sha256: sha.clone(), lesson: l })
            .collect();
        let mut m = build_manifest(language, unit, &inputs, None);
        for (i, a) in m.audio.iter_mut().enumerate() {
            let bytes = format!("fake mp3 {i}").into_bytes();
            fs::write(clip_path(&dir, &a.key), &bytes).unwrap();
            a.sha256 = Some(sha256_hex(&bytes));
            a.size = Some(bytes.len() as u64);
        }
        packs::refresh_totals(&mut m);
        let a = tar_bytes(&m, &dir).unwrap();
        assert_eq!(a, tar_bytes(&m, &dir).unwrap());
        let mut archive = tar::Archive::new(a.as_slice());
        let names: Vec<String> =
            archive.entries().unwrap().map(|e| e.unwrap().path().unwrap().display().to_string()).collect();
        assert_eq!(names[0], "manifest.json");
        assert_eq!(names.len(), m.audio.len() + 1);
        // A tampered clip is refused.
        fs::write(clip_path(&dir, &m.audio[0].key), b"other").unwrap();
        assert!(tar_bytes(&m, &dir).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
