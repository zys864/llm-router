use crate::{config::AppConfig, models::ModelRegistry, providers::ProviderFactory};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub registry: ModelRegistry,
    pub provider_factory: ProviderFactory,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        registry: ModelRegistry,
        provider_factory: ProviderFactory,
    ) -> Self {
        Self {
            config,
            registry,
            provider_factory,
        }
    }
}
