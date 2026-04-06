use std::{path::PathBuf, sync::Arc};

use serde::Serialize;
use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWriteExt,
    sync::Mutex,
};

use crate::error::AppError;

#[derive(Clone, Debug, Serialize)]
pub struct UsageRecord {
    pub request_id: String,
    pub caller_id: Option<String>,
    pub model: String,
    pub provider: Option<String>,
    pub attempts: usize,
    pub api_kind: String,
    pub stream: bool,
    pub status: String,
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
            request_id: request_id.into(),
            caller_id,
            model: model.into(),
            provider: Some(provider.into()),
            attempts,
            api_kind: api_kind.into(),
            stream,
            status: "success".into(),
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
            request_id: request_id.into(),
            caller_id,
            model: model.into(),
            provider: None,
            attempts,
            api_kind: api_kind.into(),
            stream,
            status: status.into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct UsageLogger {
    file: Option<Arc<Mutex<File>>>,
}

impl UsageLogger {
    pub async fn new(path: Option<PathBuf>) -> Result<Self, AppError> {
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

        Ok(Self { file })
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
        file.write_all(&line).await.map_err(|error| {
            AppError::upstream(format!("failed to write usage record: {error}"))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn appends_usage_record_as_jsonl() {
        let path = tempfile::NamedTempFile::new().unwrap();
        let logger = UsageLogger::new(Some(path.path().to_path_buf()))
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
