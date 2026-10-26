//! Where the pipeline reads and writes, relative to the repository root.

use std::path::{Path, PathBuf};

use kabar_protocol::content::Lang;

pub struct Root(PathBuf);

impl Root {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self(root.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")))
    }

    /// Downloaded sources (git-ignored).
    pub fn raw(&self) -> PathBuf {
        self.0.join("data/raw")
    }

    /// Large derived outputs that are rebuilt, not committed (git-ignored).
    pub fn build(&self) -> PathBuf {
        self.0.join("build")
    }

    pub fn freq_list(&self, lang: Lang) -> PathBuf {
        self.0.join(format!("content/freq/{}.tsv", lang.code()))
    }

    pub fn freq_exclude(&self, lang: Lang) -> PathBuf {
        self.0.join(format!("content/freq/{}.exclude.tsv", lang.code()))
    }

    pub fn gloss_dir(&self, lang: Lang) -> PathBuf {
        self.0.join(format!("content/glosses/{}", lang.code()))
    }

    pub fn bridge_dir(&self) -> PathBuf {
        self.0.join("content/bridge")
    }

    pub fn schemas(&self) -> PathBuf {
        self.0.join("schemas")
    }
}
