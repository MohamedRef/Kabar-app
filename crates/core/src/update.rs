//! App updates (`docs/RELEASE.md` §5): versions and their order, Android's
//! `versionCode`, checking a feed's Ed25519 signature, and deciding whether
//! a signed manifest is an update for this build. Pure logic: the app does
//! the downloading and installing, `kabar-tools update` the signing.

use std::cmp::Ordering;
use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
pub use ed25519_dalek::VerifyingKey;
use ed25519_dalek::{Signature, Verifier};
use kabar_protocol::update::{
    Channel, ContentFile, UPDATE_FORMAT, UpdateAsset, UpdateManifest, UpdatePlatform,
};

/// A workspace version: `1.0.0`, `1.0.0-alpha.2`, `1.0.0-beta.1`, `1.0.0-rc.1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub stage: Stage,
}

/// Pre-releases sort before their release: alpha < beta < rc < final.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Alpha(u32),
    Beta(u32),
    Rc(u32),
    Final,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('v');
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let mut parts = core.split('.').map(|p| p.parse::<u32>().ok());
        let (major, minor, patch) = (parts.next()??, parts.next()??, parts.next()??);
        if parts.next().is_some() {
            return None;
        }
        let stage = match pre {
            None => Stage::Final,
            Some(p) => {
                let (kind, n) = p.split_once('.')?;
                let n = n.parse().ok()?;
                match kind {
                    "alpha" => Stage::Alpha(n),
                    "beta" => Stage::Beta(n),
                    "rc" => Stage::Rc(n),
                    _ => return None,
                }
            }
        };
        Some(Self { major, minor, patch, stage })
    }

    /// Android's `versionCode`, the same recipe as
    /// `scripts/android-release-apk.sh`: `1.2.3-alpha.4` → 10203104. Major,
    /// minor and patch, then the stage (alpha 100+N, beta 500+N, rc 800+N, a
    /// release 999), so each update installs over the one before.
    pub fn version_code(self) -> u64 {
        let stage = match self.stage {
            Stage::Alpha(n) => 100 + u64::from(n),
            Stage::Beta(n) => 500 + u64::from(n),
            Stage::Rc(n) => 800 + u64::from(n),
            Stage::Final => 999,
        };
        (u64::from(self.major) * 10_000 + u64::from(self.minor) * 100 + u64::from(self.patch)) * 1000 + stage
    }

    /// The release channels that should offer this version: a beta also
    /// reaches everyone on alpha, a release everyone.
    pub fn channels(self) -> &'static [Channel] {
        match self.stage {
            Stage::Alpha(_) => &[Channel::Alpha],
            Stage::Beta(_) | Stage::Rc(_) => &[Channel::Alpha, Channel::Beta],
            Stage::Final => &[Channel::Alpha, Channel::Beta, Channel::Stable],
        }
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch, self.stage).cmp(&(
            other.major,
            other.minor,
            other.patch,
            other.stage,
        ))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        match self.stage {
            Stage::Alpha(n) => write!(f, "-alpha.{n}"),
            Stage::Beta(n) => write!(f, "-beta.{n}"),
            Stage::Rc(n) => write!(f, "-rc.{n}"),
            Stage::Final => Ok(()),
        }
    }
}

/// What this build is: its version, its build number and the channel it
/// follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Installed {
    pub version: Version,
    pub build: u64,
    pub channel: Channel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateError {
    /// The public key built into the app isn't a valid Ed25519 key.
    BadKey,
    /// The signature file isn't a base64 Ed25519 signature.
    BadSignature,
    /// The signature doesn't match: not signed with Kabar's key, or changed.
    NotSigned,
    /// The manifest isn't valid JSON for this format.
    BadManifest(String),
    /// Signed, but for another channel than the one asked.
    WrongChannel { asked: Channel, got: Channel },
    /// The feed is newer than this app understands: update by hand once.
    TooNew,
    /// A file name in the manifest that isn't a plain file name.
    BadFileName(String),
    /// The download's size or SHA-256 differs from the signed manifest.
    Mismatch,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadKey => write!(f, "the update key in this build is invalid"),
            Self::BadSignature => write!(f, "the update signature is unreadable"),
            Self::NotSigned => write!(f, "the update isn't signed with Kabar's key"),
            Self::BadManifest(e) => write!(f, "the update manifest is invalid: {e}"),
            Self::WrongChannel { asked, got } => {
                write!(f, "asked the {} channel, got {}", asked.as_str(), got.as_str())
            }
            Self::TooNew => write!(f, "this update needs a newer Kabar; install it by hand once"),
            Self::BadFileName(n) => write!(f, "unsafe file name in the manifest: {n}"),
            Self::Mismatch => write!(f, "the download doesn't match the signed manifest"),
        }
    }
}

