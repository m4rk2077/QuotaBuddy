use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PricingTable {
    pub version: String,
    pub checked_at: String,
    pub source_url: String,
    pub models: Vec<ModelPrice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPrice {
    pub model: String,
    pub input_per_million_usd: f64,
    pub cached_input_per_million_usd: f64,
    pub output_per_million_usd: f64,
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)] // Backward-compatible fixture contract; the UI uses LocalUsageHistory.
pub struct CodexUsageRecord {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendEstimate {
    pub amount_usd: Option<f64>,
    pub pricing_coverage_percent: f64,
    pub pricing_table_version: String,
    pub record_count: usize,
    pub is_estimate: bool,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageHistoryRange {
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "all")]
    All,
}

impl UsageHistoryRange {
    pub fn includes_day(self, start_date: &str, now_seconds: u64) -> bool {
        let Some(day_start) = parse_rfc3339_seconds(&format!("{start_date}T00:00:00Z")) else {
            return false;
        };
        if day_start > now_seconds {
            return false;
        }
        range_cutoff_seconds(self, now_seconds)
            .is_none_or(|cutoff| day_start.saturating_add(86_399) >= cutoff)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBreakdown {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
}

impl TokenBreakdown {
    fn add_assign(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.reasoning_output_tokens = self
            .reasoning_output_tokens
            .saturating_add(other.reasoning_output_tokens);
        self.total_tokens = self.total_tokens.saturating_add(other.total_tokens);
    }

    fn monotonic_delta(&self, previous: &Self) -> Option<Self> {
        if self.input_tokens < previous.input_tokens
            || self.cached_input_tokens < previous.cached_input_tokens
            || self.output_tokens < previous.output_tokens
            || self.reasoning_output_tokens < previous.reasoning_output_tokens
            || self.total_tokens < previous.total_tokens
        {
            return None;
        }
        Some(Self {
            input_tokens: self.input_tokens - previous.input_tokens,
            cached_input_tokens: self.cached_input_tokens - previous.cached_input_tokens,
            output_tokens: self.output_tokens - previous.output_tokens,
            reasoning_output_tokens: self.reasoning_output_tokens
                - previous.reasoning_output_tokens,
            total_tokens: self.total_tokens - previous.total_tokens,
        })
    }

    fn is_empty(&self) -> bool {
        self.input_tokens == 0
            && self.cached_input_tokens == 0
            && self.output_tokens == 0
            && self.reasoning_output_tokens == 0
            && self.total_tokens == 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageSummary {
    pub model_id: String,
    pub tokens: u64,
    pub breakdown: TokenBreakdown,
    pub token_share_percent: f64,
    pub cached_input_percent: f64,
    pub api_equivalent_usd: Option<f64>,
    pub is_priced: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsagePoint {
    pub date: String,
    pub tokens: u64,
    pub breakdown: TokenBreakdown,
    pub api_equivalent_usd: Option<f64>,
    pub pricing_coverage_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEquivalentEstimate {
    pub amount_usd: Option<f64>,
    pub priced_token_percent: f64,
    pub priced_tokens: u64,
    pub unpriced_tokens: u64,
    pub unpriced_models: Vec<String>,
    pub pricing_table_version: String,
    pub is_estimate: bool,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalHistoryCoverage {
    CompleteForSource,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalUsageHistory {
    pub range: UsageHistoryRange,
    pub generated_at_unix_seconds: u64,
    pub from_unix_seconds: Option<u64>,
    pub to_unix_seconds: u64,
    pub totals: TokenBreakdown,
    pub by_model: Vec<ModelUsageSummary>,
    pub daily: Vec<DailyUsagePoint>,
    pub api_equivalent: ApiEquivalentEstimate,
    pub coverage: LocalHistoryCoverage,
    pub record_count: usize,
}

#[derive(Debug, Clone)]
struct LocalUsageEvent {
    model: String,
    timestamp: Option<String>,
    tokens: TokenBreakdown,
}

#[derive(Debug, Deserialize)]
struct LocalLogLine {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    payload: Option<LocalLogPayload>,
}

#[derive(Debug, Deserialize)]
struct LocalLogPayload {
    model: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    info: Option<LocalTokenInfo>,
}

#[derive(Debug, Deserialize)]
struct LocalTokenInfo {
    last_token_usage: Option<RawTokenUsage>,
    total_token_usage: Option<RawTokenUsage>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawTokenUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    reasoning_output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

impl From<RawTokenUsage> for TokenBreakdown {
    fn from(raw: RawTokenUsage) -> Self {
        Self {
            input_tokens: raw.input_tokens,
            cached_input_tokens: raw.cached_input_tokens,
            output_tokens: raw.output_tokens,
            reasoning_output_tokens: raw.reasoning_output_tokens,
            total_tokens: if raw.total_tokens == 0 {
                raw.input_tokens.saturating_add(raw.output_tokens)
            } else {
                raw.total_tokens
            },
        }
    }
}

pub fn bundled_pricing_table() -> PricingTable {
    serde_json::from_str(include_str!("../fixtures/pricing_table_2026-07-11.json"))
        .expect("bundled pricing table is valid")
}

#[allow(dead_code)] // Retained for existing diagnostics/tests while history uses cached-token pricing.
pub fn estimate_spend(records: &[CodexUsageRecord], prices: &PricingTable) -> SpendEstimate {
    let mut amount_usd = 0.0;
    let mut record_count = 0;
    let mut total_tokens = 0_u64;
    let mut priced_tokens = 0_u64;
    for record in records {
        let tokens = record.input_tokens.saturating_add(record.output_tokens);
        total_tokens = total_tokens.saturating_add(tokens);
        let Some(price) = prices
            .models
            .iter()
            .find(|price| price.model == record.model)
        else {
            continue;
        };
        amount_usd += (record.input_tokens as f64 / 1_000_000.0) * price.input_per_million_usd
            + (record.output_tokens as f64 / 1_000_000.0) * price.output_per_million_usd;
        priced_tokens = priced_tokens.saturating_add(tokens);
        record_count += 1;
    }

    SpendEstimate {
        amount_usd: (priced_tokens > 0).then(|| rounded_usd(amount_usd)),
        pricing_coverage_percent: if total_tokens == 0 {
            0.0
        } else {
            ((priced_tokens as f64 / total_tokens as f64) * 10_000.0).round() / 100.0
        },
        pricing_table_version: prices.version.clone(),
        record_count,
        is_estimate: true,
        label: "Estimated local Codex spend (not billing)".to_owned(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    length: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
struct CachedSession {
    fingerprint: FileFingerprint,
    events: Vec<LocalUsageEvent>,
    partial: bool,
}

struct ParsedSession {
    events: Vec<LocalUsageEvent>,
    partial: bool,
}

#[derive(Default)]
pub struct SpendScanner {
    cache: HashMap<PathBuf, CachedSession>,
    last_scan_partial: bool,
    #[cfg(test)]
    parsed_file_count: usize,
}

impl SpendScanner {
    #[allow(dead_code)] // Compatibility shim over the strict event parser.
    pub fn read_usage_records(&mut self, directory: &Path) -> io::Result<Vec<CodexUsageRecord>> {
        let events = self.read_events(directory, None)?;
        let mut by_model: HashMap<String, CodexUsageRecord> = HashMap::new();
        for event in events {
            let record = by_model
                .entry(event.model.clone())
                .or_insert_with(|| CodexUsageRecord {
                    model: event.model,
                    input_tokens: 0,
                    output_tokens: 0,
                });
            record.input_tokens = record
                .input_tokens
                .saturating_add(event.tokens.input_tokens);
            record.output_tokens = record
                .output_tokens
                .saturating_add(event.tokens.output_tokens);
        }
        let mut records: Vec<_> = by_model.into_values().collect();
        records.sort_by(|left, right| left.model.cmp(&right.model));
        Ok(records)
    }

    pub fn read_local_usage_history(
        &mut self,
        directory: &Path,
        range: UsageHistoryRange,
        now: SystemTime,
    ) -> io::Result<LocalUsageHistory> {
        if !directory.exists() {
            self.cache.clear();
            let mut history =
                aggregate_local_usage_history(&[], range, now, &bundled_pricing_table());
            history.coverage = LocalHistoryCoverage::Unavailable;
            return Ok(history);
        }
        let now_seconds = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let modified_cutoff = range_cutoff_seconds(range, now_seconds)
            .map(|seconds| UNIX_EPOCH + Duration::from_secs(seconds));
        let events = self.read_events(directory, modified_cutoff)?;
        let mut history =
            aggregate_local_usage_history(&events, range, now, &bundled_pricing_table());
        if self.last_scan_partial {
            history.coverage = LocalHistoryCoverage::Partial;
        }
        Ok(history)
    }

    fn read_events(
        &mut self,
        directory: &Path,
        modified_cutoff: Option<SystemTime>,
    ) -> io::Result<Vec<LocalUsageEvent>> {
        if !directory.exists() {
            self.cache.clear();
            self.last_scan_partial = false;
            return Ok(Vec::new());
        }
        self.last_scan_partial = false;
        let mut seen_paths = HashSet::new();
        let mut events = Vec::new();
        self.scan_directory(directory, modified_cutoff, &mut seen_paths, &mut events)?;
        self.cache.retain(|path, _| seen_paths.contains(path));
        Ok(events)
    }

    fn scan_directory(
        &mut self,
        directory: &Path,
        modified_cutoff: Option<SystemTime>,
        seen_paths: &mut HashSet<PathBuf>,
        events: &mut Vec<LocalUsageEvent>,
    ) -> io::Result<()> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                self.scan_directory(&entry.path(), modified_cutoff, seen_paths, events)?;
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
                continue;
            }
            seen_paths.insert(path.clone());
            let metadata = entry.metadata()?;
            let modified = metadata.modified().ok();
            if modified_cutoff
                .is_some_and(|cutoff| modified.is_some_and(|modified| modified < cutoff))
            {
                continue;
            }
            let fingerprint = FileFingerprint {
                length: metadata.len(),
                modified,
            };
            if let Some(cached) = self
                .cache
                .get(&path)
                .filter(|cached| cached.fingerprint == fingerprint)
            {
                self.last_scan_partial |= cached.partial;
                events.extend(cached.events.iter().cloned());
                continue;
            }

            let previous = self.cache.get(&path).cloned();
            let parsed = File::open(&path)
                .map(BufReader::new)
                .and_then(parse_local_usage_reader);
            let parsed = match parsed {
                Ok(parsed) => parsed,
                Err(_) => {
                    self.last_scan_partial = true;
                    if let Some(previous) = previous {
                        events.extend(previous.events.iter().cloned());
                    }
                    continue;
                }
            };
            #[cfg(test)]
            {
                self.parsed_file_count += 1;
            }
            self.last_scan_partial |= parsed.partial;
            events.extend(parsed.events.iter().cloned());
            self.cache.insert(
                path,
                CachedSession {
                    fingerprint,
                    events: parsed.events,
                    partial: parsed.partial,
                },
            );
        }
        Ok(())
    }
}

fn parse_local_usage_reader(reader: impl BufRead) -> io::Result<ParsedSession> {
    let mut events = Vec::new();
    let mut partial = false;
    let mut current_model = None;
    let mut previous_total: Option<TokenBreakdown> = None;
    for line in reader.lines() {
        let line = line?;
        if !line.contains("\"turn_context\"") && !line.contains("\"token_count\"") {
            continue;
        }
        let Ok(record) = serde_json::from_str::<LocalLogLine>(&line) else {
            partial = true;
            continue;
        };
        let Some(payload) = record.payload else {
            continue;
        };
        match (record.kind.as_deref(), payload.kind.as_deref()) {
            (Some("turn_context"), _) => {
                current_model = payload.model.and_then(normalize_model_id);
                partial |= current_model.is_none();
            }
            (Some("event_msg"), Some("token_count")) => {
                let Some(info) = payload.info else {
                    partial = true;
                    continue;
                };
                let total = info.total_token_usage.map(TokenBreakdown::from);
                let tokens = info.last_token_usage.map(TokenBreakdown::from).or_else(|| {
                    total.as_ref().and_then(|current| match &previous_total {
                        Some(previous) => current.monotonic_delta(previous),
                        None => Some(current.clone()),
                    })
                });
                if let Some(total) = total {
                    previous_total = Some(total);
                }
                let Some(tokens) = tokens else {
                    partial = true;
                    continue;
                };
                let Some(model) = current_model.clone() else {
                    partial = true;
                    continue;
                };
                if tokens.is_empty() {
                    continue;
                }
                partial |= record
                    .timestamp
                    .as_deref()
                    .and_then(parse_rfc3339_seconds)
                    .is_none();
                events.push(LocalUsageEvent {
                    model,
                    timestamp: record.timestamp,
                    tokens,
                });
            }
            _ => {}
        }
    }
    Ok(ParsedSession { events, partial })
}

fn normalize_model_id(model: String) -> Option<String> {
    let model = model.trim();
    (!model.is_empty()
        && model.len() <= 128
        && model.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/' | b':')
        }))
    .then(|| model.to_owned())
}

fn aggregate_local_usage_history(
    events: &[LocalUsageEvent],
    range: UsageHistoryRange,
    now: SystemTime,
    prices: &PricingTable,
) -> LocalUsageHistory {
    let now_seconds = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let cutoff = range_cutoff_seconds(range, now_seconds);
    let mut total = TokenBreakdown::default();
    let mut by_model: HashMap<String, TokenBreakdown> = HashMap::new();
    let mut by_day: BTreeMap<String, HashMap<String, TokenBreakdown>> = BTreeMap::new();
    let selected_events: Vec<_> = events
        .iter()
        .filter(|event| match cutoff {
            None => true,
            Some(cutoff) => event
                .timestamp
                .as_deref()
                .and_then(parse_rfc3339_seconds)
                .is_some_and(|timestamp| timestamp >= cutoff && timestamp <= now_seconds),
        })
        .collect();
    for event in &selected_events {
        total.add_assign(&event.tokens);
        by_model
            .entry(event.model.clone())
            .or_default()
            .add_assign(&event.tokens);
        if let Some(date) = event
            .timestamp
            .as_deref()
            .and_then(|timestamp| timestamp.get(0..10))
        {
            by_day
                .entry(date.to_owned())
                .or_default()
                .entry(event.model.clone())
                .or_default()
                .add_assign(&event.tokens);
        }
    }
    let denominator = total.total_tokens as f64;
    let mut models: Vec<_> = by_model
        .into_iter()
        .map(|(model, tokens)| {
            let api_equivalent_usd = api_equivalent_usd_for_tokens(&model, &tokens, prices);
            ModelUsageSummary {
                model_id: model,
                tokens: tokens.total_tokens,
                token_share_percent: if denominator == 0.0 {
                    0.0
                } else {
                    ((tokens.total_tokens as f64 / denominator) * 10_000.0).round() / 100.0
                },
                cached_input_percent: if tokens.input_tokens == 0 {
                    0.0
                } else {
                    ((tokens.cached_input_tokens as f64 / tokens.input_tokens as f64) * 10_000.0)
                        .round()
                        / 100.0
                },
                api_equivalent_usd,
                is_priced: api_equivalent_usd.is_some(),
                breakdown: tokens,
            }
        })
        .collect();
    models.sort_by(|left, right| {
        right
            .tokens
            .cmp(&left.tokens)
            .then_with(|| left.model_id.cmp(&right.model_id))
    });
    let api_equivalent = summarize_api_equivalent(&models, prices);
    let daily = by_day
        .into_iter()
        .map(|(date, models)| {
            let mut breakdown = TokenBreakdown::default();
            let mut amount = 0.0;
            let mut priced_tokens = 0_u64;
            for (model, tokens) in models {
                breakdown.add_assign(&tokens);
                if let Some(model_amount) = api_equivalent_usd_for_tokens(&model, &tokens, prices) {
                    amount += model_amount;
                    priced_tokens = priced_tokens.saturating_add(tokens.total_tokens);
                }
            }
            let total_tokens = breakdown.total_tokens;
            DailyUsagePoint {
                date,
                tokens: total_tokens,
                breakdown,
                api_equivalent_usd: (priced_tokens > 0).then(|| rounded_usd(amount)),
                pricing_coverage_percent: if total_tokens == 0 {
                    0.0
                } else {
                    ((priced_tokens as f64 / total_tokens as f64) * 10_000.0).round() / 100.0
                },
            }
        })
        .collect();

    let coverage = if total.total_tokens > 0 && api_equivalent.priced_token_percent < 100.0 {
        LocalHistoryCoverage::Partial
    } else {
        LocalHistoryCoverage::CompleteForSource
    };
    LocalUsageHistory {
        range,
        generated_at_unix_seconds: now_seconds,
        from_unix_seconds: cutoff.or_else(|| {
            selected_events
                .iter()
                .filter_map(|event| event.timestamp.as_deref().and_then(parse_rfc3339_seconds))
                .min()
        }),
        to_unix_seconds: now_seconds,
        totals: total,
        by_model: models,
        daily,
        api_equivalent,
        coverage,
        record_count: selected_events.len(),
    }
}

fn api_equivalent_usd_for_tokens(
    model: &str,
    tokens: &TokenBreakdown,
    prices: &PricingTable,
) -> Option<f64> {
    let price = prices.models.iter().find(|price| price.model == model)?;
    let uncached_input = tokens
        .input_tokens
        .saturating_sub(tokens.cached_input_tokens);
    Some(rounded_usd(
        (uncached_input as f64 / 1_000_000.0) * price.input_per_million_usd
            + (tokens.cached_input_tokens as f64 / 1_000_000.0)
                * price.cached_input_per_million_usd
            + (tokens.output_tokens as f64 / 1_000_000.0) * price.output_per_million_usd,
    ))
}

fn range_cutoff_seconds(range: UsageHistoryRange, now_seconds: u64) -> Option<u64> {
    match range {
        UsageHistoryRange::SevenDays => Some(now_seconds.saturating_sub(7 * 24 * 60 * 60)),
        UsageHistoryRange::ThirtyDays => Some(now_seconds.saturating_sub(30 * 24 * 60 * 60)),
        UsageHistoryRange::All => None,
    }
}

fn parse_rfc3339_seconds(timestamp: &str) -> Option<u64> {
    let bytes = timestamp.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || !matches!(bytes.get(10), Some(b'T' | b't' | b' '))
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return None;
    }
    let year = parse_decimal(bytes.get(0..4)?)? as i32;
    let month = parse_decimal(bytes.get(5..7)?)?;
    let day = parse_decimal(bytes.get(8..10)?)?;
    let hour = parse_decimal(bytes.get(11..13)?)?;
    let minute = parse_decimal(bytes.get(14..16)?)?;
    let second = parse_decimal(bytes.get(17..19)?)?;
    if !valid_date(year, month, day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let timezone_index = timestamp[19..]
        .find(['Z', 'z', '+', '-'])
        .map(|index| index + 19)?;
    let timezone = bytes[timezone_index];
    let offset_seconds = match timezone {
        b'Z' | b'z' if timezone_index + 1 == bytes.len() => 0_i64,
        b'+' | b'-' if timezone_index + 6 == bytes.len() => {
            if bytes.get(timezone_index + 3) != Some(&b':') {
                return None;
            }
            let offset_hour = parse_decimal(bytes.get(timezone_index + 1..timezone_index + 3)?)?;
            let offset_minute = parse_decimal(bytes.get(timezone_index + 4..timezone_index + 6)?)?;
            if offset_hour > 23 || offset_minute > 59 {
                return None;
            }
            let value = (offset_hour * 60 * 60 + offset_minute * 60) as i64;
            if timezone == b'+' {
                value
            } else {
                -value
            }
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    let seconds = days
        .checked_mul(86_400)?
        .checked_add((hour * 3_600 + minute * 60 + second) as i64)?
        .checked_sub(offset_seconds)?;
    u64::try_from(seconds).ok()
}

fn parse_decimal(bytes: &[u8]) -> Option<u32> {
    bytes.iter().try_fold(0_u32, |value, byte| {
        byte.is_ascii_digit()
            .then(|| value * 10 + u32::from(byte - b'0'))
    })
}

fn valid_date(year: i32, month: u32, day: u32) -> bool {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    day >= 1 && day <= days
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let adjusted_month = month as i32 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146_097 + day_of_era - 719_468)
}

fn summarize_api_equivalent(
    models: &[ModelUsageSummary],
    prices: &PricingTable,
) -> ApiEquivalentEstimate {
    let mut amount_usd = 0.0;
    let mut priced_tokens = 0_u64;
    let mut unpriced_tokens = 0_u64;
    let mut unpriced_models = Vec::new();
    for model in models {
        if let Some(amount) = model.api_equivalent_usd {
            amount_usd += amount;
            priced_tokens = priced_tokens.saturating_add(model.tokens);
        } else {
            unpriced_tokens = unpriced_tokens.saturating_add(model.tokens);
            unpriced_models.push(model.model_id.clone());
        }
    }
    let total = priced_tokens.saturating_add(unpriced_tokens);
    ApiEquivalentEstimate {
        amount_usd: (priced_tokens > 0).then(|| rounded_usd(amount_usd)),
        priced_token_percent: if total == 0 {
            0.0
        } else {
            ((priced_tokens as f64 / total as f64) * 10_000.0).round() / 100.0
        },
        priced_tokens,
        unpriced_tokens,
        unpriced_models,
        pricing_table_version: prices.version.clone(),
        is_estimate: true,
        label: "API-equivalent estimate from local Codex tokens (not billing)".to_owned(),
    }
}

fn rounded_usd(amount: f64) -> f64 {
    (amount * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn calculates_explicitly_labeled_estimate_from_anonymized_local_fixture() {
        let records: Vec<CodexUsageRecord> =
            serde_json::from_str(include_str!("../fixtures/codex_usage.json"))
                .expect("anonymized fixture is valid");

        let estimate = estimate_spend(&records, &bundled_pricing_table());

        assert_eq!(estimate.amount_usd, Some(0.02625));
        assert_eq!(estimate.pricing_coverage_percent, 100.0);
        assert_eq!(estimate.pricing_table_version, "2026-07-11");
        assert!(estimate.is_estimate);
        assert!(estimate.label.contains("not billing"));
    }

    #[test]
    fn history_attributes_last_token_usage_to_the_active_turn_model() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("quotabuddy-history-{unique}"));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("session.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-10T10:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5.6-sol\"}}\n",
                "{\"timestamp\":\"2026-07-10T10:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":1000,\"cached_input_tokens\":400,\"output_tokens\":200,\"reasoning_output_tokens\":50,\"total_tokens\":1200}}}}\n",
                "{\"timestamp\":\"2026-07-10T11:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5.6-terra\"}}\n",
                "{\"timestamp\":\"2026-07-10T11:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":2000,\"cached_input_tokens\":1000,\"output_tokens\":500,\"reasoning_output_tokens\":100,\"total_tokens\":2500}}}}\n",
                "{\"timestamp\":\"2026-07-10T12:00:00Z\",\"type\":\"other\",\"payload\":{\"model\":\"must-not-be-read\",\"input_tokens\":999999}}\n"
            ),
        )
        .unwrap();

        let mut scanner = SpendScanner::default();
        let history = scanner
            .read_local_usage_history(
                &directory,
                UsageHistoryRange::All,
                UNIX_EPOCH + std::time::Duration::from_secs(1_783_800_000),
            )
            .unwrap();

        assert_eq!(history.record_count, 2);
        assert_eq!(history.by_model.len(), 2);
        assert_eq!(history.by_model[0].model_id, "gpt-5.6-terra");
        assert_eq!(history.by_model[0].breakdown.cached_input_tokens, 1000);
        assert_eq!(history.by_model[0].breakdown.reasoning_output_tokens, 100);
        assert_eq!(history.by_model[1].model_id, "gpt-5.6-sol");
        assert_eq!(history.totals.total_tokens, 3700);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn history_falls_back_to_monotonic_cumulative_deltas() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("quotabuddy-delta-{unique}"));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("session.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-10T10:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5.3-codex\"}}\n",
                "{\"timestamp\":\"2026-07-10T10:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":1000,\"cached_input_tokens\":200,\"output_tokens\":100,\"reasoning_output_tokens\":20,\"total_tokens\":1100}}}}\n",
                "{\"timestamp\":\"2026-07-10T10:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":1800,\"cached_input_tokens\":500,\"output_tokens\":250,\"reasoning_output_tokens\":50,\"total_tokens\":2050}}}}\n"
            ),
        )
        .unwrap();

        let history = SpendScanner::default()
            .read_local_usage_history(
                &directory,
                UsageHistoryRange::All,
                UNIX_EPOCH + std::time::Duration::from_secs(1_783_800_000),
            )
            .unwrap();

        assert_eq!(history.record_count, 2);
        assert_eq!(history.totals.input_tokens, 1800);
        assert_eq!(history.totals.cached_input_tokens, 500);
        assert_eq!(history.totals.output_tokens, 250);
        assert_eq!(history.totals.reasoning_output_tokens, 50);
        assert_eq!(history.totals.total_tokens, 2050);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn api_equivalent_prices_cached_input_once_and_reports_unknown_coverage() {
        let events = vec![
            LocalUsageEvent {
                model: "gpt-5.6-sol".to_owned(),
                timestamp: Some("2026-07-10T10:00:00Z".to_owned()),
                tokens: TokenBreakdown {
                    input_tokens: 1000,
                    cached_input_tokens: 400,
                    output_tokens: 200,
                    reasoning_output_tokens: 50,
                    total_tokens: 1200,
                },
            },
            LocalUsageEvent {
                model: "unpriced-local-model".to_owned(),
                timestamp: Some("2026-07-10T11:00:00Z".to_owned()),
                tokens: TokenBreakdown {
                    input_tokens: 1000,
                    output_tokens: 1000,
                    total_tokens: 2000,
                    ..TokenBreakdown::default()
                },
            },
        ];

        let history = aggregate_local_usage_history(
            &events,
            UsageHistoryRange::All,
            UNIX_EPOCH + std::time::Duration::from_secs(1_783_800_000),
            &bundled_pricing_table(),
        );

        let priced = history
            .by_model
            .iter()
            .find(|model| model.model_id == "gpt-5.6-sol")
            .unwrap();
        assert_eq!(priced.api_equivalent_usd, Some(0.0092));
        assert_eq!(history.api_equivalent.amount_usd, Some(0.0092));
        assert_eq!(history.api_equivalent.priced_token_percent, 37.5);
        assert_eq!(history.coverage, LocalHistoryCoverage::Partial);
        assert_eq!(history.api_equivalent.priced_tokens, 1200);
        assert_eq!(history.api_equivalent.unpriced_tokens, 2000);
        assert_eq!(
            history.api_equivalent.unpriced_models,
            vec!["unpriced-local-model"]
        );
        assert_eq!(history.api_equivalent.pricing_table_version, "2026-07-11");
        assert_eq!(history.daily[0].api_equivalent_usd, Some(0.0092));
        assert_eq!(history.daily[0].pricing_coverage_percent, 37.5);
    }

    #[test]
    fn history_ranges_filter_events_and_keep_daily_points_chronological() {
        let event = |timestamp: &str, total_tokens| LocalUsageEvent {
            model: "gpt-5.3-codex".to_owned(),
            timestamp: Some(timestamp.to_owned()),
            tokens: TokenBreakdown {
                input_tokens: total_tokens,
                total_tokens,
                ..TokenBreakdown::default()
            },
        };
        let events = vec![
            event("2026-07-10T12:00:00Z", 100),
            event("2026-07-02T12:00:00Z", 200),
            event("2026-06-01T12:00:00Z", 300),
        ];
        let now = UNIX_EPOCH + std::time::Duration::from_secs(1_783_771_200);

        let seven_days = aggregate_local_usage_history(
            &events,
            UsageHistoryRange::SevenDays,
            now,
            &bundled_pricing_table(),
        );
        let thirty_days = aggregate_local_usage_history(
            &events,
            UsageHistoryRange::ThirtyDays,
            now,
            &bundled_pricing_table(),
        );
        let all = aggregate_local_usage_history(
            &events,
            UsageHistoryRange::All,
            now,
            &bundled_pricing_table(),
        );

        assert_eq!(seven_days.totals.total_tokens, 100);
        assert_eq!(seven_days.daily[0].date, "2026-07-10");
        assert_eq!(thirty_days.totals.total_tokens, 300);
        assert_eq!(
            thirty_days
                .daily
                .iter()
                .map(|point| point.date.as_str())
                .collect::<Vec<_>>(),
            vec!["2026-07-02", "2026-07-10"]
        );
        assert_eq!(all.totals.total_tokens, 600);
        assert_eq!(seven_days.from_unix_seconds, Some(1_783_166_400));
    }

    #[test]
    fn history_scanner_reuses_fingerprints_and_skips_files_older_than_the_range() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("quotabuddy-cache-{unique}"));
        fs::create_dir_all(&directory).unwrap();
        let session = directory.join("session.jsonl");
        let write_session = |input_tokens: u64| {
            fs::write(
                &session,
                concat!(
                    "{\"timestamp\":\"2026-07-10T10:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5.3-codex\"}}\n",
                    "{\"timestamp\":\"2026-07-10T10:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":__INPUT__,\"total_tokens\":__INPUT__}}}}\n"
                )
                .replace("__INPUT__", &input_tokens.to_string()),
            )
            .unwrap();
        };
        write_session(100);
        let mut scanner = SpendScanner::default();
        let now = UNIX_EPOCH + std::time::Duration::from_secs(1_783_771_200);

        let first = scanner
            .read_local_usage_history(&directory, UsageHistoryRange::SevenDays, now)
            .unwrap();
        let parsed_after_first = scanner.parsed_file_count;
        let second = scanner
            .read_local_usage_history(&directory, UsageHistoryRange::SevenDays, now)
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(parsed_after_first, 1);
        assert_eq!(scanner.parsed_file_count, parsed_after_first);

        write_session(2500);
        let changed = scanner
            .read_local_usage_history(&directory, UsageHistoryRange::SevenDays, now)
            .unwrap();
        assert_eq!(changed.totals.input_tokens, 2500);
        assert_eq!(scanner.parsed_file_count, parsed_after_first + 1);

        let far_future = now + std::time::Duration::from_secs(90 * 24 * 60 * 60);
        let skipped = SpendScanner::default()
            .read_local_usage_history(&directory, UsageHistoryRange::SevenDays, far_future)
            .unwrap();
        assert_eq!(skipped.record_count, 0);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn narrowing_a_range_does_not_evict_cached_older_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("quotabuddy-range-cache-{unique}"));
        fs::create_dir_all(&directory).unwrap();
        let session = |date: &str| {
            format!(
                "{{\"timestamp\":\"{date}T10:00:00Z\",\"type\":\"turn_context\",\"payload\":{{\"model\":\"gpt-5.3-codex\"}}}}\n{{\"timestamp\":\"{date}T10:01:00Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{{\"last_token_usage\":{{\"input_tokens\":100,\"total_tokens\":100}}}}}}}}\n"
            )
        };
        let recent = directory.join("recent.jsonl");
        let old = directory.join("old.jsonl");
        fs::write(&recent, session("2026-07-11")).unwrap();
        fs::write(&old, session("2026-05-01")).unwrap();
        let now = UNIX_EPOCH + std::time::Duration::from_secs(1_783_771_200);
        File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(now - std::time::Duration::from_secs(60 * 24 * 60 * 60)),
            )
            .unwrap();
        let mut scanner = SpendScanner::default();

        let all = scanner
            .read_local_usage_history(&directory, UsageHistoryRange::All, now)
            .unwrap();
        let parsed_after_all = scanner.parsed_file_count;
        let seven_days = scanner
            .read_local_usage_history(&directory, UsageHistoryRange::SevenDays, now)
            .unwrap();
        let all_again = scanner
            .read_local_usage_history(&directory, UsageHistoryRange::All, now)
            .unwrap();

        assert_eq!(all.record_count, 2);
        assert_eq!(seven_days.record_count, 1);
        assert_eq!(all_again.record_count, 2);
        assert_eq!(parsed_after_all, 2);
        assert_eq!(scanner.parsed_file_count, parsed_after_all);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn malformed_records_and_untrusted_model_ids_make_coverage_partial() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("quotabuddy-partial-{unique}"));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("session.jsonl"),
            concat!(
                "{\"type\":\"turn_context\",malformed}\n",
                "{\"timestamp\":\"2026-07-11T10:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"model with unsafe text\"}}\n",
                "{\"timestamp\":\"2026-07-11T10:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":100,\"total_tokens\":100}}}}\n"
            ),
        )
        .unwrap();
        let mut scanner = SpendScanner::default();

