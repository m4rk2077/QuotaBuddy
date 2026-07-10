use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};

use crate::core::{
    Availability, MetricKind, ResetMetadata, SnapshotError, SnapshotStatus, UsageMetric,
    UsageSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterFailure {
    Unavailable,
    ExpiredSession,
    Transient,
}

impl AdapterFailure {
    fn error(self) -> SnapshotError {
        match self {
            Self::Unavailable => SnapshotError {
                code: "unavailable".to_owned(),
                message: "Codex local usage is unavailable.".to_owned(),
            },
            Self::ExpiredSession => SnapshotError {
                code: "reauth_required".to_owned(),
                message: "Sign in again with codex login, then refresh.".to_owned(),
            },
            Self::Transient => SnapshotError {
                code: "retry_later".to_owned(),
                message: "Codex could not refresh right now. Try again shortly.".to_owned(),
            },
        }
    }

    fn status(self) -> SnapshotStatus {
        match self {
            Self::Unavailable => SnapshotStatus::Unavailable,
            Self::ExpiredSession => SnapshotStatus::ReauthRequired,
            Self::Transient => SnapshotStatus::Failed,
        }
    }
}

#[derive(Default)]
pub struct SnapshotCache {
    last_success: Option<UsageSnapshot>,
}

impl SnapshotCache {
    pub fn store_success(&mut self, snapshot: UsageSnapshot) -> UsageSnapshot {
        self.last_success = Some(snapshot.clone());
        snapshot
    }

    pub fn failure(&self, failure: AdapterFailure) -> UsageSnapshot {
        if let Some(last_success) = &self.last_success {
            let mut stale = last_success.clone();
            stale.availability.usage_available = false;
            stale.status = failure.status();
            stale.error = Some(failure.error());
            stale.is_stale = true;
            return stale;
        }

        unavailable_snapshot(failure)
    }
}

pub fn refresh_snapshot(cache: &mut SnapshotCache, executable: &str) -> UsageSnapshot {
    match read_live_snapshot(executable) {
        Ok(snapshot) => cache.store_success(snapshot),
        Err(failure) => cache.failure(failure),
    }
}

fn unavailable_snapshot(failure: AdapterFailure) -> UsageSnapshot {
    UsageSnapshot {
        provider: crate::core::ProviderId::Codex,
        availability: Availability {
            client_detected: true,
            usage_available: false,
        },
        metrics: Vec::new(),
        reset: None,
        last_successful_refresh_at: None,
        status: failure.status(),
        error: Some(failure.error()),
        is_stale: false,
    }
}

fn read_live_snapshot(executable: &str) -> Result<UsageSnapshot, AdapterFailure> {
    let mut rpc = JsonRpcClient::start(executable)?;
    rpc.request(
        "initialize",
        json!({
            "clientInfo": { "name": "QuotaBuddy", "version": env!("CARGO_PKG_VERSION") }
        }),
    )?;
    let rate_limits = rpc.request("account/rateLimits/read", json!({}))?;

    // Usage is read through the same authenticated local session. Its token and bucket
    // fields are intentionally discarded: this slice exposes rate limits only. Discovery
    // marks this experimental method optional, so an error cannot discard valid limits.
    let _usage = rpc.request("account/usage/read", json!({}));

    normalize_rate_limit_value(&rate_limits, &utc_now())
}

struct JsonRpcClient {
    child: Arc<Mutex<Child>>,
    completed: Arc<AtomicBool>,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl JsonRpcClient {
    fn start(executable: &str) -> Result<Self, AdapterFailure> {
        let mut command = if cfg!(target_os = "windows") {
            let mut command = Command::new("cmd.exe");
            // The executable comes only from the fixed detector candidate list.
            command.args([
                "/d",
                "/s",
                "/c",
                &format!("{executable} app-server --stdio"),
            ]);
            command
        } else {
            let mut command = Command::new(executable);
            command.args(["app-server", "--stdio"]);
            command
        };

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| AdapterFailure::Unavailable)?;
        let stdin = child.stdin.take().ok_or(AdapterFailure::Unavailable)?;
        let stdout = child.stdout.take().ok_or(AdapterFailure::Unavailable)?;

        let child = Arc::new(Mutex::new(child));
        let completed = Arc::new(AtomicBool::new(false));
        let watchdog_child = Arc::clone(&child);
        let watchdog_complete = Arc::clone(&completed);
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(8));
            if !watchdog_complete.load(Ordering::Relaxed) {
                if let Ok(mut child) = watchdog_child.lock() {
                    terminate_process_tree(&mut child);
                }
            }
        });

        Ok(Self {
            child,
            completed,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, AdapterFailure> {
        let request_id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        });
        serde_json::to_writer(&mut self.stdin, &request).map_err(|_| AdapterFailure::Transient)?;
        self.stdin
            .write_all(b"\n")
            .map_err(|_| AdapterFailure::Transient)?;
        self.stdin.flush().map_err(|_| AdapterFailure::Transient)?;

        let mut line = String::new();
        for _ in 0..64 {
            line.clear();
            if self
                .stdout
                .read_line(&mut line)
                .map_err(|_| AdapterFailure::Transient)?
                == 0
            {
                return Err(AdapterFailure::Transient);
            }

            let response: Value =
                serde_json::from_str(&line).map_err(|_| AdapterFailure::Transient)?;
            if response.get("id").and_then(Value::as_u64) != Some(request_id) {
                continue;
            }
            if response.get("error").is_some() {
                return Err(classify_rpc_value(&response));
            }
            return response
                .get("result")
                .cloned()
                .ok_or(AdapterFailure::Transient);
        }

        Err(AdapterFailure::Transient)
    }
}

