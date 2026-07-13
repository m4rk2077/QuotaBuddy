import { describe, expect, it } from "vitest";

import type { UsageMetric, UsageSnapshot } from "./contracts";
import { deriveCodexLimitReached, formatResetCountdown } from "./limit-game";

const now = Date.parse("2026-07-13T12:00:00Z");

function metric(kind: UsageMetric["kind"], usedPercentage: number | null, resetAt?: string): UsageMetric {
  return {
    kind,
    label: kind,
    usedPercentage,
    remaining: null,
    total: null,
    isEstimate: false,
    reset: resetAt ? { resetsAt: resetAt, label: "reset" } : null,
  };
}

function snapshot(overrides: Partial<UsageSnapshot> = {}): UsageSnapshot {
  return {
    provider: "codex",
    availability: { clientDetected: true, usageAvailable: true },
    metrics: [metric("session", 100, "2026-07-13T14:00:00Z")],
    reset: null,
    lastSuccessfulRefreshAt: "2026-07-13T12:00:00Z",
    status: "healthy",
    error: null,
    isStale: false,
    ...overrides,
  };
}

describe("deriveCodexLimitReached", () => {
  it("activates for a reached session limit with a future reset", () => {
    expect(deriveCodexLimitReached(snapshot(), now)).toEqual({
      reached: [{ slot: "session", resetAt: "2026-07-13T14:00:00Z" }],
      effectiveResetAt: "2026-07-13T14:00:00Z",
    });
  });

  it("uses the legacy snapshot reset only for the session limit", () => {
    const value = snapshot({
      metrics: [metric("session", 100), metric("weekly", 100)],
      reset: { resetsAt: "2026-07-13T13:00:00Z", label: "session" },
    });
    expect(deriveCodexLimitReached(value, now)?.reached).toEqual([
      { slot: "session", resetAt: "2026-07-13T13:00:00Z" },
    ]);
  });

  it("uses the later reset when session and weekly limits are both reached", () => {
    const value = snapshot({
      metrics: [
        metric("session", 100, "2026-07-13T14:00:00Z"),
        metric("cycle", 100, "2026-07-18T12:00:00Z"),
      ],
    });
    expect(deriveCodexLimitReached(value, now)).toEqual({
      reached: [
        { slot: "session", resetAt: "2026-07-13T14:00:00Z" },
        { slot: "weekly", resetAt: "2026-07-18T12:00:00Z" },
      ],
      effectiveResetAt: "2026-07-18T12:00:00Z",
    });
  });

  it.each([
    { status: "failed" as const },
    { status: "unavailable" as const },
    { status: "reauthRequired" as const },
    { isStale: true },
    { availability: { clientDetected: true, usageAvailable: false } },
    { provider: "cursor" as const },
  ])("does not activate for an unsafe snapshot: %o", (overrides) => {
    expect(deriveCodexLimitReached(snapshot(overrides), now)).toBeNull();
  });

  it("ignores values below 100 and resets that are missing, malformed, or past", () => {
    const value = snapshot({
      metrics: [
        metric("session", 99.9, "2026-07-13T14:00:00Z"),
        metric("weekly", 100),
        metric("cycle", 100, "not-a-date"),
        metric("weekly", 100, "2026-07-13T11:59:00Z"),
      ],
    });
    expect(deriveCodexLimitReached(value, now)).toBeNull();
  });
});

describe("formatResetCountdown", () => {
  it("formats days, hours, minutes, and a short countdown", () => {
    expect(formatResetCountdown("2026-07-15T15:04:05Z", now)).toBe("2d 03h 04m");
    expect(formatResetCountdown("2026-07-13T15:04:05Z", now)).toBe("03h 04m 05s");
    expect(formatResetCountdown("2026-07-13T12:04:05Z", now)).toBe("04m 05s");
  });
});
