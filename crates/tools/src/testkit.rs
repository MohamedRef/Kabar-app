//! Test helpers: the real repository (read-only), sandboxes holding copies
//! of parts of it, and running a command line the way `main` does.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser as _;

use crate::paths::Root;

/// The repository this crate was built from.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// A temporary repository root holding copies of chosen repo paths.
pub struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    /// An empty root.
    pub fn empty() -> Self {
        Self { dir: tempfile::tempdir().unwrap() }
    }

    /// A root with copies of these repo-relative files and directories.
    pub fn with(paths: &[&str]) -> Self {
        let s = Self::empty();
        for p in paths {
            copy(&repo().join(p), &s.path().join(p));
        }
        s
    }

    /// Copies the files of a repo directory whose names start with one of `prefixes`.
    pub fn copy_matching(&self, dir: &str, prefixes: &[&str]) {
        for e in fs::read_dir(repo().join(dir)).unwrap() {
            let name = e.unwrap().file_name().into_string().unwrap();
            if prefixes.iter().any(|p| name.starts_with(p)) {
                let rel = format!("{dir}/{name}");
                copy(&repo().join(&rel), &self.path().join(&rel));
            }
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn root(&self) -> Root {
        Root::new(Some(self.path().to_path_buf()))
    }

    /// Writes `text` to a root-relative file, creating its folders.
    pub fn write(&self, rel: &str, text: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    pub fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.path().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.path().join(rel).exists()
    }

    /// Runs `kabar-tools --root <this sandbox> <args…>`.
    pub fn cli(&self, args: &[&str]) -> Result<()> {
        let root = self.path().display().to_string();
        let mut all = vec!["kabar-tools", "--root", root.as_str()];
        all.extend_from_slice(args);
        crate::dispatch(crate::Cli::try_parse_from(all)?)
    }
}

fn copy(from: &Path, to: &Path) {
    if from.is_dir() {
        fs::create_dir_all(to).unwrap();
        for e in fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            copy(&e.path(), &to.join(e.file_name()));
        }
    } else {
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::copy(from, to).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
    }
}

/// Whatever a command printed, for commands that write to a `Vec<u8>`.
pub fn text(out: Vec<u8>) -> String {
    String::from_utf8(out).unwrap()
}