        let history = scanner
            .read_local_usage_history(
                &directory,
                UsageHistoryRange::SevenDays,
                UNIX_EPOCH + std::time::Duration::from_secs(1_783_771_200),
            )
            .unwrap();

        assert_eq!(history.coverage, LocalHistoryCoverage::Partial);
        assert!(history.by_model.is_empty());
        assert_eq!(history.record_count, 0);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn public_history_contract_uses_frontend_names_and_scalar_series_tokens() {
        let history = aggregate_local_usage_history(
            &[LocalUsageEvent {
                model: "gpt-5.6-terra".to_owned(),
                timestamp: Some("2026-07-10T10:00:00Z".to_owned()),
                tokens: TokenBreakdown {
                    input_tokens: 100,
                    cached_input_tokens: 40,
                    output_tokens: 20,
                    reasoning_output_tokens: 5,
                    total_tokens: 120,
                },
            }],
            UsageHistoryRange::All,
            UNIX_EPOCH + std::time::Duration::from_secs(1_783_800_000),
            &bundled_pricing_table(),
        );

        let json = serde_json::to_value(history).unwrap();
        assert_eq!(json["range"], "all");
        assert_eq!(json["totals"]["cachedInputTokens"], 40);
        assert_eq!(json["byModel"][0]["modelId"], "gpt-5.6-terra");
        assert_eq!(json["byModel"][0]["tokens"], 120);
        assert!(json["byModel"][0]["tokenSharePercent"].is_number());
        assert!(json["byModel"][0]["cachedInputPercent"].is_number());
        assert_eq!(json["daily"][0]["tokens"], 120);
        assert!(json["apiEquivalent"]["amountUsd"].is_number());
        assert!(json["apiEquivalent"]["pricedTokenPercent"].is_number());
        assert_eq!(json["coverage"], "completeForSource");
    }

