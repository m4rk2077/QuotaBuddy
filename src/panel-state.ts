import type { UsageSnapshot } from "./contracts";

export function shouldShowEmptyState(loading: boolean, snapshots: UsageSnapshot[]): boolean {
  return !loading && snapshots.length === 0;
}
