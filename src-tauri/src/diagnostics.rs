use serde::Serialize;
#[cfg(test)]
use serde_json::Value;
use std::{fs, io, path::Path};

#[cfg(test)]
use crate::core::redact_sensitive_text;
use crate::spend::SpendEstimate;

#[cfg(test)]
const SENSITIVE_KEYWORDS: &[&str] = &[
    "token",
    "session",
    "secret",
    "password",
    "authorization",
    "credential",
    "api_key",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticExport {
    pub schema_version: &'static str,
    pub pricing_table_version: String,
    pub estimated_spend: SpendEstimate,
    pub local_log_record_count: usize,
    pub notes: Vec<String>,
}

#[cfg(test)]
pub fn redact_diagnostics(input: &str) -> String {
    match serde_json::from_str::<Value>(input) {
        Ok(value) => serde_json::to_string_pretty(&redact_value(value))
            .expect("redacted JSON always serializes"),
        Err(_) => redact_sensitive_text(input),
    }
}

pub fn write_diagnostic_export(destination: &Path, estimate: SpendEstimate) -> io::Result<()> {
    let report = DiagnosticExport {
        schema_version: "1",
        pricing_table_version: estimate.pricing_table_version.clone(),
        local_log_record_count: estimate.record_count,
        estimated_spend: estimate,
        notes: vec![
            "Export contains no raw local log content.".to_owned(),
            "Token, session, and secret fields are excluded from diagnostics.".to_owned(),
        ],
    };
    let content =
        serde_json::to_string_pretty(&report).expect("diagnostic report always serializes");
    fs::write(destination, content)
}

#[cfg(test)]
fn redact_value(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| !is_sensitive_key(key))
                .map(|(key, value)| (key, redact_value(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_value).collect()),
        Value::String(value) => Value::String(redact_sensitive_text(&value)),
        other => other,
    }
}

#[cfg(test)]
fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
    SENSITIVE_KEYWORDS
        .iter()
        .any(|keyword| normalized.contains(&keyword.replace('_', "")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_diagnostics_remove_tokens_sessions_and_secret_values() {
        let exported = redact_diagnostics(
            r#"{"token":"token-value","session_id":"session-value","api_key":"key-value","status":"failed"}"#,
        );

        assert!(!exported.contains("token-value"));
        assert!(!exported.contains("session-value"));
        assert!(!exported.contains("key-value"));
        assert!(exported.contains("failed"));
    }
}
