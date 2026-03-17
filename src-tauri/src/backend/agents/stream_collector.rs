//! Collect one LLM step stream into a structured `StepOutput`.
//!
//! Responsibilities:
//! - normalize stream events into text/reasoning/tool-call buffers;
//! - mirror event deltas to websocket timeline events;
//! - register filesystem tool calls by `call_id` for strong executor binding.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aquaregia::{ErrorCode, ReasoningPart, StreamEvent, TextStream, ToolCall, Usage};
use futures_util::StreamExt;

use crate::backend::agents::tools::requires_tool_call_binding;
use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::domain::ChatTurn;
use crate::backend::models::events::{
    ReasoningFinishedPayload, ReasoningStartedPayload, ReasoningTokenPayload, TokenPayload,
    ToolRequestedPayload,
};
use crate::backend::BackendState;

pub(crate) struct StepOutput {
    pub text: String,
    pub reasoning_text: String,
    pub reasoning_parts: Vec<ReasoningPart>,
    pub usage: Usage,
    pub tool_calls: Vec<ToolCall>,
}

/// Drain the provider stream for a single step and build normalized output.
pub(crate) async fn collect_step_stream(
    state: &BackendState,
    turn: &ChatTurn,
    step: u8,
    mut stream: TextStream,
    tool_calls: &Arc<Mutex<HashMap<String, ToolCall>>>,
) -> AppResult<StepOutput> {
    let mut step_text = String::new();
    let mut step_reasoning_text = String::new();
    let mut step_reasoning_parts = Vec::<ReasoningPart>::new();
    let mut reasoning_part_index_by_block = HashMap::<String, usize>::new();
    let mut step_usage = Usage::default();
    let mut step_tool_calls = Vec::<ToolCall>::new();

    while let Some(event) = stream.next().await {
        let event = match event {
            Ok(event) => event,
            Err(err) if err.code == ErrorCode::Cancelled => {
                return Err(AppError::Cancelled(err.message));
            }
            Err(err) => return Err(err.into()),
        };

        match event {
            StreamEvent::ReasoningStarted {
                block_id,
                provider_metadata,
            } => {
                let block_id_for_map = block_id.clone();
                let part_index = step_reasoning_parts.len();
                step_reasoning_parts.push(ReasoningPart {
                    text: String::new(),
                    provider_metadata: provider_metadata.clone(),
                });
                reasoning_part_index_by_block.insert(block_id_for_map, part_index);
                state.ws_hub.emit_turn_event(
                    &turn.id,
                    "chat.step.reasoning.started",
                    ReasoningStartedPayload {
                        session_id: turn.session_id.clone(),
                        turn_id: turn.id.clone(),
                        step,
                        block_id,
                        provider_metadata,
                    },
                )?;
            }
            StreamEvent::ReasoningDelta {
                block_id,
                text,
                provider_metadata,
            } => {
                if !text.is_empty() {
                    step_reasoning_text.push_str(&text);
                }
                if let Some(index) = reasoning_part_index_by_block.get(&block_id).copied() {
                    if let Some(part) = step_reasoning_parts.get_mut(index) {
                        if !text.is_empty() {
                            part.text.push_str(&text);
                        }
                        if provider_metadata.is_some() {
                            part.provider_metadata = provider_metadata.clone();
                        }
                    }
                } else {
                    let part_index = step_reasoning_parts.len();
                    step_reasoning_parts.push(ReasoningPart {
                        text: text.clone(),
                        provider_metadata: provider_metadata.clone(),
                    });
                    reasoning_part_index_by_block.insert(block_id.clone(), part_index);
                }
                state.ws_hub.emit_turn_event(
                    &turn.id,
                    "chat.step.reasoning.token",
                    ReasoningTokenPayload {
                        session_id: turn.session_id.clone(),
                        turn_id: turn.id.clone(),
                        step,
                        block_id,
                        text,
                        provider_metadata,
                    },
                )?;
            }
            StreamEvent::ReasoningDone {
                block_id,
                provider_metadata,
            } => {
                if let Some(index) = reasoning_part_index_by_block.remove(&block_id) {
                    if let Some(part) = step_reasoning_parts.get_mut(index) {
                        if provider_metadata.is_some() {
                            part.provider_metadata = provider_metadata.clone();
                        }
                    }
                }
                state.ws_hub.emit_turn_event(
                    &turn.id,
                    "chat.step.reasoning.finished",
                    ReasoningFinishedPayload {
                        session_id: turn.session_id.clone(),
                        turn_id: turn.id.clone(),
                        step,
                        block_id,
                        provider_metadata,
                    },
                )?;
            }
            StreamEvent::TextDelta { text } => {
                if !text.is_empty() {
                    step_text.push_str(&text);
                    state.ws_hub.emit_turn_event(
                        &turn.id,
                        "chat.step.token",
                        TokenPayload {
                            session_id: turn.session_id.clone(),
                            turn_id: turn.id.clone(),
                            step,
                            text,
                        },
                    )?;
                }
            }
            StreamEvent::ToolCallReady { call } => {
                if requires_tool_call_binding(&call.tool_name) {
                    // Filesystem executors later claim by id; duplicate ids are treated as protocol errors.
                    let mut registry = tool_calls.lock().map_err(|_| {
                        AppError::Agent("tool call registry lock poisoned".to_string())
                    })?;
                    if registry
                        .insert(call.call_id.clone(), call.clone())
                        .is_some()
                    {
                        return Err(AppError::Agent(format!(
                            "duplicate tool call id received: `{}`",
                            call.call_id
                        )));
                    }
                }
                step_tool_calls.push(call.clone());
                state.ws_hub.emit_turn_event(
                    &turn.id,
                    "chat.step.tool.requested",
                    ToolRequestedPayload {
                        session_id: turn.session_id.clone(),
                        turn_id: turn.id.clone(),
                        step,
                        state: "started".to_string(),
                        tool_call: call,
                        approval: None,
                    },
                )?;
            }
            StreamEvent::Usage { usage } => {
                step_usage += usage;
            }
            StreamEvent::Done => break,
        }
    }

    Ok(StepOutput {
        text: step_text,
        reasoning_text: step_reasoning_text,
        reasoning_parts: step_reasoning_parts,
        usage: step_usage,
        tool_calls: step_tool_calls,
    })
}
