import { invoke } from "@tauri-apps/api/core";

import type { SpendEstimate, UsageSnapshot } from "./contracts";

export function getUsageSnapshots(): Promise<UsageSnapshot[]> {
  return invoke<UsageSnapshot[]>("get_usage_snapshots");
}

export function getLocalSpendEstimate(): Promise<SpendEstimate> {
  return invoke<SpendEstimate>("get_local_spend_estimate");
}

export function exportRedactedDiagnostics(): Promise<string> {
  return invoke<string>("export_redacted_diagnostics");
}
