//! Session context compaction driven by the model itself.

use aquaregia::{BoundClient, CancellationToken, GenerateTextRequest, Message, OpenAiCompatible};

use crate::backend::agents::message_builder::build_turn_messages;
use crate::backend::agents::summarizer::{format_chat_records_for_compaction, truncate};
use crate::backend::agents::token_estimator::estimate_tokens_for_messages;
use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::domain::{ChatMessage, SessionContextSummary};
use crate::backend::models::events::AgentMemoryCompactedPayload;
use crate::backend::BackendState;

const COMPACTION_MAX_OUTPUT_TOKENS: u32 = 1200;
const COMPACTION_TEMPERATURE: f32 = 0.1;
const COMPACTION_PROMPT: &str = include_str!("prompts/templates/CONTEXT_COMPACTION.md");

/// Compact persisted session context into the structured summary when thresholds are exceeded.
pub(crate) async fn maybe_compact_session_context(
    state: &BackendState,
    client: &BoundClient<OpenAiCompatible>,
    session_id: &str,
    model: &str,
    compact_threshold: usize,
    force: bool,
    token: &CancellationToken,
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

    let split_index = active_records.len().saturating_sub(1);
    if split_index == 0 {
        return Ok(None);
    }

    let compacted_slice = &active_records[..split_index];
    let compacted_ids = compacted_slice
        .iter()
        .map(|message| message.id.clone())
        .collect::<Vec<_>>();
    let existing_summary = state.storage.get_session_context_summary(session_id)?;
    let next_summary =
        compact_chat_records(client, model, token, &existing_summary, compacted_slice).await?;

    if next_summary != existing_summary {
        state
            .storage
            .upsert_session_context_summary(session_id, &next_summary)?;
    }
    let changed = state.storage.mark_messages(&compacted_ids, "compressed")?;
    if changed == 0 {
        return Ok(None);
    }

    let payload = AgentMemoryCompactedPayload {
        session_id: session_id.to_string(),
        compacted_messages: changed,
        summary_preview: truncate(&next_summary.render_for_prompt(), 320),
    };
    let _ = state.ws_hub.emit("agent.memory.compacted", payload.clone());
    Ok(Some(payload))
}

async fn compact_chat_records(
    client: &BoundClient<OpenAiCompatible>,
    model: &str,
    token: &CancellationToken,
    existing_summary: &SessionContextSummary,
    records: &[ChatMessage],
) -> AppResult<SessionContextSummary> {
    let transcript = format_chat_records_for_compaction(records);
    if transcript.trim().is_empty() {
        return Ok(existing_summary.clone());
    }

    let request = GenerateTextRequest::builder(model.to_string())
        .messages(build_compaction_messages(existing_summary, &transcript))
        .temperature(COMPACTION_TEMPERATURE)
        .max_output_tokens(COMPACTION_MAX_OUTPUT_TOKENS)
        .cancellation_token(token.clone())
        .build()
        .map_err(|err| AppError::Agent(err.to_string()))?;
    let response = client.generate(request).await?;
    parse_compaction_response(&response.output_text)
}

fn build_compaction_messages(
    existing_summary: &SessionContextSummary,
    transcript: &str,
) -> Vec<Message> {
    let summary_json =
        serde_json::to_string_pretty(existing_summary).unwrap_or_else(|_| "{}".to_string());
    vec![
        Message::system_text(COMPACTION_PROMPT),
        Message::user_text(format!(
            "Existing summary JSON:\n```json\n{summary_json}\n```\n\nOlder conversation messages:\n{transcript}\n\nReturn the full updated JSON object only."
        )),
    ]
}

fn parse_compaction_response(output_text: &str) -> AppResult<SessionContextSummary> {
    let trimmed = output_text.trim();
    if trimmed.is_empty() {
        return Err(AppError::Agent(
            "compaction model returned empty output".to_string(),
        ));
    }

    let json_text = strip_json_fence(trimmed);
    let summary = serde_json::from_str::<SessionContextSummary>(json_text)
        .map_err(|err| AppError::Agent(format!("invalid compaction json: {err}")))?;
    Ok(summary.normalize())
}

fn strip_json_fence(value: &str) -> &str {
    value
        .strip_prefix("```json")
        .and_then(|inner| inner.strip_suffix("```"))
        .map(str::trim)
        .or_else(|| {
            value
                .strip_prefix("```")
                .and_then(|inner| inner.strip_suffix("```"))
                .map(str::trim)
        })
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use crate::backend::agents::context_compactor::parse_compaction_response;

    #[test]
    fn parse_compaction_response_accepts_fenced_json() {
        let summary = parse_compaction_response(
            "```json\n{\"current_goal\":\"ship\",\"pending_actions\":[\"- test\"]}\n```",
        )
        .expect("summary");

        assert_eq!(summary.current_goal, "ship");
        assert_eq!(summary.pending_actions, vec!["- test".to_string()]);
    }
}