impl std::error::Error for UpdateError {}

/// Parses a base64 Ed25519 public key (32 bytes). Lines starting with `#`
/// and blank lines are skipped, so a key file can carry a comment.
pub fn parse_public_key(text: &str) -> Result<VerifyingKey, UpdateError> {
    let line = key_line(text).ok_or(UpdateError::BadKey)?;
    let bytes: [u8; 32] = B64.decode(line).ok().and_then(|b| b.try_into().ok()).ok_or(UpdateError::BadKey)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| UpdateError::BadKey)
}

/// The first line that isn't blank or a `#` comment.
pub fn key_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))
}

/// Checks `sig` (base64) over the exact bytes of `update.json`, then parses
/// it. Nothing in an unsigned or altered manifest is ever looked at.
pub fn verify_manifest(json: &[u8], sig: &str, key: &VerifyingKey) -> Result<UpdateManifest, UpdateError> {
    let sig: [u8; 64] =
        B64.decode(sig.trim()).ok().and_then(|b| b.try_into().ok()).ok_or(UpdateError::BadSignature)?;
    key.verify(json, &Signature::from_bytes(&sig)).map_err(|_| UpdateError::NotSigned)?;
    // The format is read first, so a newer feed says "too new" rather than
    // failing on a field this build doesn't know.
    #[derive(serde::Deserialize)]
    struct Head {
        format: u32,
    }
    let head: Head = serde_json::from_slice(json).map_err(|e| UpdateError::BadManifest(e.to_string()))?;
    if head.format > UPDATE_FORMAT {
        return Err(UpdateError::TooNew);
    }
    let m: UpdateManifest =
        serde_json::from_slice(json).map_err(|e| UpdateError::BadManifest(e.to_string()))?;
    if Version::parse(&m.version).is_none() {
        return Err(UpdateError::BadManifest(format!("version {:?}", m.version)));
    }
    if let Some(c) = &m.content
        && !is_plain_file_name(&c.file)
    {
        return Err(UpdateError::BadFileName(c.file.clone()));
    }
    for a in &m.assets {
        if !is_plain_file_name(&a.file) {
            return Err(UpdateError::BadFileName(a.file.clone()));
        }
    }
    Ok(m)
}

/// A name with no directory part, so it can only resolve next to `update.json`.
pub fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

/// What a verified manifest means for this build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    UpToDate,
    /// A newer build for this platform.
    Available(UpdateAsset),
    /// Newer, but published without a build for this platform (yet).
    NotForThisPlatform,
}

/// Is `m` (already verified) newer than `me`, and is there a build for
/// `platform`? A manifest for another channel is refused, so an old feed
/// can't be passed off as the one asked for.
pub fn decide(me: &Installed, m: &UpdateManifest, platform: UpdatePlatform) -> Result<Decision, UpdateError> {
    if m.channel != me.channel {
        return Err(UpdateError::WrongChannel { asked: me.channel, got: m.channel });
    }
    let version = Version::parse(&m.version).ok_or_else(|| UpdateError::BadManifest(m.version.clone()))?;
    let asset = m.asset(platform);
    if (version, asset.map_or(m.build, |a| m.build_of(a))) <= (me.version, me.build) {
        return Ok(Decision::UpToDate);
    }
    Ok(match asset {
        Some(a) => Decision::Available(a.clone()),
        None => Decision::NotForThisPlatform,
    })
}

/// The feed's content bundle when it should replace the one in use
/// (`current`: the `(version, build)` of the feed that delivered it, or None
/// for the content built in). It must come from this build or a later one,
/// since an older build's content may lack what this one has, and be newer
/// than the bundle in use.
pub fn content_update<'m>(
    me: &Installed,
    m: &'m UpdateManifest,
    current: Option<(Version, u64)>,
) -> Result<Option<&'m ContentFile>, UpdateError> {
    if m.channel != me.channel {
        return Err(UpdateError::WrongChannel { asked: me.channel, got: m.channel });
    }
    let version = Version::parse(&m.version).ok_or_else(|| UpdateError::BadManifest(m.version.clone()))?;
    let feed = (version, m.build);
    let fresh = feed >= (me.version, me.build) && current.is_none_or(|c| feed > c);
    Ok(m.content.as_ref().filter(|_| fresh))
}

/// Checks a finished download against its signed manifest row.
pub fn check_download(asset: &UpdateAsset, size: u64, sha256_hex: &str) -> Result<(), UpdateError> {
    if asset.size == size && asset.sha256.eq_ignore_ascii_case(sha256_hex) {
        Ok(())
    } else {
        Err(UpdateError::Mismatch)
    }
}