impl Drop for JsonRpcClient {
    fn drop(&mut self) {
        self.completed.store(true, Ordering::Relaxed);
        if let Ok(mut child) = self.child.lock() {
            terminate_process_tree(&mut child);
            let _ = child.wait();
        }
    }
}

fn terminate_process_tree(child: &mut Child) {
    #[cfg(target_os = "windows")]
    {
        // `cmd.exe /c` may otherwise leave the Codex/Node child running after a timeout.
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .output();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = child.kill();
    }
}

#[cfg(test)]
fn normalize_rate_limits(
    response: &str,
    refreshed_at: &str,
) -> Result<UsageSnapshot, AdapterFailure> {
    let response: Value = serde_json::from_str(response).map_err(|_| AdapterFailure::Transient)?;
    normalize_rate_limit_value(&response, refreshed_at)
}

fn normalize_rate_limit_value(
    response: &Value,
    refreshed_at: &str,
) -> Result<UsageSnapshot, AdapterFailure> {
    if response.get("error").is_some() {
        return Err(classify_rpc_value(response));
    }
    let rate_limits = response
        .get("rateLimits")
        .ok_or(AdapterFailure::Transient)?;
    let mut metrics = Vec::new();
    let mut reset = None;

    for (field, kind, label) in [
        ("primary", MetricKind::Session, "Session limit"),
        ("secondary", MetricKind::Cycle, "Longer limit"),
    ] {
        let Some(window) = rate_limits.get(field) else {
            continue;
        };
        let Some(used_percent) = window.get("usedPercent").and_then(Value::as_f64) else {
            continue;
        };
        if !used_percent.is_finite() {
            continue;
        }
        let used_percent = used_percent.clamp(0.0, 100.0);
        let remaining = 100.0 - used_percent;
        metrics.push(UsageMetric {
            kind,
            label: label.to_owned(),
            used_percentage: Some(used_percent),
            remaining: Some(format!("{remaining:.0}% remaining")),
            total: None,
            is_estimate: false,
        });

        if reset.is_none() {
            reset = window
                .get("resetsAt")
                .and_then(normalize_reset_timestamp)
                .map(|resets_at| ResetMetadata {
                    resets_at,
                    label: "Reset time shown in your local timezone.".to_owned(),
                });
        }
    }

    if metrics.is_empty() {
        return Err(AdapterFailure::Transient);
    }

    Ok(UsageSnapshot {
        provider: crate::core::ProviderId::Codex,
        availability: Availability {
            client_detected: true,
            usage_available: true,
        },
        metrics,
        reset,
        last_successful_refresh_at: Some(refreshed_at.to_owned()),
        status: SnapshotStatus::Healthy,
        error: None,
        is_stale: false,
    })
}

