use crate::{
    child_process,
    core::{DetectedClient, ProviderId},
};
use serde::{Deserialize, Serialize};

const CLIENT_CANDIDATES: &[(ProviderId, &[&str])] = &[
    (ProviderId::Codex, &["codex.cmd", "codex.exe", "codex"]),
    (
        ProviderId::ClaudeCode,
        &["claude.cmd", "claude.exe", "claude"],
    ),
    (ProviderId::Cursor, &["cursor.cmd", "cursor.exe", "cursor"]),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageIntegration {
    Native,
    OptInBridge,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapability {
    pub provider: ProviderId,
    pub client_detected: bool,
    pub usage_integration: UsageIntegration,
}

/// Returns only clients found on PATH. Missing clients are deliberately absent.
pub fn detect_installed_clients() -> Vec<DetectedClient> {
    detect_with(command_exists)
}

pub fn provider_capabilities() -> Vec<ProviderCapability> {
    let detected = detect_installed_clients();
    [
        (ProviderId::Codex, UsageIntegration::Native),
        (ProviderId::ClaudeCode, UsageIntegration::OptInBridge),
        (ProviderId::Cursor, UsageIntegration::Unavailable),
    ]
    .into_iter()
    .map(|(provider, usage_integration)| ProviderCapability {
        provider,
        client_detected: detected.iter().any(|client| client.provider == provider),
        usage_integration,
    })
    .collect()
}

pub fn detect_with<F>(exists: F) -> Vec<DetectedClient>
where
    F: Fn(&str) -> bool,
{
    CLIENT_CANDIDATES
        .iter()
        .filter_map(|(provider, candidates)| {
            candidates
                .iter()
                .find(|candidate| exists(candidate))
                .map(|executable| DetectedClient {
                    provider: *provider,
                    executable: (*executable).to_owned(),
                })
        })
        .collect()
}

fn command_exists(executable: &str) -> bool {
    let lookup = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    child_process::command(lookup)
        .arg(executable)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omits_clients_not_found_by_detector() {
        let detected = detect_with(|executable| executable == "codex.exe");

        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].provider, ProviderId::Codex);
    }

    #[test]
    fn detects_each_supported_client_without_claiming_usage_access() {
        let detected =
            detect_with(|executable| ["codex.exe", "claude.cmd", "cursor"].contains(&executable));

        assert_eq!(detected.len(), 3);
        assert_eq!(detected[0].provider, ProviderId::Codex);
        assert_eq!(detected[1].provider, ProviderId::ClaudeCode);
        assert_eq!(detected[2].provider, ProviderId::Cursor);
    }

    #[test]
    fn returns_no_clients_when_nothing_is_installed() {
        assert!(detect_with(|_| false).is_empty());
    }
}
