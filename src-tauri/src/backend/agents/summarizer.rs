//! Text extraction helpers shared by compaction and token estimation.

use aquaregia::{ContentPart, Message, MessageRole};
use serde_json::Value;

use crate::backend::models::domain::ChatMessage;

pub(crate) fn extract_message_text(message: &Message) -> String {
    format_parts_text(message.parts())
}

pub(crate) fn extract_text_from_parts_value(parts_json: &Value) -> String {
    serde_json::from_value::<Vec<ContentPart>>(parts_json.clone())
        .map(|parts| format_parts_text(&parts))
        .unwrap_or_default()
}

pub(crate) fn format_chat_records_for_compaction(records: &[ChatMessage]) -> String {
    records
        .iter()
        .enumerate()
        .filter_map(|(index, message)| {
            let content = extract_text_from_parts_value(&message.parts_json);
            if content.trim().is_empty() {
                return None;
            }
            Some(format!(
                "### Message {}\nrole: {}\ncontent:\n{}",
                index + 1,
                message.role,
                content.trim()
            ))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn truncate(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = input.chars().take(max_chars).collect::<String>();
    out.push('…');
    out
}

fn format_parts_text(parts: &[ContentPart]) -> String {
    let mut chunks = Vec::<String>::new();

    for part in parts {
        match part {
            ContentPart::Text(text) => push_non_empty(&mut chunks, text),
            ContentPart::Reasoning(reasoning) => push_non_empty(&mut chunks, &reasoning.text),
            ContentPart::ToolCall(call) => {
                chunks.push(format!("tool_call {} {}", call.tool_name, call.args_json))
            }
            ContentPart::ToolResult(result) => {
                chunks.push(format!("tool_result {}", result.output_json))
            }
        }
    }

    chunks.join("\n")
}

fn push_non_empty(chunks: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        chunks.push(trimmed.to_string());
    }
}

fn _role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::backend::agents::summarizer::format_chat_records_for_compaction;
    use crate::backend::models::domain::{ChatMessage, MessageRole};

    #[test]
    fn format_chat_records_for_compaction_keeps_role_and_content() {
        let records = vec![ChatMessage {
            id: "m1".to_string(),
            session_id: "s1".to_string(),
            role: MessageRole::Assistant,
            parts_json: json!([
                { "Text": "done" },
                { "ToolCall": { "call_id": "c1", "tool_name": "read_text_file", "args_json": { "path": "README.md" } } }
            ]),
            turn_id: Some("t1".to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }];

        let formatted = format_chat_records_for_compaction(&records);
        assert!(formatted.contains("role: assistant"));
        assert!(formatted.contains("done"));
        assert!(formatted.contains("tool_call read_text_file"));
    }
}
