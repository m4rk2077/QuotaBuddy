import { describe, expect, it } from "vitest";

import { defaultMonitorPreferences, MonitorPreferenceSaveCoordinator, updateAlertThresholds } from "./monitor-controls";

describe("alert threshold input", () => {
  it.each(["", " ", "0", "-1", "1.5", "101", "NaN", "1e2"])("rejects invalid input %j before an optimistic save", (rawValue) => {
    const current = [80, 95];

    expect(updateAlertThresholds(current, 0, rawValue)).toBeNull();
    expect(current).toEqual([80, 95]);
  });

  it.each([
    ["1", [1, 95]],
    ["42", [42, 95]],
    ["100", [100, 95]],
  ])("accepts an integer percentage %s from 1 through 100", (rawValue, expected) => {
    expect(updateAlertThresholds([80, 95], 0, rawValue as string)).toEqual(expected);
  });

  it("rejects an out-of-range field index", () => {
    expect(updateAlertThresholds([80, 95], 2, "50")).toBeNull();
  });
});

describe("preference save coordination", () => {
  const changed = (warning: number) => ({ ...defaultMonitorPreferences, alertThresholds: [warning, 95] });

  it("returns to the initial confirmed state when two optimistic saves fail", async () => {
    const coordinator = new MonitorPreferenceSaveCoordinator(defaultMonitorPreferences);
    const first = coordinator.begin(changed(70));
    const second = coordinator.begin(changed(60));

    const staleFailure = await coordinator.settle(first, Promise.reject(new Error("first failed")));
    const latestFailure = await coordinator.settle(second, Promise.reject(new Error("second failed")));

    expect(staleFailure).toMatchObject({ applyToView: false, saving: true, failed: true });
    expect(latestFailure).toEqual({ preferences: defaultMonitorPreferences, applyToView: true, saving: false, failed: true });
  });

  it("rolls a failed latest save back to the previous confirmed success", async () => {
    const coordinator = new MonitorPreferenceSaveCoordinator(defaultMonitorPreferences);
    const firstPreferences = changed(70);
    const first = coordinator.begin(firstPreferences);
    const second = coordinator.begin(changed(60));

    const staleSuccess = await coordinator.settle(first, Promise.resolve(firstPreferences));
    const latestFailure = await coordinator.settle(second, Promise.reject(new Error("second failed")));

    expect(staleSuccess).toMatchObject({ preferences: changed(60), applyToView: false, saving: true, failed: false });
    expect(latestFailure).toEqual({ preferences: firstPreferences, applyToView: true, saving: false, failed: true });
  });

  it("never lets a stale completion overwrite the latest optimistic view", async () => {
    const coordinator = new MonitorPreferenceSaveCoordinator(defaultMonitorPreferences);
    const first = coordinator.begin(changed(70));
    const secondPreferences = changed(60);
    const second = coordinator.begin(secondPreferences);

    const staleSuccess = await coordinator.settle(first, Promise.resolve(changed(70)));
    const latestSuccess = await coordinator.settle(second, Promise.resolve(secondPreferences));

    expect(staleSuccess).toEqual({ preferences: secondPreferences, applyToView: false, saving: true, failed: false });
    expect(latestSuccess).toEqual({ preferences: secondPreferences, applyToView: true, saving: false, failed: false });
  });
});
