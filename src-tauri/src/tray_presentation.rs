use crate::{
    core::{MetricKind, ProviderId, SnapshotStatus, UsageSnapshot},
    monitor_controls::{Language, MonitorPreferences},
};

const MAX_TOOLTIP_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraySeverity {
    Healthy,
    Warning,
    Critical,
    Stale,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconKey {
    Healthy,
    Warning,
    Critical,
    Stale,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayPresentation {
    pub severity: TraySeverity,
    pub tooltip: String,
    pub icon_key: TrayIconKey,
}

pub fn build_tray_presentation(
    snapshots: &[UsageSnapshot],
    preferences: &MonitorPreferences,
) -> TrayPresentation {
    let Some(snapshot) = snapshots
        .iter()
        .find(|snapshot| snapshot.provider == ProviderId::Codex)
    else {
        return unavailable_presentation(preferences.language);
    };

    let has_normalized_metric = snapshot
        .metrics
        .iter()
        .any(|metric| metric.used_percentage.is_some_and(|used| used.is_finite()));
    let severity = if snapshot.is_stale
        && snapshot.status == SnapshotStatus::Failed
        && has_normalized_metric
    {
        TraySeverity::Stale
    } else if snapshot.status != SnapshotStatus::Healthy || !snapshot.availability.usage_available {
        TraySeverity::Unavailable
    } else {
        snapshot
            .metrics
            .iter()
            .filter_map(|metric| metric.used_percentage)
            .filter(|used| used.is_finite())
            .fold(TraySeverity::Healthy, |severity, used| {
                severity.max(usage_severity(used, &preferences.alert_thresholds))
            })
    };

    let mut kinds = Vec::new();
    for kind in &preferences.pinned_metrics {
        if kinds.len() == 2 {
            break;
        }
        if *kind != MetricKind::EstimatedSpend && !kinds.contains(kind) {
            kinds.push(*kind);
        }
    }

    let metrics: Vec<_> = kinds
        .into_iter()
        .filter_map(|kind| {
            let used = snapshot
                .metrics
                .iter()
                .find(|metric| metric.kind == kind)?
                .used_percentage?;
            used.is_finite().then(|| {
                let remaining = (100.0 - used.clamp(0.0, 100.0)).round() as u8;
                format!(
                    "{} {remaining}%",
                    localized_metric_label(preferences.language, kind)
                )
            })
        })
        .collect();

    let fallback = if severity == TraySeverity::Unavailable {
        localized_unavailable(preferences.language).to_owned()
    } else {
        localized_fallback(preferences.language)
    };
    let has_metrics = !metrics.is_empty();
    let mut tooltip = if !has_metrics {
        fallback
    } else {
        metrics.join(" • ")
    };
    if severity == TraySeverity::Stale {
        tooltip.push_str(" • ");
        tooltip.push_str(localized_stale(preferences.language));
    } else if severity == TraySeverity::Unavailable && has_metrics {
        tooltip.push_str(" • ");
        tooltip.push_str(localized_unavailable_marker(preferences.language));
    }
    let tooltip = limit_chars(tooltip, MAX_TOOLTIP_CHARS);
    let icon_key = severity.into();

    TrayPresentation {
        severity,
        tooltip,
        icon_key,
    }
}

#[derive(Default)]
pub struct TrayPresentationTracker {
    last_tooltip: Option<String>,
    last_icon_key: Option<TrayIconKey>,
}

impl TrayPresentationTracker {
    pub fn confirm_applied(&mut self, presentation: &TrayPresentation) {
        self.last_tooltip = Some(presentation.tooltip.clone());
        self.last_icon_key = Some(presentation.icon_key);
    }

    pub fn apply_tooltip_if_changed<E, F>(
        &mut self,
        next: &TrayPresentation,
        apply: F,
    ) -> Result<bool, E>
    where
        F: FnOnce(&str) -> Result<(), E>,
    {
        if self.last_tooltip.as_deref() == Some(&next.tooltip) {
            return Ok(false);
        }
        apply(&next.tooltip)?;
        self.last_tooltip = Some(next.tooltip.clone());
        Ok(true)
    }

    pub fn apply_icon_if_changed<E, F>(
        &mut self,
        next: &TrayPresentation,
        apply: F,
    ) -> Result<bool, E>
    where
        F: FnOnce(TrayIconKey) -> Result<(), E>,
    {
        if self.last_icon_key == Some(next.icon_key) {
            return Ok(false);
        }
        apply(next.icon_key)?;
        self.last_icon_key = Some(next.icon_key);
        Ok(true)
    }
}

impl TraySeverity {
    fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Healthy => 0,
            Self::Warning => 1,
            Self::Critical => 2,
            Self::Stale => 3,
            Self::Unavailable => 4,
        }
    }
}

