use std::{collections::HashMap, path::Path};

use tokio::fs;

use crate::{error::AppError, usage::UsageRecord};

#[derive(Clone, Debug, Default)]
pub struct UsageSummary {
    pub total_requests: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub by_caller: HashMap<String, usize>,
    pub by_model: HashMap<String, usize>,
    pub by_provider: HashMap<String, usize>,
    pub estimated_cost_total: f64,
}

pub struct UsageAggregator {
    records: Vec<UsageRecord>,
}

impl UsageAggregator {
    pub async fn from_path(path: &Path) -> Result<Self, AppError> {
        let body = fs::read_to_string(path)
            .await
            .map_err(|error| AppError::upstream(format!("failed to read usage log: {error}")))?;
        let mut records = Vec::new();
        for line in body.lines().filter(|line| !line.trim().is_empty()) {
            if let Ok(record) = serde_json::from_str::<UsageRecord>(line) {
                records.push(record);
            }
        }
        Ok(Self { records })
    }

    pub fn summarize(&self) -> Result<UsageSummary, AppError> {
        let mut summary = UsageSummary::default();
        for record in &self.records {
            summary.total_requests += 1;
            if record.status == "success" {
                summary.success_count += 1;
            } else {
                summary.failure_count += 1;
            }
            if let Some(caller) = &record.caller_id {
                *summary.by_caller.entry(caller.clone()).or_insert(0) += 1;
            }
            *summary.by_model.entry(record.model.clone()).or_insert(0) += 1;
            if let Some(provider) = &record.provider {
                *summary.by_provider.entry(provider.clone()).or_insert(0) += 1;
            }
            if let Some(estimate) = &record.estimated_cost {
                summary.estimated_cost_total += estimate.amount;
            }
        }
        Ok(summary)
    }

    pub fn recover_success_counts(&self) -> HashMap<String, u64> {
        let mut counts = HashMap::new();
        for record in &self.records {
            if record.status == "success" {
                if let Some(caller) = &record.caller_id {
                    *counts.entry(caller.clone()).or_insert(0) += 1;
                }
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn aggregates_usage_by_caller_and_model() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            "{\"timestamp\":\"2026-04-06T00:00:00Z\",\"request_id\":\"req_1\",\"caller_id\":\"team-alpha\",\"model\":\"gpt-4.1\",\"provider\":\"openai\",\"attempts\":1,\"api_kind\":\"chat_completions\",\"stream\":false,\"status\":\"success\",\"input_tokens\":null,\"output_tokens\":null,\"estimated_cost\":null}\n{\"timestamp\":\"2026-04-06T00:00:01Z\",\"request_id\":\"req_2\",\"caller_id\":\"team-alpha\",\"model\":\"gpt-4.1\",\"provider\":\"openai\",\"attempts\":1,\"api_kind\":\"chat_completions\",\"stream\":false,\"status\":\"success\",\"input_tokens\":null,\"output_tokens\":null,\"estimated_cost\":null}\n",
        )
        .unwrap();
        let summary = UsageAggregator::from_path(file.path())
            .await
            .unwrap()
            .summarize()
            .unwrap();

        assert_eq!(summary.total_requests, 2);
        assert_eq!(summary.by_caller["team-alpha"], 2);
    }
}
