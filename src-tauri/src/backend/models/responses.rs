use aquaregia::AgentStep;
use serde::{Deserialize, Serialize};

use super::domain::{
    AgentConfigPayload, AgentProfile, ChatMessage, ChatSession, ChatTurn, MemoryRecord,
    ProfileTarget, ProviderAccount, ProviderProfile, ToolApproval,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BootstrapPayload {
    pub provider_profiles: Vec<ProviderProfile>,
    pub provider_accounts: Vec<ProviderAccount>,
    pub sessions: Vec<ChatSession>,
    #[serde(default)]
    pub recent_workspaces: Vec<WorkspaceRootInfo>,
    pub messages: Vec<ChatMessage>,
    pub approvals: Vec<ToolApproval>,
    pub turns: Vec<ChatTurn>,
    pub last_opened_session_id: Option<String>,
    #[serde(default)]
    pub agent_config: AgentConfigPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRootInfo {
    pub path: String,
    pub last_used_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecordSummary {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySystemSearchHit {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySystemListPayload {
    pub entries: Vec<MemoryRecordSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySystemSearchPayload {
    pub results: Vec<MemorySystemSearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySystemGetPayload {
    pub entry: MemoryRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySystemWritePayload {
    pub entry: MemoryRecord,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySystemDeletePayload {
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileGetPayload {
    pub profiles: Vec<AgentProfile>,
    pub needs_onboarding: bool,
    pub missing_targets: Vec<ProfileTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileWritePayload {
    pub profile: AgentProfile,
    pub needs_onboarding: bool,
    pub missing_targets: Vec<ProfileTarget>,
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
    #[serde(default)]
    pub recent_workspaces: Vec<WorkspaceRootInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedSessionsPayload {
    pub sessions: Vec<ChatSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStepsListPayload {
    pub turn_id: String,
    pub steps: Vec<AgentStep>,
}
