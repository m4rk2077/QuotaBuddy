use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

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
    let priced_records: Vec<_> = records
        .iter()
        .filter_map(|record| {
            prices
                .models
                .iter()
                .find(|price| price.model == record.model)
                .map(|price| (record, price))
        })
        .collect();
    let amount_usd = priced_records
        .iter()
        .map(|(record, price)| {
            (record.input_tokens as f64 / 1_000_000.0) * price.input_per_million_usd
                + (record.output_tokens as f64 / 1_000_000.0) * price.output_per_million_usd
        })
        .sum::<f64>();

    SpendEstimate {
        amount_usd: (amount_usd * 100_000.0).round() / 100_000.0,
        pricing_table_version: prices.version.clone(),
        record_count: priced_records.len(),
        is_estimate: true,
        label: "Estimated local Codex spend (not billing)".to_owned(),
    }
}

pub fn read_usage_records(directory: &Path) -> io::Result<Vec<CodexUsageRecord>> {
    let mut records = Vec::new();
    if !directory.exists() {
        return Ok(records);
    }

    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            records.extend(read_usage_records(&entry.path())?);
            continue;
        }
        if entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("jsonl")
        {
            continue;
        }
        let content = fs::read_to_string(entry.path())?;
        records.extend(parse_session_usage(&content));
    }
    Ok(records)
}

fn parse_session_usage(content: &str) -> Vec<CodexUsageRecord> {
    let values: Vec<_> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect();
    let model = values.iter().find_map(find_model);
    let (mut input_tokens, mut output_tokens) = (0, 0);
    for value in &values {
        collect_token_totals(value, &mut input_tokens, &mut output_tokens);
    }
    match (model, input_tokens, output_tokens) {
        (Some(model), input_tokens, output_tokens) if input_tokens > 0 || output_tokens > 0 => {
            vec![CodexUsageRecord {
                model,
                input_tokens,
                output_tokens,
            }]
        }
        _ => Vec::new(),
    }
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

#[allow(dead_code)]
fn parse_usage_records(line: &str) -> Vec<CodexUsageRecord> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    collect_usage_records(&value, &mut records);
    records
}

#[allow(dead_code)]
fn collect_usage_records(value: &serde_json::Value, records: &mut Vec<CodexUsageRecord>) {
    match value {
        serde_json::Value::Object(object) => {
            let model = object.get("model").and_then(serde_json::Value::as_str);
            let input_tokens = object
                .get("input_tokens")
                .and_then(serde_json::Value::as_u64);
            let output_tokens = object
                .get("output_tokens")
                .and_then(serde_json::Value::as_u64);
            if let (Some(model), Some(input_tokens), Some(output_tokens)) =
                (model, input_tokens, output_tokens)
            {
                records.push(CodexUsageRecord {
                    model: model.to_owned(),
                    input_tokens,
                    output_tokens,
                });
            }
            for child in object.values() {
                collect_usage_records(child, records);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                collect_usage_records(child, records);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let records = parse_session_usage("{\"type\":\"session_meta\",\"payload\":{\"model\":\"gpt-5-codex\"}}\n{\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":3000,\"output_tokens\":1000}}}}\n{\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":5000,\"output_tokens\":2000}}}}");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].model, "gpt-5-codex");
        assert_eq!(records[0].input_tokens, 5000);
    }
}
