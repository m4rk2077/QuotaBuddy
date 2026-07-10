import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import brandIcon from "../src-tauri/icons/icon.png";
import type { SpendEstimate, UsageMetric, UsageSnapshot } from "./contracts";
import { canPinMetric, defaultMonitorPreferences, getMonitorPreferences, saveMonitorPreferences, type MonitorPreferences } from "./monitor-controls";
import { getMetricPresentation, getSnapshotDisplayState, selectOverviewMetrics, shouldShowEmptyState, type MetricSeverity } from "./panel-state";
import { createSingleFlightRefresh } from "./refresh-state";
import { exportRedactedDiagnostics, getLocalSpendEstimate, getUsageSnapshots } from "./usage";
import "./App.css";

type View = "overview" | "settings";
type Copy = typeof en;

function App() {
  const [view, setView] = useState<View>("overview");
  const [snapshots, setSnapshots] = useState<UsageSnapshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [preferences, setPreferences] = useState<MonitorPreferences>(defaultMonitorPreferences);
  const [savingPreferences, setSavingPreferences] = useState(false);
  const [estimate, setEstimate] = useState<SpendEstimate | null>(null);
  const [estimateLoading, setEstimateLoading] = useState(true);
  const [estimateError, setEstimateError] = useState(false);
  const [diagnosticStatus, setDiagnosticStatus] = useState<string | null>(null);
  const [diagnosticError, setDiagnosticError] = useState(false);
  const pendingPreferenceSave = useRef(Promise.resolve());
  const text = useMemo(() => preferences.language === "ptBr" ? { ...en, ...ptBr } : en, [preferences.language]);
  const loadFailedText = useRef(text.loadFailed);
  loadFailedText.current = text.loadFailed;
  const usageRefresh = useRef<(() => Promise<void>) | null>(null);
  const spendRefresh = useRef<(() => Promise<void>) | null>(null);
  if (!usageRefresh.current) {
    usageRefresh.current = createSingleFlightRefresh(getUsageSnapshots, {
      loading: (value) => {
        setLoading(value);
        if (value) setLoadError(null);
      },
      success: setSnapshots,
      failure: () => setLoadError(loadFailedText.current),
    });
  }
  if (!spendRefresh.current) {
    spendRefresh.current = createSingleFlightRefresh(getLocalSpendEstimate, {
      loading: (value) => {
        setEstimateLoading(value);
        if (value) setEstimateError(false);
      },
      success: (value) => {
        setEstimate(value);
        setEstimateError(false);
      },
      failure: () => {
        setEstimate(null);
        setEstimateError(true);
      },
    });
  }

  const refresh = useCallback(() => {
    void usageRefresh.current?.();
    void spendRefresh.current?.();
  }, []);

  useEffect(() => {
    void getMonitorPreferences().then(setPreferences).catch(() => setLoadError(text.preferencesFailed));
  }, [text.preferencesFailed]);

  useEffect(() => {
    void refresh();
    const interval = window.setInterval(() => void refresh(), 5 * 60 * 1000);
    return () => window.clearInterval(interval);
  }, [refresh]);

  useEffect(() => {
    document.documentElement.dataset.theme = preferences.theme;
  }, [preferences.theme]);

  useEffect(() => {
    void invoke<"desktop-acrylic" | "solid">("get_window_backdrop")
      .then((mode) => { document.documentElement.dataset.backdrop = mode; })
      .catch(() => { document.documentElement.dataset.backdrop = "solid"; });
  }, []);

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (view === "settings") setView("overview");
      else void invoke("hide_main_window");
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [view]);

  const updatePreferences = async (next: MonitorPreferences) => {
    setPreferences(next);
    setSavingPreferences(true);
    const save = pendingPreferenceSave.current.then(() => saveMonitorPreferences(next));
    pendingPreferenceSave.current = save.then(() => undefined, () => undefined);
    try {
      setPreferences(await save);
      void refresh();
    } catch {
      setLoadError(text.preferencesSaveFailed);
    } finally {
      setSavingPreferences(false);
    }
  };

  const exportDiagnostics = useCallback(async () => {
    setDiagnosticStatus(null);
    try {
      const path = await exportRedactedDiagnostics();
      setDiagnosticStatus(`${text.diagnosticsSaved} ${path}`);
      setDiagnosticError(false);
    } catch {
      setDiagnosticStatus(text.diagnosticsFailed);
      setDiagnosticError(true);
    }
  }, [text]);

  return (
    <main className="app-shell">
      {view === "overview" ? (
        <Overview
          snapshots={snapshots}
          loading={loading}
          loadError={loadError}
          estimate={estimate}
          estimateLoading={estimateLoading}
          estimateError={estimateError}
          preferences={preferences}
          text={text}
          onRefresh={() => void refresh()}
          onSettings={() => setView("settings")}
        />
      ) : (
        <SettingsPanel
          preferences={preferences}
          text={text}
          saving={savingPreferences}
          diagnosticStatus={diagnosticStatus}
          diagnosticError={diagnosticError}
          onBack={() => setView("overview")}
          onChange={updatePreferences}
          onExport={exportDiagnostics}
        />
      )}
    </main>
  );
}

