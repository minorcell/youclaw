//! Agent 运行期子模块。
//!
//! 当前主要承载工具系统，后续可继续扩展为 `planner` / `executor` 等模块。
mod turn_execution;
mod turn_start;

pub mod context_compactor;
pub mod context_constants;
pub mod memory;
pub mod message_builder;
pub mod stream_collector;
pub mod summarizer;
pub mod token_estimator;
pub mod tool_dispatcher;
pub mod tool_result_processor;
pub mod tools;
pub mod workspace;

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::domain::ChatTurn;
use crate::backend::models::events::{TurnCancelledPayload, TurnFailedPayload};
use crate::backend::BackendState;

#[cfg(test)]
use crate::backend::agents::context_compactor::compact_in_memory_messages;
#[cfg(test)]
use crate::backend::agents::context_constants::{STEP_SUMMARY_MARKER, SUMMARY_MARKER};
#[cfg(test)]
use crate::backend::agents::summarizer::extract_message_text;

pub fn spawn_turn(state: BackendState, turn: ChatTurn) {
    tokio::spawn(async move {
        let turn_id = turn.id.clone();
        let session_id = turn.session_id.clone();
        let result = turn_execution::execute_turn(state.clone(), turn).await;
        if let Err(err) = result {
            if let Ok(updated_turn) = state.storage.update_turn(
                &turn_id,
                turn_execution::err_status(&err),
                None,
                Some(&err.message()),
            ) {
                let _ = state
                    .storage
                    .update_turn_usage_metric(&updated_turn, None, None);
            }
            let payload = if matches!(err, AppError::Cancelled(_)) {
                serde_json::to_value(TurnCancelledPayload {
                    session_id,
                    turn_id: turn_id.clone(),
                })
                .unwrap_or_default()
            } else {
                serde_json::to_value(TurnFailedPayload {
                    session_id,
                    turn_id: turn_id.clone(),
                    error: err.message(),
                })
                .unwrap_or_default()
            };
            let _ = state.ws_hub.emit_turn_event(
                &turn_id,
                if matches!(err, AppError::Cancelled(_)) {
                    "chat.turn.cancelled"
                } else {
                    "chat.turn.failed"
                },
                payload,
            );
        }
        state.unregister_turn(&turn_id);
    });
}

pub fn start_turn(state: BackendState, session_id: String, text: String) -> AppResult<String> {
    turn_start::start_turn(state, session_id, text)
}

#[cfg(test)]
mod tests {
    use aquaregia::Message;

    use super::{
        compact_in_memory_messages, extract_message_text, turn_execution::clamp_max_steps,
        turn_execution::resolve_context_window_tokens, STEP_SUMMARY_MARKER, SUMMARY_MARKER,
    };
    use crate::backend::agents::turn_execution::{MAX_MAX_STEPS, MIN_MAX_STEPS};

    #[test]
    fn in_memory_compaction_keeps_latest_message() {
        let mut messages = vec![
            Message::system_text("system"),
            Message::user_text("u1"),
            Message::assistant_text("a1"),
            Message::user_text("u2"),
            Message::assistant_text("a2"),
            Message::user_text("u3"),
        ];

        let summary = compact_in_memory_messages(&mut messages).expect("summary");
        assert!(!summary.is_empty());
        assert_eq!(messages.len(), 3);
        assert!(extract_message_text(&messages[1]).starts_with(STEP_SUMMARY_MARKER));
        assert_eq!(extract_message_text(&messages[2]), "u3");
    }

    #[test]
    fn in_memory_compaction_preserves_previous_summary_slot() {
        let mut messages = vec![
            Message::system_text("system"),
            Message::user_text(format!("{SUMMARY_MARKER}\nold")),
            Message::user_text("u1"),
            Message::assistant_text("a1"),
            Message::user_text("u2"),
        ];

        compact_in_memory_messages(&mut messages).expect("summary");
        assert!(extract_message_text(&messages[1]).starts_with(SUMMARY_MARKER));
        assert!(extract_message_text(&messages[2]).starts_with(STEP_SUMMARY_MARKER));
        assert_eq!(extract_message_text(&messages[3]), "u2");
    }

    #[test]
    fn clamp_max_steps_keeps_bounds() {
        assert_eq!(clamp_max_steps(0), MIN_MAX_STEPS);
        assert_eq!(clamp_max_steps(8), MIN_MAX_STEPS);
        assert_eq!(clamp_max_steps(64), 64);
        assert_eq!(clamp_max_steps(u8::MAX), MAX_MAX_STEPS);
    }

    #[test]
    fn resolve_context_window_tokens_prefers_model_override() {
        assert_eq!(resolve_context_window_tokens(120_000, None), 120_000);
        assert_eq!(
            resolve_context_window_tokens(120_000, Some(150_000)),
            150_000
        );
    }
}
