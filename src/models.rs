use serde::Serialize;

use crate::{
    config::{ModelCapabilities, ModelConfig, ModelPricing},
    providers::ProviderKind,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModelTarget {
    pub provider: ProviderKind,
    pub upstream_name: String,
    pub priority: u32,
    pub capabilities: ModelCapabilities,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModelRecord {
    pub public_name: String,
    pub capabilities: ModelCapabilities,
    pub pricing: Option<ModelPricing>,
    pub targets: Vec<ModelTarget>,
}

impl ModelRecord {
    pub fn new(
        public_name: impl Into<String>,
        provider: ProviderKind,
        upstream_name: impl Into<String>,
    ) -> Self {
        Self {
            public_name: public_name.into(),
            capabilities: ModelCapabilities::all(),
            pricing: None,
            targets: vec![ModelTarget {
                provider,
                upstream_name: upstream_name.into(),
                priority: 100,
                capabilities: ModelCapabilities::all(),
            }],
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
                .map(|config| ModelRecord {
                    public_name: config.public_name.clone(),
                    capabilities: config.capabilities.clone(),
                    pricing: config.pricing.clone(),
                    targets: config
                        .targets
                        .iter()
                        .map(|target| ModelTarget {
                            provider: target.provider,
                            upstream_name: target.upstream_name.clone(),
                            priority: target.priority,
                            capabilities: target.capabilities.clone(),
                        })
                        .collect(),
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