function Overview({ snapshots, loading, loadError, estimate, estimateLoading, estimateError, preferences, text, onRefresh, onSettings }: {
  snapshots: UsageSnapshot[];
  loading: boolean;
  loadError: string | null;
  estimate: SpendEstimate | null;
  estimateLoading: boolean;
  estimateError: boolean;
  preferences: MonitorPreferences;
  text: Copy;
  onRefresh: () => void;
  onSettings: () => void;
}) {
  const snapshot = snapshots.find((item) => item.provider === "codex") ?? snapshots[0];
  const selected = snapshot ? selectOverviewMetrics(snapshot) : { session: null, weekly: null };
  const status = getHeaderStatus(snapshot, loading, loadError, text);
  const initialFailure = !loading && Boolean(loadError) && !snapshot;

  return <div className="overview">
    <PanelHeader status={status} />
    {initialFailure ? <FailureState text={text} /> : shouldShowEmptyState(loading, snapshots) ? <EmptyState text={text} /> : (
      <section className="metric-stack" aria-live="polite" aria-busy={loading}>
        <MetricCard slot="session" metric={selected.session} snapshot={snapshot} loading={loading} thresholds={preferences.alertThresholds} text={text} />
        <MetricCard slot="weekly" metric={selected.weekly} snapshot={snapshot} loading={loading} thresholds={preferences.alertThresholds} text={text} />
      </section>
    )}
    <SpendRow estimate={estimate} loading={estimateLoading} unavailable={estimateError} text={text} />
    <footer className="panel-footer">
      <span className="privacy-note">{text.localOnly}</span>
      <div className="footer-actions">
        <IconButton label={loading ? text.refreshing : text.refresh} onClick={onRefresh} disabled={loading}><RefreshIcon /></IconButton>
        <IconButton label={text.settings} onClick={onSettings}><SettingsIcon /></IconButton>
      </div>
    </footer>
  </div>;
}

function EmptyState({ text }: { text: Copy }) {
  return <section className="empty-state" aria-label={text.noClient}>
    <div className="empty-mark" aria-hidden="true"><span /></div>
    <h2>{text.noClient}</h2>
    <p>{text.noClientDescription}</p>
  </section>;
}

function FailureState({ text }: { text: Copy }) {
  return <section className="empty-state failure-state" aria-label={text.updateFailed}>
    <div className="empty-mark" aria-hidden="true"><span /></div>
    <h2>{text.updateFailed}</h2>
    <p>{text.updateFailedDescription}</p>
  </section>;
}

function PanelHeader({ status }: { status: { label: string; tone: "healthy" | "warning" | "danger" | "muted" } }) {
  return <header className="panel-header">
    <img className="brand-icon" src={brandIcon} alt="" />
    <div className="brand-copy">
      <h1>QuotaBuddy</h1>
      <p className={`update-status ${status.tone}`}><span className="status-dot" aria-hidden="true" />{status.label}</p>
    </div>
  </header>;
}

