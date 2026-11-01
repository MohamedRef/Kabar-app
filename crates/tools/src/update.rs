//! App update feeds (`docs/RELEASE.md` §5): the signing key, `update.json`
//! and its signature. The checks the app runs live in `kabar_core::update`.
//!
//! - `update keygen` makes the Ed25519 key pair: the public half goes into
//!   the app (`crates/app/update-key.pub`, committed), the secret half is
//!   printed once for the Keychain (`just update-keygen`) and never written
//!   into the repository.
//! - `update manifest` hashes the builds and writes `update.json` next to
//!   copies of them, ready to upload.
//! - `update sign` signs `update.json` with the secret from
//!   `KABAR_UPDATE_SECRET` and checks the result against the public key.
//! - `update verify` runs the app's checks on a feed folder.
//! - `update changes` shows what the next build's "what's new" would say
//!   (`crate::changes`).
//! - `update bundle` packs the app's content and stylesheets into the
//!   content bundle (`kabar_protocol::bundle`) that `manifest --content`
//!   publishes, so content changes reach devices with no install.
//! - `manifest --carry PLATFORM` keeps that platform's file from
//!   `--previous` as it is: a content-only feed carries the app over, and
//!   its history entry is marked `content_only`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use clap::{Args, Subcommand};
use ed25519_dalek::{Signer, SigningKey};
use kabar_core::packs::sha256_hex;
use kabar_core::update::{Version, check_download, key_line, parse_public_key, verify_manifest};
use kabar_protocol::update::{
    BuildChanges, Channel, ContentFile, MANIFEST_FILE, SIGNATURE_FILE, UPDATE_FORMAT, UpdateAsset,
    UpdateManifest, UpdateNotes, UpdatePlatform,
};

use crate::changes;

use crate::paths::Root;

/// The release public key the app builds in.
const RELEASE_KEY_FILE: &str = "crates/app/update-key.pub";

#[derive(Subcommand)]
pub enum Cmd {
    /// Make a signing key pair: writes the public key, prints the secret once.
    Keygen {
        /// Where the public key goes (default: the app's release key file).
        #[arg(long)]
        public_out: Option<PathBuf>,
        /// Replace a key that is already there. Builds signed for the old
        /// key then can't update by themselves any more.
        #[arg(long)]
        force: bool,
    },
    /// Hash the builds and write `update.json` (plus copies of the builds) into `--out`.
    Manifest(Box<ManifestArgs>),
    /// Sign `<dir>/update.json` with `KABAR_UPDATE_SECRET` (base64) into `update.json.sig`.
    Sign {
        /// The feed folder.
        dir: PathBuf,
        /// Public key to check the signature against (default: the app's release key).
        #[arg(long)]
        public_key: Option<PathBuf>,
    },
    /// Check a feed folder the way the app does: signature, then every file.
    Verify {
        dir: PathBuf,
        #[arg(long)]
        public_key: Option<PathBuf>,
        /// A platform whose build was carried from the previous feed
        /// (`manifest --carry`): its file is already on the release, so it
        /// may be missing here; when it is here, it is checked as usual.
        #[arg(long = "carried", value_parser = parse_platform)]
        carried: Vec<UpdatePlatform>,
    },
    /// Print what `changes/` adds after `--since` (default: only HEAD), and check every file there.
    Changes {
        #[arg(long)]
        since: Option<String>,
    },
    /// Pack the content and stylesheets the app builds in into `<out>/kabar-content.bundle`.
    Bundle {
        #[arg(long, default_value = "dist/bundle")]
        out: PathBuf,
    },
}

