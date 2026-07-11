import { invoke } from "@tauri-apps/api/core";

import type { ProviderCapability, SpendEstimate, UsageHistory, UsageHistoryRange, UsageSnapshot } from "./contracts";

export function getUsageSnapshots(): Promise<UsageSnapshot[]> {
  return invoke<UsageSnapshot[]>("get_usage_snapshots");
}

export function getLocalSpendEstimate(): Promise<SpendEstimate> {
  return invoke<SpendEstimate>("get_local_spend_estimate");
}

export function getUsageHistory(range: UsageHistoryRange): Promise<UsageHistory> {
  return invoke<UsageHistory>("get_usage_history", { range });
}

export function getProviderCapabilities(): Promise<ProviderCapability[]> {
  return invoke<ProviderCapability[]>("get_provider_capabilities");
}

export function exportRedactedDiagnostics(): Promise<string> {
  return invoke<string>("export_redacted_diagnostics");
}
