use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::core::{MetricKind, ProviderId, UsageSnapshot};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorPreferences {
    pub theme: Theme,
    pub language: Language,
    pub start_with_windows: bool,
    pub pinned_metrics: Vec<MetricKind>,
    pub alert_thresholds: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    En,
    PtBr,
}

impl Default for MonitorPreferences {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            language: Language::En,
            start_with_windows: false,
            pinned_metrics: Vec::new(),
            alert_thresholds: vec![80, 95],
        }
    }
}

pub fn validate_preferences(
    mut preferences: MonitorPreferences,
) -> Result<MonitorPreferences, String> {
    if preferences
        .pinned_metrics
        .iter()
        .any(|kind| matches!(kind, MetricKind::EstimatedSpend))
    {
        return Err("Only Codex usage metrics can be pinned.".to_owned());
    }
    let mut unique = Vec::new();
    for kind in preferences.pinned_metrics {
        if !unique.contains(&kind) {
            unique.push(kind);
        }
    }
    preferences.pinned_metrics = unique;
    if preferences.pinned_metrics.len() > 2 {
        return Err("Choose at most two tray metrics.".to_owned());
    }
    if preferences.alert_thresholds.is_empty()
        || preferences
            .alert_thresholds
            .iter()
            .any(|threshold| *threshold == 0 || *threshold > 100)
    {
        return Err("Alert thresholds must be between 1 and 100.".to_owned());
    }
    preferences.alert_thresholds.sort_unstable();
    preferences.alert_thresholds.dedup();
    Ok(preferences)
}

#[derive(Default)]
pub struct AlertTracker {
    above: BTreeMap<(MetricKind, u8), bool>,
}

pub fn crossing_alerts(
    tracker: &mut AlertTracker,
    snapshot: &UsageSnapshot,
    thresholds: &[u8],
) -> Vec<(String, u8)> {
    if snapshot.provider != ProviderId::Codex {
        return Vec::new();
    }
    let mut alerts = Vec::new();
    for metric in &snapshot.metrics {
        let Some(used) = metric.used_percentage else {
            continue;
        };
        for threshold in thresholds {
            let key = (metric.kind, *threshold);
            let is_above = used >= f64::from(*threshold);
            let was_above = tracker.above.insert(key, is_above);
            if is_above && was_above == Some(false) {
                alerts.push((metric.label.clone(), *threshold));
            }
        }
    }
    alerts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Availability, SnapshotStatus, UsageMetric};

    fn snapshot(used: f64) -> UsageSnapshot {
        UsageSnapshot {
            provider: ProviderId::Codex,
            availability: Availability {
                client_detected: true,
                usage_available: true,
            },
            metrics: vec![UsageMetric {
                kind: MetricKind::Session,
                label: "Session limit".to_owned(),
                used_percentage: Some(used),
                remaining: None,
                total: None,
                is_estimate: false,
            }],
            reset: None,
            last_successful_refresh_at: None,
            status: SnapshotStatus::Healthy,
            error: None,
            is_stale: false,
        }
    }

    #[test]
    fn defaults_to_english_dark_and_standard_thresholds() {
        assert_eq!(
            MonitorPreferences::default(),
            MonitorPreferences {
                theme: Theme::Dark,
                language: Language::En,
                start_with_windows: false,
                pinned_metrics: vec![],
                alert_thresholds: vec![80, 95]
            }
        );
    }

    #[test]
    fn rejects_more_than_two_tray_metrics() {
        let preferences = MonitorPreferences {
            pinned_metrics: vec![MetricKind::Session, MetricKind::Cycle, MetricKind::Weekly],
            ..Default::default()
        };
        assert!(validate_preferences(preferences).is_err());
    }

    #[test]
    fn accepts_duplicate_metric_input_when_only_two_distinct_metrics_are_pinned() {
        let preferences = MonitorPreferences {
            pinned_metrics: vec![MetricKind::Session, MetricKind::Session, MetricKind::Cycle],
            ..Default::default()
        };
        assert_eq!(
            validate_preferences(preferences).unwrap().pinned_metrics,
            vec![MetricKind::Session, MetricKind::Cycle]
        );
    }

    #[test]
    fn alerts_only_when_usage_crosses_a_threshold_and_rearms_below_it() {
        let mut tracker = AlertTracker::default();
        assert!(crossing_alerts(&mut tracker, &snapshot(79.0), &[80, 95]).is_empty());
        assert_eq!(
            crossing_alerts(&mut tracker, &snapshot(81.0), &[80, 95]),
            vec![("Session limit".to_owned(), 80)]
        );
        assert!(crossing_alerts(&mut tracker, &snapshot(90.0), &[80, 95]).is_empty());
        assert_eq!(
            crossing_alerts(&mut tracker, &snapshot(97.0), &[80, 95]),
            vec![("Session limit".to_owned(), 95)]
        );
        assert!(crossing_alerts(&mut tracker, &snapshot(70.0), &[80, 95]).is_empty());
        assert_eq!(
            crossing_alerts(&mut tracker, &snapshot(81.0), &[80, 95]),
            vec![("Session limit".to_owned(), 80)]
        );
    }

    #[test]
    fn does_not_alert_for_an_initial_already_high_usage_value() {
        let mut tracker = AlertTracker::default();
        assert!(crossing_alerts(&mut tracker, &snapshot(97.0), &[80, 95]).is_empty());
    }
}
