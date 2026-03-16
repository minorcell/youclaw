//! Summary/text extraction helpers shared by compaction and token estimation.

use aquaregia::{ContentPart, Message, MessageRole};
use serde_json::Value;

use crate::backend::agents::context_constants::SUMMARY_CHAR_LIMIT;
use crate::backend::models::{now_timestamp, ChatMessage};

/// Summarize persisted chat records into short bullet lines.
pub(crate) fn summarize_chat_records(records: &[ChatMessage]) -> String {
    let mut lines = Vec::new();
    for message in records {
        let content = extract_text_from_parts_value(&message.parts_json);
        if content.trim().is_empty() {
            continue;
        }
        lines.push(format!(
            "- [{}] {}",
            message.role,
            truncate(content.trim(), 240)
        ));
    }

    if lines.is_empty() {
        return String::new();
    }

    let body = lines.join("\n");
    truncate(&body, SUMMARY_CHAR_LIMIT / 2)
}

/// Summarize in-memory messages (including tool calls/results) into bullet lines.
pub(crate) fn summarize_messages(messages: &[Message]) -> String {
    let mut lines = Vec::new();
    for message in messages {
        let content = extract_message_text(message);
        if content.trim().is_empty() {
            continue;
        }
        let role = role_label(message.role());
        lines.push(format!("- [{role}] {}", truncate(content.trim(), 220)));
    }
    truncate(&lines.join("\n"), SUMMARY_CHAR_LIMIT / 2)
}

/// Merge previous summary with new compacted slice summary.
pub(crate) fn merge_summaries(previous: &str, addition: &str) -> String {
    if previous.trim().is_empty() {
        return truncate(addition.trim(), SUMMARY_CHAR_LIMIT);
    }
    let merged = format!(
        "{previous}\n\n## Compressed at {}\n{addition}",
        now_timestamp()
    );
    truncate(&merged, SUMMARY_CHAR_LIMIT)
}

/// Flatten aquaregia message parts into a plain-text representation.
pub(crate) fn extract_message_text(message: &Message) -> String {
    let mut text = String::new();
    for part in message.parts() {
        match part {
            ContentPart::Text(value) => {
                if !value.is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(value);
                }
            }
            ContentPart::Reasoning(reasoning) => {
                if !reasoning.text.is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&reasoning.text);
                }
            }
            ContentPart::ToolCall(call) => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&format!("tool_call {} {}", call.tool_name, call.args_json));
            }
            ContentPart::ToolResult(result) => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&format!("tool_result {}", result.output_json));
            }
        }
    }
    text
}

/// Flatten persisted `parts_json` payload into plain text.
pub(crate) fn extract_text_from_parts_value(parts_json: &Value) -> String {
    let mut chunks = Vec::new();
    if let Some(parts) = parts_json.as_array() {
        for part in parts {
            if let Some(text) = part.get("Text").and_then(|value| value.as_str()) {
                if !text.trim().is_empty() {
                    chunks.push(text.to_string());
                }
                continue;
            }
            if let Some(text) = part
                .get("Reasoning")
                .and_then(|value| value.get("text"))
                .and_then(|value| value.as_str())
            {
                if !text.trim().is_empty() {
                    chunks.push(text.to_string());
                }
                continue;
            }
            if let Some(tool_call) = part.get("ToolCall") {
                chunks.push(format!("tool_call {}", tool_call));
                continue;
            }
            if let Some(tool_result) = part.get("ToolResult") {
                chunks.push(format!("tool_result {}", tool_result));
            }
        }
    }
    chunks.join("\n")
}

/// UTF-8 safe char-based truncation helper for summaries/preview fields.
pub(crate) fn truncate(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = input.chars().take(max_chars).collect::<String>();
    out.push('…');
    out
}

fn role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}
