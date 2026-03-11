use aquaregia::{AgentStep, ToolCall, ToolResult, Usage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    AgentConfigPayload, ChatMessage, ChatTurn, ChatSession, ProviderAccount, ProviderProfile,
    ToolApproval,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFileInfo {
    pub path: String,
    pub size: u64,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BootstrapPayload {
    pub provider_profiles: Vec<ProviderProfile>,
    pub provider_accounts: Vec<ProviderAccount>,
    pub sessions: Vec<ChatSession>,
    pub messages: Vec<ChatMessage>,
    pub approvals: Vec<ToolApproval>,
    pub turns: Vec<ChatTurn>,
    pub last_opened_session_id: Option<String>,
    #[serde(default)]
    pub agent_config: AgentConfigPayload,
    #[serde(default)]
    pub workspace_files: Vec<WorkspaceFileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFileReadRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFileReadPayload {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFileWriteRequest {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFileWritePayload {
    pub path: String,
    pub written: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFilesPayload {
    pub files: Vec<WorkspaceFileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchRequest {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchHit {
    pub path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchPayload {
    pub query: String,
    pub hits: Vec<MemorySearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryGetRequest {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryGetPayload {
    pub path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub total_lines: u32,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReindexPayload {
    pub indexed_chunks: u32,
    pub files_indexed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersChangedPayload {
    pub provider_profiles: Vec<ProviderProfile>,
    pub provider_accounts: Vec<ProviderAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsChangedPayload {
    pub sessions: Vec<ChatSession>,
    pub last_opened_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionReadyPayload {
    pub server_time: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStartedPayload {
    pub session_id: String,
    pub turn: ChatTurn,
    pub user_message: ChatMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPayload {
    pub session_id: String,
    pub turn_id: String,
    pub step: u8,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStartedPayload {
    pub session_id: String,
    pub turn_id: String,
    pub step: u8,
    pub block_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTokenPayload {
    pub session_id: String,
    pub turn_id: String,
    pub step: u8,
    pub block_id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningFinishedPayload {
    pub session_id: String,
    pub turn_id: String,
    pub step: u8,
    pub block_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepStartedPayload {
    pub session_id: String,
    pub turn_id: String,
    pub step: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepFinishedPayload {
    pub session_id: String,
    pub turn_id: String,
    pub step: AgentStep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequestedPayload {
    pub session_id: String,
    pub turn_id: String,
    pub step: u8,
    pub state: String,
    pub tool_call: ToolCall,
    pub approval: Option<ToolApproval>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFinishedPayload {
    pub session_id: String,
    pub turn_id: String,
    pub step: u8,
    pub tool_call: ToolCall,
    pub tool_result: ToolResult,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnFinishedPayload {
    pub session_id: String,
    pub turn: ChatTurn,
    pub new_messages: Vec<ChatMessage>,
    pub usage_total: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnFailedPayload {
    pub session_id: String,
    pub turn_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnCancelledPayload {
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStepsListRequest {
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStepsListPayload {
    pub turn_id: String,
    pub steps: Vec<AgentStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemoryCompactedPayload {
    pub session_id: String,
    pub compacted_messages: u32,
    pub summary_preview: String,
}