#[derive(Args)]
pub struct ManifestArgs {
    #[arg(long, value_parser = parse_channel)]
    channel: Channel,
    /// Default: the workspace version in `Cargo.toml`.
    #[arg(long)]
    version: Option<String>,
    /// Build number (the CI run number for previews).
    #[arg(long, default_value_t = 0)]
    build: u64,
    #[arg(long)]
    commit: Option<String>,
    /// `<platform>=<path>`, e.g. `android-arm64=dist/android/kabar.apk`. Repeat per build.
    #[arg(long = "asset", value_parser = parse_asset)]
    assets: Vec<(UpdatePlatform, PathBuf)>,
    /// Keep this platform's file from `--previous` as it is (a content-only feed).
    #[arg(long = "carry", value_parser = parse_platform)]
    carry: Vec<UpdatePlatform>,
    /// The content bundle from `update bundle`.
    #[arg(long)]
    content: Option<PathBuf>,
    #[arg(long, default_value = "")]
    notes_en: String,
    #[arg(long, default_value = "")]
    notes_ar_eg: String,
    #[arg(long, default_value = "")]
    notes_ar: String,
    /// The channel's current `update.json` (a missing file is fine): its
    /// history is kept, and with `--changes` its commit is where the new
    /// changes start.
    #[arg(long)]
    previous: Option<PathBuf>,
    /// Gather `changes/` since the previous feed's commit into this build's
    /// history entry (and the notes, when none are given).
    #[arg(long)]
    changes: bool,
    /// Feed folder to write (emptied first).
    #[arg(long, default_value = "dist/update")]
    out: PathBuf,
}

fn parse_channel(s: &str) -> Result<Channel, String> {
    Channel::parse(s).ok_or_else(|| format!("unknown channel {s:?} (preview, alpha, beta, stable)"))
}

fn parse_platform(s: &str) -> Result<UpdatePlatform, String> {
    UpdatePlatform::parse(s).ok_or_else(|| format!("unknown platform {s:?}"))
}

