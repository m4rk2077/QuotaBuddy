export type ProviderId = "codex" | "claudeCode" | "cursor";

export type MetricKind = "session" | "weekly" | "cycle" | "credits" | "estimatedSpend";

export type UsageSnapshot = {
  provider: ProviderId;
  availability: {
    clientDetected: boolean;
    usageAvailable: boolean;
  };
  metrics: Array<{
    kind: MetricKind;
    label: string;
    usedPercentage: number | null;
    remaining: string | null;
    total: string | null;
    isEstimate: boolean;
  }>;
  reset: { resetsAt: string; label: string } | null;
  lastSuccessfulRefreshAt: string | null;
  status: "healthy" | "unavailable" | "failed" | "reauthRequired";
  error: { code: string; message: string } | null;
  isStale: boolean;
};
