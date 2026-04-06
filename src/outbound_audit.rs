use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    sync::Mutex,
};

use crate::error::AppError;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutboundAuditEvent {
    pub timestamp: String,
    pub request_id: Option<String>,
    pub operation: String,
    pub target_kind: String,
    pub target_name: String,
    pub action: String,
    pub result: String,
    pub latency_ms: Option<u128>,
    pub status_code: Option<u16>,
    pub bytes_in: Option<u64>,
    pub bytes_out: Option<u64>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl OutboundAuditEvent {
    pub fn file_event(
        operation: impl Into<String>,
        target_name: impl Into<String>,
        action: impl Into<String>,
        result: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: None,
            operation: operation.into(),
            target_kind: "file".into(),
            target_name: target_name.into(),
            action: action.into(),
            result: result.into(),
            latency_ms: None,
            status_code: None,
            bytes_in: None,
            bytes_out: None,
            error_kind: None,
            error_message: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        self.request_id = request_id;
        self
    }

    pub fn with_latency_ms(mut self, latency_ms: u128) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = Some(status_code);
        self
    }

    pub fn with_bytes_in(mut self, bytes_in: u64) -> Self {
        self.bytes_in = Some(bytes_in);
        self
    }

    pub fn with_bytes_out(mut self, bytes_out: u64) -> Self {
        self.bytes_out = Some(bytes_out);
        self
    }

    pub fn with_error(mut self, kind: impl Into<String>, message: impl Into<String>) -> Self {
        self.error_kind = Some(kind.into());
        self.error_message = Some(message.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Clone, Default)]
pub struct OutboundAuditLogger {
    file: Option<Arc<Mutex<File>>>,
}

impl OutboundAuditLogger {
    pub async fn new(path: Option<PathBuf>) -> Result<Self, AppError> {
        Self::new_blocking(path)
    }

    pub fn new_blocking(path: Option<PathBuf>) -> Result<Self, AppError> {
        let file = match path {
            Some(path) => {
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|error| {
                        AppError::upstream(format!("failed to open outbound audit log: {error}"))
                    })?;
                Some(Arc::new(Mutex::new(file)))
            }
            None => None,
        };

        Ok(Self { file })
    }

    pub async fn append(&self, event: OutboundAuditEvent) -> Result<(), AppError> {
        self.append_blocking(event)
    }

    pub fn append_blocking(&self, event: OutboundAuditEvent) -> Result<(), AppError> {
        let Some(file) = &self.file else {
            return Ok(());
        };

        let mut file = file.lock().unwrap();
        let mut line = serde_json::to_vec(&event).map_err(|error| {
            AppError::upstream(format!("failed to serialize outbound audit log: {error}"))
        })?;
        line.push(b'\n');
        file.write_all(&line).map_err(|error| {
            AppError::upstream(format!("failed to write outbound audit log: {error}"))
        })?;
        file.flush().map_err(|error| {
            AppError::upstream(format!("failed to flush outbound audit log: {error}"))
        })?;
        Ok(())
    }

    pub async fn append_warn(&self, event: OutboundAuditEvent, context: &str) {
        if let Err(error) = self.append_blocking(event) {
            tracing::warn!(context = context, error = %error, "failed to write outbound audit log");
        }
    }

    pub fn append_warn_blocking(&self, event: OutboundAuditEvent, context: &str) {
        if let Err(error) = self.append_blocking(event) {
            tracing::warn!(context = context, error = %error, "failed to write outbound audit log");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn appends_outbound_audit_event_as_jsonl() {
        let path = tempfile::NamedTempFile::new().unwrap();
        let logger = OutboundAuditLogger::new(Some(path.path().to_path_buf()))
            .await
            .unwrap();

        logger
            .append(
                OutboundAuditEvent::file_event(
                    "config_load",
                    "/tmp/config.json",
                    "read",
                    "success",
                )
                .with_bytes_in(42),
            )
            .await
            .unwrap();

        let body = std::fs::read_to_string(path.path()).unwrap();
        assert!(body.contains("\"target_kind\":\"file\""));
        assert!(body.contains("\"bytes_in\":42"));
    }
}