fn parse_asset(s: &str) -> Result<(UpdatePlatform, PathBuf), String> {
    let (p, path) = s.split_once('=').ok_or("expected <platform>=<path>")?;
    let platform = UpdatePlatform::parse(p).ok_or_else(|| format!("unknown platform {p:?}"))?;
    Ok((platform, PathBuf::from(path)))
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Keygen { public_out, force } => {
            let out = public_out.unwrap_or_else(|| root.path().join(RELEASE_KEY_FILE));
            let (public, secret) = keygen(&out, force)?;
            eprintln!("public key {public} → {}", out.display());
            // stdout carries only the secret, for the Keychain.
            println!("{secret}");
            Ok(())
        }
        Cmd::Manifest(args) => {
            let args = *args;
            let version = match args.version {
                Some(v) => v,
                None => workspace_version(root)?,
            };
            let previous: Option<UpdateManifest> = match &args.previous {
                Some(p) if p.exists() => {
                    let m: UpdateManifest = serde_json::from_slice(&fs::read(p)?)
                        .with_context(|| format!("previous feed {}", p.display()))?;
                    ensure!(m.channel == args.channel, "the previous feed is for {}", m.channel.as_str());
                    Some(m)
                }
                _ => None,
            };
            ensure!(
                !args.assets.is_empty() || !args.carry.is_empty(),
                "give at least one --asset or --carry"
            );
            let carried = args
                .carry
                .iter()
                .map(|p| {
                    let m = previous.as_ref().context("--carry needs a --previous feed")?;
                    ensure!(
                        m.version == version,
                        "--carry: the previous feed is {}, not {version}",
                        m.version
                    );
                    let a = m
                        .asset(*p)
                        .with_context(|| format!("--carry: no {} in the previous feed", p.as_str()))?;
                    Ok(UpdateAsset { build: Some(m.build_of(a)), ..a.clone() })
                })
                .collect::<Result<Vec<_>>>()?;
            let mut notes = UpdateNotes { en: args.notes_en, ar_eg: args.notes_ar_eg, ar: args.notes_ar };
            let mut entry = None;
            if args.changes {
                let since = previous.as_ref().and_then(|m| m.commit.as_deref());
                let found = changes::collect(root.path(), since)?;
                eprintln!("{} change(s) since {}", found.len(), since.unwrap_or("HEAD^"));
                if notes == UpdateNotes::default() {
                    notes = changes::notes_from(&found);
                }
                entry = (!found.is_empty()).then(|| BuildChanges {
                    version: version.clone(),
                    build: args.build,
                    published_at: now_rfc3339(),
                    changes: found,
                    content_only: !carried.is_empty(),
                });
            }
            let history = changes::history(previous.as_ref(), entry, &changes::retro(root.path())?);
            let m = write_manifest(
                &args.out,
                args.channel,
                &version,
                args.build,
                args.commit,
                Files { given: &args.assets, carried: &carried, content: args.content.as_deref() },
                WhatsNew { notes, history },
            )?;
            eprintln!(
                "{} {} build {} → {} ({} file(s))",
                m.channel.as_str(),
                m.version,
                m.build,
                args.out.join(MANIFEST_FILE).display(),
                m.assets.len()
            );
            Ok(())
        }
        Cmd::Sign { dir, public_key } => {
            let secret = std::env::var("KABAR_UPDATE_SECRET").context("KABAR_UPDATE_SECRET is not set")?;
            let public =
                fs::read_to_string(public_key.unwrap_or_else(|| root.path().join(RELEASE_KEY_FILE)))?;
            sign(&dir, &secret, &public)?;
            eprintln!("signed {}", dir.join(SIGNATURE_FILE).display());
            Ok(())
        }
        Cmd::Verify { dir, public_key, carried } => {
            let public =
                fs::read_to_string(public_key.unwrap_or_else(|| root.path().join(RELEASE_KEY_FILE)))?;
            let m = verify(&dir, &public, &carried)?;
            eprintln!(
                "ok: {} {} build {}, {} file(s) match",
                m.channel.as_str(),
                m.version,
                m.build,
                m.assets.len()
            );
            Ok(())
        }
        Cmd::Bundle { out } => {
            let (path, n) = bundle(root.path(), &out)?;
            eprintln!("{n} file(s) → {} ({} KB)", path.display(), fs::metadata(&path)?.len() / 1024);
            Ok(())
        }
        Cmd::Changes { since } => {
            let n = changes::check_all(root.path())?;
            eprintln!("{n} file(s) in {}/ check out", changes::DIR);
            for c in changes::collect(root.path(), since.as_deref())? {
                println!("{:?}\t{}\t{}", c.kind, c.text.ar_eg, c.text.en);
            }
            Ok(())
        }
    }
}

fn workspace_version(root: &Root) -> Result<String> {
    let toml = fs::read_to_string(root.path().join("Cargo.toml"))?;
    toml.lines()
        .find_map(|l| l.strip_prefix("version = \"")?.strip_suffix('"').map(str::to_owned))
        .context("no version in Cargo.toml")
}