function MetricCard({ slot, metric, snapshot, loading, thresholds, text }: {
  slot: "session" | "weekly";
  metric: UsageMetric | null;
  snapshot?: UsageSnapshot;
  loading: boolean;
  thresholds: number[];
  text: Copy;
}) {
  const title = slot === "session" ? text.session : text.weekly;
  if (loading && !metric) return <MetricSkeleton title={title} />;

  const presentation = metric ? getMetricPresentation(metric, thresholds) : { remainingPercentage: null, severity: "unavailable" as const };
  const stateLabel = getMetricStateLabel(snapshot, presentation.severity, text);
  const reset = formatReset(metric?.reset?.resetsAt, text);

  return <article className={`metric-card ${presentation.severity}`} aria-label={`${title}: ${stateLabel}`}>
    <div className="metric-card-heading">
      <h2>{title}</h2>
      <span className="reset-time"><ClockIcon />{reset ?? text.resetUnknown}</span>
    </div>
    <div className="metric-body">
      <MetricRing value={presentation.remainingPercentage} severity={presentation.severity} label={stateLabel} />
      <div className="metric-summary">
        {presentation.remainingPercentage === null ? (
          <strong className="metric-unavailable">—</strong>
        ) : (
          <strong className="metric-percentage">{presentation.remainingPercentage}%</strong>
        )}
        <span className="remaining-label">{stateLabel}</span>
        <div className="progress-track" role="progressbar" aria-label={`${title}: ${stateLabel}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={presentation.remainingPercentage ?? undefined}>
          <span style={{ width: `${presentation.remainingPercentage ?? 0}%` }} />
        </div>
      </div>
    </div>
  </article>;
}

function MetricRing({ value, severity, label }: { value: number | null; severity: MetricSeverity; label: string }) {
  const radius = 35;
  const circumference = 2 * Math.PI * radius;
  const dash = value === null ? 0 : circumference * (value / 100);
  return <div className={`metric-ring ${severity}`} role="img" aria-label={label}>
    <svg viewBox="0 0 84 84" aria-hidden="true">
      <circle className="ring-track" cx="42" cy="42" r={radius} />
      <circle className="ring-value" cx="42" cy="42" r={radius} strokeDasharray={`${dash} ${circumference}`} />
    </svg>
    <strong aria-hidden="true">{value === null ? "—" : `${value}%`}</strong>
  </div>;
}

function MetricSkeleton({ title }: { title: string }) {
  return <article className="metric-card skeleton" aria-label={`${title}, loading`}>
    <div className="metric-card-heading"><h2>{title}</h2><span className="skeleton-line short" /></div>
    <div className="metric-body"><span className="skeleton-ring" /><div className="metric-summary"><span className="skeleton-line value" /><span className="skeleton-line" /><span className="skeleton-line bar" /></div></div>
  </article>;
}

function SpendRow({ estimate, loading, unavailable, text }: { estimate: SpendEstimate | null; loading: boolean; unavailable: boolean; text: Copy }) {
  const amount = estimate ? new Intl.NumberFormat(text.locale, { style: "currency", currency: "USD" }).format(estimate.amountUsd) : "—";
  return <section className="spend-row" aria-label={text.estimatedSpend} aria-busy={loading}>
    <WalletIcon />
    <div><span>{text.estimatedSpend}</span><small>{loading ? text.calculatingEstimate : estimate?.isEstimate ? text.estimate : text.localData}</small></div>
    <strong>{loading ? "…" : unavailable ? text.unavailable : amount}</strong>
  </section>;
}

function SettingsPanel({ preferences, text, saving, diagnosticStatus, diagnosticError, onBack, onChange, onExport }: {
  preferences: MonitorPreferences;
  text: Copy;
  saving: boolean;
  diagnosticStatus: string | null;
  diagnosticError: boolean;
  onBack: () => void;
  onChange: (preferences: MonitorPreferences) => Promise<void>;
  onExport: () => void;
}) {
  const toggleMetric = (kind: MonitorPreferences["pinnedMetrics"][number]) => {
    const pinnedMetrics = preferences.pinnedMetrics.includes(kind) ? preferences.pinnedMetrics.filter((value) => value !== kind) : [...preferences.pinnedMetrics, kind];
    void onChange({ ...preferences, pinnedMetrics });
  };
  const setThreshold = (index: number, value: number) => {
    const alertThresholds = [...preferences.alertThresholds];
    alertThresholds[index] = value;
    void onChange({ ...preferences, alertThresholds });
  };
  const metrics: MonitorPreferences["pinnedMetrics"][number][] = ["session", "cycle"];

  return <section className="settings-panel" aria-labelledby="settings-title">
    <header className="settings-header">
      <IconButton label={text.back} onClick={onBack}><BackIcon /></IconButton>
      <div><h1 id="settings-title">{text.settings}</h1><p>{saving ? text.saving : text.settingsSubtitle}</p></div>
    </header>
    <div className="settings-scroll">
      <div className="settings-group">
        <h2>{text.appearance}</h2>
        <label>{text.theme}<select value={preferences.theme} onChange={(event) => void onChange({ ...preferences, theme: event.target.value as MonitorPreferences["theme"] })}><option value="dark">{text.dark}</option><option value="light">{text.light}</option></select></label>
        <label>{text.language}<select value={preferences.language} onChange={(event) => void onChange({ ...preferences, language: event.target.value as MonitorPreferences["language"] })}><option value="en">English</option><option value="ptBr">Português (Brasil)</option></select></label>
      </div>
      <div className="settings-group">
        <h2>{text.behavior}</h2>
        <label className="check"><input type="checkbox" checked={preferences.startWithWindows} onChange={(event) => void onChange({ ...preferences, startWithWindows: event.target.checked })} /><span>{text.startup}</span></label>
      </div>
      <fieldset className="settings-group"><legend>{text.alerts}</legend>{preferences.alertThresholds.map((threshold, index) => <label key={index}>{index === 0 ? text.warningAt : text.criticalAt}<span className="number-field"><input type="number" min="1" max="100" value={threshold} onChange={(event) => setThreshold(index, Number(event.target.value))} />%</span></label>)}</fieldset>
      <fieldset className="settings-group"><legend>{text.tray}</legend>{metrics.map((kind) => <label className="check" key={kind}><input type="checkbox" checked={preferences.pinnedMetrics.includes(kind)} disabled={!canPinMetric(preferences.pinnedMetrics, kind)} onChange={() => toggleMetric(kind)} /><span>{kind === "session" ? text.session : text.weekly}</span></label>)}</fieldset>
      <div className="settings-group diagnostics">
        <h2>{text.privacy}</h2><p>{text.diagnosticsDescription}</p>
        <button className="secondary-button" type="button" onClick={onExport}>{text.exportDiagnostics}</button>
        {diagnosticStatus ? <p className={`diagnostic-status${diagnosticError ? " error" : ""}`} role="status">{diagnosticStatus}</p> : null}
      </div>
    </div>
  </section>;
}

function IconButton({ label, children, onClick, disabled = false }: { label: string; children: React.ReactNode; onClick: () => void; disabled?: boolean }) {
  return <button className="icon-button" type="button" aria-label={label} title={label} onClick={onClick} disabled={disabled}>{children}</button>;
}

function getHeaderStatus(snapshot: UsageSnapshot | undefined, loading: boolean, error: string | null, text: Copy) {
  if (loading) return { label: text.refreshing, tone: "muted" as const };
  if (error) return { label: text.updateFailed, tone: "danger" as const };
  if (!snapshot) return { label: text.noClient, tone: "warning" as const };
  const state = getSnapshotDisplayState(snapshot);
  if (state === "reauthRequired") return { label: text.reauthRequired, tone: "danger" as const };
  if (state === "failed") return { label: snapshot.isStale ? text.failedWithCache : text.updateFailed, tone: "danger" as const };
  if (state === "unavailable") return { label: text.unavailable, tone: "warning" as const };
  if (state === "stale") return { label: text.stale, tone: "warning" as const };
  const updated = snapshot.lastSuccessfulRefreshAt ? formatUpdated(snapshot.lastSuccessfulRefreshAt, text) : text.updatedNow;
  return { label: updated, tone: "healthy" as const };
}

function getMetricStateLabel(snapshot: UsageSnapshot | undefined, severity: MetricSeverity, text: Copy) {
  if (snapshot) {
    const state = getSnapshotDisplayState(snapshot);
    if (state === "reauthRequired") return text.reauthShort;
    if (state === "failed") return snapshot.isStale ? text.failedCachedShort : text.updateFailed;
    if (state === "stale") return text.staleShort;
  }
  if (severity === "unavailable") return text.unavailable;
  if (severity === "critical") return text.criticalRemaining;
  if (severity === "warning") return text.warningRemaining;
  return text.remaining;
}

function formatUpdated(value: string, text: Copy) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return text.updatedNow;
  const minutes = Math.max(0, Math.floor((Date.now() - date.getTime()) / 60_000));
  if (minutes < 1) return text.updatedNow;
  return text.updatedMinutes.replace("{minutes}", String(minutes));
}

function formatReset(value: string | undefined, text: Copy) {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  const totalMinutes = Math.max(0, Math.ceil((date.getTime() - Date.now()) / 60_000));
  if (totalMinutes === 0) return text.resetNow;
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  const duration = days > 0 ? `${days}d ${hours}h` : hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
  return text.resetsIn.replace("{duration}", duration);
}

const Icon = ({ children }: { children: React.ReactNode }) => <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">{children}</svg>;
const RefreshIcon = () => <Icon><path d="M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6" /></Icon>;
const SettingsIcon = () => <Icon><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1A1.7 1.7 0 0 0 9 4.6 1.7 1.7 0 0 0 10 3v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z" /></Icon>;
const ClockIcon = () => <Icon><circle cx="12" cy="12" r="8" /><path d="M12 7v5l3 2" /></Icon>;
const WalletIcon = () => <Icon><path d="M3 6.5h16a2 2 0 0 1 2 2v9H5a2 2 0 0 1-2-2v-9Zm0 0 13-2v2M16 12h5" /><circle cx="16" cy="12" r=".6" /></Icon>;
const BackIcon = () => <Icon><path d="m15 18-6-6 6-6" /></Icon>;

const en = {
  locale: "en-US", loadFailed: "Could not update usage", preferencesFailed: "Preferences unavailable", preferencesSaveFailed: "Could not save preferences",
  refresh: "Refresh", refreshing: "Updating…", updateFailed: "Update failed", updateFailedDescription: "QuotaBuddy could not read local usage. Try again from the refresh button.", failedWithCache: "Update failed · showing cached data", updatedNow: "Updated just now", updatedMinutes: "Updated {minutes} min ago", noClient: "Codex not detected", noClientDescription: "Open or sign in to Codex, then refresh. QuotaBuddy will keep checking locally.",
  session: "Session limit", weekly: "Weekly limit", remaining: "remaining", warningRemaining: "remaining · attention", criticalRemaining: "remaining · critical", unavailable: "Unavailable", stale: "Data may be out of date", staleShort: "remaining · stale", failedCachedShort: "remaining · update failed", reauthRequired: "Sign in to Codex again", reauthShort: "Sign in required",
  resetsIn: "resets in {duration}", resetNow: "resets now", resetUnknown: "reset unavailable", estimatedSpend: "Estimated spend", calculatingEstimate: "Calculating locally…", estimate: "Estimate", localData: "Local data", localOnly: "Local & private",
  settings: "Settings", back: "Back to usage", settingsSubtitle: "Local monitor preferences", saving: "Saving…", appearance: "Appearance", theme: "Theme", dark: "Dark", light: "Light", language: "Language", behavior: "Behavior", startup: "Start QuotaBuddy with Windows", alerts: "Usage alerts", warningAt: "Warning at", criticalAt: "Critical at", tray: "Tray metrics (up to two)", privacy: "Privacy & diagnostics", diagnosticsDescription: "Exports a redacted local diagnostic file. Credentials are never included.", exportDiagnostics: "Export redacted diagnostics", diagnosticsSaved: "Saved:", diagnosticsFailed: "Could not export diagnostics.",
};

const ptBr: Partial<Copy> = {
  locale: "pt-BR", loadFailed: "Não foi possível atualizar o uso", preferencesFailed: "Preferências indisponíveis", preferencesSaveFailed: "Não foi possível salvar as preferências",
  refresh: "Atualizar", refreshing: "Atualizando…", updateFailed: "Falha na atualização", updateFailedDescription: "O QuotaBuddy não conseguiu ler o uso local. Tente novamente pelo botão de atualizar.", failedWithCache: "Falha ao atualizar · exibindo dados salvos", updatedNow: "Atualizado agora", updatedMinutes: "Atualizado há {minutes} min", noClient: "Codex não detectado", noClientDescription: "Abra ou entre no Codex e atualize. O QuotaBuddy continuará verificando localmente.",
  session: "Limite da sessão", weekly: "Limite semanal", remaining: "restante", warningRemaining: "restante · atenção", criticalRemaining: "restante · crítico", unavailable: "Indisponível", stale: "Dados podem estar desatualizados", staleShort: "restante · desatualizado", failedCachedShort: "restante · falha ao atualizar", reauthRequired: "Entre novamente no Codex", reauthShort: "Login necessário",
  resetsIn: "reinicia em {duration}", resetNow: "reinicia agora", resetUnknown: "reinício indisponível", estimatedSpend: "Gasto estimado", calculatingEstimate: "Calculando localmente…", estimate: "Estimativa", localData: "Dados locais", localOnly: "Local e privado",
  settings: "Configurações", back: "Voltar ao uso", settingsSubtitle: "Preferências do monitor local", saving: "Salvando…", appearance: "Aparência", theme: "Tema", dark: "Escuro", light: "Claro", language: "Idioma", behavior: "Comportamento", startup: "Iniciar o QuotaBuddy com o Windows", alerts: "Alertas de uso", warningAt: "Atenção em", criticalAt: "Crítico em", tray: "Métricas da bandeja (até duas)", privacy: "Privacidade e diagnóstico", diagnosticsDescription: "Exporta um arquivo local redigido. Credenciais nunca são incluídas.", exportDiagnostics: "Exportar diagnóstico redigido", diagnosticsSaved: "Salvo:", diagnosticsFailed: "Não foi possível exportar o diagnóstico.",
};

export default App;
