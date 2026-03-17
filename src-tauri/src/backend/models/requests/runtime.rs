use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFileReadRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFileWriteRequest {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchRequest {
    pub query: String,
    #[serde(
        default,
        rename = "maxResults",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_results: Option<u32>,
    #[serde(default, rename = "minScore", skip_serializing_if = "Option::is_none")]
    pub min_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryGetRequest {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStepsListRequest {
    pub turn_id: String,
}
