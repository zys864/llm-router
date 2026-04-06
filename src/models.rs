use serde::Serialize;

use crate::{config::ModelConfig, providers::ProviderKind};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModelRecord {
    pub public_name: String,
    pub provider: ProviderKind,
    pub upstream_name: String,
}

impl ModelRecord {
    pub fn new(
        public_name: impl Into<String>,
        provider: ProviderKind,
        upstream_name: impl Into<String>,
    ) -> Self {
        Self {
            public_name: public_name.into(),
            provider,
            upstream_name: upstream_name.into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ModelRegistry {
    records: Vec<ModelRecord>,
}

impl ModelRegistry {
    pub fn new(records: Vec<ModelRecord>) -> Self {
        Self { records }
    }

    pub fn from_configs(configs: &[ModelConfig]) -> Self {
        Self::new(
            configs
                .iter()
                .map(|config| {
                    ModelRecord::new(
                        config.public_name.clone(),
                        config.provider,
                        config.upstream_name.clone(),
                    )
                })
                .collect(),
        )
    }

    pub fn get(&self, public_name: &str) -> Option<&ModelRecord> {
        self.records
            .iter()
            .find(|record| record.public_name == public_name)
    }

    pub fn all(&self) -> &[ModelRecord] {
        &self.records
    }
}
