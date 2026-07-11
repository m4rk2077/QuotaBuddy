import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { AccountUsageHistory, LocalUsageHistory, UsageHistory, UsageHistoryRange, UsageProfile } from "./contracts";
import { formatDuration, formatPercent, formatShortDate, formatTokenCount, formatUsdEquivalent, getBarHeight } from "./history-format";
import { getUsageHistory, getUsageSnapshots } from "./usage";

type HistoryScope = "account" | "local";
type HistoryLanguage = "en" | "ptBr";
type HistoryCopy = typeof historyEn;

export function HistoryPanel({ language, onBack }: { language: HistoryLanguage; onBack: () => void }) {
  const [range, setRange] = useState<UsageHistoryRange>("7d");
  const [scope, setScope] = useState<HistoryScope>("account");
  const [history, setHistory] = useState<UsageHistory | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const requestSequence = useRef(0);
  const initialLoad = useRef(true);
  const text = language === "ptBr" ? historyPtBr : historyEn;
  const locale = language === "ptBr" ? "pt-BR" : "en-US";

  const load = useCallback(async (refreshAccount = false) => {
    const request = ++requestSequence.current;
    setLoading(true);
    setError(false);
    setHistory(null);
    try {
      if (refreshAccount) await getUsageSnapshots().catch(() => undefined);
      const next = await getUsageHistory(range);
      if (request === requestSequence.current) setHistory(next);
    } catch {
      if (request === requestSequence.current) setError(true);
    } finally {
      if (request === requestSequence.current) setLoading(false);
    }
  }, [range]);

  useEffect(() => {
    const refreshAccount = initialLoad.current;
    initialLoad.current = false;
    void load(refreshAccount);
    return () => { requestSequence.current += 1; };
  }, [load]);

  const selectScopeFromKeyboard = (event: React.KeyboardEvent<HTMLButtonElement>, current: HistoryScope) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const next = current === "account" ? "local" : "account";
    setScope(next);
    const buttons = event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>("[role='tab']");
    buttons?.[next === "account" ? 0 : 1]?.focus();
  };

  return <section className="history-panel" aria-labelledby="history-title">
    <header className="settings-header history-header">
      <button className="icon-button" type="button" aria-label={text.back} title={text.back} onClick={onBack}><BackIcon /></button>
      <div className="history-heading-copy"><h1 id="history-title">{text.title}</h1><p>{text.subtitle}</p></div>
      <button className="icon-button history-refresh" type="button" aria-label={text.refresh} title={text.refresh} onClick={() => void load(true)} disabled={loading}><RefreshIcon /></button>
    </header>

    <div className="history-scroll">
      <section className="history-controls" aria-label={text.filters}>
        <div className="history-range" role="group" aria-label={text.range}>
          {(["7d", "30d", "all"] as const).map((value) => <button key={value} type="button" className={range === value ? "selected" : ""} aria-pressed={range === value} onClick={() => setRange(value)}>{text.ranges[value]}</button>)}
        </div>
        <div className="history-tabs" role="tablist" aria-label={text.scope}>
          <button id="history-account-tab" type="button" role="tab" aria-selected={scope === "account"} aria-controls="history-account-panel" tabIndex={scope === "account" ? 0 : -1} className={scope === "account" ? "selected" : ""} onClick={() => setScope("account")} onKeyDown={(event) => selectScopeFromKeyboard(event, "account")}>{text.account}</button>
          <button id="history-local-tab" type="button" role="tab" aria-selected={scope === "local"} aria-controls="history-local-panel" tabIndex={scope === "local" ? 0 : -1} className={scope === "local" ? "selected" : ""} onClick={() => setScope("local")} onKeyDown={(event) => selectScopeFromKeyboard(event, "local")}>{text.thisPc}</button>
        </div>
      </section>

      <div className="history-content" aria-live="polite" aria-busy={loading}>
        {loading ? <HistoryLoading text={text} /> : error || !history ? <HistoryError text={text} onRetry={() => void load(true)} /> : scope === "account" ? (
          <div id="history-account-panel" role="tabpanel" aria-labelledby="history-account-tab"><AccountHistory account={history.account} profile={history.profile} locale={locale} text={text} /></div>
        ) : (
          <div id="history-local-panel" role="tabpanel" aria-labelledby="history-local-tab"><LocalHistory local={history.local} locale={locale} text={text} /></div>
        )}
      </div>
    </div>
  </section>;
}

