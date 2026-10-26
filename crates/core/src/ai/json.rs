//! Getting strict JSON out of model text, and hashing inputs for the cache.

use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Models wrap JSON in ```json fences or a sentence of preamble. Returns the
/// outermost object/array, or the trimmed text if none is found.
pub fn extract_json(text: &str) -> &str {
    let t = text.trim();
    let start = t.find(['{', '[']);
    let end = t.rfind(['}', ']']);
    match (start, end) {
        (Some(s), Some(e)) if e > s => &t[s..=e],
        _ => t,
    }
}

/// Parses the JSON in `text`; the error message is fed back to the model on retry.
pub fn parse<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    serde_json::from_str(extract_json(text)).map_err(|e| e.to_string())
}

/// JSON with object keys sorted at every level, so the same input hashes the
/// same in the app and in Kabar Cloud whatever serde_json features are on.
pub fn canonical(v: &Value) -> String {
    let mut out = String::new();
    write_canonical(v, &mut out);
    out
}

fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                write_canonical(&map[k], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

/// SHA-256 hex of the canonical JSON.
pub fn input_hash(v: &Value) -> String {
    Sha256::digest(canonical(v).as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_from_fences_and_preamble() {
        assert_eq!(extract_json("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(extract_json("Here you go: {\"a\":{\"b\":2}} Hope it helps!"), "{\"a\":{\"b\":2}}");
        assert_eq!(extract_json("  [1,2] "), "[1,2]");
        assert_eq!(extract_json("no json"), "no json");
    }

    #[test]
    fn hash_ignores_key_order() {
        let a = json!({"text": "piye dhe", "meta": {"x": 1, "y": [1, 2]}});
        let b = json!({"meta": {"y": [1, 2], "x": 1}, "text": "piye dhe"});
        assert_eq!(canonical(&a), r#"{"meta":{"x":1,"y":[1,2]},"text":"piye dhe"}"#);
        assert_eq!(input_hash(&a), input_hash(&b));
        assert_eq!(input_hash(&a).len(), 64);
        assert_ne!(input_hash(&a), input_hash(&json!({"text": "piye"})));
    }
}
