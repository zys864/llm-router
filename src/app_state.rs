use crate::{
    auth::AuthService, config::AppConfig, models::ModelRegistry, providers::ProviderFactory,
    quota::QuotaStore, usage::UsageLogger,
};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub registry: ModelRegistry,
    pub provider_factory: ProviderFactory,
    pub auth: AuthService,
    pub quota: QuotaStore,
    pub usage_logger: UsageLogger,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        registry: ModelRegistry,
        provider_factory: ProviderFactory,
        auth: AuthService,
        quota: QuotaStore,
        usage_logger: UsageLogger,
    ) -> Self {
        Self {
            config,
            registry,
            provider_factory,
            auth,
            quota,
            usage_logger,
        }
    }
}