/// Writes the public key file and returns (public, secret), both base64.
pub fn keygen(public_out: &Path, force: bool) -> Result<(String, String)> {
    if !force && fs::read_to_string(public_out).is_ok_and(|t| key_line(&t).is_some()) {
        bail!(
            "{} already holds a key; installed builds trust it. Pass --force only if that key is lost.",
            public_out.display()
        );
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
    let key = SigningKey::from_bytes(&seed);
    let public = B64.encode(key.verifying_key().as_bytes());
    let text = format!(
        "# Kabar's update key (Ed25519, public half). Builds trust update feeds\n\
         # signed with its secret half, which lives only in the Mac's Keychain\n\
         # (`just update-keygen`). Replacing it strands every installed build.\n\
         {public}\n"
    );
    if let Some(dir) = public_out.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(public_out, text)?;
    Ok((public, B64.encode(seed)))
}

/// Packs every file the app builds in (`kabar_protocol::bundle::SOURCES`)
/// and every stylesheet under `crates/app` into `<out>/kabar-content.bundle`.
/// Each file must be UTF-8, as build.rs requires. Returns the path and the
/// number of files.
pub fn bundle(root: &Path, out: &Path) -> Result<(PathBuf, usize)> {
    use kabar_protocol::bundle::{CSS_PREFIX, SOURCES, encode, source_files};
    let mut files = Vec::new();
    for source in SOURCES {
        files.extend(source_files(root, *source));
    }
    let app = root.join("crates/app");
    let mut dirs = vec![app.join("src"), app.join("assets")];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).with_context(|| dir.display().to_string())? {
            let path = entry?.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "css") {
                let rel = path.strip_prefix(&app)?.to_string_lossy().replace('\\', "/");
                files.push((format!("{CSS_PREFIX}{rel}"), path));
            }
        }
    }
    let mut entries = Vec::new();
    for (key, path) in files {
        let raw = fs::read(&path).with_context(|| path.display().to_string())?;
        ensure!(std::str::from_utf8(&raw).is_ok(), "{} isn't UTF-8", path.display());
        entries.push((key, miniz_oxide::deflate::compress_to_vec(&raw, 10)));
    }
    entries.sort();
    let bytes = encode(&entries);
    check_bundle(&bytes).context("the content would make a broken bundle")?;
    fs::create_dir_all(out)?;
    let path = out.join("kabar-content.bundle");
    fs::write(&path, bytes)?;
    Ok((path, entries.len()))
}

/// The checks a device runs before it takes a bundle
/// (`kabar_core::bundle_check`, within the unpack limits), so nothing is
/// published that devices would refuse. Lists every problem.
pub fn check_bundle(bytes: &[u8]) -> Result<()> {
    use kabar_protocol::bundle::{MAX_BYTES, MAX_FILE_UNPACKED, MAX_UNPACKED, decode};
    ensure!(bytes.len() as u64 <= MAX_BYTES, "{} bytes is over the {MAX_BYTES}-byte limit", bytes.len());
    let entries = decode(bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut total = 0u64;
    let mut files = Vec::with_capacity(entries.len());
    for (key, packed) in entries {
        let raw = miniz_oxide::inflate::decompress_to_vec_with_limit(&packed, MAX_FILE_UNPACKED)
            .map_err(|_| anyhow::anyhow!("{key} doesn't unpack (or is over {MAX_FILE_UNPACKED} bytes)"))?;
        total += raw.len() as u64;
        ensure!(total <= MAX_UNPACKED, "the files unpack to over {MAX_UNPACKED} bytes");
        let text = String::from_utf8(raw).map_err(|_| anyhow::anyhow!("{key} isn't UTF-8"))?;
        files.push((key, text));
    }
    for key in files.iter().map(|(k, _)| k).filter(|k| !kabar_core::bundle_check::known_key(k)) {
        println!("bundle: {key} is unknown to this build (an older app ignores it)");
    }
    let issues = kabar_core::bundle_check::check_files(&files);
    for i in &issues {
        println!("bundle: {i}");
    }
    ensure!(issues.is_empty(), "{} content problem(s) in the bundle", issues.len());
    Ok(())
}

/// The files a manifest names.
pub struct Files<'a> {
    /// Builds to hash and copy, by platform.
    pub given: &'a [(UpdatePlatform, PathBuf)],
    /// Rows kept from the previous feed as they are (`--carry`).
    pub carried: &'a [UpdateAsset],
    /// The content bundle.
    pub content: Option<&'a Path>,
}

/// The manifest's notes and history (`crate::changes`).
#[derive(Default)]
pub struct WhatsNew {
    pub notes: UpdateNotes,
    pub history: Vec<BuildChanges>,
}

