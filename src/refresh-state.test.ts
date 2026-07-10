import { describe, expect, it } from "vitest";

import { runSpendRefresh, runUsageRefresh } from "./refresh-state";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("independent refresh cycles", () => {
  it("settles usage while spend is still pending", async () => {
    const usage = deferred<string[]>();
    const spend = deferred<number>();
    const events: string[] = [];

    const usageRun = runUsageRefresh(() => usage.promise, {
      loading: (value) => events.push(`usage:${value}`),
      success: () => events.push("usage:success"),
      failure: () => events.push("usage:failure"),
    });
    const spendRun = runSpendRefresh(() => spend.promise, {
      loading: (value) => events.push(`spend:${value}`),
      success: () => events.push("spend:success"),
      failure: () => events.push("spend:failure"),
    });

    usage.resolve(["ready"]);
    await usageRun;

    expect(events).toEqual(["usage:true", "spend:true", "usage:success", "usage:false"]);

    spend.resolve(42);
    await spendRun;
    expect(events.slice(-2)).toEqual(["spend:success", "spend:false"]);
  });

  it("spend failure does not invoke usage failure", async () => {
    const events: string[] = [];
    await runSpendRefresh(() => Promise.reject(new Error("slow scanner failed")), {
      loading: (value) => events.push(`spend:${value}`),
      success: () => events.push("spend:success"),
      failure: () => events.push("spend:failure"),
    });

    expect(events).toEqual(["spend:true", "spend:failure", "spend:false"]);
  });
});
