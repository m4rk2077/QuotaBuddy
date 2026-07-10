export type ProviderId = "codex" | "claudeCode" | "cursor";

export type UsageSnapshot = {
  provider: ProviderId;
  availability: {
    clientDetected: boolean;
    usageAvailable: boolean;
  };
  metrics: Array<{
    kind: "session" | "weekly" | "cycle" | "credits" | "estimatedSpend";
    label: string;
    usedPercentage: number | null;
    remaining: string | null;
    total: string | null;
    isEstimate: boolean;
  }>;
  reset: { resetsAt: string; label: string } | null;
  lastSuccessfulRefreshAt: string | null;
  status: "healthy" | "unavailable" | "failed";
  error: { code: string; message: string } | null;
  isStale: boolean;
};