#[cfg(test)]
fn classify_rpc_error(response: &str) -> AdapterFailure {
    let response: Value = match serde_json::from_str(response) {
        Ok(response) => response,
        Err(_) => return AdapterFailure::Transient,
    };
    classify_rpc_value(&response)
}

fn classify_rpc_value(response: &Value) -> AdapterFailure {
    let message = response
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ["login", "auth", "session", "unauthorized", "forbidden"]
        .iter()
        .any(|needle| message.contains(needle))
    {
        AdapterFailure::ExpiredSession
    } else {
        AdapterFailure::Transient
    }
}

fn normalize_reset_timestamp(value: &Value) -> Option<String> {
    let raw = value.as_i64()?;
    let seconds = if raw.abs() >= 100_000_000_000 {
        raw / 1_000
    } else {
        raw
    };
    unix_seconds_to_rfc3339(seconds)
}

fn unix_seconds_to_rfc3339(seconds: i64) -> Option<String> {
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days)?;
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn civil_from_days(days_since_epoch: i64) -> Option<(i64, i64, i64)> {
    let z = days_since_epoch.checked_add(719_468)?;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_prime = (5 * doy + 2) / 153;
    let day = doy - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    Some((year, month, day))
}

fn utc_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    unix_seconds_to_rfc3339(seconds).unwrap_or_else(|| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_only_allowed_rate_limit_fields() {
        let response = include_str!("../fixtures/codex/normal.json");
        let snapshot =
            normalize_rate_limits(response, "2026-07-10T10:00:00Z").expect("normal fixture");

        assert_eq!(snapshot.metrics.len(), 2);
        assert_eq!(snapshot.metrics[0].kind, MetricKind::Session);
        assert_eq!(snapshot.metrics[0].used_percentage, Some(28.0));
        assert_eq!(
            snapshot.metrics[0].remaining.as_deref(),
            Some("72% remaining")
        );
        assert!(snapshot.reset.is_some());
        assert!(snapshot.error.is_none());
    }

    #[test]
    fn normalizes_a_reached_limit_without_special_provider_text() {
        let response = include_str!("../fixtures/codex/limit_reached.json");
        let snapshot =
            normalize_rate_limits(response, "2026-07-10T10:00:00Z").expect("limit fixture");

        assert_eq!(snapshot.metrics[0].used_percentage, Some(100.0));
        assert_eq!(
            snapshot.metrics[0].remaining.as_deref(),
            Some("0% remaining")
        );
    }

    #[test]
    fn accepts_millisecond_reset_times() {
        let response = include_str!("../fixtures/codex/reset_pending.json");
        let snapshot =
            normalize_rate_limits(response, "2026-07-10T10:00:00Z").expect("reset fixture");

        assert_eq!(snapshot.reset.unwrap().resets_at, "2026-07-11T12:00:00Z");
    }

    #[test]
    fn classifies_expired_sessions_without_returning_provider_diagnostics() {
        let response = include_str!("../fixtures/codex/session_expired.json");

        assert_eq!(classify_rpc_error(response), AdapterFailure::ExpiredSession);
    }

    #[test]
    fn classifies_transient_failures_without_returning_provider_diagnostics() {
        let response = include_str!("../fixtures/codex/transient_failure.json");

        assert_eq!(classify_rpc_error(response), AdapterFailure::Transient);
    }

    #[test]
    fn retains_last_successful_snapshot_when_refresh_fails() {
        let success = normalize_rate_limits(
            include_str!("../fixtures/codex/normal.json"),
            "2026-07-10T10:00:00Z",
        )
        .expect("normal fixture");
        let mut cache = SnapshotCache::default();

        assert!(!cache.store_success(success).is_stale);
        let stale = cache.failure(AdapterFailure::Transient);

        assert!(stale.is_stale);
        assert_eq!(stale.status, SnapshotStatus::Failed);
        assert_eq!(stale.error.unwrap().code, "retry_later");
        assert_eq!(stale.metrics[0].used_percentage, Some(28.0));
    }
}
