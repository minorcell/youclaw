use aquaregia::{AgentStep, ToolCall, ToolResult, Usage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::domain::{ChatMessage, ChatTurn, ToolApproval};

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
pub struct AgentMemoryCompactedPayload {
    pub session_id: String,
    pub compacted_messages: u32,
    pub summary_preview: String,
}
