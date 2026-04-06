use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWriteExt,
    sync::Mutex,
};

use crate::outbound_audit::{OutboundAuditEvent, OutboundAuditLogger};
use crate::{error::AppError, pricing::CostEstimate};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageRecord {
    pub timestamp: String,
    pub request_id: String,
    pub caller_id: Option<String>,
    pub model: String,
    pub provider: Option<String>,
    pub attempts: usize,
    pub api_kind: String,
    pub stream: bool,
    pub status: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub estimated_cost: Option<CostEstimate>,
}

impl UsageRecord {
    pub fn success(
        request_id: impl Into<String>,
        model: impl Into<String>,
        provider: impl Into<String>,
        attempts: usize,
        api_kind: impl Into<String>,
        stream: bool,
        caller_id: Option<String>,
    ) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.into(),
            caller_id,
            model: model.into(),
            provider: Some(provider.into()),
            attempts,
            api_kind: api_kind.into(),
            stream,
            status: "success".into(),
            input_tokens: None,
            output_tokens: None,
            estimated_cost: None,
        }
    }

    pub fn failure(
        request_id: impl Into<String>,
        model: impl Into<String>,
        attempts: usize,
        api_kind: impl Into<String>,
        stream: bool,
        status: impl Into<String>,
        caller_id: Option<String>,
    ) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.into(),
            caller_id,
            model: model.into(),
            provider: None,
            attempts,
            api_kind: api_kind.into(),
            stream,
            status: status.into(),
            input_tokens: None,
            output_tokens: None,
            estimated_cost: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct UsageLogger {
    file: Option<Arc<Mutex<File>>>,
    path: Option<String>,
    outbound_audit_logger: OutboundAuditLogger,
}

impl UsageLogger {
    pub async fn new(
        path: Option<PathBuf>,
        outbound_audit_logger: OutboundAuditLogger,
    ) -> Result<Self, AppError> {
        let path_string = path.as_ref().map(|path| path.display().to_string());
        let file = match path {
            Some(path) => {
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .await
                    .map_err(|error| {
                        AppError::upstream(format!("failed to open usage log: {error}"))
                    })?;
                Some(Arc::new(Mutex::new(file)))
            }
            None => None,
        };

        Ok(Self {
            file,
            path: path_string,
            outbound_audit_logger,
        })
    }

    pub async fn append(&self, record: UsageRecord) -> Result<(), AppError> {
        let Some(file) = &self.file else {
            return Ok(());
        };

        let mut file = file.lock().await;
        let mut line = serde_json::to_vec(&record).map_err(|error| {
            AppError::upstream(format!("failed to serialize usage record: {error}"))
        })?;
        line.push(b'\n');
        let bytes_out = line.len() as u64;
        let started_at = std::time::Instant::now();
        file.write_all(&line).await.map_err(|error| {
            AppError::upstream(format!("failed to write usage record: {error}"))
        })?;
        file.flush().await.map_err(|error| {
            AppError::upstream(format!("failed to flush usage record: {error}"))
        })?;
        if let Some(path) = &self.path {
            self.outbound_audit_logger
                .append_warn(
                    OutboundAuditEvent::file_event(
                        "usage_log_write",
                        path.clone(),
                        "write",
                        "success",
                    )
                    .with_latency_ms(started_at.elapsed().as_millis())
                    .with_bytes_out(bytes_out),
                    "usage_log_write",
                )
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn appends_usage_record_as_jsonl() {
        let path = tempfile::NamedTempFile::new().unwrap();
        let logger = UsageLogger::new(
            Some(path.path().to_path_buf()),
            OutboundAuditLogger::default(),
        )
        .await
        .unwrap();

        logger
            .append(UsageRecord::success(
                "req_123",
                "gpt-4.1",
                "openai",
                1,
                "chat_completions",
                false,
                None,
            ))
            .await
            .unwrap();

        let body = std::fs::read_to_string(path.path()).unwrap();
        assert!(body.contains("\"request_id\":\"req_123\""));
    }
}
