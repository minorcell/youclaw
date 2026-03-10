use std::collections::HashMap;

use aquaregia::{AgentStep, ContentPart, Message, MessageRole, ToolCall, ToolResult, Usage};
use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::backend::errors::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub id: String,
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model_name: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModel {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<ProviderModel>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub provider_profile_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub parts_json: Value,
    pub run_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolApproval {
    pub id: String,
    pub session_id: String,
    pub run_id: String,
    pub call_id: String,
    pub action: String,
    pub path: String,
    pub preview_json: Value,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRun {
    pub id: String,
    pub session_id: String,
    pub status: String,
    pub user_message: String,
    pub output_text: String,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BootstrapPayload {
    pub provider_profiles: Vec<ProviderProfile>,
    pub provider_accounts: Vec<ProviderAccount>,
    pub sessions: Vec<ChatSession>,
    pub messages: Vec<ChatMessage>,
    pub approvals: Vec<ToolApproval>,
    pub runs: Vec<ChatRun>,
    pub last_opened_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsKind {
    Request,
    Response,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEnvelope {
    pub id: String,
    pub kind: WsKind,
    pub name: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WsErrorPayload>,
}

impl WsEnvelope {
    pub fn event(name: impl Into<String>, payload: impl Serialize) -> AppResult<Self> {
        let payload = serde_json::to_value(payload)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            kind: WsKind::Event,
            name: name.into(),
            payload,
            ok: None,
            error: None,
        })
    }

    pub fn response_ok(
        id: impl Into<String>,
        name: impl Into<String>,
        payload: impl Serialize,
    ) -> AppResult<Self> {
        let payload = serde_json::to_value(payload)?;
        Ok(Self {
            id: id.into(),
            kind: WsKind::Response,
            name: name.into(),
            payload,
            ok: Some(true),
            error: None,
        })
    }

    pub fn response_error(id: impl Into<String>, name: impl Into<String>, err: &AppError) -> Self {
        Self {
            id: id.into(),
            kind: WsKind::Response,
            name: name.into(),
            payload: Value::Null,
            ok: Some(false),
            error: Some(WsErrorPayload {
                code: err.code().to_string(),
                message: err.message(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BootstrapRequest {
    #[serde(default)]
    pub heartbeat: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProviderRequest {
    pub profile_name: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProviderRequest {
    pub id: String,
    pub profile_name: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProviderModelRequest {
    pub provider_id: String,
    pub model_name: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProviderModelRequest {
    pub id: String,
    pub model_name: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProviderModelRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestProviderModelRequest {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub provider_profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindSessionProviderRequest {
    pub session_id: String,
    pub provider_profile_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSendRequest {
    pub session_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCancelRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolApprovalResolveRequest {
    pub approval_id: String,
    pub approved: bool,
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
pub struct RunStartedPayload {
    pub session_id: String,
    pub run: ChatRun,
    pub user_message: ChatMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPayload {
    pub session_id: String,
    pub run_id: String,
    pub step: u8,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStartedPayload {
    pub session_id: String,
    pub run_id: String,
    pub step: u8,
    pub block_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTokenPayload {
    pub session_id: String,
    pub run_id: String,
    pub step: u8,
    pub block_id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningFinishedPayload {
    pub session_id: String,
    pub run_id: String,
    pub step: u8,
    pub block_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepStartedPayload {
    pub session_id: String,
    pub run_id: String,
    pub step: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepFinishedPayload {
    pub session_id: String,
    pub run_id: String,
    pub step: AgentStep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequestedPayload {
    pub session_id: String,
    pub run_id: String,
    pub step: u8,
    pub state: String,
    pub tool_call: ToolCall,
    pub approval: Option<ToolApproval>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFinishedPayload {
    pub session_id: String,
    pub run_id: String,
    pub step: u8,
    pub tool_call: ToolCall,
    pub tool_result: ToolResult,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFinishedPayload {
    pub session_id: String,
    pub run: ChatRun,
    pub messages: Vec<ChatMessage>,
    pub usage_total: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFailedPayload {
    pub session_id: String,
    pub run_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCancelledPayload {
    pub session_id: String,
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    pub server_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredProviders {
    #[serde(default)]
    pub accounts: Vec<ProviderAccount>,
    #[serde(default)]
    pub profiles: Vec<LegacyProviderProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyProviderProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn now_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true)
}

pub fn flatten_provider_profiles(accounts: &[ProviderAccount]) -> Vec<ProviderProfile> {
    let mut profiles = Vec::new();
    for account in accounts {
        for model in &account.models {
            let model_name = if model.name.trim().is_empty() {
                model.model.clone()
            } else {
                model.name.clone()
            };
            profiles.push(ProviderProfile {
                id: model.id.clone(),
                provider_id: account.id.clone(),
                model_name,
                name: account.name.clone(),
                base_url: account.base_url.clone(),
                api_key: account.api_key.clone(),
                model: model.model.clone(),
                created_at: model.created_at.clone(),
                updated_at: model.updated_at.clone(),
            });
        }
    }
    profiles.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    profiles
}

pub fn migrate_provider_accounts_from_legacy(
    profiles: Vec<LegacyProviderProfile>,
) -> Vec<ProviderAccount> {
    let mut grouped: HashMap<(String, String, String), ProviderAccount> = HashMap::new();
    for profile in profiles {
        let key = (
            profile.name.clone(),
            profile.base_url.clone(),
            profile.api_key.clone(),
        );
        let created_at = profile.created_at.clone();
        let updated_at = profile.updated_at.clone();
        let account = grouped.entry(key).or_insert_with(|| ProviderAccount {
            id: Uuid::new_v4().to_string(),
            name: profile.name.clone(),
            base_url: profile.base_url.clone(),
            api_key: profile.api_key.clone(),
            models: Vec::new(),
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
        });
        account.models.push(ProviderModel {
            id: profile.id,
            provider_id: account.id.clone(),
            name: profile.model.clone(),
            model: profile.model,
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
        });
        if account.created_at > created_at {
            account.created_at = created_at;
        }
        if account.updated_at < updated_at {
            account.updated_at = updated_at;
        }
    }

    let mut accounts = grouped.into_values().collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    for account in &mut accounts {
        account
            .models
            .sort_by(|left, right| left.created_at.cmp(&right.created_at));
    }
    accounts
}

pub fn normalize_provider_accounts(accounts: &mut [ProviderAccount]) -> bool {
    let mut changed = false;
    for account in accounts {
        if account.id.trim().is_empty() {
            account.id = Uuid::new_v4().to_string();
            changed = true;
        }
        if account.created_at.trim().is_empty() {
            account.created_at = now_timestamp();
            changed = true;
        }
        if account.updated_at.trim().is_empty() {
            account.updated_at = account.created_at.clone();
            changed = true;
        }
        for model in &mut account.models {
            if model.id.trim().is_empty() {
                model.id = Uuid::new_v4().to_string();
                changed = true;
            }
            if model.provider_id != account.id {
                model.provider_id = account.id.clone();
                changed = true;
            }
            if model.name.trim().is_empty() {
                model.name = model.model.clone();
                changed = true;
            }
            if model.created_at.trim().is_empty() {
                model.created_at = now_timestamp();
                changed = true;
            }
            if model.updated_at.trim().is_empty() {
                model.updated_at = model.created_at.clone();
                changed = true;
            }
        }
    }
    changed
}

pub fn new_provider_account(req: CreateProviderRequest) -> ProviderAccount {
    let now = now_timestamp();
    ProviderAccount {
        id: Uuid::new_v4().to_string(),
        name: req.profile_name,
        base_url: req.base_url,
        api_key: req.api_key,
        models: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    }
}

pub fn update_provider_account(
    existing: &ProviderAccount,
    req: UpdateProviderRequest,
) -> ProviderAccount {
    ProviderAccount {
        id: existing.id.clone(),
        name: req.profile_name,
        base_url: req.base_url,
        api_key: req.api_key,
        models: existing.models.clone(),
        created_at: existing.created_at.clone(),
        updated_at: now_timestamp(),
    }
}

pub fn new_provider_model(req: CreateProviderModelRequest) -> ProviderModel {
    let now = now_timestamp();
    ProviderModel {
        id: Uuid::new_v4().to_string(),
        provider_id: req.provider_id,
        name: req.model_name,
        model: req.model,
        created_at: now.clone(),
        updated_at: now,
    }
}

pub fn update_provider_model(
    existing: &ProviderModel,
    req: UpdateProviderModelRequest,
) -> ProviderModel {
    ProviderModel {
        id: existing.id.clone(),
        provider_id: existing.provider_id.clone(),
        name: req.model_name,
        model: req.model,
        created_at: existing.created_at.clone(),
        updated_at: now_timestamp(),
    }
}

pub fn new_chat_session(provider_profile_id: Option<String>) -> ChatSession {
    let now = now_timestamp();
    ChatSession {
        id: Uuid::new_v4().to_string(),
        title: "New chat".to_string(),
        provider_profile_id,
        created_at: now.clone(),
        updated_at: now,
        last_run_at: None,
    }
}

pub fn new_chat_run(session_id: impl Into<String>, text: impl Into<String>) -> ChatRun {
    ChatRun {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.into(),
        status: "running".to_string(),
        user_message: text.into(),
        output_text: String::new(),
        created_at: now_timestamp(),
        finished_at: None,
        error_message: None,
    }
}

pub fn new_user_chat_message(
    session_id: impl Into<String>,
    run_id: impl Into<String>,
    text: impl Into<String>,
) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.into(),
        role: "user".to_string(),
        parts_json: json!([ContentPart::Text(text.into())]),
        run_id: Some(run_id.into()),
        created_at: now_timestamp(),
    }
}

pub fn new_tool_approval(
    session_id: impl Into<String>,
    run_id: impl Into<String>,
    call_id: impl Into<String>,
    action: impl Into<String>,
    path: impl Into<String>,
    preview_json: Value,
) -> ToolApproval {
    ToolApproval {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.into(),
        run_id: run_id.into(),
        call_id: call_id.into(),
        action: action.into(),
        path: path.into(),
        preview_json,
        status: "pending".to_string(),
        created_at: now_timestamp(),
        resolved_at: None,
    }
}

pub fn title_from_first_prompt(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "New chat".to_string();
    }
    let mut title = trimmed.chars().take(48).collect::<String>();
    if trimmed.chars().count() > 48 {
        title.push('…');
    }
    title
}

pub fn role_to_string(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

pub fn string_to_role(role: &str) -> AppResult<MessageRole> {
    match role {
        "system" => Ok(MessageRole::System),
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "tool" => Ok(MessageRole::Tool),
        other => Err(AppError::Validation(format!(
            "unknown message role `{other}`"
        ))),
    }
}

pub fn record_from_message(
    session_id: &str,
    run_id: &str,
    message: &Message,
) -> AppResult<ChatMessage> {
    Ok(ChatMessage {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: role_to_string(message.role()).to_string(),
        parts_json: serde_json::to_value(message.parts())?,
        run_id: Some(run_id.to_string()),
        created_at: now_timestamp(),
    })
}

pub fn message_from_record(record: &ChatMessage) -> AppResult<Message> {
    let role = string_to_role(&record.role)?;
    let parts = serde_json::from_value::<Vec<ContentPart>>(record.parts_json.clone())?;
    Message::new(role, parts).map_err(|err| AppError::Validation(err.to_string()))
}
