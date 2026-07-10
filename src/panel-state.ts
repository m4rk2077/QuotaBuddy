import type { UsageMetric, UsageSnapshot } from "./contracts";

export function shouldShowEmptyState(loading: boolean, snapshots: UsageSnapshot[]): boolean {
  return !loading && snapshots.length === 0;
}

export type MetricSeverity = "healthy" | "warning" | "critical" | "unavailable";
export type SnapshotDisplayState = "healthy" | "stale" | "failed" | "unavailable" | "reauthRequired";

export function getSnapshotDisplayState(snapshot: UsageSnapshot): SnapshotDisplayState {
  if (snapshot.status === "reauthRequired") return "reauthRequired";
  if (snapshot.status === "failed") return "failed";
  if (snapshot.status === "unavailable") return "unavailable";
  if (snapshot.isStale) return "stale";
  return "healthy";
}

export function selectOverviewMetrics(snapshot: UsageSnapshot): { session: UsageMetric | null; weekly: UsageMetric | null } {
  const session = snapshot.metrics.find((metric) => metric.kind === "session") ?? null;
  const weekly = snapshot.metrics.find((metric) => metric.kind === "weekly" || metric.kind === "cycle") ?? null;
  return {
    session: session ? { ...session, reset: session.reset ?? snapshot.reset } : null,
    weekly: weekly ? { ...weekly, reset: weekly.reset ?? null } : null,
  };
}

export function getMetricPresentation(metric: UsageMetric, thresholds: number[]): { remainingPercentage: number | null; severity: MetricSeverity } {
  if (metric.usedPercentage === null || !Number.isFinite(metric.usedPercentage)) {
    return { remainingPercentage: null, severity: "unavailable" };
  }
  const used = Math.min(100, Math.max(0, metric.usedPercentage));
  const [warning = 80, critical = 95] = [...thresholds].sort((a, b) => a - b);
  const severity = used >= critical ? "critical" : used >= warning ? "warning" : "healthy";
  return { remainingPercentage: Math.round(100 - used), severity };
}
