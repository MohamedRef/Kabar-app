//! App updates over the air (`docs/RELEASE.md` §5): what an update feed
//! publishes and what a device checks before it installs anything.
//!
//! A feed is two files on a GitHub release of the (public) Kabar repository,
//! one release per channel (`update-alpha`, `update-beta`, `update-stable`,
//! and `preview` for the CI builds of `main`):
//!
//! ```text
//! update.json       an UpdateManifest
//! update.json.sig   Ed25519 signature of update.json's exact bytes (base64)
//! kabar-….apk       the Android build      ┐ named by UpdateAsset::file,
//! Kabar-….app.zip   the Mac build (ditto)  ┘ checked by UpdateAsset::sha256
//! ```
//!
//! The signing key never leaves the Mac's Keychain (or, for `preview`, the
//! CI cache); the app carries only the public key. A device refuses a
//! manifest whose signature, channel or version doesn't check out, and an
//! asset whose size or SHA-256 differs from the signed manifest.

use serde::{Deserialize, Serialize};

/// Bumped when the manifest changes incompatibly. A device ignores feeds with
/// a newer format and says Kabar itself needs a manual update.
pub const UPDATE_FORMAT: u32 = 1;
/// The manifest's file name in a feed.
pub const MANIFEST_FILE: &str = "update.json";
/// The detached signature's file name in a feed.
pub const SIGNATURE_FILE: &str = "update.json.sig";

/// Where a build gets its updates from. A build signed with the release key
/// follows `Alpha`, `Beta` or `Stable` (the learner picks); a CI preview build
/// is signed with the preview key and can only follow `Preview` (Android
/// refuses an APK signed with another key anyway).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Every merge to `main`, built and signed by CI.
    Preview,
    Alpha,
    Beta,
    Stable,
}

impl Channel {
    pub const RELEASE: [Self; 3] = [Self::Alpha, Self::Beta, Self::Stable];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Alpha => "alpha",
            Self::Beta => "beta",
            Self::Stable => "stable",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::Preview, Self::Alpha, Self::Beta, Self::Stable].into_iter().find(|c| c.as_str() == s.trim())
    }

    /// The GitHub release tag that carries this channel's feed.
    pub fn release_tag(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Alpha => "update-alpha",
            Self::Beta => "update-beta",
            Self::Stable => "update-stable",
        }
    }
}

/// Which build an asset is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdatePlatform {
    /// The S24 Ultra and Tab S11.
    AndroidArm64,
    /// Kabar.app for Apple silicon.
    MacosArm64,
    /// Kabar.app for an Intel Mac.
    #[serde(rename = "macos-x86_64")]
    MacosX86_64,
}

impl UpdatePlatform {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AndroidArm64 => "android-arm64",
            Self::MacosArm64 => "macos-arm64",
            Self::MacosX86_64 => "macos-x86_64",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::AndroidArm64, Self::MacosArm64, Self::MacosX86_64].into_iter().find(|p| p.as_str() == s.trim())
    }
}

/// `update.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateManifest {
    /// [`UPDATE_FORMAT`] when written.
    pub format: u32,
    /// The channel this feed is for; a device checks it matches the one it
    /// asked, so a stable feed can't be swapped for an older alpha one.
    pub channel: Channel,
    /// The workspace version, e.g. `1.0.0-alpha.2`.
    pub version: String,
    /// Orders builds of the same version (the CI run number for previews; 0
    /// for releases, which always bump `version`).
    #[serde(default)]
    pub build: u64,
    /// The commit it was built from (shown in Settings › About).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// RFC 3339, when the feed was signed.
    pub published_at: String,
    /// What's new, a line or two per language; shown before installing.
    #[serde(default)]
    pub notes: UpdateNotes,
    pub assets: Vec<UpdateAsset>,
    /// What each recent build changed, newest first (this build, when it
    /// changed anything visible, then up to [`HISTORY_BUILDS`] before it), so
    /// the app can say what's new before installing and keep a history.
    /// Older apps ignore it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<BuildChanges>,
}

impl UpdateManifest {
    pub fn asset(&self, platform: UpdatePlatform) -> Option<&UpdateAsset> {
        self.assets.iter().find(|a| a.platform == platform)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateNotes {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub en: String,
    /// Egyptian Arabic (the app's default language).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ar_eg: String,
    /// Modern Standard Arabic.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ar: String,
}

/// How many builds [`UpdateManifest::history`] keeps.
pub const HISTORY_BUILDS: usize = 40;

/// One build's changes, as the learner reads them: written per pull request
/// in `changes/*.yaml` (`docs/RELEASE.md` §5) and gathered by CI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildChanges {
    pub version: String,
    #[serde(default)]
    pub build: u64,
    /// RFC 3339, when the build was published.
    pub published_at: String,
    pub changes: Vec<Change>,
}

impl BuildChanges {
    /// `version+build`, the same key the app uses for an offer.
    pub fn key(&self) -> String {
        format!("{}+{}", self.version, self.build)
    }
}

/// One line of "what's new".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub kind: ChangeKind,
    /// The line itself; Egyptian Arabic is required, the others fall back to it.
    #[serde(flatten)]
    pub text: UpdateNotes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Something the learner couldn't do before.
    New,
    /// Something that works better or looks better.
    Improved,
    /// Something that was broken.
    Fixed,
}

impl ChangeKind {
    pub const ALL: [Self; 3] = [Self::New, Self::Improved, Self::Fixed];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateAsset {
    pub platform: UpdatePlatform,
    /// File name next to `update.json` in the same release. Never a URL, so
    /// a signed manifest can only point inside its own feed.
    pub file: String,
    pub size: u64,
    /// SHA-256 of the file, lowercase hex.
    pub sha256: String,
    /// Android's `versionCode` of the APK, which must grow for an install
    /// over the old app to succeed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_code: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_and_platform_names_round_trip() {
        for c in [Channel::Preview, Channel::Alpha, Channel::Beta, Channel::Stable] {
            assert_eq!(Channel::parse(c.as_str()), Some(c));
            assert_eq!(serde_json::to_string(&c).unwrap(), format!("\"{}\"", c.as_str()));
        }
        for p in [UpdatePlatform::AndroidArm64, UpdatePlatform::MacosArm64, UpdatePlatform::MacosX86_64] {
            assert_eq!(UpdatePlatform::parse(p.as_str()), Some(p));
            assert_eq!(serde_json::to_string(&p).unwrap(), format!("\"{}\"", p.as_str()));
        }
        assert_eq!(Channel::parse("nightly"), None);
    }

    #[test]
    fn manifest_fixture_parses() {
        let m: UpdateManifest =
            serde_json::from_str(include_str!("../../../fixtures/update/update.json")).unwrap();
        assert_eq!(m.channel, Channel::Alpha);
        assert_eq!(m.asset(UpdatePlatform::AndroidArm64).unwrap().version_code, Some(10_000_102));
        assert!(m.asset(UpdatePlatform::MacosX86_64).is_none());
        let h = &m.history[0];
        assert_eq!((h.key().as_str(), h.changes[0].kind), ("1.0.0-alpha.2+0", ChangeKind::New));
        assert!(!h.changes[0].text.ar_eg.is_empty());
    }

    #[test]
    fn a_manifest_without_history_still_parses() {
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/update/update.json")).unwrap();
        v.as_object_mut().unwrap().remove("history");
        let m: UpdateManifest = serde_json::from_value(v).unwrap();
        assert!(m.history.is_empty());
        // and none is written back
        assert!(!serde_json::to_string(&m).unwrap().contains("history"));
    }
}
