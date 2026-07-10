import { describe, expect, it } from "vitest";

import { createSingleFlightRefresh } from "./refresh-state";

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

    const refreshUsage = createSingleFlightRefresh(() => usage.promise, {
      loading: (value) => events.push(`usage:${value}`),
      success: () => events.push("usage:success"),
      failure: () => events.push("usage:failure"),
    });
    const refreshSpend = createSingleFlightRefresh(() => spend.promise, {
      loading: (value) => events.push(`spend:${value}`),
      success: () => events.push("spend:success"),
      failure: () => events.push("spend:failure"),
    });

    const usageRun = refreshUsage();
    const spendRun = refreshSpend();
    usage.resolve(["ready"]);
    await usageRun;

    expect(events).toEqual(["usage:true", "spend:true", "usage:success", "usage:false"]);

    spend.resolve(42);
    await spendRun;
    expect(events.slice(-2)).toEqual(["spend:success", "spend:false"]);
  });

  it("spend failure does not invoke usage failure", async () => {
    const events: string[] = [];
    const refreshSpend = createSingleFlightRefresh(() => Promise.reject(new Error("slow scanner failed")), {
      loading: (value) => events.push(`spend:${value}`),
      success: () => events.push("spend:success"),
      failure: () => events.push("spend:failure"),
    });
    await refreshSpend();

    expect(events).toEqual(["spend:true", "spend:failure", "spend:false"]);
  });

  it("coalesces repeated requests until the active refresh settles", async () => {
    const pending = deferred<number>();
    let requests = 0;
    const events: string[] = [];
    const refresh = createSingleFlightRefresh(() => {
      requests += 1;
      return pending.promise;
    }, {
      loading: (value) => events.push(`loading:${value}`),
      success: () => events.push("success"),
      failure: () => events.push("failure"),
    });

    const first = refresh();
    const second = refresh();

    expect(first).toBe(second);
    expect(requests).toBe(1);
    pending.resolve(42);
    await first;
    expect(events).toEqual(["loading:true", "success", "loading:false"]);
  });
});
