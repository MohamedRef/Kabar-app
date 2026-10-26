//! Worker configuration: `[vars]` from wrangler.toml plus Worker secrets.
//! Built from a lookup function so tests don't need the Cloudflare runtime.

use std::fmt;

use kabar_protocol::ai::EngineId;
use kabar_protocol::cloud::{EngineStatus, VaultStatus};

use crate::tokens::TokenKey;

pub const DEFAULT_WORKERS_AI_MODEL: &str = "@cf/aisingapore/gemma-sea-lion-v4-27b-it";
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-opus-5-5";

/// A key from the vault. Never printed, never sent to a device.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString(***)")
    }
}

/// Hard stops, kept under each service's free limit (see docs/PLAN.md §8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    pub workers_ai_daily_requests: u64,
    pub gemini_daily_requests: u64,
    pub claude_monthly_requests: u64,
    /// Azure F0 gives 500k neural characters a month.
    pub tts_monthly_chars: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            workers_ai_daily_requests: 300,
            gemini_daily_requests: 1_000,
            claude_monthly_requests: 300,
            tts_monthly_chars: 450_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    /// OAuth client ids whose Google ID tokens we accept (`aud`).
    pub google_client_ids: Vec<String>,
    /// Lower-cased emails allowed to sign in.
    pub allowed_emails: Vec<String>,
    pub token_key: Option<TokenKey>,
    pub workers_ai_model: String,
    /// The `AI` binding exists (always true on a real deployment).
    pub workers_ai_bound: bool,
    pub gemini_model: String,
    pub gemini_key: Option<SecretString>,
    pub claude_model: String,
    pub claude_key: Option<SecretString>,
    pub claude_enabled: bool,
    pub azure_key: Option<SecretString>,
    pub azure_region: Option<String>,
    /// Firebase service account JSON (Worker secret `FCM_SERVICE_ACCOUNT`),
    /// for Kak Nur's push reminders.
    pub fcm_service_account: Option<SecretString>,
    pub limits: Limits,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{0} is not set")]
    Missing(&'static str),
    #[error("{0} is invalid: {1}")]
    Invalid(&'static str, String),
}

fn list(s: &str) -> Vec<String> {
    s.split([',', '\n', ' ']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect()
}

impl Config {
    /// `get` returns a var or secret by name; empty strings count as unset.
    pub fn from_lookup(
        get: impl Fn(&str) -> Option<String>,
        workers_ai_bound: bool,
    ) -> Result<Self, ConfigError> {
        let get = |name: &str| get(name).map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        let secret = |name: &str| get(name).map(SecretString);
        let num = |name: &'static str, default: u64| -> Result<u64, ConfigError> {
            match get(name) {
                None => Ok(default),
                Some(v) => v.parse().map_err(|_| ConfigError::Invalid(name, v)),
            }
        };
        let token_key = match get("TOKEN_SIGNING_KEY") {
            None => None,
            Some(v) => {
                Some(TokenKey::from_base64(&v).map_err(|e| ConfigError::Invalid("TOKEN_SIGNING_KEY", e))?)
            }
        };
        let d = Limits::default();
        Ok(Self {
            google_client_ids: get("GOOGLE_CLIENT_IDS").map(|v| list(&v)).unwrap_or_default(),
            allowed_emails: get("ALLOWED_EMAILS")
                .map(|v| list(&v).into_iter().map(|e| e.to_lowercase()).collect())
                .unwrap_or_default(),
            token_key,
            workers_ai_model: get("WORKERS_AI_MODEL").unwrap_or_else(|| DEFAULT_WORKERS_AI_MODEL.into()),
            workers_ai_bound,
            gemini_model: get("GEMINI_MODEL").unwrap_or_else(|| DEFAULT_GEMINI_MODEL.into()),
            gemini_key: secret("GEMINI_API_KEY"),
            claude_model: get("CLAUDE_MODEL").unwrap_or_else(|| DEFAULT_CLAUDE_MODEL.into()),
            claude_key: secret("ANTHROPIC_API_KEY"),
            claude_enabled: get("CLAUDE_ENABLED").is_some_and(|v| v == "true"),
            azure_key: secret("AZURE_SPEECH_KEY"),
            azure_region: get("AZURE_SPEECH_REGION"),
            fcm_service_account: secret("FCM_SERVICE_ACCOUNT"),
            limits: Limits {
                workers_ai_daily_requests: num("WORKERS_AI_DAILY_REQUESTS", d.workers_ai_daily_requests)?,
                gemini_daily_requests: num("GEMINI_DAILY_REQUESTS", d.gemini_daily_requests)?,
                claude_monthly_requests: num("CLAUDE_MONTHLY_REQUESTS", d.claude_monthly_requests)?,
                tts_monthly_chars: num("TTS_MONTHLY_CHARS", d.tts_monthly_chars)?,
            },
        })
    }

    /// Sign-in needs these three; everything else is optional.
    pub fn auth_ready(&self) -> Result<&TokenKey, ConfigError> {
        if self.google_client_ids.is_empty() {
            return Err(ConfigError::Missing("GOOGLE_CLIENT_IDS"));
        }
        if self.allowed_emails.is_empty() {
            return Err(ConfigError::Missing("ALLOWED_EMAILS"));
        }
        self.token_key.as_ref().ok_or(ConfigError::Missing("TOKEN_SIGNING_KEY"))
    }

    pub fn email_allowed(&self, email: &str) -> bool {
        let email = email.trim().to_lowercase();
        self.allowed_emails.contains(&email)
    }

    pub fn tts_ready(&self) -> bool {
        self.azure_key.is_some() && self.azure_region.is_some()
    }

    /// The key vault's public face: which engines can run. Never the keys.
    pub fn vault_status(&self) -> VaultStatus {
        let engine = |engine, configured, enabled, model: &str| EngineStatus {
            engine,
            configured,
            enabled,
            model: Some(model.to_owned()),
        };
        VaultStatus {
            engines: vec![
                engine(EngineId::WorkersAi, self.workers_ai_bound, true, &self.workers_ai_model),
                engine(EngineId::GeminiFree, self.gemini_key.is_some(), true, &self.gemini_model),
                engine(EngineId::Claude, self.claude_key.is_some(), self.claude_enabled, &self.claude_model),
            ],
            tts_configured: self.tts_ready(),
            push_configured: self.fcm_service_account.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn cfg(pairs: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<String, String> =
            pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Config::from_lookup(|k| map.get(k).cloned(), true)
    }

    #[test]
    fn defaults_keep_claude_off_and_limits_under_free_tiers() {
        let c = cfg(&[]).unwrap();
        assert!(!c.claude_enabled);
        assert_eq!(c.workers_ai_model, DEFAULT_WORKERS_AI_MODEL);
        assert!(c.limits.tts_monthly_chars < 500_000);
        assert_eq!(c.auth_ready().unwrap_err(), ConfigError::Missing("GOOGLE_CLIENT_IDS"));
        let status = c.vault_status();
        assert!(!status.tts_configured);
        assert!(!status.push_configured);
        assert!(status.engines.iter().any(|e| e.engine == EngineId::Claude && !e.enabled && !e.configured));
    }

    #[test]
    fn allowlist_is_case_insensitive_and_comma_separated() {
        let c = cfg(&[("ALLOWED_EMAILS", " Learner@Example.com, other@example.com ")]).unwrap();
        assert!(c.email_allowed("learner@example.com"));
        assert!(c.email_allowed("OTHER@example.com"));
        assert!(!c.email_allowed("someone@example.com"));
    }

    #[test]
    fn secrets_never_print() {
        let c = cfg(&[("GEMINI_API_KEY", "AIzaVerySecret")]).unwrap();
        assert!(!format!("{c:?}").contains("AIzaVerySecret"));
        assert_eq!(c.gemini_key.unwrap().expose(), "AIzaVerySecret");
    }

    #[test]
    fn bad_numbers_are_reported() {
        assert_eq!(
            cfg(&[("TTS_MONTHLY_CHARS", "lots")]).unwrap_err(),
            ConfigError::Invalid("TTS_MONTHLY_CHARS", "lots".into())
        );
    }
}
