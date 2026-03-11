use aquaregia::{ContentPart, Message};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::backend::errors::AppResult;

use super::{now_timestamp, MessageRole, RunStatus};

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
    pub role: MessageRole,
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
    pub status: RunStatus,
    pub user_message: String,
    pub output_text: String,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
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
        status: RunStatus::Running,
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
        role: MessageRole::User,
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

pub fn record_from_message(
    session_id: &str,
    run_id: &str,
    message: &Message,
) -> AppResult<ChatMessage> {
    Ok(ChatMessage {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: MessageRole::from(message.role()),
        parts_json: serde_json::to_value(message.parts())?,
        run_id: Some(run_id.to_string()),
        created_at: now_timestamp(),
    })
}

pub fn message_from_record(record: &ChatMessage) -> AppResult<Message> {
    let role: aquaregia::MessageRole = record.role.into();
    let parts = serde_json::from_value::<Vec<ContentPart>>(record.parts_json.clone())?;
    Message::new(role, parts)
        .map_err(|err| crate::backend::errors::AppError::Validation(err.to_string()))
}
