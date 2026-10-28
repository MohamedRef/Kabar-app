// Where the app's built-in content comes from. Shared, as source, by
// `crates/app/build.rs` (which `include!`s this file to embed it) and
// `kabar_protocol::bundle` (which `kabar-tools bundle build` uses to pack
// the same files for the update feed), so both name every file alike.
// Only std here: build.rs has no dependencies on the workspace.

/// One part of the built-in content.
#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// Every `.yaml` under `dir` (recursively), by file stem.
    Table { name: &'static str, dir: &'static str },
    /// Every `.jsonl` directly in `dir`, by file stem, in name order.
    List { name: &'static str, dir: &'static str },
    /// One file.
    Single { name: &'static str, path: &'static str },
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Table { name, .. } | Source::List { name, .. } | Source::Single { name, .. } => name,
        }
    }
}

/// Everything the app embeds and a content bundle replaces, by repository path.
pub const SOURCES: &[Source] = &[
    Source::Table { name: "LESSONS", dir: "content/lessons" },
    Source::Table { name: "TESTS", dir: "content/tests" },
    Source::Table { name: "STORIES", dir: "content/stories" },
    Source::Table { name: "RADIO", dir: "content/radio" },
    Source::Table { name: "PERSONAS", dir: "content/personas" },
    // The grammar library for "Explain this sentence".
    Source::Table { name: "GRAMMAR", dir: "content/grammar" },
    Source::List { name: "GLOSS_BATCHES", dir: "content/glosses/id" },
    Source::List { name: "GLOSS_BATCHES_JV", dir: "content/glosses/jv" },
    Source::Single { name: "BRIDGE_ID", path: "content/bridge/id.jsonl" },
    Source::Single { name: "FREQ_ID", path: "content/freq/id.tsv" },
    Source::Single { name: "SPEECH_LEVELS", path: "content/javanese/speech-levels.jsonl" },
    Source::Single { name: "CAST", path: "content/cast.yaml" },
    Source::Single { name: "CURRICULUM_ID", path: "curriculum/indonesian.yaml" },
    Source::Single { name: "CURRICULUM_JV", path: "curriculum/javanese.yaml" },
];

/// A stylesheet's key in a bundle: this, then its path under `crates/app`.
pub const CSS_PREFIX: &str = "css/";

/// The files of one source as `(key, path)`, in a stable order. A table
/// row's key is `NAME/<stem>`; a single file's is `NAME`.
pub fn source_files(root: &std::path::Path, source: Source) -> Vec<(String, std::path::PathBuf)> {
    fn yaml_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                yaml_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "yaml") {
                out.push(path);
            }
        }
    }
    let stem = |p: &std::path::Path| p.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    match source {
        Source::Single { name, path } => vec![(name.to_string(), root.join(path))],
        Source::Table { name, dir } => {
            let mut files = Vec::new();
            yaml_files(&root.join(dir), &mut files);
            files.sort();
            files.into_iter().map(|f| (format!("{name}/{}", stem(&f)), f)).collect()
        }
        Source::List { name, dir } => {
            let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
                        .collect()
                })
                .unwrap_or_default();
            files.sort();
            files.into_iter().map(|f| (format!("{name}/{}", stem(&f)), f)).collect()
        }
    }
}
