use crate::{
    config::ModelCapabilities,
    error::AppError,
    models::{ModelRegistry, ModelTarget},
    providers::ProviderKind,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    ChatCompletions,
    Responses,
    Streaming,
}

#[derive(Clone, Debug)]
pub struct ModelRoute {
    pub provider: ProviderKind,
    pub public_name: String,
    pub upstream_name: String,
    pub capabilities: ModelCapabilities,
}

#[derive(Clone, Debug)]
pub struct RoutePlan {
    pub public_name: String,
    pub targets: Vec<ModelRoute>,
}

pub fn resolve_route_plan(
    registry: &ModelRegistry,
    public_name: &str,
    capability: Capability,
) -> Result<RoutePlan, AppError> {
    let record = registry
        .get(public_name)
        .ok_or_else(|| AppError::model_not_found(public_name))?;

    let mut targets = record
        .targets
        .iter()
        .filter(|target| supports_capability(target, capability))
        .map(|target| ModelRoute {
            provider: target.provider,
            public_name: record.public_name.clone(),
            upstream_name: target.upstream_name.clone(),
            capabilities: target.capabilities.clone(),
        })
        .collect::<Vec<_>>();

    targets.sort_by(|left, right| {
        right
            .capabilities
            .streaming
            .cmp(&left.capabilities.streaming)
    });
    targets.sort_by(|left, right| {
        let left_priority = record
            .targets
            .iter()
            .find(|target| {
                target.provider == left.provider && target.upstream_name == left.upstream_name
            })
            .map(|target| target.priority)
            .unwrap_or_default();
        let right_priority = record
            .targets
            .iter()
            .find(|target| {
                target.provider == right.provider && target.upstream_name == right.upstream_name
            })
            .map(|target| target.priority)
            .unwrap_or_default();
        right_priority.cmp(&left_priority)
    });

    if targets.is_empty() {
        return Err(AppError::validation(format!(
            "model `{public_name}` does not support requested capability"
        )));
    }

    Ok(RoutePlan {
        public_name: record.public_name.clone(),
        targets,
    })
}

fn supports_capability(target: &ModelTarget, capability: Capability) -> bool {
    match capability {
        Capability::ChatCompletions => target.capabilities.chat_completions,
        Capability::Responses => target.capabilities.responses,
        Capability::Streaming => target.capabilities.streaming,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::ModelCapabilities,
        models::{ModelRecord, ModelTarget},
    };

    #[test]
    fn builds_route_plan_in_priority_order() {
        let registry = sample_registry_with_fallback();
        let plan =
            resolve_route_plan(&registry, "claude-sonnet-4", Capability::ChatCompletions).unwrap();

        assert_eq!(plan.targets.len(), 2);
        assert_eq!(plan.targets[0].provider, ProviderKind::Anthropic);
    }

    #[test]
    fn filters_out_targets_without_streaming_capability() {
        let registry = sample_registry_with_fallback();
        let plan = resolve_route_plan(&registry, "claude-sonnet-4", Capability::Streaming).unwrap();

        assert!(
            plan.targets
                .iter()
                .all(|target| target.capabilities.streaming)
        );
        assert_eq!(plan.targets.len(), 1);
    }

    fn sample_registry_with_fallback() -> ModelRegistry {
        ModelRegistry::new(vec![ModelRecord {
            public_name: "claude-sonnet-4".into(),
            capabilities: ModelCapabilities::all(),
            pricing: None,
            targets: vec![
                ModelTarget {
                    provider: ProviderKind::Anthropic,
                    upstream_name: "claude-sonnet-4-20250514".into(),
                    priority: 100,
                    capabilities: ModelCapabilities::all(),
                },
                ModelTarget {
                    provider: ProviderKind::OpenAi,
                    upstream_name: "gpt-4.1".into(),
                    priority: 50,
                    capabilities: ModelCapabilities {
                        chat_completions: true,
                        responses: true,
                        streaming: false,
                    },
                },
            ],
        }])
    }
}
