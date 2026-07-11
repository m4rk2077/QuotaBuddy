export function formatTokenCount(value: number | null, locale: string): string {
  if (value === null) return "—";
  const safeValue = finiteNonNegative(value);
  if (safeValue < 1_000) {
    return new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(safeValue);
  }

  return new Intl.NumberFormat(locale, {
    notation: "compact",
    compactDisplay: "short",
    maximumFractionDigits: 1,
  }).format(safeValue);
}

export function formatPercent(value: number): string {
  const safeValue = Math.min(100, finiteNonNegative(value));
  const formatted = safeValue.toFixed(2).replace(/\.?0+$/, "");
  return `${formatted}%`;
}

export function formatUsdEquivalent(value: number | null, locale: string): string {
  if (value === null || !Number.isFinite(value) || value < 0) return "—";
  const digits = value > 0 && value < 0.01 ? 4 : 2;
  return new Intl.NumberFormat(locale, {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

export function formatDuration(seconds: number | null, locale: string): string {
  if (seconds === null) return "—";
  const safeSeconds = Math.round(finiteNonNegative(seconds));
  if (safeSeconds < 60) return `${safeSeconds}s`;
  const totalMinutes = Math.round(safeSeconds / 60);
  if (totalMinutes < 60) return `${totalMinutes}min`;
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  const hourLabel = new Intl.NumberFormat(locale).format(hours);
  return minutes === 0 ? `${hourLabel}h` : `${hourLabel}h ${minutes}min`;
}

export function formatShortDate(value: string, locale: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(locale, { day: "2-digit", month: "short", timeZone: "UTC" }).format(date);
}

export function getBarHeight(tokens: number, maximum: number): number {
  const safeTokens = finiteNonNegative(tokens);
  const safeMaximum = finiteNonNegative(maximum);
  if (safeTokens === 0 || safeMaximum === 0) return 0;
  return Math.max(6, Math.min(100, (safeTokens / safeMaximum) * 100));
}

function finiteNonNegative(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}
