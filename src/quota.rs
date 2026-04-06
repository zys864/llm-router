use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use crate::{auth::AuthenticatedCaller, config::ProxyKeyConfig, error::AppError};

#[derive(Clone)]
pub struct QuotaStore {
    counters: Arc<Mutex<HashMap<String, u64>>>,
    limits: HashMap<String, u64>,
}

impl QuotaStore {
    pub fn new(configs: Vec<ProxyKeyConfig>) -> Self {
        Self {
            counters: Arc::new(Mutex::new(HashMap::new())),
            limits: configs
                .into_iter()
                .map(|config| (config.id, config.max_requests))
                .collect(),
        }
    }

    pub async fn try_acquire(&self, caller_id: &str) -> Result<(), AppError> {
        let Some(limit) = self.limits.get(caller_id).copied() else {
            return Ok(());
        };

        let mut counters = self.counters.lock().await;
        let counter = counters.entry(caller_id.to_string()).or_insert(0);
        if *counter >= limit {
            return Err(AppError::rate_limit("proxy request quota exceeded"));
        }
        *counter += 1;
        Ok(())
    }

    pub async fn try_acquire_optional(
        &self,
        caller: Option<&AuthenticatedCaller>,
    ) -> Result<(), AppError> {
        if let Some(caller) = caller {
            self.try_acquire(&caller.id).await
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_requests_after_quota_is_exhausted() {
        let quota = QuotaStore::new(vec![ProxyKeyConfig {
            id: "team-alpha".into(),
            api_key: "lr_live_alpha".into(),
            max_requests: 1,
        }]);
        assert!(quota.try_acquire("team-alpha").await.is_ok());
        assert!(quota.try_acquire("team-alpha").await.is_err());
    }
}
