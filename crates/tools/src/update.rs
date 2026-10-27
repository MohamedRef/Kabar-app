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
    Channel, MANIFEST_FILE, SIGNATURE_FILE, UPDATE_FORMAT, UpdateAsset, UpdateManifest, UpdateNotes,
    UpdatePlatform,
};

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
    Manifest(ManifestArgs),
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
    #[arg(long = "asset", value_parser = parse_asset, required = true)]
    assets: Vec<(UpdatePlatform, PathBuf)>,
    #[arg(long, default_value = "")]
    notes_en: String,
    #[arg(long, default_value = "")]
    notes_ar_eg: String,
    #[arg(long, default_value = "")]
    notes_ar: String,
    /// Feed folder to write (emptied first).
    #[arg(long, default_value = "dist/update")]
    out: PathBuf,
}

fn parse_channel(s: &str) -> Result<Channel, String> {
    Channel::parse(s).ok_or_else(|| format!("unknown channel {s:?} (preview, alpha, beta, stable)"))
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
            let version = match args.version {
                Some(v) => v,
                None => workspace_version(root)?,
            };
            let m = write_manifest(
                &args.out,
                args.channel,
                &version,
                args.build,
                args.commit,
                &args.assets,
                UpdateNotes { en: args.notes_en, ar_eg: args.notes_ar_eg, ar: args.notes_ar },
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
        Cmd::Verify { dir, public_key } => {
            let public =
                fs::read_to_string(public_key.unwrap_or_else(|| root.path().join(RELEASE_KEY_FILE)))?;
            let m = verify(&dir, &public)?;
            eprintln!(
                "ok: {} {} build {}, {} file(s) match",
                m.channel.as_str(),
                m.version,
                m.build,
                m.assets.len()
            );
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

pub fn write_manifest(
    out: &Path,
    channel: Channel,
    version: &str,
    build: u64,
    commit: Option<String>,
    files: &[(UpdatePlatform, PathBuf)],
    notes: UpdateNotes,
) -> Result<UpdateManifest> {
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
    for (platform, path) in files {
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
        });
    }
    let m = UpdateManifest {
        format: UPDATE_FORMAT,
        channel,
        version: version.to_owned(),
        build,
        commit,
        published_at: now_rfc3339(),
        notes,
        assets,
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

pub fn verify(dir: &Path, public: &str) -> Result<UpdateManifest> {
    let key = parse_public_key(public).map_err(|e| anyhow::anyhow!("{e}"))?;
    let json = fs::read(dir.join(MANIFEST_FILE))?;
    let sig = fs::read_to_string(dir.join(SIGNATURE_FILE))?;
    let m = verify_manifest(&json, &sig, &key).map_err(|e| anyhow::anyhow!("{e}"))?;
    for a in &m.assets {
        let bytes = fs::read(dir.join(&a.file)).with_context(|| a.file.clone())?;
        check_download(a, bytes.len() as u64, &sha256_hex(&bytes))
            .map_err(|e| anyhow::anyhow!("{}: {e}", a.file))?;
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
            &[(UpdatePlatform::AndroidArm64, apk)],
            UpdateNotes::default(),
        )
        .unwrap();
        assert_eq!(m.assets[0].file, "kabar-1.0.0-alpha.2-android-arm64.apk");
        assert_eq!(m.assets[0].version_code, Some(10_000_102));

        // Signing with a secret the app doesn't trust is refused.
        let stranger = B64.encode([1u8; 32]);
        assert!(sign(&feed, &stranger, &public).is_err());
        assert!(!feed.join(SIGNATURE_FILE).exists());

        sign(&feed, &secret, &public).unwrap();
        assert_eq!(verify(&feed, &public).unwrap(), m);

        // A swapped build fails verification.
        fs::write(feed.join(&m.assets[0].file), b"something else").unwrap();
        assert!(verify(&feed, &public).is_err());
    }

    #[test]
    fn versions_stay_on_their_channels() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x");
        fs::write(&f, b"x").unwrap();
        let assets = [(UpdatePlatform::MacosArm64, f)];
        let out = dir.path().join("feed");
        assert!(
            write_manifest(&out, Channel::Stable, "1.0.0-alpha.1", 0, None, &assets, UpdateNotes::default())
                .is_err()
        );
        assert!(
            write_manifest(&out, Channel::Beta, "1.0.0", 0, None, &assets, UpdateNotes::default()).is_ok()
        );
        let preview =
            write_manifest(&out, Channel::Preview, "0.1.0", 7, None, &assets, UpdateNotes::default());
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
                },
                UpdateAsset {
                    platform: UpdatePlatform::MacosArm64,
                    file: "kabar-1.0.0-alpha.2-macos-arm64.app.zip".into(),
                    size: 4,
                    sha256: sha256_hex(b"zip!"),
                    version_code: None,
                },
            ],
        };
        let json = serde_json::to_vec_pretty(&m).unwrap();
        fs::write(root.join(MANIFEST_FILE), &json).unwrap();
        fs::write(root.join(SIGNATURE_FILE), format!("{}\n", B64.encode(key.sign(&json).to_bytes())))
            .unwrap();
    }
}