function AccountHistory({ account, profile, locale, text }: { account: AccountUsageHistory | null; profile: UsageProfile; locale: string; text: HistoryCopy }) {
  const scopeTitle = profile.planType ? `Codex ${formatPlanType(profile.planType)}` : text.accountScope;
  return <div className="history-stack">
    <section className="history-profile-card">
      <div className="history-eyebrow">{text.allClients}</div>
      <div className="history-profile-title"><strong title={profile.scopeLabel}>{scopeTitle}</strong><div className="profile-badges">{profile.authMode ? <span>{profile.authMode}</span> : null}</div></div>
      <p>{text.sharedScopeDescription}</p>
      <HermesStatus profile={profile} text={text} />
    </section>

    {!account ? <HistoryEmpty title={text.accountUnavailable} description={text.accountUnavailableDescription} /> : <>
      <section className="account-summary" aria-label={text.accountSummary}>
        <div className="history-stat primary"><span>{text.lifetimeTokens}</span><strong>{formatTokenCount(account.summary.lifetimeTokens, locale)}</strong></div>
        <div className="history-stat"><span>{text.currentStreak}</span><strong>{formatDays(account.summary.currentStreakDays, locale, text)}</strong></div>
      </section>
      <section className="history-detail-card" aria-label={text.accountDetails}>
        <HistoryDetail label={text.peakDay} value={formatTokenCount(account.summary.peakDailyTokens, locale)} />
        <HistoryDetail label={text.longestStreak} value={formatDays(account.summary.longestStreakDays, locale, text)} />
        <HistoryDetail label={text.longestTurn} value={formatDuration(account.summary.longestRunningTurnSeconds, locale)} />
      </section>
      <DailyChart data={account.daily.map((item) => ({ date: item.startDate, tokens: item.tokens }))} title={text.accountActivity} emptyLabel={text.noAccountActivity} peakLabel={text.peak} locale={locale} />
      <p className="history-source-note">{text.accountHistoryNote}</p>
    </>}
  </div>;
}

