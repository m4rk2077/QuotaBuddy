import { useCallback, useEffect, useState } from "react";

import type { UsageSnapshot } from "./contracts";
import { shouldShowEmptyState } from "./panel-state";
import { getUsageSnapshots } from "./usage";
import "./App.css";

const providerLabels: Record<UsageSnapshot["provider"], string> = {
  codex: "Codex",
  claudeCode: "Claude Code",
  cursor: "Cursor",
};

function resetMessage(snapshot: UsageSnapshot): string | null {
  if (!snapshot.reset) return null;
  const date = new Date(snapshot.reset.resetsAt);
  if (Number.isNaN(date.getTime())) return snapshot.reset.label;
  return `Resets ${date.toLocaleString()}`;
}

function App() {
  const [snapshots, setSnapshots] = useState<UsageSnapshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      setSnapshots(await getUsageSnapshots());
    } catch {
      setLoadError("QuotaBuddy could not load local usage snapshots.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const interval = window.setInterval(() => void refresh(), 5 * 60 * 1000);
    return () => window.clearInterval(interval);
  }, [refresh]);

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <p className="eyebrow">LOCAL USAGE MONITOR</p>
          <h1>QuotaBuddy</h1>
          <p className="subtitle">Only detected clients appear here. Credentials stay in the native core.</p>
        </div>
        <button className="refresh" type="button" onClick={() => void refresh()} disabled={loading}>
          {loading ? "Refreshing…" : "Refresh"}
        </button>
      </header>

      {loadError ? <p className="notice error">{loadError}</p> : null}

      {shouldShowEmptyState(loading, snapshots) ? (
        <section className="empty-state" aria-label="No supported clients detected">
          <span aria-hidden="true">⌁</span>
          <h2>No supported client detected</h2>
          <p>Install or add Codex to your PATH. QuotaBuddy will keep this panel uncluttered until then.</p>
        </section>
      ) : null}

      <section className="snapshot-grid" aria-live="polite">
        {snapshots.map((snapshot) => (
          <SnapshotCard key={snapshot.provider} snapshot={snapshot} />
        ))}
      </section>
    </main>
  );
}

function SnapshotCard({ snapshot }: { snapshot: UsageSnapshot }) {
  const reset = resetMessage(snapshot);

  return (
    <article className="snapshot-card">
            <div className="card-title">
              <h2>{providerLabels[snapshot.provider]}</h2>
              <span className={`status ${snapshot.status}`}>{snapshot.status}</span>
            </div>
            {snapshot.metrics.length > 0 ? (
              snapshot.metrics.map((metric) => (
                <div className="metric" key={metric.kind}>
                  <div><span>{metric.label}</span><strong>{metric.remaining ?? "Unavailable"}</strong></div>
                  {metric.usedPercentage !== null ? <progress value={metric.usedPercentage} max="100" /> : null}
                  {metric.isEstimate ? <small>Estimate only</small> : null}
                </div>
              ))
            ) : (
              <p className="unavailable">Usage adapter arrives in a later ticket.</p>
            )}
            {reset ? <p className="reset">{reset}</p> : null}
            {snapshot.isStale ? <p className="notice">Showing last successful snapshot.</p> : null}
            {snapshot.error ? <p className="notice">{snapshot.error.message}</p> : null}
    </article>
  );
}

export default App;
