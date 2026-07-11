use std::{
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// What QuotaBuddy can infer from Hermes' non-secret active configuration.
///
/// Credential stores are deliberately not inspected: even a structural JSON read could
/// materialize OAuth tokens in this process. Consequently, `Configured` only means an
/// `openai-codex` provider entry was found outside the active `model` block, while `Active`
/// means the active `model.provider` (or top-level `active_provider`) is `openai-codex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HermesStatus {
    NotDetected,
    Configured,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HermesConfidence {
    Unknown,
    Inferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HermesProviderContext {
    pub status: HermesStatus,
    pub confidence: HermesConfidence,
}

impl Default for HermesProviderContext {
    fn default() -> Self {
        Self {
            status: HermesStatus::NotDetected,
            confidence: HermesConfidence::Unknown,
        }
    }
}

pub fn detect_hermes_provider() -> HermesProviderContext {
    hermes_home()
        .map(|home| detect_hermes_provider_at(&home))
        .unwrap_or_default()
}

fn hermes_home() -> Option<PathBuf> {
    if let Some(explicit) = env::var_os("HERMES_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(explicit));
    }

    #[cfg(target_os = "windows")]
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        return Some(PathBuf::from(local_app_data).join("hermes"));
    }

    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".hermes"))
}

fn detect_hermes_provider_at(home: &Path) -> HermesProviderContext {
    let config_path = home.join("config.yaml");
    let Ok(file) = File::open(config_path) else {
        return HermesProviderContext::default();
    };

    detect_hermes_provider_from_reader(BufReader::new(file))
}

fn detect_hermes_provider_from_reader(reader: impl BufRead) -> HermesProviderContext {
    let mut active_model_indent = None;
    let mut configured = false;

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = line.len().saturating_sub(line.trim_start().len());
        if let Some(model_indent) = active_model_indent {
            if indent <= model_indent {
                active_model_indent = None;
            }
        }

        let Some((raw_key, raw_value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = raw_key.trim();

        if indent == 0 && key == "model" && raw_value.trim().is_empty() {
            active_model_indent = Some(indent);
            continue;
        }

        // Only these public provider slugs are inspected. No auth store, token field, API key,
        // arbitrary YAML value, account label or identity is read into the result.
        if key != "provider" && key != "active_provider" {
            continue;
        }
        let Some(provider) = normalize_provider_slug(raw_value) else {
            continue;
        };
        if provider != "openai-codex" {
            continue;
        }

        let is_active_provider = (key == "active_provider" && indent == 0)
            || (key == "provider"
                && active_model_indent.is_some_and(|model_indent| indent > model_indent));
        if is_active_provider {
            return HermesProviderContext {
                status: HermesStatus::Active,
                confidence: HermesConfidence::Inferred,
            };
        }
        configured = true;
    }

    if configured {
        HermesProviderContext {
            status: HermesStatus::Configured,
            confidence: HermesConfidence::Inferred,
        }
    } else {
        HermesProviderContext::default()
    }
}

fn normalize_provider_slug(raw_value: &str) -> Option<&str> {
    let value = raw_value
        .split('#')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches(['\'', '"']);
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn detect(source: &str) -> HermesProviderContext {
        detect_hermes_provider_from_reader(Cursor::new(source))
    }

    #[test]
    fn active_model_provider_is_inferred_without_credentials() {
        let detected = detect(
            r#"
model:
  name: gpt-5.3-codex
  provider: openai-codex
access_token: must-never-be-returned
"#,
        );

        assert_eq!(detected.status, HermesStatus::Active);
        assert_eq!(detected.confidence, HermesConfidence::Inferred);
    }

    #[test]
    fn auto_model_provider_is_not_an_active_codex_consumer() {
        let detected = detect(
            r#"
model:
  name: anthropic/claude-opus-4.6
  provider: auto
profiles:
  fallback:
    provider: openai-codex
"#,
        );

        assert_eq!(detected.status, HermesStatus::Configured);
        assert_ne!(detected.status, HermesStatus::Active);
    }

    #[test]
    fn unrelated_provider_is_not_detected() {
        let detected = detect(
            r#"
model:
  provider: auto
api_key: sk-should-not-be-inspected
"#,
        );

        assert_eq!(detected, HermesProviderContext::default());
    }

    #[test]
    fn top_level_active_provider_is_active() {
        let detected = detect("active_provider: 'openai-codex' # public provider slug\n");

        assert_eq!(detected.status, HermesStatus::Active);
    }
}
