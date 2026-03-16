//! Lightweight token estimation helpers used by compaction thresholds.

use aquaregia::{Message, MessageRole};
use tiktoken_rs::{get_bpe_from_model, CoreBPE};

use crate::backend::agents::summarizer::extract_message_text;

/// Estimate total token usage for a message list.
pub(crate) fn estimate_tokens_for_messages(messages: &[Message], model: &str) -> usize {
    let mut joined = String::new();
    for message in messages {
        joined.push_str(role_label(message.role()));
        joined.push(':');
        joined.push_str(&extract_message_text(message));
        joined.push('\n');
    }
    estimate_text_tokens(&joined, model)
}

/// Estimate tokens for arbitrary text; fallback to char-based approximation.
pub(crate) fn estimate_text_tokens(text: &str, model: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    if let Some(tokenizer) = tokenizer_for_model(model) {
        return tokenizer.encode_with_special_tokens(text).len();
    }
    text.chars().count().saturating_add(3) / 4
}

fn tokenizer_for_model(model: &str) -> Option<CoreBPE> {
    get_bpe_from_model(model).ok()
}

fn role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}
