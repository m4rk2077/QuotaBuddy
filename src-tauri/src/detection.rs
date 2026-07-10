use std::process::Command;

use crate::core::{DetectedClient, ProviderId};

const CLIENT_CANDIDATES: &[(ProviderId, &[&str])] = &[
    (ProviderId::Codex, &["codex.exe", "codex"]),
    (ProviderId::ClaudeCode, &["claude.exe", "claude"]),
    (ProviderId::Cursor, &["cursor.exe", "cursor"]),
];

/// Returns only clients found on PATH. Missing clients are deliberately absent.
pub fn detect_installed_clients() -> Vec<DetectedClient> {
    detect_with(command_exists)
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
    Command::new(lookup)
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
    fn returns_no_clients_when_nothing_is_installed() {
        assert!(detect_with(|_| false).is_empty());
    }
}
