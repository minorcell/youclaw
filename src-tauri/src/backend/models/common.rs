use aquaregia::MessageRole;
use chrono::SecondsFormat;

use crate::backend::errors::{AppError, AppResult};

pub fn now_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true)
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
