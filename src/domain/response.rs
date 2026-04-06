use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UnifiedUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnifiedResponse {
    pub text: String,
    pub finish_reason: Option<String>,
    pub usage: Option<UnifiedUsage>,
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Debug)]
pub enum StreamEvent {
    Started,
    DeltaText(String),
    Usage(UnifiedUsage),
    Completed,
    Error(String),
}
