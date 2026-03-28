//! Build and shape conversation messages passed to the LLM.

use aquaregia::{ContentPart, Message, MessageRole, ReasoningPart, ToolCall};

use crate::backend::agents::context_constants::{MEMORY_HINT_MARKER, SUMMARY_MARKER};
use crate::backend::agents::summarizer::extract_message_text;
use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::domain::SessionContextSummary;
use crate::backend::BackendState;

const MEMORY_HINT_MESSAGE: &str = include_str!("prompts/templates/MEMORY_HINT.xml");

/// Build turn input: system prompt + optional compressed summary + active messages.
pub(crate) fn build_turn_messages(
    state: &BackendState,
    session_id: &str,
) -> AppResult<Vec<Message>> {
    let summary = state.storage.get_session_context_summary(session_id)?;
    build_turn_messages_with_summary(state, session_id, &summary)
}

pub(crate) fn build_turn_messages_with_summary(
    state: &BackendState,
    session_id: &str,
    summary: &SessionContextSummary,
) -> AppResult<Vec<Message>> {
    let mut messages = Vec::new();
    let session = state.storage.get_session(session_id)?;
    let workspace_path = session
        .workspace_path
        .as_deref()
        .ok_or_else(|| AppError::Validation("session has no bound workspace".to_string()))?;
    let profiles = state.profile_service().list_all()?;
    messages.push(Message::system_text(state.workspace.build_system_prompt(
        std::path::Path::new(workspace_path),
        &profiles,
    )?));

    if let Some(summary_message) = make_summary_message(summary) {
        messages.push(summary_message);
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

/// Inject a runtime memory reflection hint right before the latest real user message.
pub(crate) fn inject_memory_hint(messages: &mut Vec<Message>) {
    let insert_index = messages
        .iter()
        .enumerate()
        .rev()
        .find(|(_, message)| is_conversation_user_message(message))
        .map(|(index, _)| index);

    if let Some(index) = insert_index {
        messages.insert(index, Message::user_text(MEMORY_HINT_MESSAGE.trim().to_string()));
    }
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

fn is_conversation_user_message(message: &Message) -> bool {
    message.role() == MessageRole::User && !is_runtime_injected_user_message(message)
}

fn is_runtime_injected_user_message(message: &Message) -> bool {
    let text = extract_message_text(message);
    text.starts_with(SUMMARY_MARKER) || text.starts_with(MEMORY_HINT_MARKER)
}

pub(crate) fn make_summary_message(summary: &SessionContextSummary) -> Option<Message> {
    if summary.is_empty() {
        return None;
    }

    let rendered = summary.render_for_prompt();
    Some(Message::user_text(format!("{SUMMARY_MARKER}\n{rendered}")))
}

#[cfg(test)]
mod tests {
    use aquaregia::{Message, MessageRole};

    use crate::backend::agents::context_constants::MEMORY_HINT_MARKER;
    use crate::backend::agents::message_builder::{
        extract_message_text, inject_memory_hint, make_summary_message,
    };
    use crate::backend::models::domain::SessionContextSummary;

    #[test]
    fn inject_memory_hint_places_hint_before_latest_user_message() {
        let mut messages = vec![
            Message::system_text("system"),
            Message::user_text("older user"),
            Message::assistant_text("assistant"),
            Message::user_text("latest user"),
        ];

        inject_memory_hint(&mut messages);

        assert_eq!(messages.len(), 5);
        assert_eq!(messages[3].role(), MessageRole::User);
        assert!(extract_message_text(&messages[3]).starts_with(MEMORY_HINT_MARKER));
        assert_eq!(extract_message_text(&messages[4]), "latest user");
    }

    #[test]
    fn inject_memory_hint_ignores_summary_message_when_finding_latest_user() {
        let summary = make_summary_message(&SessionContextSummary {
            current_goal: "ship".to_string(),
            ..SessionContextSummary::default()
        })
        .expect("summary");
        let mut messages = vec![
            Message::system_text("system"),
            summary,
            Message::user_text("latest user"),
        ];

        inject_memory_hint(&mut messages);

        assert_eq!(messages.len(), 4);
        assert!(extract_message_text(&messages[2]).starts_with(MEMORY_HINT_MARKER));
        assert_eq!(extract_message_text(&messages[3]), "latest user");
    }
}
