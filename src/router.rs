use crate::{error::AppError, models::ModelRegistry, providers::ProviderKind};

#[derive(Clone, Debug)]
pub struct ModelRoute {
    pub provider: ProviderKind,
    pub public_name: String,
    pub upstream_name: String,
}

pub fn resolve_model(registry: &ModelRegistry, public_name: &str) -> Result<ModelRoute, AppError> {
    let record = registry
        .get(public_name)
        .ok_or_else(|| AppError::model_not_found(public_name))?;

    Ok(ModelRoute {
        provider: record.provider,
        public_name: record.public_name.clone(),
        upstream_name: record.upstream_name.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModelRecord;

    #[test]
    fn resolves_known_model() {
        let registry = ModelRegistry::new(vec![ModelRecord::new(
            "gpt-4.1",
            ProviderKind::OpenAi,
            "gpt-4.1",
        )]);

        let target = resolve_model(&registry, "gpt-4.1").unwrap();
        assert_eq!(target.provider, ProviderKind::OpenAi);
    }

    #[test]
    fn rejects_unknown_model() {
        let registry = ModelRegistry::new(vec![]);
        assert!(resolve_model(&registry, "missing").is_err());
    }
}