impl From<TraySeverity> for TrayIconKey {
    fn from(severity: TraySeverity) -> Self {
        match severity {
            TraySeverity::Healthy => Self::Healthy,
            TraySeverity::Warning => Self::Warning,
            TraySeverity::Critical => Self::Critical,
            TraySeverity::Stale => Self::Stale,
            TraySeverity::Unavailable => Self::Unavailable,
        }
    }
}

fn usage_severity(used: f64, thresholds: &[u8]) -> TraySeverity {
    let mut thresholds = thresholds.to_vec();
    thresholds.sort_unstable();
    let warning = f64::from(thresholds.first().copied().unwrap_or(80));
    let critical = f64::from(thresholds.get(1).copied().unwrap_or(95));

    if used >= critical {
        TraySeverity::Critical
    } else if used >= warning {
        TraySeverity::Warning
    } else {
        TraySeverity::Healthy
    }
}

fn localized_metric_label(language: Language, kind: MetricKind) -> &'static str {
    match (language, kind) {
        (Language::PtBr, MetricKind::Session) => "Sessão",
        (Language::PtBr, MetricKind::Weekly | MetricKind::Cycle) => "Semana",
        (Language::PtBr, MetricKind::Credits) => "Créditos",
        (_, MetricKind::Session) => "Session",
        (_, MetricKind::Weekly | MetricKind::Cycle) => "Week",
        (_, MetricKind::Credits) => "Credits",
        (_, _) => "Usage",
    }
}

fn localized_fallback(language: Language) -> String {
    match language {
        Language::En => "QuotaBuddy — local usage monitor".to_owned(),
        Language::PtBr => "QuotaBuddy — monitor local de uso".to_owned(),
    }
}

fn localized_stale(language: Language) -> &'static str {
    match language {
        Language::En => "stale",
        Language::PtBr => "desatualizado",
    }
}

fn localized_unavailable(language: Language) -> &'static str {
    match language {
        Language::En => "QuotaBuddy — usage unavailable",
        Language::PtBr => "QuotaBuddy — uso indisponível",
    }
}

fn localized_unavailable_marker(language: Language) -> &'static str {
    match language {
        Language::En => "unavailable",
        Language::PtBr => "indisponível",
    }
}

fn unavailable_presentation(language: Language) -> TrayPresentation {
    TrayPresentation {
        severity: TraySeverity::Unavailable,
        tooltip: localized_unavailable(language).to_owned(),
        icon_key: TrayIconKey::Unavailable,
    }
}

