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
  pinnedMetrics: ["session", "cycle"],
  alertThresholds: [80, 95],
};

export function canPinMetric(pinnedMetrics: MetricKind[], kind: MetricKind): boolean {
  return pinnedMetrics.includes(kind) || pinnedMetrics.length < 2;
}

export function updateAlertThresholds(thresholds: number[], index: number, rawValue: string): number[] | null {
  const normalized = rawValue.trim();
  if (!/^\d{1,3}$/.test(normalized)) return null;

  const value = Number(normalized);
  if (!Number.isInteger(value) || value < 1 || value > 100 || index < 0 || index >= thresholds.length) return null;

  const next = [...thresholds];
  next[index] = value;
  return next;
}

type MonitorPreferenceSaveTicket = {
  version: number;
};

export type MonitorPreferenceSaveResolution = {
  preferences: MonitorPreferences;
  applyToView: boolean;
  saving: boolean;
  failed: boolean;
};

export class MonitorPreferenceSaveCoordinator {
  private confirmed: MonitorPreferences;
  private current: MonitorPreferences;
  private latestVersion = 0;
  private confirmedVersion = 0;
  private pending = 0;

  constructor(initial: MonitorPreferences) {
    this.confirmed = initial;
    this.current = initial;
  }

  hydrate(confirmed: MonitorPreferences): MonitorPreferences | null {
    this.confirmed = confirmed;
    if (this.pending > 0) return null;
    this.current = confirmed;
    return confirmed;
  }

  begin(optimistic: MonitorPreferences): MonitorPreferenceSaveTicket {
    const ticket = { version: this.latestVersion + 1 };
    this.latestVersion = ticket.version;
    this.current = optimistic;
    this.pending += 1;
    return ticket;
  }

  async settle(ticket: MonitorPreferenceSaveTicket, save: Promise<MonitorPreferences>): Promise<MonitorPreferenceSaveResolution> {
    let saved: MonitorPreferences | null = null;
    let failed = false;
    try {
      saved = await save;
      if (ticket.version >= this.confirmedVersion) {
        this.confirmed = saved;
        this.confirmedVersion = ticket.version;
      }
    } catch {
      failed = true;
    }

    const applyToView = ticket.version === this.latestVersion;
    if (applyToView) {
      this.current = failed ? this.confirmed : (saved ?? this.confirmed);
    }
    this.pending = Math.max(0, this.pending - 1);

    return {
      preferences: this.current,
      applyToView,
      saving: this.pending > 0,
      failed,
    };
  }
}

export function getMonitorPreferences(): Promise<MonitorPreferences> {
  return invoke<MonitorPreferences>("get_monitor_preferences");
}

export function saveMonitorPreferences(preferences: MonitorPreferences): Promise<MonitorPreferences> {
  return invoke<MonitorPreferences>("save_monitor_preferences", { preferences });
}
