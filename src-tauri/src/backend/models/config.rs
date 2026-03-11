use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActiveHoursConfig {
    pub start: String,
    pub end: String,
}

impl Default for AgentActiveHoursConfig {
    fn default() -> Self {
        Self {
            start: "08:00".to_string(),
            end: "22:00".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeartbeatConfig {
    pub enabled: bool,
    pub every: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_hours: Option<AgentActiveHoursConfig>,
}

impl Default for AgentHeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            every: "30m".to_string(),
            target: "main".to_string(),
            active_hours: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigPayload {
    pub max_steps: u8,
    pub max_input_tokens: u32,
    pub compact_ratio: f32,
    pub keep_recent: u32,
    pub language: String,
    pub heartbeat: AgentHeartbeatConfig,
}

impl Default for AgentConfigPayload {
    fn default() -> Self {
        Self {
            max_steps: 8,
            max_input_tokens: 32768,
            compact_ratio: 0.7,
            keep_recent: 8,
            language: "zh".to_string(),
            heartbeat: AgentHeartbeatConfig::default(),
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
    pub keep_recent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat: Option<AgentHeartbeatConfig>,
}
