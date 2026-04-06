use std::pin::Pin;

use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

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
    TextDelta(String),
    Usage(UnifiedUsage),
    Completed,
    Error(String),
}

pub type EventResult = Result<StreamEvent, AppError>;
pub type EventStream = Pin<Box<dyn Stream<Item = EventResult> + Send>>;
