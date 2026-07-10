use regex::Regex;
use serde::{Deserialize, Serialize};

/// Provider-neutral data allowed to cross the Rust-to-frontend boundary.
/// Credential and session material intentionally have no representation here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub provider: ProviderId,
    pub availability: Availability,
    pub metrics: Vec<UsageMetric>,
    pub reset: Option<ResetMetadata>,
    pub last_successful_refresh_at: Option<String>,
    pub status: SnapshotStatus,
    pub error: Option<SnapshotError>,
    pub is_stale: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderId {
    Codex,
    ClaudeCode,
    Cursor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    pub client_detected: bool,
    pub usage_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetric {
    pub kind: MetricKind,
    pub label: String,
    pub used_percentage: Option<f64>,
    pub remaining: Option<String>,
    pub total: Option<String>,
    pub is_estimate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetricKind {
    Session,
    Weekly,
    Cycle,
    Credits,
    EstimatedSpend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetMetadata {
    pub resets_at: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotStatus {
    Healthy,
    Unavailable,
    Failed,
    ReauthRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedClient {
    pub provider: ProviderId,
    pub executable: String,
}

/// Last boundary before a normalized snapshot crosses into the webview.
/// Adapter text is treated as untrusted because provider responses can contain diagnostics.
pub fn sanitize_for_frontend(mut snapshot: UsageSnapshot) -> UsageSnapshot {
    for metric in &mut snapshot.metrics {
        metric.label = redact_sensitive_text(&metric.label);
        metric.remaining = metric.remaining.as_deref().map(redact_sensitive_text);
        metric.total = metric.total.as_deref().map(redact_sensitive_text);
    }

    if let Some(reset) = &mut snapshot.reset {
        reset.resets_at = redact_sensitive_text(&reset.resets_at);
        reset.label = redact_sensitive_text(&reset.label);
    }

    snapshot.last_successful_refresh_at = snapshot
        .last_successful_refresh_at
        .as_deref()
        .map(redact_sensitive_text);

    if let Some(error) = &mut snapshot.error {
        error.code = redact_sensitive_text(&error.code);
        error.message = redact_sensitive_text(&error.message);
    }

    snapshot
}

/// Removes common credential shapes before a diagnostic string can be persisted or displayed.
pub fn redact_sensitive_text(input: &str) -> String {
    let authorization = Regex::new(r"(?i)(authorization\s*:\s*(?:bearer\s+)?)[^\s,;]+")
        .expect("static authorization redaction regex is valid");
    let fields = Regex::new(
        r#"(?i)((?:api[_-]?key|access[_-]?token|refresh[_-]?token|token|secret|password)"?\s*[:=]\s*"?)[^"\s,;}\]]+"#,
    )
    .expect("static field redaction regex is valid");
    let bearer = Regex::new(r"(?i)(bearer\s+)[A-Za-z0-9._~+\-/=]+")
        .expect("static bearer redaction regex is valid");

    let redacted = authorization.replace_all(input, "$1[REDACTED]");
    let redacted = fields.replace_all(&redacted, "$1[REDACTED]");
    bearer.replace_all(&redacted, "$1[REDACTED]").into_owned()
}

pub fn log_redacted(message: &str) {
    eprintln!("{}", redact_sensitive_text(message));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_fixture_round_trips_without_credential_fields() {
        let fixture = include_str!("../fixtures/usage_snapshot.json");
        let snapshot: UsageSnapshot =
            serde_json::from_str(fixture).expect("valid contract fixture");
        let serialized = serde_json::to_string(&snapshot).expect("serializable contract");

        assert_eq!(snapshot.provider, ProviderId::Codex);
        assert!(snapshot.availability.client_detected);
        assert!(snapshot.availability.usage_available);
        assert!(!serialized.to_lowercase().contains("token"));
        assert!(!serialized.to_lowercase().contains("secret"));
        assert!(!serialized.to_lowercase().contains("credential"));
    }

    #[test]
    fn diagnostics_redact_bearer_and_named_secrets() {
        let source = "Authorization: Bearer top-secret-value api_key=another-secret {\"token\":\"third-secret\"}";
        let redacted = redact_sensitive_text(source);

        assert!(!redacted.contains("top-secret-value"));
        assert!(!redacted.contains("another-secret"));
        assert!(!redacted.contains("third-secret"));
        assert_eq!(redacted.matches("[REDACTED]").count(), 3);
    }

    #[test]
    fn frontend_boundary_redacts_untrusted_snapshot_text() {
        let snapshot = UsageSnapshot {
            provider: ProviderId::Codex,
            availability: Availability {
                client_detected: true,
                usage_available: false,
            },
            metrics: vec![UsageMetric {
                kind: MetricKind::Session,
                label: "token=metric-secret".to_owned(),
                used_percentage: None,
                remaining: Some("Bearer remaining-secret".to_owned()),
                total: None,
                is_estimate: false,
            }],
            reset: None,
            last_successful_refresh_at: None,
            status: SnapshotStatus::Failed,
            error: Some(SnapshotError {
                code: "api_key=error-secret".to_owned(),
                message: "Authorization: Bearer message-secret".to_owned(),
            }),
            is_stale: true,
        };

        let serialized = serde_json::to_string(&sanitize_for_frontend(snapshot))
            .expect("sanitized snapshot serializes");

        for secret in [
            "metric-secret",
            "remaining-secret",
            "error-secret",
            "message-secret",
        ] {
            assert!(!serialized.contains(secret));
        }
    }
}