function formatPlanType(value: string) {
  return value
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function HermesStatus({ profile, text }: { profile: UsageProfile; text: HistoryCopy }) {
  const label = profile.hermesLabel || "Hermes";
  const copy = profile.hermesStatus === "active" ? text.hermesActive : profile.hermesStatus === "configured" ? text.hermesConfigured : text.hermesNotDetected;
  return <div className={`hermes-status ${profile.hermesStatus}`}><span className="hermes-dot" aria-hidden="true" /><span>{copy.replace("{label}", label)}</span></div>;
}

function LocalHistory({ local, locale, text }: { local: LocalUsageHistory; locale: string; text: HistoryCopy }) {
  const hasData = local.totals.totalTokens > 0 || local.byModel.length > 0 || local.daily.length > 0;
  return <div className="history-stack">
    <CoverageStatus coverage={local.coverage} text={text} />
    {!hasData ? <HistoryEmpty title={local.coverage === "indexing" ? text.indexingTitle : text.noLocalHistory} description={local.coverage === "indexing" ? text.indexingDescription : text.noLocalHistoryDescription} /> : <>
      <section className="local-summary" aria-label={text.localSummary}>
        <div className="history-stat primary"><span>{text.periodTokens}</span><strong>{formatTokenCount(local.totals.totalTokens, locale)}</strong></div>
        <div className="history-stat api-stat"><span>{text.apiEquivalent}</span><strong>{formatUsdEquivalent(local.apiEquivalent.amountUsd, locale)}</strong><small>{text.pricedCoverage.replace("{percent}", formatPercent(local.apiEquivalent.pricedTokenPercent))}</small></div>
      </section>
      <section className="token-breakdown" aria-label={text.tokenBreakdown}>
        <TokenDetail label={text.input} value={local.totals.inputTokens} locale={locale} />
        <TokenDetail label={text.cached} value={local.totals.cachedInputTokens} locale={locale} />
        <TokenDetail label={text.output} value={local.totals.outputTokens} locale={locale} />
        <TokenDetail label={text.reasoning} value={local.totals.reasoningOutputTokens} locale={locale} />
      </section>
      <DailyChart data={local.daily} title={text.localActivity} emptyLabel={text.noLocalActivity} peakLabel={text.peak} locale={locale} />
      <section className="model-section" aria-labelledby="model-section-title">
        <div className="section-heading"><div><h2 id="model-section-title">{text.models}</h2><p>{text.modelsSubtitle}</p></div><span>{local.byModel.length}</span></div>
        {local.byModel.length === 0 ? <p className="inline-empty">{text.noModels}</p> : <div className="model-list">{local.byModel.map((model) => <ModelRow key={model.modelId} model={model} locale={locale} text={text} />)}</div>}
      </section>
    </>}
    <p className="api-disclaimer"><InfoIcon />{text.apiDisclaimer}</p>
    <p className="history-source-note">{text.localHistoryNote}</p>
  </div>;
}

function CoverageStatus({ coverage, text }: { coverage: LocalUsageHistory["coverage"]; text: HistoryCopy }) {
  if (coverage === "completeForSource") return null;
  const labels = { partial: text.coveragePartial, indexing: text.coverageIndexing, unavailable: text.coverageUnavailable };
  return <div className={`coverage-status ${coverage}`} role="status"><span aria-hidden="true" />{labels[coverage]}</div>;
}

function TokenDetail({ label, value, locale }: { label: string; value: number; locale: string }) {
  return <div><span>{label}</span><strong>{formatTokenCount(value, locale)}</strong></div>;
}

function HistoryDetail({ label, value }: { label: string; value: string }) {
  return <div><span>{label}</span><strong>{value}</strong></div>;
}

function formatDays(value: number | null, locale: string, text: HistoryCopy): string {
  return value === null ? "—" : text.days.replace("{count}", formatTokenCount(value, locale));
}

function ModelRow({ model, locale, text }: { model: LocalUsageHistory["byModel"][number]; locale: string; text: HistoryCopy }) {
  return <article className="model-row">
    <div className="model-heading"><strong title={model.modelId}>{model.modelId}</strong><span>{formatTokenCount(model.tokens, locale)} · {formatPercent(model.tokenSharePercent)}</span></div>
    <div className="model-share" role="progressbar" aria-label={`${model.modelId}: ${formatPercent(model.tokenSharePercent)}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.max(0, Math.min(100, model.tokenSharePercent))}><span style={{ width: formatPercent(model.tokenSharePercent) }} /></div>
    <div className="model-meta"><span>{text.cachedPercent.replace("{percent}", formatPercent(model.cachedInputPercent))}</span><strong>{text.apiShort} {formatUsdEquivalent(model.apiEquivalentUsd, locale)}</strong></div>
  </article>;
}

function DailyChart({ data, title, emptyLabel, peakLabel, locale }: { data: Array<{ date: string; tokens: number }>; title: string; emptyLabel: string; peakLabel: string; locale: string }) {
  const maximum = useMemo(() => Math.max(0, ...data.map((item) => item.tokens)), [data]);
  if (data.length === 0 || maximum === 0) return <section className="daily-chart empty-chart"><div className="section-heading"><h2>{title}</h2></div><p>{emptyLabel}</p></section>;
  return <section className="daily-chart" aria-label={title}>
    <div className="section-heading"><h2>{title}</h2><strong>{formatTokenCount(maximum, locale)} <small>{peakLabel}</small></strong></div>
    <div className="chart-scroll">
      <div className="chart-bars">{data.map((item, index) => <div className="chart-column" key={`${item.date}-${index}`} title={`${formatShortDate(item.date, locale)} · ${formatTokenCount(item.tokens, locale)}`} aria-label={`${formatShortDate(item.date, locale)}: ${formatTokenCount(item.tokens, locale)}`}><span className="chart-bar" style={{ height: `${getBarHeight(item.tokens, maximum)}%` }} /><small>{index === 0 || index === data.length - 1 ? formatShortDate(item.date, locale) : ""}</small></div>)}</div>
    </div>
  </section>;
}

function HistoryLoading({ text }: { text: HistoryCopy }) {
  return <div className="history-loading" role="status"><span className="history-loading-line wide" /><span className="history-loading-line" /><span className="history-loading-card" /><span>{text.loading}</span></div>;
}

function HistoryError({ text, onRetry }: { text: HistoryCopy; onRetry: () => void }) {
  return <div className="history-state"><div className="history-state-mark error" aria-hidden="true">!</div><h2>{text.errorTitle}</h2><p>{text.errorDescription}</p><button type="button" className="secondary-button" onClick={onRetry}>{text.tryAgain}</button></div>;
}

function HistoryEmpty({ title, description }: { title: string; description: string }) {
  return <div className="history-state compact"><div className="history-state-mark" aria-hidden="true"><ChartIcon /></div><h2>{title}</h2><p>{description}</p></div>;
}

const Icon = ({ children }: { children: React.ReactNode }) => <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">{children}</svg>;
const BackIcon = () => <Icon><path d="m15 18-6-6 6-6" /></Icon>;
const RefreshIcon = () => <Icon><path d="M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6" /></Icon>;
const ChartIcon = () => <Icon><path d="M4 19V9m6 10V5m6 14v-7m4 7H2" /></Icon>;
const InfoIcon = () => <Icon><circle cx="12" cy="12" r="9" /><path d="M12 11v5m0-8h.01" /></Icon>;

const historyEn = {
  title: "History", subtitle: "Usage profile & model activity", back: "Back to usage", refresh: "Refresh history", filters: "History filters", range: "Time range", scope: "Usage scope", ranges: { "7d": "7 days", "30d": "30 days", all: "All" }, account: "Account", thisPc: "This PC",
  loading: "Reading local history…", errorTitle: "History unavailable", errorDescription: "QuotaBuddy could not read this history right now.", tryAgain: "Try again", allClients: "Account · all clients", accountScope: "Codex account", sharedScopeDescription: "Shared quota across Codex app, CLI, IDE, Web, and other clients using this account/workspace.",
  hermesActive: "{label} can be using this same quota. Its share cannot be separated by client.", hermesConfigured: "{label} is configured. If it uses this account/workspace, it shares the quota; its share cannot be attributed.", hermesNotDetected: "{label} was not detected for this profile.",
  accountUnavailable: "Account history unavailable", accountUnavailableDescription: "Local model history may still be available under This PC.", accountSummary: "Account usage summary", lifetimeTokens: "Lifetime tokens", currentStreak: "Current streak", days: "{count} days", accountDetails: "Account usage details", peakDay: "Peak day", longestStreak: "Longest streak", longestTurn: "Longest turn", accountActivity: "Account activity", noAccountActivity: "No daily account activity was returned.", accountHistoryNote: "Account totals are aggregated by Codex. They are not attributable to a specific app, device, or client.",
  localSummary: "Local usage summary", periodTokens: "Tokens in period", apiEquivalent: "API equivalent", pricedCoverage: "{percent} priced", tokenBreakdown: "Token breakdown", input: "Input", cached: "Cached input", output: "Output", reasoning: "Reasoning", localActivity: "Local activity", noLocalActivity: "No local activity for this range.", models: "Models", modelsSubtitle: "Local token share", noModels: "No model breakdown is available.", noLocalHistory: "No local history yet", noLocalHistoryDescription: "Use Codex on this PC, then refresh to build a private local history.", indexingTitle: "Building local history", indexingDescription: "QuotaBuddy is indexing the local Codex sessions. This may take a moment on first use.",
  coveragePartial: "Partial local coverage — some sessions or prices could not be resolved.", coverageIndexing: "Indexing local Codex history…", coverageUnavailable: "Local history source is unavailable.", cachedPercent: "{percent} cached input", apiShort: "API eq.", apiDisclaimer: "Estimated API equivalent — not a subscription charge.", localHistoryNote: "This model breakdown only includes Codex sessions found on this PC. Direct Hermes use and other devices are not included.", peak: "peak",
};

const historyPtBr: HistoryCopy = {
  title: "Histórico", subtitle: "Perfil de uso e atividade por modelo", back: "Voltar ao uso", refresh: "Atualizar histórico", filters: "Filtros do histórico", range: "Período", scope: "Escopo de uso", ranges: { "7d": "7 dias", "30d": "30 dias", all: "Tudo" }, account: "Conta", thisPc: "Neste PC",
  loading: "Lendo histórico local…", errorTitle: "Histórico indisponível", errorDescription: "O QuotaBuddy não conseguiu ler este histórico agora.", tryAgain: "Tentar novamente", allClients: "Conta · todos os clientes", accountScope: "Conta Codex", sharedScopeDescription: "Cota compartilhada por Codex app, CLI, IDE, Web e outros clientes nesta conta/workspace.",
  hermesActive: "{label} pode estar usando esta mesma cota. A parte dele não pode ser separada por cliente.", hermesConfigured: "{label} está configurado. Se usar esta conta/workspace, compartilha a cota; a parte dele não é atribuível.", hermesNotDetected: "{label} não foi detectado neste perfil.",
  accountUnavailable: "Histórico da conta indisponível", accountUnavailableDescription: "O histórico local por modelo ainda pode estar disponível em Neste PC.", accountSummary: "Resumo de uso da conta", lifetimeTokens: "Tokens acumulados", currentStreak: "Sequência atual", days: "{count} dias", accountDetails: "Detalhes de uso da conta", peakDay: "Pico diário", longestStreak: "Maior sequência", longestTurn: "Turno mais longo", accountActivity: "Atividade da conta", noAccountActivity: "Nenhuma atividade diária da conta foi retornada.", accountHistoryNote: "Os totais da conta são agregados pelo Codex. Não podem ser atribuídos a um app, dispositivo ou cliente específico.",
  localSummary: "Resumo do uso local", periodTokens: "Tokens no período", apiEquivalent: "Equivalente API", pricedCoverage: "{percent} precificados", tokenBreakdown: "Detalhes dos tokens", input: "Entrada", cached: "Entrada em cache", output: "Saída", reasoning: "Raciocínio", localActivity: "Atividade local", noLocalActivity: "Sem atividade local neste período.", models: "Modelos", modelsSubtitle: "Participação local de tokens", noModels: "A divisão por modelo não está disponível.", noLocalHistory: "Ainda não há histórico local", noLocalHistoryDescription: "Use o Codex neste PC e atualize para criar um histórico local e privado.", indexingTitle: "Criando o histórico local", indexingDescription: "O QuotaBuddy está indexando as sessões locais do Codex. No primeiro uso, isso pode levar um momento.",
  coveragePartial: "Cobertura local parcial — algumas sessões ou preços não foram resolvidos.", coverageIndexing: "Indexando o histórico local do Codex…", coverageUnavailable: "A fonte do histórico local está indisponível.", cachedPercent: "{percent} de entrada em cache", apiShort: "Eq. API", apiDisclaimer: "Equivalente API estimado — não é cobrança da assinatura.", localHistoryNote: "Esta divisão por modelo inclui apenas sessões Codex encontradas neste PC. Uso direto do Hermes e outros dispositivos não aparece aqui.", peak: "pico",
};
