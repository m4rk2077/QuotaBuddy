import { describe, expect, it } from "vitest";

import type { UsageMetric, UsageSnapshot } from "./contracts";
import { getMetricPresentation, getSnapshotDisplayState, selectOverviewMetrics } from "./panel-state";

const metric = (overrides: Partial<UsageMetric>): UsageMetric => ({
  kind: "session",
  label: "Session limit",
  usedPercentage: 28,
  remaining: "72% remaining",
  total: null,
  isEstimate: false,
  reset: null,
  ...overrides,
});

const snapshot: UsageSnapshot = {
  provider: "codex",
  availability: { clientDetected: true, usageAvailable: true },
  metrics: [],
  reset: { resetsAt: "2026-07-11T12:00:00Z", label: "legacy" },
  lastSuccessfulRefreshAt: "2026-07-10T10:00:00Z",
  status: "healthy",
  error: null,
  isStale: false,
};

describe("compact overview metric selection", () => {
  it("maps cycle to the weekly slot without sharing the session reset", () => {
    const session = metric({ reset: null });
    const weekly = metric({ kind: "cycle", reset: { resetsAt: "2026-07-16T12:00:00Z", label: "weekly" } });

    const selected = selectOverviewMetrics({ ...snapshot, metrics: [session, weekly] });

    expect(selected.session?.reset?.resetsAt).toBe("2026-07-11T12:00:00Z");
    expect(selected.weekly?.reset?.resetsAt).toBe("2026-07-16T12:00:00Z");
  });

  it("never copies the legacy session reset into an absent weekly reset", () => {
    const selected = selectOverviewMetrics({ ...snapshot, metrics: [metric({}), metric({ kind: "cycle" })] });

    expect(selected.session?.reset?.resetsAt).toBe("2026-07-11T12:00:00Z");
    expect(selected.weekly?.reset).toBeNull();
  });
});

describe("compact metric presentation", () => {
  it("derives and clamps remaining percentage from used percentage", () => {
    expect(getMetricPresentation(metric({ usedPercentage: 28 }), [80, 95])).toMatchObject({ remainingPercentage: 72, severity: "healthy" });
    expect(getMetricPresentation(metric({ usedPercentage: 140 }), [80, 95]).remainingPercentage).toBe(0);
  });

  it("uses thresholds to add a textual warning severity", () => {
    expect(getMetricPresentation(metric({ usedPercentage: 84 }), [80, 95]).severity).toBe("warning");
    expect(getMetricPresentation(metric({ usedPercentage: 97 }), [80, 95]).severity).toBe("critical");
  });

  it("keeps unavailable metrics honest", () => {
    expect(getMetricPresentation(metric({ usedPercentage: null }), [80, 95])).toEqual({ remainingPercentage: null, severity: "unavailable" });
  });
});

describe("snapshot state priority", () => {
  it("keeps a failed refresh explicit when cached data is stale", () => {
    expect(getSnapshotDisplayState({ ...snapshot, status: "failed", isStale: true })).toBe("failed");
  });

  it("distinguishes stale healthy data from unavailable and reauthentication", () => {
    expect(getSnapshotDisplayState({ ...snapshot, isStale: true })).toBe("stale");
    expect(getSnapshotDisplayState({ ...snapshot, status: "unavailable" })).toBe("unavailable");
    expect(getSnapshotDisplayState({ ...snapshot, status: "reauthRequired" })).toBe("reauthRequired");
  });
});
