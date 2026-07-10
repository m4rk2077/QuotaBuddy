import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { UsageSnapshot } from "./contracts";
import { shouldShowEmptyState } from "./panel-state";
import { canPinMetric, defaultMonitorPreferences, getMonitorPreferences, saveMonitorPreferences, type MonitorPreferences } from "./monitor-controls";
import { exportRedactedDiagnostics, getLocalSpendEstimate, getUsageSnapshots } from "./usage";
import "./App.css";

const providerLabels: Record<UsageSnapshot["provider"], string> = {
  codex: "Codex",
  claudeCode: "Claude Code",
  cursor: "Cursor",
};

function resetMessage(snapshot: UsageSnapshot, text: typeof en): string | null {
  if (!snapshot.reset) return null;
  const date = new Date(snapshot.reset.resetsAt);
  if (Number.isNaN(date.getTime())) return snapshot.reset.label;
  return `${text.resets} ${date.toLocaleString()}`;
}

function App() {
  const [snapshots, setSnapshots] = useState<UsageSnapshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [preferences, setPreferences] = useState<MonitorPreferences>(defaultMonitorPreferences);
  const [savingPreferences, setSavingPreferences] = useState(false);
  const pendingPreferenceSave = useRef(Promise.resolve());
  const [estimate, setEstimate] = useState<UsageSnapshot["metrics"][number] | null>(null);
  const [estimateError, setEstimateError] = useState<string | null>(null);
  const [diagnosticStatus, setDiagnosticStatus] = useState<string | null>(null);
  const text = { ...en, ...(preferences.language === "ptBr" ? ptBr : {}) };

  const refresh = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      setSnapshots(await getUsageSnapshots());
    } catch {
      setLoadError("QuotaBuddy could not load local usage snapshots.");
    }
    try {
      const nextEstimate = await getLocalSpendEstimate();
      setEstimate({
        kind: "estimatedSpend",
        label: nextEstimate.label,
        usedPercentage: null,
        remaining: `$${nextEstimate.amountUsd.toFixed(5)} (pricing ${nextEstimate.pricingTableVersion})`,
        total: null,
        isEstimate: nextEstimate.isEstimate,
      });
      setEstimateError(null);
    } catch {
      setEstimate(null);
      setEstimateError(text.estimateUnavailable);
    } finally {
      setLoading(false);
    }
  }, [text.estimateUnavailable]);

  const exportDiagnostics = useCallback(async () => {
    setDiagnosticStatus(null);
    try {
      const path = await exportRedactedDiagnostics();
      setDiagnosticStatus(`${text.diagnosticsSaved} ${path}`);
    } catch {
      setDiagnosticStatus(text.diagnosticsFailed);
    }
  }, [text.diagnosticsFailed, text.diagnosticsSaved]);

  useEffect(() => {
    void refresh();
    void getMonitorPreferences().then(setPreferences).catch(() => setLoadError("QuotaBuddy could not load monitor preferences."));
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
    const hideOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") void invoke("hide_main_window");
    };
    window.addEventListener("keydown", hideOnEscape);
    return () => window.removeEventListener("keydown", hideOnEscape);
  }, []);

  const updatePreferences = async (next: MonitorPreferences) => {
    setPreferences(next);
    setSavingPreferences(true);
    const save = pendingPreferenceSave.current.then(() => saveMonitorPreferences(next));
    pendingPreferenceSave.current = save.then(() => undefined, () => undefined);
    try {
      setPreferences(await save);
      void refresh();
    } catch {
      setLoadError("QuotaBuddy could not save monitor preferences.");
    } finally {
      setSavingPreferences(false);
    }
  };

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <p className="eyebrow">{text.eyebrow}</p>
          <h1>QuotaBuddy</h1>
          <p className="subtitle">{text.subtitle}</p>
        </div>
        <button className="refresh" type="button" onClick={() => void refresh()} disabled={loading}>
          {loading ? text.refreshing : text.refresh}
        </button>
        <button className="refresh" type="button" onClick={() => void exportDiagnostics()}>
          {text.exportDiagnostics}
        </button>
      </header>

      {loadError ? <p className="notice error">{loadError}</p> : null}
      {estimateError ? <p className="notice error">{estimateError}</p> : null}
      {diagnosticStatus ? <p className="notice">{diagnosticStatus}</p> : null}

      {estimate ? (
        <section className="snapshot-card" aria-label={text.estimatedSpend}>
          <div className="metric">
            <div><span>{estimate.label}</span><strong>{estimate.remaining}</strong></div>
            <small>{text.estimateDescription}</small>
          </div>
        </section>
      ) : null}

      {shouldShowEmptyState(loading, snapshots) ? (
        <section className="empty-state" aria-label="No supported clients detected">
          <span aria-hidden="true">⌁</span>
          <h2>{text.noClient}</h2>
          <p>{text.noClientDescription}</p>
        </section>
      ) : null}

      <section className="snapshot-grid" aria-live="polite">
        {snapshots.map((snapshot) => (
          <SnapshotCard key={snapshot.provider} snapshot={snapshot} text={text} />
        ))}
      </section>
      <Settings preferences={preferences} text={text} saving={savingPreferences} onChange={updatePreferences} />
    </main>
  );
}

