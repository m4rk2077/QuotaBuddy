import { invoke } from "@tauri-apps/api/core";
import type { MetricKind } from "./contracts";

export type Theme = "light" | "dark";
export type Language = "en" | "ptBr";

export type MonitorPreferences = {
  theme: Theme;
  language: Language;
  startWithWindows: boolean;
  pinnedMetrics: MetricKind[];
  alertThresholds: number[];
};

export const defaultMonitorPreferences: MonitorPreferences = {
  theme: "dark",
  language: "en",
  startWithWindows: false,
  pinnedMetrics: [],
  alertThresholds: [80, 95],
};

export function canPinMetric(pinnedMetrics: MetricKind[], kind: MetricKind): boolean {
  return pinnedMetrics.includes(kind) || pinnedMetrics.length < 2;
}

export function getMonitorPreferences(): Promise<MonitorPreferences> {
  return invoke<MonitorPreferences>("get_monitor_preferences");
}

export function saveMonitorPreferences(preferences: MonitorPreferences): Promise<MonitorPreferences> {
  return invoke<MonitorPreferences>("save_monitor_preferences", { preferences });
}