    #[test]
    fn scanner_reuses_unchanged_files_and_reparses_changed_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("quotabuddy-spend-{unique}"));
        fs::create_dir_all(&directory).unwrap();
        let session = directory.join("session.jsonl");
        fs::write(
            &session,
            concat!(
                "{\"timestamp\":\"2026-07-10T10:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5-codex\"}}\n",
                "{\"timestamp\":\"2026-07-10T10:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":100,\"output_tokens\":20,\"total_tokens\":120}}}}\n"
            ),
        )
        .unwrap();
        let mut scanner = SpendScanner::default();

        let first = scanner.read_usage_records(&directory).unwrap();
        let parsed_after_first = scanner.parsed_file_count;
        let second = scanner.read_usage_records(&directory).unwrap();

        assert_eq!(first, second);
        assert_eq!(scanner.parsed_file_count, parsed_after_first);

        fs::write(
            &session,
            concat!(
                "{\"timestamp\":\"2026-07-10T10:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5-codex\"}}\n",
                "{\"timestamp\":\"2026-07-10T10:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":2500,\"output_tokens\":400,\"total_tokens\":2900}}}}\n"
            ),
        )
        .unwrap();
        let changed = scanner.read_usage_records(&directory).unwrap();

        assert_eq!(scanner.parsed_file_count, parsed_after_first + 1);
        assert_eq!(changed[0].input_tokens, 2500);

        fs::remove_file(&session).unwrap();
        assert!(scanner.read_usage_records(&directory).unwrap().is_empty());
        assert!(scanner.cache.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }
}