/// The platform this binary was built for, if updates exist for it.
pub fn this_platform() -> Option<UpdatePlatform> {
    if cfg!(all(target_os = "android", target_arch = "aarch64")) {
        Some(UpdatePlatform::AndroidArm64)
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some(UpdatePlatform::MacosArm64)
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some(UpdatePlatform::MacosX86_64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use kabar_protocol::update::UpdateNotes;

    use super::*;

    /// The key that signed `fixtures/update/update.json` (test only; seed
    /// 0x4b repeated, see `kabar-tools update sign --help`).
    const FIXTURE_PUBLIC_KEY: &str = include_str!("../../../fixtures/update/test-key.pub");

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn manifest(channel: Channel, version: &str, build: u64) -> UpdateManifest {
        UpdateManifest {
            format: UPDATE_FORMAT,
            channel,
            version: version.into(),
            build,
            commit: None,
            published_at: "2026-10-27T00:00:00Z".into(),
            notes: UpdateNotes::default(),
            assets: vec![UpdateAsset {
                platform: UpdatePlatform::AndroidArm64,
                file: format!("kabar-{version}.apk"),
                size: 3,
                sha256: "ab".repeat(32),
                version_code: Some(v(version).version_code()),
                build: None,
            }],
            history: Vec::new(),
            content: Some(ContentFile {
                file: format!("kabar-content-{version}.bundle"),
                size: 3,
                sha256: "cd".repeat(32),
                ui_build: build,
            }),
        }
    }

    #[test]
    fn a_content_only_feed_never_offers_the_app_again() {
        let me = Installed { version: v("0.3.0"), build: 40, channel: Channel::Preview };
        let mut m = manifest(Channel::Preview, "0.3.0", 45);
        m.assets[0].build = Some(40);
        let android = UpdatePlatform::AndroidArm64;
        assert_eq!(decide(&me, &m, android), Ok(Decision::UpToDate), "the app it carries is the one running");
        assert!(content_update(&me, &m, None).unwrap().is_some(), "the new bundle still comes");
        assert!(content_update(&me, &m, Some((v("0.3.0"), 45))).unwrap().is_none(), "once");
        assert!(content_update(&me, &m, Some((v("0.3.0"), 44))).unwrap().is_some());
        let newer_app = Installed { build: 46, ..me };
        assert!(content_update(&newer_app, &m, None).unwrap().is_none(), "an older feed's content never");
        let older = Installed { build: 38, ..me };
        assert!(matches!(decide(&older, &m, android), Ok(Decision::Available(_))));
    }

    #[test]
    fn versions_order_alpha_beta_rc_release() {
        let order = [
            "0.1.0",
            "1.0.0-alpha.1",
            "1.0.0-alpha.2",
            "1.0.0-alpha.10",
            "1.0.0-beta.1",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1-alpha.1",
            "1.1.0",
        ];
        for pair in order.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
        for s in order {
            assert_eq!(v(s).to_string(), s);
        }
        assert_eq!(v("v1.2.3"), v("1.2.3"));
        for bad in ["1.0", "1.0.0.0", "1.0.0-nightly.1", "1.0.0-alpha", "x.1.2", ""] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn version_codes_match_the_release_script() {
        // The examples in scripts/android-release-apk.sh and docs/RUN.md.
        assert_eq!(v("1.2.3-alpha.4").version_code(), 10_203_104);
        assert_eq!(v("1.0.0-alpha.1").version_code(), 10_000_101);
        assert_eq!(v("0.1.0").version_code(), 100_999);
        // Every step up the ladder is a bigger versionCode.
        let ladder =
            ["1.0.0-alpha.1", "1.0.0-alpha.2", "1.0.0-beta.1", "1.0.0-rc.1", "1.0.0", "1.0.1-alpha.1"];
        for pair in ladder.windows(2) {
            assert!(v(pair[0]).version_code() < v(pair[1]).version_code());
        }
    }

    #[test]
    fn betas_and_releases_reach_alpha_followers() {
        assert_eq!(v("1.0.0-alpha.3").channels(), [Channel::Alpha]);
        assert_eq!(v("1.0.0-beta.1").channels(), [Channel::Alpha, Channel::Beta]);
        assert_eq!(v("1.0.0").channels(), [Channel::Alpha, Channel::Beta, Channel::Stable]);
    }

    #[test]
    fn the_signed_fixture_verifies_and_any_change_is_refused() {
        let key = parse_public_key(FIXTURE_PUBLIC_KEY).unwrap();
        let json = include_bytes!("../../../fixtures/update/update.json");
        let sig = include_str!("../../../fixtures/update/update.json.sig");
        let m = verify_manifest(json, sig, &key).unwrap();
        assert_eq!(m.version, "1.0.0-alpha.2");

        // One byte changed anywhere: refused before parsing.
        let mut changed = json.to_vec();
        let at = changed.iter().position(|b| *b == b'2').unwrap();
        changed[at] = b'9';
        assert_eq!(verify_manifest(&changed, sig, &key), Err(UpdateError::NotSigned));
        // Another key's signature: refused.
        let other = SigningKey::from_bytes(&[7; 32]);
        let forged = B64.encode(other.sign(json).to_bytes());
        assert_eq!(verify_manifest(json, &forged, &key), Err(UpdateError::NotSigned));
        assert_eq!(verify_manifest(json, "not base64!", &key), Err(UpdateError::BadSignature));
    }

    #[test]
    fn newer_formats_and_path_tricks_are_refused_even_when_signed() {
        let signer = SigningKey::from_bytes(&[9; 32]);
        let key = signer.verifying_key();
        let sign = |json: &[u8]| B64.encode(signer.sign(json).to_bytes());

        let future = br#"{"format": 99, "something": "new"}"#;
        assert_eq!(verify_manifest(future, &sign(future), &key), Err(UpdateError::TooNew));

        let mut m = manifest(Channel::Alpha, "1.0.0-alpha.2", 0);
        m.assets[0].file = "../../etc/passwd".into();
        let json = serde_json::to_vec(&m).unwrap();
        assert!(matches!(verify_manifest(&json, &sign(&json), &key), Err(UpdateError::BadFileName(_))));
        for name in ["https://evil.example/x.apk", "a/b.apk", "..", ""] {
            assert!(!is_plain_file_name(name), "{name}");
        }
        assert!(is_plain_file_name("Kabar-1.0.0-alpha.2-macos-arm64.app.zip"));
    }

    #[test]
    fn decide_wants_newer_same_channel_and_this_platform() {
        let me = Installed { version: v("1.0.0-alpha.1"), build: 0, channel: Channel::Alpha };
        let android = UpdatePlatform::AndroidArm64;
        assert_eq!(
            decide(&me, &manifest(Channel::Alpha, "1.0.0-alpha.1", 0), android),
            Ok(Decision::UpToDate)
        );
        assert_eq!(decide(&me, &manifest(Channel::Alpha, "0.9.0", 0), android), Ok(Decision::UpToDate));
        let newer = manifest(Channel::Alpha, "1.0.0-alpha.2", 0);
        assert!(
            matches!(decide(&me, &newer, android), Ok(Decision::Available(a)) if a.file == "kabar-1.0.0-alpha.2.apk")
        );
        assert_eq!(decide(&me, &newer, UpdatePlatform::MacosArm64), Ok(Decision::NotForThisPlatform));
        assert_eq!(
            decide(&me, &manifest(Channel::Stable, "2.0.0", 0), android),
            Err(UpdateError::WrongChannel { asked: Channel::Alpha, got: Channel::Stable })
        );

        // Previews share a version and move by build number.
        let preview = Installed { version: v("0.1.0"), build: 140, channel: Channel::Preview };
        assert_eq!(
            decide(&preview, &manifest(Channel::Preview, "0.1.0", 140), android),
            Ok(Decision::UpToDate)
        );
        assert!(matches!(
            decide(&preview, &manifest(Channel::Preview, "0.1.0", 141), android),
            Ok(Decision::Available(_))
        ));
    }

    #[test]
    fn downloads_must_match_size_and_hash() {
        let a = &manifest(Channel::Alpha, "1.0.0-alpha.2", 0).assets[0];
        assert_eq!(check_download(a, 3, &"AB".repeat(32)), Ok(()));
        assert_eq!(check_download(a, 4, &"ab".repeat(32)), Err(UpdateError::Mismatch));
        assert_eq!(check_download(a, 3, &"cd".repeat(32)), Err(UpdateError::Mismatch));
    }

    #[test]
    fn key_files_may_carry_comments() {
        let key = SigningKey::from_bytes(&[3; 32]).verifying_key();
        let text = format!("# Kabar release update key\n\n{}\n", B64.encode(key.as_bytes()));
        assert_eq!(parse_public_key(&text), Ok(key));
        assert_eq!(parse_public_key("# nothing yet\n"), Err(UpdateError::BadKey));
        assert_eq!(parse_public_key("AAAA"), Err(UpdateError::BadKey));
    }
}