pub fn write_manifest(
    out: &Path,
    channel: Channel,
    version: &str,
    build: u64,
    commit: Option<String>,
    files: Files,
    whats_new: WhatsNew,
) -> Result<UpdateManifest> {
    let WhatsNew { notes, history } = whats_new;
    let parsed = Version::parse(version).with_context(|| format!("version {version:?}"))?;
    if channel != Channel::Preview {
        ensure!(
            parsed.channels().contains(&channel),
            "{version} doesn't belong on the {} channel",
            channel.as_str()
        );
    }
    if out.exists() {
        fs::remove_dir_all(out)?;
    }
    fs::create_dir_all(out)?;
    let mut assets = Vec::new();
    for (platform, path) in files.given {
        ensure!(
            !assets.iter().any(|a: &UpdateAsset| a.platform == *platform),
            "two builds for {}",
            platform.as_str()
        );
        let bytes = fs::read(path).with_context(|| path.display().to_string())?;
        let ext = match platform {
            UpdatePlatform::AndroidArm64 => "apk",
            UpdatePlatform::MacosArm64 | UpdatePlatform::MacosX86_64 => "app.zip",
        };
        // Preview builds all share one version, so the build number keeps
        // each file name unique: replacing a live file mid-upload would hand
        // devices still holding the old feed a 404 or a hash mismatch (#295).
        let file = match channel {
            Channel::Preview => format!("kabar-{version}-b{build}-{}.{ext}", platform.as_str()),
            _ => format!("kabar-{version}-{}.{ext}", platform.as_str()),
        };
        fs::write(out.join(&file), &bytes)?;
        assets.push(UpdateAsset {
            platform: *platform,
            file,
            size: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
            version_code: (*platform == UpdatePlatform::AndroidArm64).then(|| parsed.version_code()),
            build: None,
        });
    }
    for a in files.carried {
        ensure!(
            !assets.iter().any(|b: &UpdateAsset| b.platform == a.platform),
            "{} is both carried and given",
            a.platform.as_str()
        );
        assets.push(a.clone());
    }
    let content = match files.content {
        None => None,
        Some(path) => {
            let bytes = fs::read(path).with_context(|| path.display().to_string())?;
            check_bundle(&bytes).with_context(|| path.display().to_string())?;
            let file = match channel {
                Channel::Preview => format!("kabar-content-{version}-b{build}.bundle"),
                _ => format!("kabar-content-{version}.bundle"),
            };
            fs::write(out.join(&file), &bytes)?;
            // Its stylesheets fit the app this feed carries (older than the
            // feed when carried over).
            let ui_build = assets.iter().map(|a| a.build.unwrap_or(build)).min().unwrap_or(build);
            Some(ContentFile { file, size: bytes.len() as u64, sha256: sha256_hex(&bytes), ui_build })
        }
    };
    let m = UpdateManifest {
        format: UPDATE_FORMAT,
        channel,
        version: version.to_owned(),
        build,
        commit,
        published_at: now_rfc3339(),
        notes,
        assets,
        history,
        content,
    };
    fs::write(out.join(MANIFEST_FILE), serde_json::to_vec_pretty(&m)?)?;
    Ok(m)
}

/// Signs `update.json` and checks the signature against `public` before
/// writing it, so a feed is never signed with a key the app doesn't trust.
pub fn sign(dir: &Path, secret_b64: &str, public: &str) -> Result<()> {
    let seed: [u8; 32] = B64
        .decode(secret_b64.trim())
        .ok()
        .and_then(|b| b.try_into().ok())
        .context("KABAR_UPDATE_SECRET isn't a base64 32-byte key")?;
    let key = SigningKey::from_bytes(&seed);
    let json = fs::read(dir.join(MANIFEST_FILE)).context("no update.json; run `update manifest` first")?;
    let sig = B64.encode(key.sign(&json).to_bytes());
    let trusted = parse_public_key(public).map_err(|e| anyhow::anyhow!("{e}"))?;
    ensure!(
        trusted == key.verifying_key(),
        "this secret doesn't belong to the public key the app trusts ({RELEASE_KEY_FILE})"
    );
    verify_manifest(&json, &sig, &trusted).map_err(|e| anyhow::anyhow!("{e}"))?;
    fs::write(dir.join(SIGNATURE_FILE), format!("{sig}\n"))?;
    Ok(())
}

