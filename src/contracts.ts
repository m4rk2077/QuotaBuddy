export type ProviderId = "codex" | "claudeCode" | "cursor";

export type MetricKind = "session" | "weekly" | "cycle" | "credits" | "estimatedSpend";

export type ResetMetadata = { resetsAt: string; label: string };

export type UsageMetric = {
  kind: MetricKind;
  label: string;
  usedPercentage: number | null;
  remaining: string | null;
  total: string | null;
  isEstimate: boolean;
  reset?: ResetMetadata | null;
};

export type UsageSnapshot = {
  provider: ProviderId;
  availability: {
    clientDetected: boolean;
    usageAvailable: boolean;
  };
  metrics: UsageMetric[];
  reset: ResetMetadata | null;
  lastSuccessfulRefreshAt: string | null;
  status: "healthy" | "unavailable" | "failed" | "reauthRequired";
  error: { code: string; message: string } | null;
  isStale: boolean;
};

export type SpendEstimate = {
  amountUsd: number;
  pricingTableVersion: string;
  recordCount: number;
  isEstimate: boolean;
  label: string;
};
