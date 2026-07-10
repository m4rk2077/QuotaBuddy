import { describe, expect, it } from "vitest";

import type { UsageSnapshot } from "./contracts";
import { shouldShowEmptyState } from "./panel-state";
import { canPinMetric, defaultMonitorPreferences } from "./monitor-controls";

describe("UsageSnapshot frontend boundary", () => {
  it("accepts normalized snapshot metadata without credential material", () => {
    const snapshot: UsageSnapshot = {
      provider: "codex",
      availability: { clientDetected: true, usageAvailable: true },
      metrics: [],
      reset: null,
      lastSuccessfulRefreshAt: "2026-07-10T10:00:00Z",
      status: "healthy",
      error: null,
      isStale: false,
    };

    expect(Object.keys(snapshot)).not.toContain("token");
    expect(Object.keys(snapshot)).not.toContain("credentials");
    expect(snapshot.availability.clientDetected).toBe(true);
  });
});

describe("monitor controls external behavior", () => {
  it("defaults to English, dark theme, and 80%/95% alerts", () => {
    expect(defaultMonitorPreferences).toMatchObject({ language: "en", theme: "dark", alertThresholds: [80, 95] });
  });

  it("allows no more than two pinned tray metrics", () => {
    expect(canPinMetric(["session", "cycle"], "weekly")).toBe(false);
    expect(canPinMetric(["session", "cycle"], "session")).toBe(true);
  });
});

describe("empty panel state", () => {
  it("shows the no-client panel after a completed empty local scan", () => {
    expect(shouldShowEmptyState(false, [])).toBe(true);
    expect(shouldShowEmptyState(true, [])).toBe(false);
  });
});