/// Checks `dir`'s feed: the signature, then every file it names. A build of
/// a `carried` platform may be absent (it is already on the release).
pub fn verify(dir: &Path, public: &str, carried: &[UpdatePlatform]) -> Result<UpdateManifest> {
    let key = parse_public_key(public).map_err(|e| anyhow::anyhow!("{e}"))?;
    let json = fs::read(dir.join(MANIFEST_FILE))?;
    let sig = fs::read_to_string(dir.join(SIGNATURE_FILE))?;
    let m = verify_manifest(&json, &sig, &key).map_err(|e| anyhow::anyhow!("{e}"))?;
    for a in &m.assets {
        let path = dir.join(&a.file);
        if carried.contains(&a.platform) && !path.exists() {
            continue;
        }
        let bytes = fs::read(&path).with_context(|| a.file.clone())?;
        check_download(a, bytes.len() as u64, &sha256_hex(&bytes))
            .map_err(|e| anyhow::anyhow!("{}: {e}", a.file))?;
    }
    if let Some(c) = &m.content {
        let bytes = fs::read(dir.join(&c.file)).with_context(|| c.file.clone())?;
        ensure!(
            bytes.len() as u64 == c.size && sha256_hex(&bytes).eq_ignore_ascii_case(&c.sha256),
            "{}: the download doesn't match the signed manifest",
            c.file
        );
        // What a device checks next: a feed devices would refuse is broken.
        check_bundle(&bytes).with_context(|| c.file.clone())?;
    }
    Ok(m)
}

fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seed of the test key behind `fixtures/update/` (never a real key).
    const FIXTURE_SEED: [u8; 32] = [0x4b; 32];

    /// The smallest bundle devices take.
    fn sample_bundle() -> Vec<u8> {
        let entries: Vec<_> = kabar_core::bundle_check::sample_files("characters: []\n")
            .into_iter()
            .map(|(k, t)| (k, miniz_oxide::deflate::compress_to_vec(t.as_bytes(), 10)))
            .collect();
        kabar_protocol::bundle::encode(&entries)
    }

    #[test]
    fn a_bundle_devices_would_refuse_is_never_published() {
        assert!(check_bundle(&sample_bundle()).is_ok());
        let mut files = kabar_core::bundle_check::sample_files("characters: []\n");
        files.retain(|(k, _)| k != "CAST");
        let entries: Vec<_> = files
            .into_iter()
            .map(|(k, t)| (k, miniz_oxide::deflate::compress_to_vec(t.as_bytes(), 10)))
            .collect();
        let err = check_bundle(&kabar_protocol::bundle::encode(&entries)).unwrap_err();
        assert!(err.to_string().contains("1 content problem"), "{err}");
        assert!(check_bundle(b"KABARCB1\x05").is_err(), "a cut bundle");
    }

    fn files(given: &[(UpdatePlatform, PathBuf)]) -> Files<'_> {
        Files { given, carried: &[], content: None }
    }

    #[test]
    fn a_content_only_feed_carries_the_app_and_a_new_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let apk = dir.path().join("kabar.apk");
        fs::write(&apk, b"apk").unwrap();
        let bundle = dir.path().join("kabar-content.bundle");
        fs::write(&bundle, sample_bundle()).unwrap();
        let given = [(UpdatePlatform::AndroidArm64, apk)];
        let content = Files { given: &given, carried: &[], content: Some(&bundle) };
        let first = write_manifest(
            &dir.path().join("a"),
            Channel::Preview,
            "0.3.0",
            40,
            None,
            content,
            WhatsNew::default(),
        )
        .unwrap();
        assert_eq!(first.content.as_ref().unwrap().ui_build, 40);

        let carried = [UpdateAsset { build: Some(40), ..first.assets[0].clone() }];
        let content = Files { given: &[], carried: &carried, content: Some(&bundle) };
        let next = write_manifest(
            &dir.path().join("b"),
            Channel::Preview,
            "0.3.0",
            45,
            None,
            content,
            WhatsNew::default(),
        )
        .unwrap();
        let app = next.asset(UpdatePlatform::AndroidArm64).unwrap();
        assert_eq!((app.file.as_str(), next.build_of(app)), ("kabar-0.3.0-b40-android-arm64.apk", 40));
        let c = next.content.unwrap();
        assert_eq!(
            (c.file.as_str(), c.ui_build),
            ("kabar-content-0.3.0-b45.bundle", 40),
            "the look fits the app it carries"
        );
        assert!(dir.path().join("b").join(&c.file).exists());

        let junk = dir.path().join("junk.bundle");
        fs::write(&junk, b"nope").unwrap();
        let content = Files { given: &given, carried: &[], content: Some(&junk) };
        assert!(
            write_manifest(
                &dir.path().join("c"),
                Channel::Preview,
                "0.3.0",
                46,
                None,
                content,
                WhatsNew::default()
            )
            .is_err()
        );
    }

    #[test]
    fn the_repo_bundles() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = tempfile::tempdir().unwrap();
        let (path, n) = bundle(&root, out.path()).unwrap();
        let entries = kabar_protocol::bundle::decode(&fs::read(path).unwrap()).unwrap();
        assert_eq!(entries.len(), n);
        let keys: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
        for key in ["LESSONS/bipa1.u01.l01", "CURRICULUM_ID", "css/assets/main.css"] {
            assert!(keys.contains(&key), "{key}");
        }
    }

    #[test]
    fn keygen_manifest_sign_verify_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let pub_file = dir.path().join("update-key.pub");
        let (_, secret) = keygen(&pub_file, false).unwrap();
        assert!(keygen(&pub_file, false).is_err(), "an existing key is never replaced silently");
        let public = fs::read_to_string(&pub_file).unwrap();

        let apk = dir.path().join("kabar.apk");
        fs::write(&apk, b"not really an apk").unwrap();
        let feed = dir.path().join("feed");
        let m = write_manifest(
            &feed,
            Channel::Alpha,
            "1.0.0-alpha.2",
            0,
            Some("abc1234".into()),
            Files { given: &[(UpdatePlatform::AndroidArm64, apk)], carried: &[], content: None },
            WhatsNew::default(),
        )
        .unwrap();
        assert_eq!(m.assets[0].file, "kabar-1.0.0-alpha.2-android-arm64.apk");
        assert_eq!(m.assets[0].version_code, Some(10_000_102));

        // Signing with a secret the app doesn't trust is refused.
        let stranger = B64.encode([1u8; 32]);
        assert!(sign(&feed, &stranger, &public).is_err());
        assert!(!feed.join(SIGNATURE_FILE).exists());

        sign(&feed, &secret, &public).unwrap();
        assert_eq!(verify(&feed, &public, &[]).unwrap(), m);

        // A carried build (already on the release) may be missing here, but
        // only when it's named as carried; when present it's still checked.
        let apk_path = feed.join(&m.assets[0].file);
        let bytes = fs::read(&apk_path).unwrap();
        fs::remove_file(&apk_path).unwrap();
        assert!(verify(&feed, &public, &[]).is_err());
        assert_eq!(verify(&feed, &public, &[UpdatePlatform::AndroidArm64]).unwrap(), m);
        fs::write(&apk_path, &bytes).unwrap();

        // A swapped build fails verification, carried or not.
        fs::write(&apk_path, b"something else").unwrap();
        assert!(verify(&feed, &public, &[]).is_err());
        assert!(verify(&feed, &public, &[UpdatePlatform::AndroidArm64]).is_err());
    }

    #[test]
    fn versions_stay_on_their_channels() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x");
        fs::write(&f, b"x").unwrap();
        let assets = [(UpdatePlatform::MacosArm64, f)];
        let out = dir.path().join("feed");
        assert!(
            write_manifest(
                &out,
                Channel::Stable,
                "1.0.0-alpha.1",
                0,
                None,
                files(&assets),
                WhatsNew::default()
            )
            .is_err()
        );
        assert!(
            write_manifest(&out, Channel::Beta, "1.0.0", 0, None, files(&assets), WhatsNew::default())
                .is_ok()
        );
        let preview =
            write_manifest(&out, Channel::Preview, "0.1.0", 7, None, files(&assets), WhatsNew::default());
        assert_eq!(
            preview.unwrap().assets[0].file,
            "kabar-0.1.0-b7-macos-arm64.app.zip",
            "preview names carry the build"
        );
    }

    #[test]
    fn dates_are_rfc3339() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_753), (2026, 10, 27));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    /// Regenerates the signed fixture the core and app tests read:
    /// `cargo test -p kabar-tools write_update_fixture -- --ignored`.
    #[test]
    #[ignore]
    fn write_update_fixture() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/update");
        fs::create_dir_all(&root).unwrap();
        let key = SigningKey::from_bytes(&FIXTURE_SEED);
        let public = B64.encode(key.verifying_key().as_bytes());
        fs::write(
            root.join("test-key.pub"),
            format!(
                "# Test key for fixtures/update only (seed 0x4b × 32). Never trusted by a build.\n{public}\n"
            ),
        )
        .unwrap();
        // The app tests write these bytes as the feed's APK (no .apk is tracked).
        let apk = b"fixture apk bytes".to_vec();
        let m = UpdateManifest {
            format: UPDATE_FORMAT,
            channel: Channel::Alpha,
            version: "1.0.0-alpha.2".into(),
            build: 0,
            commit: Some("0123abc".into()),
            published_at: "2026-10-27T08:00:00Z".into(),
            notes: UpdateNotes {
                en: "Fixture: faster Reader, new Radio episodes.".into(),
                ar_eg: "تجربة: القارئ أسرع، وحلقات راديو جديدة.".into(),
                ar: "تجربة: القارئ أسرع، وحلقات راديو جديدة.".into(),
            },
            assets: vec![
                UpdateAsset {
                    platform: UpdatePlatform::AndroidArm64,
                    file: "kabar-1.0.0-alpha.2-android-arm64.apk".into(),
                    size: apk.len() as u64,
                    sha256: sha256_hex(&apk),
                    version_code: Some(10_000_102),
                    build: None,
                },
                UpdateAsset {
                    platform: UpdatePlatform::MacosArm64,
                    file: "kabar-1.0.0-alpha.2-macos-arm64.app.zip".into(),
                    size: 4,
                    sha256: sha256_hex(b"zip!"),
                    version_code: None,
                    build: None,
                },
            ],
            history: vec![
                BuildChanges {
                    version: "1.0.0-alpha.2".into(),
                    build: 0,
                    published_at: "2026-10-27T08:00:00Z".into(),
                    changes: changes::parse(
                        "- kind: new\n  ar_eg: حلقات راديو جديدة\n  en: New Radio episodes\n\
                         - kind: improved\n  ar_eg: القارئ بقى أسرع\n  en: The Reader is faster\n\
                         - kind: fixed\n  ar_eg: الصوت مبقاش بيقطع في الدرس\n  en: Lesson audio no longer cuts out\n",
                    )
                    .unwrap(),
                    content_only: false,
                },
                BuildChanges {
                    version: "1.0.0-alpha.1".into(),
                    build: 0,
                    published_at: "2026-10-20T08:00:00Z".into(),
                    changes: changes::parse("- kind: new\n  ar_eg: أول نسخة ألفا\n  en: The first alpha\n").unwrap(),
                    content_only: false,
                },
            ],
            content: None,
        };
        let json = serde_json::to_vec_pretty(&m).unwrap();
        fs::write(root.join(MANIFEST_FILE), &json).unwrap();
        fs::write(root.join(SIGNATURE_FILE), format!("{}\n", B64.encode(key.sign(&json).to_bytes())))
            .unwrap();
    }
}
