use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[path = "provider_context.rs"]
pub mod provider_context;

use provider_context::{
    detect_hermes_provider, HermesConfidence, HermesProviderContext, HermesStatus,
};

use crate::{
    child_process,
    core::{
        Availability, MetricKind, ResetMetadata, SnapshotError, SnapshotStatus, UsageMetric,
        UsageSnapshot,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterFailure {
    Unavailable,
    ExpiredSession,
    Transient,
}

/// Allowlisted account-wide Codex activity. This never contains identity or credential fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTokenActivity {
    pub summary: AccountTokenActivitySummary,
    pub daily_buckets: Vec<AccountTokenActivityDailyBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTokenActivitySummary {
    pub lifetime_tokens: Option<i64>,
    pub peak_daily_tokens: Option<i64>,
    pub longest_running_turn_sec: Option<i64>,
    pub current_streak_days: Option<i64>,
    pub longest_streak_days: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTokenActivityDailyBucket {
    pub start_date: String,
    pub tokens: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexAuthMode {
    Chatgpt,
    ApiKey,
    AmazonBedrock,
    Unknown,
}

/// Privacy-safe description of the active quota scope and Hermes configuration inference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProfileScope {
    pub auth_mode: CodexAuthMode,
    pub plan_type: Option<String>,
    pub scope_label: String,
    pub hermes_status: HermesStatus,
    pub hermes_confidence: HermesConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountContext {
    pub profile_scope: CodexProfileScope,
    pub token_activity: Option<AccountTokenActivity>,
}

/// Combined refresh result for callers that need the optional account context.
/// Existing callers can keep using [`refresh_snapshot`] unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRefreshResult {
    pub snapshot: UsageSnapshot,
    pub context: Option<CodexAccountContext>,
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

#[allow(dead_code)] // Kept as the snapshot-only adapter boundary for existing native callers.
pub fn refresh_snapshot(cache: &mut SnapshotCache, executable: &str) -> UsageSnapshot {
    refresh_snapshot_with_context(cache, executable).snapshot
}

pub fn refresh_snapshot_with_context(
    cache: &mut SnapshotCache,
    executable: &str,
) -> CodexRefreshResult {
    match read_live_snapshot_with_context(executable) {
        Ok(mut result) => {
            result.snapshot = cache.store_success(result.snapshot);
            result
        }
        Err(failure) => CodexRefreshResult {
            snapshot: cache.failure(failure),
            context: None,
        },
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

fn read_live_snapshot_with_context(executable: &str) -> Result<CodexRefreshResult, AdapterFailure> {
    let mut rpc = JsonRpcClient::start(executable)?;
    rpc.request(
        "initialize",
        json!({
            "clientInfo": { "name": "QuotaBuddy", "version": env!("CARGO_PKG_VERSION") }
        }),
    )?;
    rpc.notify("initialized", json!({}))?;

    // Fetch the mandatory quota first. If either optional RPC is absent, errors, or times out on
    // an older Codex build, the already-normalized rate limits still remain usable.
    let rate_limits = rpc.request("account/rateLimits/read", json!({}))?;

    // Account metadata is optional and never proactively refreshes credentials. The response is
    // normalized through an allowlist that discards email, identifiers, claims and raw payloads.
    let account = rpc
        .request("account/read", json!({ "refreshToken": false }))
        .ok();

    // Usage is read through the same authenticated local session. It remains best-effort: older
    // Codex versions may not expose this method, and that must never discard valid rate limits.
    let usage = rpc.request("account/usage/read", json!({})).ok();

    normalize_live_values(
        &rate_limits,
        account.as_ref(),
        usage.as_ref(),
        detect_hermes_provider(),
        &utc_now(),
    )
}

fn normalize_live_values(
    rate_limits: &Value,
    account: Option<&Value>,
    usage: Option<&Value>,
    hermes: HermesProviderContext,
    refreshed_at: &str,
) -> Result<CodexRefreshResult, AdapterFailure> {
    let snapshot = normalize_rate_limit_value(rate_limits, refreshed_at)?;
    let context = CodexAccountContext {
        profile_scope: normalize_profile_scope(account, hermes),
        token_activity: usage.and_then(normalize_account_token_activity),
    };

    Ok(CodexRefreshResult {
        snapshot,
        context: Some(context),
    })
}

fn normalize_profile_scope(
    account: Option<&Value>,
    hermes: HermesProviderContext,
) -> CodexProfileScope {
    let account_type = account
        .and_then(|response| response.get("account"))
        .and_then(|account| account.get("type"))
        .and_then(Value::as_str);
    let auth_mode = match account_type {
        Some("chatgpt") => CodexAuthMode::Chatgpt,
        Some("apiKey") => CodexAuthMode::ApiKey,
        Some("amazonBedrock") => CodexAuthMode::AmazonBedrock,
        _ => CodexAuthMode::Unknown,
    };
    let plan_type = if auth_mode == CodexAuthMode::Chatgpt {
        account
            .and_then(|response| response.pointer("/account/planType"))
            .and_then(Value::as_str)
            .and_then(normalize_plan_type)
            .map(str::to_owned)
    } else {
        None
    };
    let scope_label = match auth_mode {
        CodexAuthMode::Chatgpt => "OpenAI account · shared Codex quota",
        CodexAuthMode::ApiKey => "OpenAI API account",
        CodexAuthMode::AmazonBedrock => "Amazon Bedrock account",
        CodexAuthMode::Unknown => "Codex account quota",
    }
    .to_owned();

    CodexProfileScope {
        auth_mode,
        plan_type,
        scope_label,
        hermes_status: hermes.status,
        hermes_confidence: hermes.confidence,
    }
}

fn normalize_account_token_activity(response: &Value) -> Option<AccountTokenActivity> {
    let summary = response.get("summary")?;
    let summary = AccountTokenActivitySummary {
        lifetime_tokens: nonnegative_i64(summary.get("lifetimeTokens")),
        peak_daily_tokens: nonnegative_i64(summary.get("peakDailyTokens")),
        longest_running_turn_sec: nonnegative_i64(summary.get("longestRunningTurnSec")),
        current_streak_days: nonnegative_i64(summary.get("currentStreakDays")),
        longest_streak_days: nonnegative_i64(summary.get("longestStreakDays")),
    };
    let mut daily_buckets: Vec<_> = response
        .get("dailyUsageBuckets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|bucket| {
            let start_date = bucket.get("startDate").and_then(Value::as_str)?;
            if !is_iso_date(start_date) {
                return None;
            }
            let tokens = nonnegative_i64(bucket.get("tokens"))?;
            Some(AccountTokenActivityDailyBucket {
                start_date: start_date.to_owned(),
                tokens,
            })
        })
        .collect();
    daily_buckets.sort_by(|left, right| left.start_date.cmp(&right.start_date));
    let excess = daily_buckets.len().saturating_sub(1_000);
    daily_buckets.drain(..excess);

    Some(AccountTokenActivity {
        summary,
        daily_buckets,
    })
}

fn nonnegative_i64(value: Option<&Value>) -> Option<i64> {
    const MAX_SAFE_JSON_INTEGER: i64 = 9_007_199_254_740_991;
    value
        .and_then(Value::as_i64)
        .filter(|value| (0..=MAX_SAFE_JSON_INTEGER).contains(value))
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

fn normalize_plan_type(value: &str) -> Option<&str> {
    [
        "free",
        "go",
        "plus",
        "pro",
        "prolite",
        "team",
        "self_serve_business_usage_based",
        "business",
        "enterprise_cbp_usage_based",
        "enterprise",
        "edu",
        "unknown",
    ]
    .contains(&value)
    .then_some(value)
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
            let mut command = child_process::command("cmd.exe");
            // The executable comes only from the fixed detector candidate list.
            command.args([
                "/d",
                "/s",
                "/c",
                &format!("{executable} app-server --stdio"),
            ]);
            command
        } else {
            let mut command = child_process::command(executable);
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

    fn notify(&mut self, method: &str, params: Value) -> Result<(), AdapterFailure> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        serde_json::to_writer(&mut self.stdin, &notification)
            .map_err(|_| AdapterFailure::Transient)?;
        self.stdin
            .write_all(b"\n")
            .map_err(|_| AdapterFailure::Transient)?;
        self.stdin.flush().map_err(|_| AdapterFailure::Transient)
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
        let _ = child_process::command("taskkill")
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
        let metric_reset = window
            .get("resetsAt")
            .and_then(normalize_reset_timestamp)
            .map(|resets_at| ResetMetadata {
                resets_at,
                label: "Reset time shown in your local timezone.".to_owned(),
            });
        metrics.push(UsageMetric {
            kind,
            label: label.to_owned(),
            used_percentage: Some(used_percent),
            remaining: Some(format!("{remaining:.0}% remaining")),
            total: None,
            is_estimate: false,
            reset: metric_reset.clone(),
        });

        if reset.is_none() {
            reset = metric_reset;
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
            snapshot.metrics[0]
                .reset
                .as_ref()
                .map(|reset| reset.resets_at.as_str()),
            Some("2026-07-11T12:00:00Z")
        );
        assert_eq!(
            snapshot.metrics[1]
                .reset
                .as_ref()
                .map(|reset| reset.resets_at.as_str()),
            Some("2026-07-16T12:00:00Z")
        );
        assert_eq!(
            snapshot.metrics[0].remaining.as_deref(),
            Some("72% remaining")
        );
        assert!(snapshot.reset.is_some());
        assert_eq!(
            snapshot
                .reset
                .as_ref()
                .map(|reset| reset.resets_at.as_str()),
            snapshot.metrics[0]
                .reset
                .as_ref()
                .map(|reset| reset.resets_at.as_str())
        );
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

    #[test]
    fn normalizes_official_account_activity_and_profile_schema() {
        let rate_limits: Value =
            serde_json::from_str(include_str!("../fixtures/codex/normal.json"))
                .expect("rate limit fixture");
        let account: Value =
            serde_json::from_str(include_str!("../fixtures/codex/account_context.json"))
                .expect("account fixture");
        let usage: Value =
            serde_json::from_str(include_str!("../fixtures/codex/account_usage.json"))
                .expect("usage fixture");

        let result = normalize_live_values(
            &rate_limits,
            Some(&account),
            Some(&usage),
            HermesProviderContext {
                status: HermesStatus::Active,
                confidence: HermesConfidence::Inferred,
            },
            "2026-07-10T10:00:00Z",
        )
        .expect("combined result");
        let serialized = serde_json::to_value(&result).expect("serializable result");
        let context = result.context.expect("account context");
        let activity = context.token_activity.expect("token activity");

        assert_eq!(context.profile_scope.auth_mode, CodexAuthMode::Chatgpt);
        assert_eq!(context.profile_scope.plan_type.as_deref(), Some("plus"));
        assert_eq!(context.profile_scope.hermes_status, HermesStatus::Active);
        assert_eq!(activity.summary.lifetime_tokens, Some(123_456));
        assert_eq!(activity.summary.longest_running_turn_sec, Some(360));
        assert_eq!(activity.daily_buckets.len(), 2);
        assert_eq!(
            serialized.pointer("/context/profileScope/authMode"),
            Some(&json!("chatgpt"))
        );
        assert_eq!(
            serialized.pointer("/context/tokenActivity/summary/lifetimeTokens"),
            Some(&json!(123_456))
        );
        assert_eq!(
            serialized.pointer("/context/tokenActivity/dailyBuckets/0/startDate"),
            Some(&json!("2026-07-09"))
        );
    }

    #[test]
    fn optional_account_rpcs_cannot_discard_valid_rate_limits() {
        let rate_limits: Value =
            serde_json::from_str(include_str!("../fixtures/codex/normal.json"))
                .expect("rate limit fixture");

        let result = normalize_live_values(
            &rate_limits,
            None,
            None,
            HermesProviderContext::default(),
            "2026-07-10T10:00:00Z",
        )
        .expect("rate limits remain valid");

        assert_eq!(result.snapshot.status, SnapshotStatus::Healthy);
        assert_eq!(result.snapshot.metrics.len(), 2);
        assert_eq!(
            result
                .context
                .expect("safe fallback context")
                .profile_scope
                .auth_mode,
            CodexAuthMode::Unknown
        );
    }

    #[test]
    fn normalized_context_drops_identity_credentials_claims_and_raw_payloads() {
        let account: Value =
            serde_json::from_str(include_str!("../fixtures/codex/account_context.json"))
                .expect("account fixture");
        let usage: Value =
            serde_json::from_str(include_str!("../fixtures/codex/account_usage.json"))
                .expect("usage fixture");
        let context = CodexAccountContext {
            profile_scope: normalize_profile_scope(
                Some(&account),
                HermesProviderContext::default(),
            ),
            token_activity: normalize_account_token_activity(&usage),
        };
        let serialized = serde_json::to_string(&context).expect("serializable context");

        for forbidden in [
            "private@example.test",
            "account-secret-value",
            "oauth-secret-value",
            "claim-secret-value",
            "usage-secret-value",
            "raw-secret-value",
            "accountId",
            "accessToken",
            "claims",
            "rawPayload",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn malformed_or_negative_usage_values_are_not_forwarded() {
        let usage = json!({
            "summary": {
                "lifetimeTokens": -1,
                "peakDailyTokens": "not-a-number",
                "longestRunningTurnSec": null,
                "currentStreakDays": 2,
                "longestStreakDays": 7
            },
            "dailyUsageBuckets": [
                { "startDate": "not-a-date", "tokens": 99 },
                { "startDate": "2026-07-10", "tokens": -4 },
                { "startDate": "2026-07-11", "tokens": 5 }
            ]
        });

        let activity = normalize_account_token_activity(&usage).expect("summary exists");

        assert_eq!(activity.summary.lifetime_tokens, None);
        assert_eq!(activity.summary.peak_daily_tokens, None);
        assert_eq!(activity.summary.current_streak_days, Some(2));
        assert_eq!(activity.daily_buckets.len(), 1);
        assert_eq!(activity.daily_buckets[0].tokens, 5);
    }

    #[test]
    fn account_activity_keeps_the_most_recent_thousand_daily_buckets() {
        let daily_usage_buckets: Vec<_> = (0..=1_001)
            .map(|year| {
                json!({
                    "startDate": format!("{year:04}-01-01"),
                    "tokens": year
                })
            })
            .collect();
        let usage = json!({
            "summary": {},
            "dailyUsageBuckets": daily_usage_buckets
        });

        let activity = normalize_account_token_activity(&usage).expect("summary exists");

        assert_eq!(activity.daily_buckets.len(), 1_000);
        assert_eq!(activity.daily_buckets[0].start_date, "0002-01-01");
        assert_eq!(
            activity
                .daily_buckets
                .last()
                .expect("latest bucket")
                .start_date,
            "1001-01-01"
        );
    }
}
