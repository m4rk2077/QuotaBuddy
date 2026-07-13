import type { UsageMetric, UsageSnapshot } from "./contracts";

export type ReachedLimitSlot = "session" | "weekly";

export type ReachedLimit = {
  slot: ReachedLimitSlot;
  resetAt: string;
};

export type CodexLimitReached = {
  reached: ReachedLimit[];
  effectiveResetAt: string;
};

function metricSlot(metric: UsageMetric): ReachedLimitSlot | null {
  if (metric.kind === "session") return "session";
  if (metric.kind === "weekly" || metric.kind === "cycle") return "weekly";
  return null;
}

function metricResetAt(metric: UsageMetric, snapshot: UsageSnapshot): string | null {
  return metric.reset?.resetsAt ?? (metric.kind === "session" ? snapshot.reset?.resetsAt ?? null : null);
}

export function deriveCodexLimitReached(snapshot: UsageSnapshot | undefined, now = Date.now()): CodexLimitReached | null {
  if (!snapshot || snapshot.provider !== "codex") return null;
  if (snapshot.status !== "healthy" || snapshot.isStale) return null;
  if (!snapshot.availability.clientDetected || !snapshot.availability.usageAvailable) return null;

  const reached = snapshot.metrics.flatMap((metric): ReachedLimit[] => {
    const slot = metricSlot(metric);
    if (!slot || metric.usedPercentage === null || !Number.isFinite(metric.usedPercentage) || metric.usedPercentage < 100) return [];
    const resetAt = metricResetAt(metric, snapshot);
    if (!resetAt) return [];
    const resetTime = new Date(resetAt).getTime();
    if (!Number.isFinite(resetTime) || resetTime <= now) return [];
    return [{ slot, resetAt }];
  });

  if (reached.length === 0) return null;

  reached.sort((a, b) => {
    if (a.slot !== b.slot) return a.slot === "session" ? -1 : 1;
    return new Date(a.resetAt).getTime() - new Date(b.resetAt).getTime();
  });

  const unique = reached.filter((item, index) => reached.findIndex((candidate) => candidate.slot === item.slot) === index);
  const effectiveResetAt = unique.reduce((latest, item) => (
    new Date(item.resetAt).getTime() > new Date(latest).getTime() ? item.resetAt : latest
  ), unique[0].resetAt);

  return { reached: unique, effectiveResetAt };
}

export function formatResetCountdown(value: string, now = Date.now()): string {
  const resetTime = new Date(value).getTime();
  if (!Number.isFinite(resetTime)) return "--:--";
  const totalSeconds = Math.max(0, Math.ceil((resetTime - now) / 1000));
  const days = Math.floor(totalSeconds / 86_400);
  const hours = Math.floor((totalSeconds % 86_400) / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  const two = (part: number) => String(part).padStart(2, "0");
  if (days > 0) return `${days}d ${two(hours)}h ${two(minutes)}m`;
  if (hours > 0) return `${two(hours)}h ${two(minutes)}m ${two(seconds)}s`;
  return `${two(minutes)}m ${two(seconds)}s`;
}