function SnapshotCard({ snapshot, text }: { snapshot: UsageSnapshot; text: typeof en }) {
  const reset = resetMessage(snapshot, text);

  return (
    <article className="snapshot-card">
            <div className="card-title">
              <h2>{providerLabels[snapshot.provider]}</h2>
              <span className={`status ${snapshot.status}`}>{text[snapshot.status]}</span>
            </div>
            {snapshot.metrics.length > 0 ? (
              snapshot.metrics.map((metric) => (
                <div className="metric" key={metric.kind}>
                  <div><span>{metric.kind === "session" ? text.session : text.longer}</span><strong>{metric.remaining ?? text.unavailable}</strong></div>
                  {metric.usedPercentage !== null ? <progress value={metric.usedPercentage} max="100" /> : null}
                  {metric.isEstimate ? <small>{text.estimate}</small> : null}
                </div>
              ))
            ) : (
              <p className="unavailable">{text.unavailable}</p>
            )}
            {reset ? <p className="reset">{reset}</p> : null}
            {snapshot.isStale ? <p className="notice">{text.stale}</p> : null}
            {snapshot.error ? <p className="notice">{snapshot.error.message}</p> : null}
    </article>
  );
}

function Settings({ preferences, text, saving, onChange }: { preferences: MonitorPreferences; text: typeof en; saving: boolean; onChange: (preferences: MonitorPreferences) => Promise<void> }) {
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

  return <section className="settings" aria-label={text.settings}>
    <h2>{text.settings}</h2>
    <div className="settings-grid">
      <label>{text.theme}<select value={preferences.theme} onChange={(event) => void onChange({ ...preferences, theme: event.target.value as MonitorPreferences["theme"] })}><option value="dark">{text.dark}</option><option value="light">{text.light}</option></select></label>
      <label>{text.language}<select value={preferences.language} onChange={(event) => void onChange({ ...preferences, language: event.target.value as MonitorPreferences["language"] })}><option value="en">English</option><option value="ptBr">Português (Brasil)</option></select></label>
      <label className="check"><input type="checkbox" checked={preferences.startWithWindows} onChange={(event) => void onChange({ ...preferences, startWithWindows: event.target.checked })} /> {text.startup}</label>
      <fieldset><legend>{text.alerts}</legend>{preferences.alertThresholds.map((threshold, index) => <label key={index}>{text.threshold}<input type="number" min="1" max="100" value={threshold} onChange={(event) => setThreshold(index, Number(event.target.value))} />%</label>)}</fieldset>
      <fieldset><legend>{text.tray}</legend>{metrics.map((kind) => <label className="check" key={kind}><input type="checkbox" checked={preferences.pinnedMetrics.includes(kind)} disabled={!canPinMetric(preferences.pinnedMetrics, kind)} onChange={() => toggleMetric(kind)} /> {kind === "session" ? text.session : text.longer}</label>)}</fieldset>
    </div>
    {saving ? <p className="notice">{text.saving}</p> : null}
  </section>;
}

const en = { eyebrow: "LOCAL USAGE MONITOR", subtitle: "Only detected clients appear here. Credentials stay in the native core.", refresh: "Refresh", refreshing: "Refreshing…", noClient: "No supported client detected", noClientDescription: "Install or add Codex to your PATH. QuotaBuddy will keep this panel uncluttered until then.", settings: "Monitor controls", theme: "Theme", dark: "Dark", light: "Light", language: "Language", startup: "Start QuotaBuddy with Windows", alerts: "Usage alerts", threshold: "Alert at", tray: "Tray metrics (up to two)", session: "Session limit", longer: "Longer limit", saving: "Saving preferences…", resets: "Resets", unavailable: "Unavailable", estimate: "Estimate only", stale: "Showing last successful snapshot.", healthy: "healthy", failed: "failed", reauthRequired: "reauthentication required", exportDiagnostics: "Export redacted diagnostics", estimatedSpend: "Estimated local Codex spend", estimateDescription: "Estimate only. Local logs and versioned pricing; not provider billing.", estimateUnavailable: "QuotaBuddy could not calculate local spend.", diagnosticsSaved: "Redacted diagnostics saved locally:", diagnosticsFailed: "QuotaBuddy could not export redacted diagnostics." };
const ptBr = { eyebrow: "MONITOR LOCAL DE USO", subtitle: "Somente clientes detectados aparecem aqui. Credenciais permanecem no núcleo nativo.", refresh: "Atualizar", refreshing: "Atualizando…", noClient: "Nenhum cliente compatível detectado", noClientDescription: "Instale ou adicione o Codex ao seu PATH. O QuotaBuddy manterá este painel sem itens extras até lá.", settings: "Controles do monitor", theme: "Tema", dark: "Escuro", light: "Claro", language: "Idioma", startup: "Iniciar o QuotaBuddy com o Windows", alerts: "Alertas de uso", threshold: "Alertar em", tray: "Métricas da bandeja (até duas)", session: "Limite de sessão", longer: "Limite mais longo", saving: "Salvando preferências…", resets: "Redefine", unavailable: "Indisponível", estimate: "Somente estimativa", stale: "Exibindo o último registro atualizado.", healthy: "normal", failed: "falhou", reauthRequired: "nova autenticação necessária", exportDiagnostics: "Exportar diagnóstico redigido", estimatedSpend: "Gasto local estimado do Codex", estimateDescription: "Somente estimativa. Logs locais e preços versionados; não é cobrança do provedor.", estimateUnavailable: "O QuotaBuddy não pôde calcular o gasto local.", diagnosticsSaved: "Diagnóstico redigido salvo localmente:", diagnosticsFailed: "O QuotaBuddy não pôde exportar o diagnóstico redigido." };

export default App;