fn limit_chars(value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value;
    }
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        Availability, MetricKind, ProviderId, SnapshotError, SnapshotStatus, UsageMetric,
    };

    fn metric(kind: MetricKind, used_percentage: Option<f64>, untrusted_text: &str) -> UsageMetric {
        UsageMetric {
            kind,
            label: untrusted_text.to_owned(),
            used_percentage,
            remaining: Some(untrusted_text.to_owned()),
            total: Some(untrusted_text.to_owned()),
            is_estimate: false,
            reset: None,
        }
    }

    fn snapshot(status: SnapshotStatus, metrics: Vec<UsageMetric>) -> UsageSnapshot {
        UsageSnapshot {
            provider: ProviderId::Codex,
            availability: Availability {
                client_detected: true,
                usage_available: status == SnapshotStatus::Healthy,
            },
            metrics,
            reset: None,
            last_successful_refresh_at: None,
            status,
            error: None,
            is_stale: false,
        }
    }

    fn preferences(language: Language) -> MonitorPreferences {
        MonitorPreferences {
            language,
            pinned_metrics: vec![MetricKind::Session, MetricKind::Cycle],
            ..MonitorPreferences::default()
        }
    }

    #[test]
    fn presents_remaining_session_and_week_in_portuguese() {
        let snapshots = [snapshot(
            SnapshotStatus::Healthy,
            vec![
                metric(MetricKind::Session, Some(28.4), "ignored"),
                metric(MetricKind::Cycle, Some(57.2), "ignored"),
            ],
        )];

        let presentation = build_tray_presentation(&snapshots, &preferences(Language::PtBr));

        assert_eq!(presentation.tooltip, "Sessão 72% • Semana 43%");
        assert_eq!(presentation.severity, TraySeverity::Healthy);
        assert_eq!(presentation.icon_key, TrayIconKey::Healthy);
    }

    #[test]
    fn presents_remaining_session_and_week_in_english() {
        let snapshots = [snapshot(
            SnapshotStatus::Healthy,
            vec![
                metric(MetricKind::Session, Some(28.4), "ignored"),
                metric(MetricKind::Weekly, Some(57.2), "ignored"),
            ],
        )];

        let presentation = build_tray_presentation(
            &snapshots,
            &MonitorPreferences {
                language: Language::En,
                pinned_metrics: vec![MetricKind::Session, MetricKind::Weekly],
                ..MonitorPreferences::default()
            },
        );

        assert_eq!(presentation.tooltip, "Session 72% • Week 43%");
    }

    #[test]
    fn never_uses_untrusted_snapshot_text_and_limits_metrics_and_length() {
        let secret = r#"Bearer token-secret C:\Users\private\account-123 raw failure"#;
        let mut source = snapshot(
            SnapshotStatus::Healthy,
            vec![
                metric(MetricKind::Session, Some(10.0), secret),
                metric(MetricKind::Cycle, Some(20.0), secret),
                metric(MetricKind::Credits, Some(30.0), secret),
            ],
        );
        source.error = Some(SnapshotError {
            code: secret.to_owned(),
            message: secret.to_owned(),
        });
        let preferences = MonitorPreferences {
            language: Language::En,
            pinned_metrics: vec![MetricKind::Session, MetricKind::Cycle, MetricKind::Credits],
            ..MonitorPreferences::default()
        };

        let presentation = build_tray_presentation(&[source], &preferences);

        assert_eq!(presentation.tooltip, "Session 90% • Week 80%");
        assert!(presentation.tooltip.chars().count() <= 120);
        assert!(!presentation.tooltip.contains("token-secret"));
        assert!(!presentation.tooltip.contains("private"));
        assert!(!presentation.tooltip.contains("account-123"));
        assert!(!presentation.tooltip.contains("failure"));
    }

    #[test]
    fn derives_deterministic_severity_from_normalized_usage_and_status() {
        let warning = build_tray_presentation(
            &[snapshot(
                SnapshotStatus::Healthy,
                vec![metric(MetricKind::Session, Some(80.0), "ignored")],
            )],
            &preferences(Language::En),
        );
        let critical = build_tray_presentation(
            &[snapshot(
                SnapshotStatus::Healthy,
                vec![metric(MetricKind::Session, Some(95.0), "ignored")],
            )],
            &preferences(Language::En),
        );
        let unavailable = build_tray_presentation(
            &[snapshot(SnapshotStatus::ReauthRequired, vec![])],
            &preferences(Language::En),
        );

        assert_eq!(
            (warning.severity, warning.icon_key),
            (TraySeverity::Warning, TrayIconKey::Warning)
        );
        assert_eq!(
            (critical.severity, critical.icon_key),
            (TraySeverity::Critical, TrayIconKey::Critical)
        );
        assert_eq!(
            (unavailable.severity, unavailable.icon_key),
            (TraySeverity::Unavailable, TrayIconKey::Unavailable)
        );
    }

    #[test]
    fn presents_cached_transient_failures_as_stale_without_hiding_the_metrics() {
        let mut stale = snapshot(
            SnapshotStatus::Failed,
            vec![
                metric(MetricKind::Session, Some(28.0), "ignored"),
                metric(MetricKind::Cycle, Some(57.0), "ignored"),
            ],
        );
        stale.is_stale = true;

        let presentation = build_tray_presentation(&[stale], &preferences(Language::PtBr));

        assert_eq!(presentation.severity, TraySeverity::Stale);
        assert_eq!(presentation.icon_key, TrayIconKey::Stale);
        assert_eq!(
            presentation.tooltip,
            "Sessão 72% • Semana 43% • desatualizado"
        );
    }

    #[test]
    fn derives_warning_and_critical_icons_from_configured_thresholds() {
        let mut configured = preferences(Language::En);
        configured.alert_thresholds = vec![50, 75];

        let warning = build_tray_presentation(
            &[snapshot(
                SnapshotStatus::Healthy,
                vec![metric(MetricKind::Session, Some(60.0), "ignored")],
            )],
            &configured,
        );
        let critical = build_tray_presentation(
            &[snapshot(
                SnapshotStatus::Healthy,
                vec![metric(MetricKind::Session, Some(80.0), "ignored")],
            )],
            &configured,
        );

        assert_eq!(warning.icon_key, TrayIconKey::Warning);
        assert_eq!(critical.icon_key, TrayIconKey::Critical);
    }

    #[test]
    fn does_not_claim_stale_data_when_no_normalized_cached_metric_exists() {
        let mut failed = snapshot(
            SnapshotStatus::Failed,
            vec![metric(MetricKind::Session, None, "ignored")],
        );
        failed.is_stale = true;

        let presentation = build_tray_presentation(&[failed], &preferences(Language::En));

        assert_eq!(presentation.severity, TraySeverity::Unavailable);
        assert_eq!(presentation.icon_key, TrayIconKey::Unavailable);
    }

    #[test]
    fn keeps_reauthentication_more_urgent_than_cached_stale_data() {
        let mut reauth = snapshot(
            SnapshotStatus::ReauthRequired,
            vec![metric(MetricKind::Session, Some(28.0), "ignored")],
        );
        reauth.is_stale = true;

        let presentation = build_tray_presentation(&[reauth], &preferences(Language::En));

        assert_eq!(presentation.severity, TraySeverity::Unavailable);
        assert_eq!(presentation.icon_key, TrayIconKey::Unavailable);
        assert_eq!(presentation.tooltip, "Session 72% • unavailable");
    }

    #[test]
    fn keeps_unavailable_failures_unavailable_even_when_cache_is_present() {
        let mut unavailable = snapshot(
            SnapshotStatus::Unavailable,
            vec![metric(MetricKind::Session, Some(28.0), "ignored")],
        );
        unavailable.is_stale = true;

        let presentation = build_tray_presentation(&[unavailable], &preferences(Language::En));

        assert_eq!(presentation.severity, TraySeverity::Unavailable);
        assert_eq!(presentation.icon_key, TrayIconKey::Unavailable);
        assert_eq!(presentation.tooltip, "Session 72% • unavailable");
    }

    #[test]
    fn announces_unavailability_in_the_localized_tooltip() {
        let presentation = build_tray_presentation(&[], &preferences(Language::PtBr));

        assert_eq!(presentation.tooltip, "QuotaBuddy — uso indisponível");
    }

    #[test]
    fn suppresses_tooltip_and_icon_updates_independently() {
        let presentation = TrayPresentation {
            severity: TraySeverity::Healthy,
            tooltip: "Session 72% • Week 43%".to_owned(),
            icon_key: TrayIconKey::Healthy,
        };
        let mut tracker = TrayPresentationTracker::default();

        assert_eq!(
            tracker.apply_tooltip_if_changed(&presentation, |_| Ok::<_, ()>(())),
            Ok(true)
        );
        assert_eq!(
            tracker.apply_icon_if_changed(&presentation, |_| Ok::<_, ()>(())),
            Ok(true)
        );

        let icon_only_change = TrayPresentation {
            severity: TraySeverity::Warning,
            icon_key: TrayIconKey::Warning,
            ..presentation
        };
        assert_eq!(
            tracker.apply_tooltip_if_changed(&icon_only_change, |_| Ok::<_, ()>(())),
            Ok(false)
        );
        assert_eq!(
            tracker.apply_icon_if_changed(&icon_only_change, |_| Ok::<_, ()>(())),
            Ok(true)
        );
    }

    #[test]
    fn retries_an_update_that_failed_at_the_tray_boundary() {
        let presentation = TrayPresentation {
            severity: TraySeverity::Healthy,
            tooltip: "Session 72%".to_owned(),
            icon_key: TrayIconKey::Healthy,
        };
        let mut tracker = TrayPresentationTracker::default();

        assert_eq!(
            tracker.apply_tooltip_if_changed(&presentation, |_| Err("tray unavailable")),
            Err("tray unavailable")
        );
        assert_eq!(
            tracker.apply_tooltip_if_changed(&presentation, |_| Ok::<_, &str>(())),
            Ok(true)
        );
    }

    #[test]
    fn confirms_only_the_builder_state_that_was_successfully_created() {
        let presentation = TrayPresentation {
            severity: TraySeverity::Unavailable,
            tooltip: "QuotaBuddy — local usage monitor".to_owned(),
            icon_key: TrayIconKey::Unavailable,
        };
        let mut tracker = TrayPresentationTracker::default();

        tracker.confirm_applied(&presentation);

        assert_eq!(
            tracker.apply_tooltip_if_changed(&presentation, |_| panic!("tooltip was duplicated")),
            Ok::<_, ()>(false)
        );
        assert_eq!(
            tracker.apply_icon_if_changed(&presentation, |_| panic!("icon was duplicated")),
            Ok::<_, ()>(false)
        );
    }

    #[test]
    fn retries_an_icon_update_that_failed_at_the_tray_boundary() {
        let presentation = TrayPresentation {
            severity: TraySeverity::Critical,
            tooltip: "Session 2%".to_owned(),
            icon_key: TrayIconKey::Critical,
        };
        let mut tracker = TrayPresentationTracker::default();

        assert_eq!(
            tracker.apply_icon_if_changed(&presentation, |_| Err("tray unavailable")),
            Err("tray unavailable")
        );
        assert_eq!(
            tracker.apply_icon_if_changed(&presentation, |_| Ok::<_, &str>(())),
            Ok(true)
        );
    }
}
