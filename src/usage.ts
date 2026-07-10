import { invoke } from "@tauri-apps/api/core";

import type { UsageSnapshot } from "./contracts";

export function getUsageSnapshots(): Promise<UsageSnapshot[]> {
  return invoke<UsageSnapshot[]>("get_usage_snapshots");
}
