import { describe, expect, it } from "vitest";

import {
  formatDuration,
  formatPercent,
  formatShortDate,
  formatTokenCount,
  formatUsdEquivalent,
  getBarHeight,
} from "./history-format";

describe("history formatting", () => {
  it("keeps token quantities compact and readable", () => {
    expect(formatTokenCount(0, "en-US")).toBe("0");
    expect(formatTokenCount(null, "en-US")).toBe("—");
    expect(formatTokenCount(1_250, "en-US")).toBe("1.3K");
    expect(formatTokenCount(Number.NaN, "en-US")).toBe("0");
  });

  it("formats coverage and API-equivalent values without implying a charge", () => {
    expect(formatPercent(99.6)).toBe("99.6%");
    expect(formatPercent(99.97)).toBe("99.97%");
    expect(formatPercent(0.4)).toBe("0.4%");
    expect(formatUsdEquivalent(null, "en-US")).toBe("—");
    expect(formatUsdEquivalent(0.0042, "en-US")).toBe("$0.0042");
  });

  it("formats elapsed time at a glance", () => {
    expect(formatDuration(null, "en-US")).toBe("—");
    expect(formatDuration(45, "en-US")).toBe("45s");
    expect(formatDuration(3_720, "en-US")).toBe("1h 2min");
  });

  it("scales chart bars while keeping small non-zero values visible", () => {
    expect(getBarHeight(0, 100)).toBe(0);
    expect(getBarHeight(1, 100)).toBe(6);
    expect(getBarHeight(250, 100)).toBe(100);
  });

  it("formats daily chart dates in UTC and preserves invalid input", () => {
    expect(formatShortDate("2026-07-09", "en-US")).toBe("Jul 09");
    expect(formatShortDate("not-a-date", "en-US")).toBe("not-a-date");
  });
});
