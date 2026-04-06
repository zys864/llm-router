use crate::{
    access_log::AccessLogger, auth::AuthService, config::AppConfig, models::ModelRegistry,
    providers::ProviderFactory, quota::QuotaStore, usage::UsageLogger,
};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub registry: ModelRegistry,
    pub provider_factory: ProviderFactory,
    pub auth: AuthService,
    pub quota: QuotaStore,
    pub usage_logger: UsageLogger,
    pub access_logger: AccessLogger,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        registry: ModelRegistry,
        provider_factory: ProviderFactory,
        auth: AuthService,
        quota: QuotaStore,
        usage_logger: UsageLogger,
        access_logger: AccessLogger,
    ) -> Self {
        Self {
            config,
            registry,
            provider_factory,
            auth,
            quota,
            usage_logger,
            access_logger,
        }
    }
}
