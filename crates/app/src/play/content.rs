//! Build-time content shipped inside the app and parsed once, on first use.
//! A parse error becomes a friendly card, never a crash.

use std::sync::OnceLock;

use kabar_core::games::bridge::BridgeData;
use kabar_core::games::pasar::PasarData;
use kabar_core::pagi::StoryBank;

use super::logic;

const BRIDGE_JSONL: &str = include_str!("../../../../content/bridge/id.jsonl");
const SOUND_RULES_JSON: &str = include_str!("../../../../content/bridge/sound-rules.json");
const FREQ_TSV: &str = include_str!("../../../../content/freq/id.tsv");
const PASAR_TOML: &str = include_str!("../../../../content/games/pasar.toml");
const STORIES_JSONL: &str = include_str!("../../../../content/pagi/stories.jsonl");

/// How many of the most frequent words Kabar Pagi may pick new words from.
const NEW_POOL: usize = 3000;

pub type Loaded<T> = Result<T, String>;

/// Runs a parser, turning a panic (a content bug) into an error message.
fn guarded<T>(what: &str, parse: impl FnOnce() -> Result<T, String>) -> Loaded<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(parse))
        .unwrap_or_else(|_| Err(format!("{what}: the parser stopped unexpectedly")))
        .map_err(|e| {
            eprintln!("kabar: loading {what} failed: {e}");
            e
        })
}

pub fn bridge() -> &'static Loaded<BridgeData> {
    static DATA: OnceLock<Loaded<BridgeData>> = OnceLock::new();
    DATA.get_or_init(|| {
        guarded("content/bridge", || BridgeData::parse(BRIDGE_JSONL, SOUND_RULES_JSON, Some(FREQ_TSV)))
    })
}

pub fn pasar() -> &'static Loaded<PasarData> {
    static DATA: OnceLock<Loaded<PasarData>> = OnceLock::new();
    DATA.get_or_init(|| guarded("content/games/pasar.toml", || PasarData::parse(PASAR_TOML)))
}

pub fn stories() -> &'static Loaded<StoryBank> {
    static DATA: OnceLock<Loaded<StoryBank>> = OnceLock::new();
    DATA.get_or_init(|| guarded("content/pagi/stories.jsonl", || StoryBank::parse(STORIES_JSONL)))
}

/// The most frequent Indonesian words, most useful first.
pub fn frequent_words() -> &'static [String] {
    static DATA: OnceLock<Vec<String>> = OnceLock::new();
    DATA.get_or_init(|| logic::top_words(FREQ_TSV, NEW_POOL))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_and_panics_become_messages() {
        assert_eq!(guarded("x", || Ok::<_, String>(1)), Ok(1));
        assert_eq!(guarded::<u8>("x", || Err("bad line 3".into())), Err("bad line 3".into()));
        let r = guarded::<u8>("pack", || panic!("boom"));
        assert!(r.unwrap_err().starts_with("pack:"));
    }

    #[test]
    fn the_shipped_files_are_present() {
        assert!(BRIDGE_JSONL.lines().count() > 100);
        assert!(SOUND_RULES_JSON.contains("fa_p"));
        assert_eq!(frequent_words().len(), NEW_POOL);
    }
}
