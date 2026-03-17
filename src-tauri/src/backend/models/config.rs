use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigPayload {
    pub max_steps: u8,
    pub max_input_tokens: u32,
    pub compact_ratio: f32,
    pub language: String,
}

impl Default for AgentConfigPayload {
    fn default() -> Self {
        Self {
            max_steps: 64,
            max_input_tokens: 120_000,
            compact_ratio: 0.8,
            language: "zh".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}
