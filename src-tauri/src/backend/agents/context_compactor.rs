//! Session context compaction for persistent and in-memory histories.
//!
//! Responsibilities:
//! - decide when to compact based on token thresholds;
//! - generate/merge summaries for persisted records;
//! - keep an in-memory rolling step summary to cap prompt growth mid-turn.

use aquaregia::Message;

use crate::backend::agents::context_constants::{STEP_SUMMARY_MARKER, SUMMARY_MARKER};
use crate::backend::agents::message_builder::build_turn_messages;
use crate::backend::agents::summarizer::{
    extract_message_text, merge_summaries, summarize_chat_records, summarize_messages, truncate,
};
use crate::backend::agents::token_estimator::estimate_tokens_for_messages;
use crate::backend::errors::AppResult;
use crate::backend::models::events::AgentMemoryCompactedPayload;
use crate::backend::BackendState;

/// Compact persisted chat records into the session summary when thresholds are exceeded.
pub(crate) fn maybe_compact_session_context(
    state: &BackendState,
    session_id: &str,
    model: &str,
    // Trigger threshold computed by caller from effective context window * compact ratio.
    compact_threshold: usize,
    force: bool,
) -> AppResult<Option<AgentMemoryCompactedPayload>> {
    let active_records = state.storage.list_active_messages_for_session(session_id)?;
    if active_records.len() <= 1 {
        return Ok(None);
    }

    if !force {
        let assembled = build_turn_messages(state, session_id)?;
        let estimated = estimate_tokens_for_messages(&assembled, model);
        if estimated <= compact_threshold.max(1) {
            return Ok(None);
        }
    }

    // Keep only the latest active record to preserve the most recent context.
    let split_index = active_records.len().saturating_sub(1);
    if split_index == 0 {
        return Ok(None);
    }

    let compacted_slice = &active_records[..split_index];
    let compacted_ids = compacted_slice
        .iter()
        .map(|message| message.id.clone())
        .collect::<Vec<_>>();
    let addition = summarize_chat_records(compacted_slice);
    if addition.trim().is_empty() {
        return Ok(None);
    }

    let previous = state.storage.get_session_compressed_summary(session_id)?;
    let merged = merge_summaries(&previous, &addition);
    state
        .storage
        .upsert_session_compressed_summary(session_id, &merged)?;
    let changed = state.storage.mark_messages(&compacted_ids, "compressed")?;
    if changed == 0 {
        return Ok(None);
    }

    let payload = AgentMemoryCompactedPayload {
        session_id: session_id.to_string(),
        compacted_messages: changed,
        summary_preview: truncate(&merged, 320),
    };
    let _ = state.ws_hub.emit("agent.memory.compacted", payload.clone());
    Ok(Some(payload))
}

/// Compact current in-memory prompt messages while preserving recent turns.
pub(crate) fn compact_in_memory_messages(messages: &mut Vec<Message>) -> Option<String> {
    if messages.len() <= 2 {
        return None;
    }

    let prefix_end = if messages
        .get(1)
        .map(|message| message_contains_prefix(message, SUMMARY_MARKER))
        .unwrap_or(false)
    {
        2
    } else {
        1
    };

    if messages.len() <= prefix_end + 1 {
        return None;
    }

    // Keep only the newest message after the prefix block.
    let split_index = messages.len().saturating_sub(1);
    if split_index <= prefix_end {
        return None;
    }

    let removed = messages.drain(prefix_end..split_index).collect::<Vec<_>>();
    let summary = summarize_messages(&removed);
    if summary.trim().is_empty() {
        return None;
    }

    messages.insert(
        prefix_end,
        Message::user_text(format!("{STEP_SUMMARY_MARKER}\n{summary}")),
    );
    Some(summary)
}

fn message_contains_prefix(message: &Message, prefix: &str) -> bool {
    extract_message_text(message).starts_with(prefix)
}
