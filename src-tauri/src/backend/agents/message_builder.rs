//! Build and shape conversation messages passed to the LLM.

use aquaregia::{ContentPart, Message, MessageRole, ReasoningPart, ToolCall};

use crate::backend::agents::context_constants::SUMMARY_MARKER;
use crate::backend::agents::summarizer::extract_message_text;
use crate::backend::errors::{AppError, AppResult};
use crate::backend::BackendState;

/// Build turn input: system prompt + optional compressed summary + active messages.
pub(crate) fn build_turn_messages(
    state: &BackendState,
    session_id: &str,
) -> AppResult<Vec<Message>> {
    let mut messages = Vec::new();
    let session = state.storage.get_session(session_id)?;
    let workspace_path = session.workspace_path.as_deref().ok_or_else(|| {
        AppError::Validation("session has no bound workspace".to_string())
    })?;
    let profiles = state.profile_service().list_all()?;
    messages.push(Message::system_text(
        state
            .workspace
            .build_system_prompt(std::path::Path::new(workspace_path), &profiles)?,
    ));

    let compressed_summary = state.storage.get_session_compressed_summary(session_id)?;
    if !compressed_summary.trim().is_empty() {
        messages.push(Message::user_text(format!(
            "{SUMMARY_MARKER}\n{}",
            compressed_summary.trim()
        )));
    }

    messages.extend(
        state
            .storage
            .list_active_message_objects_for_session(session_id)?,
    );
    Ok(messages)
}

/// Inject one-off guidance right after system/(summary) prefix.
pub(crate) fn inject_turn_guidance(messages: &mut Vec<Message>, guidance: &str) {
    if guidance.trim().is_empty() {
        return;
    }
    let insert_index = if messages
        .get(1)
        .map(|message| message_contains_prefix(message, SUMMARY_MARKER))
        .unwrap_or(false)
    {
        2
    } else {
        1
    };
    messages.insert(
        insert_index.min(messages.len()),
        Message::user_text(guidance.to_string()),
    );
}

/// Build assistant message from streamed reasoning/text and emitted tool calls.
pub(crate) fn make_assistant_message(
    reasoning_parts: &[ReasoningPart],
    text: &str,
    tool_calls: &[ToolCall],
) -> AppResult<Message> {
    let mut parts = Vec::new();
    for reasoning in reasoning_parts {
        parts.push(ContentPart::Reasoning(reasoning.clone()));
    }
    if !text.is_empty() {
        parts.push(ContentPart::Text(text.to_string()));
    }
    for call in tool_calls {
        parts.push(ContentPart::ToolCall(call.clone()));
    }
    if parts.is_empty() {
        parts.push(ContentPart::Text(String::new()));
    }
    Message::new(MessageRole::Assistant, parts)
        .map_err(|err| AppError::Agent(format!("invalid assistant message: {err}")))
}

fn message_contains_prefix(message: &Message, prefix: &str) -> bool {
    extract_message_text(message).starts_with(prefix)
}
