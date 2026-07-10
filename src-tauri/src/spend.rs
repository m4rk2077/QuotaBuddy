use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PricingTable {
    pub version: String,
    pub models: Vec<ModelPrice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPrice {
    pub model: String,
    pub input_per_million_usd: f64,
    pub output_per_million_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodexUsageRecord {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendEstimate {
    pub amount_usd: f64,
    pub pricing_table_version: String,
    pub record_count: usize,
    pub is_estimate: bool,
    pub label: String,
}

pub fn bundled_pricing_table() -> PricingTable {
    serde_json::from_str(include_str!("../fixtures/pricing_table_2026-07-10.json"))
        .expect("bundled pricing table is valid")
}

pub fn estimate_spend(records: &[CodexUsageRecord], prices: &PricingTable) -> SpendEstimate {
    let (amount_usd, record_count) = records.iter().fold((0.0, 0), |acc, record| {
        let Some(price) = prices
            .models
            .iter()
            .find(|price| price.model == record.model)
        else {
            return acc;
        };
        (
            acc.0
                + (record.input_tokens as f64 / 1_000_000.0) * price.input_per_million_usd
                + (record.output_tokens as f64 / 1_000_000.0) * price.output_per_million_usd,
            acc.1 + 1,
        )
    });

    SpendEstimate {
        amount_usd: (amount_usd * 100_000.0).round() / 100_000.0,
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
    record: Option<CodexUsageRecord>,
}

#[derive(Default)]
pub struct SpendScanner {
    cache: HashMap<PathBuf, CachedSession>,
    #[cfg(test)]
    parsed_file_count: usize,
}

impl SpendScanner {
    pub fn read_usage_records(&mut self, directory: &Path) -> io::Result<Vec<CodexUsageRecord>> {
        if !directory.exists() {
            self.cache.clear();
            return Ok(Vec::new());
        }
        let mut next_cache = HashMap::new();
        let mut records = Vec::new();
        self.scan_directory(directory, &mut next_cache, &mut records)?;
        self.cache = next_cache;
        Ok(records)
    }

    fn scan_directory(
        &mut self,
        directory: &Path,
        next_cache: &mut HashMap<PathBuf, CachedSession>,
        records: &mut Vec<CodexUsageRecord>,
    ) -> io::Result<()> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                self.scan_directory(&entry.path(), next_cache, records)?;
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
                continue;
            }
            let metadata = entry.metadata()?;
            let fingerprint = FileFingerprint {
                length: metadata.len(),
                modified: metadata.modified().ok(),
            };
            let cached = self
                .cache
                .get(&path)
                .filter(|cached| cached.fingerprint == fingerprint)
                .cloned();
            let session = match cached {
                Some(cached) => cached,
                None => {
                    let record = parse_session_usage_reader(BufReader::new(File::open(&path)?))?;
                    #[cfg(test)]
                    {
                        self.parsed_file_count += 1;
                    }
                    CachedSession {
                        fingerprint,
                        record,
                    }
                }
            };
            if let Some(record) = &session.record {
                records.push(record.clone());
            }
            next_cache.insert(path, session);
        }
        Ok(())
    }
}

fn parse_session_usage_reader(reader: impl BufRead) -> io::Result<Option<CodexUsageRecord>> {
    let mut model = None;
    let (mut input_tokens, mut output_tokens) = (0, 0);
    for line in reader.lines() {
        let line = line?;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if model.is_none() {
            model = find_model(&value);
        }
        collect_token_totals(&value, &mut input_tokens, &mut output_tokens);
    }
    Ok(match (model, input_tokens, output_tokens) {
        (Some(model), input_tokens, output_tokens) if input_tokens > 0 || output_tokens > 0 => {
            Some(CodexUsageRecord {
                model,
                input_tokens,
                output_tokens,
            })
        }
        _ => None,
    })
}

fn find_model(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(object) => object
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or_else(|| object.values().find_map(find_model)),
        serde_json::Value::Array(values) => values.iter().find_map(find_model),
        _ => None,
    }
}

fn collect_token_totals(value: &serde_json::Value, input: &mut u64, output: &mut u64) {
    match value {
        serde_json::Value::Object(object) => {
            *input = (*input).max(
                object
                    .get("input_tokens")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            );
            *output = (*output).max(
                object
                    .get("output_tokens")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            );
            for child in object.values() {
                collect_token_totals(child, input, output);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                collect_token_totals(child, input, output);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Cursor,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn calculates_explicitly_labeled_estimate_from_anonymized_local_fixture() {
        let records: Vec<CodexUsageRecord> =
            serde_json::from_str(include_str!("../fixtures/codex_usage.json"))
                .expect("anonymized fixture is valid");

        let estimate = estimate_spend(&records, &bundled_pricing_table());

        assert_eq!(estimate.amount_usd, 0.02625);
        assert_eq!(estimate.pricing_table_version, "2026-07-10");
        assert!(estimate.is_estimate);
        assert!(estimate.label.contains("not billing"));
    }

    #[test]
    fn reads_usage_nested_in_anonymized_jsonl_log_records() {
        let record = parse_session_usage_reader(Cursor::new("{\"type\":\"session_meta\",\"payload\":{\"model\":\"gpt-5-codex\"}}\n{\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":3000,\"output_tokens\":1000}}}}\n{\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":5000,\"output_tokens\":2000}}}}"))
            .unwrap()
            .unwrap();

        assert_eq!(record.model, "gpt-5-codex");
        assert_eq!(record.input_tokens, 5000);
    }

    #[test]
    fn streaming_parser_matches_session_record_semantics() {
        let content = "{\"type\":\"session_meta\",\"payload\":{\"model\":\"gpt-5-codex\"}}\n{malformed}\n{\"payload\":{\"input_tokens\":3000,\"output_tokens\":1000}}\n{\"payload\":{\"input_tokens\":5000,\"output_tokens\":2000}}\n";

        let record = parse_session_usage_reader(Cursor::new(content)).expect("stream parses");

        assert_eq!(
            record,
            Some(CodexUsageRecord {
                model: "gpt-5-codex".to_owned(),
                input_tokens: 5000,
                output_tokens: 2000,
            })
        );
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
            "{\"model\":\"gpt-5-codex\"}\n{\"input_tokens\":100,\"output_tokens\":20}\n",
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
            "{\"model\":\"gpt-5-codex\"}\n{\"input_tokens\":250,\"output_tokens\":40}\n",
        )
        .unwrap();
        let changed = scanner.read_usage_records(&directory).unwrap();

        assert_eq!(scanner.parsed_file_count, parsed_after_first + 1);
        assert_eq!(changed[0].input_tokens, 250);

        fs::remove_file(&session).unwrap();
        assert!(scanner.read_usage_records(&directory).unwrap().is_empty());
        assert!(scanner.cache.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }
}
