//! JSON Schemas for content, generated from the Rust types in
//! `kabar_protocol::content` so the two can never drift. The schemas are what
//! generation prompts show the model and what editors validate against.

use std::fs;

use anyhow::{Result, bail};
use kabar_protocol::content::{BridgeEntry, DictEntry, GlossEntry, SoundRule};
use schemars::{JsonSchema, schema_for};

use crate::paths::Root;

/// `(file name, pretty JSON)` for every content schema.
pub fn all() -> Vec<(&'static str, String)> {
    fn render<T: JsonSchema>() -> String {
        serde_json::to_string_pretty(&schema_for!(T)).expect("schemas serialize") + "\n"
    }
    vec![
        ("gloss-entry.schema.json", render::<GlossEntry>()),
        ("bridge-entry.schema.json", render::<BridgeEntry>()),
        ("sound-rule.schema.json", render::<SoundRule>()),
        ("dict-entry.schema.json", render::<DictEntry>()),
    ]
}

pub fn run(root: &Root, check: bool) -> Result<()> {
    let dir = root.schemas();
    fs::create_dir_all(&dir)?;
    let mut stale = Vec::new();
    for (name, schema) in all() {
        let path = dir.join(name);
        if fs::read_to_string(&path).ok().as_deref() == Some(schema.as_str()) {
            continue;
        }
        if check {
            stale.push(name);
        } else {
            fs::write(&path, schema)?;
            println!("wrote schemas/{name}");
        }
    }
    if !stale.is_empty() {
        bail!("out of date: {} (run `kabar-tools schemas`)", stale.join(", "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use kabar_protocol::content::Lang;

    use super::*;
    use crate::{bridge, gloss};

    fn root() -> Root {
        Root::new(None)
    }

    #[test]
    fn committed_schemas_are_current() {
        run(&root(), true).unwrap();
    }

    fn check_jsonl(schema: &str, path: &Path) -> usize {
        let schema: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root().schemas().join(schema)).unwrap()).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let mut n = 0;
        for (i, line) in fs::read_to_string(path).unwrap().lines().enumerate() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            let errors: Vec<String> = validator.iter_errors(&value).map(|e| e.to_string()).collect();
            assert!(errors.is_empty(), "{}:{}: {errors:?}", path.display(), i + 1);
            n += 1;
        }
        n
    }

    /// Every committed content file matches its JSON Schema and passes the
    /// pipeline's own checks, so CI rejects broken content.
    #[test]
    fn committed_content_is_valid() {
        let root = root();
        let dir = root.gloss_dir(Lang::Id);
        let mut glossed = 0;
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                glossed += check_jsonl("gloss-entry.schema.json", &path);
            }
        }
        gloss::run(&root, gloss::Cmd::Validate { batch: None }).unwrap();

        let cards = root.bridge_dir().join("id.jsonl");
        if cards.exists() {
            check_jsonl("bridge-entry.schema.json", &cards);
            bridge::run(&root, bridge::Cmd::Validate).unwrap();
            let issues = gloss::crosscheck_bridge(&root, Lang::Id).unwrap();
            assert!(issues.is_empty(), "{issues:?}");
        }
        let wikt = bridge::wiktionary_path(&root);
        if wikt.exists() {
            check_jsonl("dict-entry.schema.json", &wikt);
        }
        eprintln!("{glossed} glosses checked");
    }
}
