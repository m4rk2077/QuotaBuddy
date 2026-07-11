export type ProviderId = "codex" | "claudeCode" | "cursor";

export type ProviderCapability = {
  provider: ProviderId;
  clientDetected: boolean;
  usageIntegration: "native" | "optInBridge" | "unavailable";
};

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
  amountUsd: number | null;
  pricingCoveragePercent: number;
  pricingTableVersion: string;
  recordCount: number;
  isEstimate: boolean;
  label: string;
};

export type UsageHistoryRange = "7d" | "30d" | "all";

export type UsageTokenTotals = {
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
};

export type LocalUsageHistory = {
  range: UsageHistoryRange;
  totals: UsageTokenTotals;
  byModel: Array<{
    modelId: string;
    tokens: number;
    tokenSharePercent: number;
    cachedInputPercent: number;
    apiEquivalentUsd: number | null;
  }>;
  daily: Array<{
    date: string;
    tokens: number;
    apiEquivalentUsd: number | null;
  }>;
  apiEquivalent: {
    amountUsd: number | null;
    pricedTokenPercent: number;
    pricingTableVersion: string;
    label: string;
  };
  coverage: "completeForSource" | "partial" | "indexing" | "unavailable";
};

export type AccountUsageHistory = {
  summary: {
    lifetimeTokens: number | null;
    peakDailyTokens: number | null;
    longestRunningTurnSeconds: number | null;
    currentStreakDays: number | null;
    longestStreakDays: number | null;
  };
  daily: Array<{
    startDate: string;
    tokens: number;
  }>;
};

export type UsageProfile = {
  authMode: string | null;
  planType: string | null;
  scopeLabel: string;
  hermesStatus: "notDetected" | "configured" | "active";
  hermesLabel: string;
};

export type UsageHistory = {
  local: LocalUsageHistory;
  account: AccountUsageHistory | null;
  profile: UsageProfile;
};
