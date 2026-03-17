//! Execute tool calls for one step and persist side effects.
//!
//! Responsibilities:
//! - run selected tool executors and normalize errors;
//! - inject internal metadata for filesystem tool-call binding;
//! - write tool usage metrics and timeline events;
//! - append tool results to conversation history.

use std::collections::HashMap;
use std::time::Instant;

use aquaregia::tool::{Tool, ToolExecError};
use aquaregia::{Message, ToolCall, ToolResult};
use serde_json::{json, Value};

use crate::backend::agents::tool_result_processor::ToolResultProcessor;
use crate::backend::agents::tools::{
    requires_tool_call_binding, tool_action, INTERNAL_TOOL_CALL_ID_FIELD,
};
use crate::backend::errors::AppResult;
use crate::backend::models::domain::{record_from_message, ChatMessage, ChatTurn};
use crate::backend::models::events::ToolFinishedPayload;
use crate::backend::BackendState;

/// Dispatch all tool calls in the order emitted by the model.
pub(crate) async fn handle_tool_calls(
    state: &BackendState,
    turn: &ChatTurn,
    step: u8,
    step_tool_calls: &[ToolCall],
    tool_map: &HashMap<String, Tool>,
    messages: &mut Vec<Message>,
    new_persisted_messages: &mut Vec<ChatMessage>,
) -> AppResult<Vec<ToolResult>> {
    let mut tool_results = Vec::new();
    let result_processor = ToolResultProcessor::new();

    for tool_call in step_tool_calls {
        let selected_tool = tool_map.get(&tool_call.tool_name);
        let (mut tool_result, duration_ms) = if let Some(tool) = selected_tool {
            let execution_args = if requires_tool_call_binding(&tool_call.tool_name) {
                with_internal_tool_call_id(&tool_call.args_json, &tool_call.call_id)
            } else {
                tool_call.args_json.clone()
            };
            execute_tool_call(tool, tool_call, execution_args).await
        } else {
            (
                ToolResult {
                    call_id: tool_call.call_id.clone(),
                    output_json: json!({
                        "error": format!("unknown tool `{}`", tool_call.tool_name),
                    }),
                    is_error: true,
                },
                0,
            )
        };

        tool_result.output_json =
            result_processor.process(&tool_call.tool_name, tool_result.output_json);

        let tool_action = tool_call
            .args_json
            .get("action")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .or_else(|| tool_action(&tool_call.tool_name).map(ToOwned::to_owned));
        let _ = state.storage.record_turn_tool_metric(
            &turn.id,
            &turn.session_id,
            &tool_call.call_id,
            &tool_call.tool_name,
            tool_action.as_deref(),
            &tool_call.args_json,
            if tool_result.is_error { "error" } else { "ok" },
            Some(duration_ms),
            tool_result.is_error,
        );

        state.ws_hub.emit_turn_event(
            &turn.id,
            "chat.step.tool.finished",
            ToolFinishedPayload {
                session_id: turn.session_id.clone(),
                turn_id: turn.id.clone(),
                step,
                tool_call: tool_call.clone(),
                tool_result: tool_result.clone(),
                duration_ms,
            },
        )?;
        let tool_message = Message::tool_result(tool_result.clone());
        let persisted_tool_message =
            record_from_message(&turn.session_id, &turn.id, &tool_message)?;
        messages.push(tool_message);
        new_persisted_messages.push(persisted_tool_message);
        tool_results.push(tool_result);
    }
    Ok(tool_results)
}

async fn execute_tool_call(tool: &Tool, call: &ToolCall, args_json: Value) -> (ToolResult, u64) {
    if call.tool_name != tool.descriptor.name {
        return (
            ToolResult {
                call_id: call.call_id.clone(),
                output_json: json!({
                    "error": format!("unknown tool `{}`", call.tool_name),
                }),
                is_error: true,
            },
            0,
        );
    }

    let started = Instant::now();
    let execution = tool.executor.execute(args_json).await;
    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let (output_json, is_error) = match execution {
        Ok(output_json) => (output_json, false),
        Err(ToolExecError::Execution(message)) => (json!({ "error": message }), true),
        Err(ToolExecError::Timeout) => (json!({ "error": "timeout" }), true),
    };

    (
        ToolResult {
            call_id: call.call_id.clone(),
            output_json,
            is_error,
        },
        duration_ms,
    )
}

/// Add internal call-id metadata so filesystem tools can claim exact runtime `ToolCall`.
fn with_internal_tool_call_id(args_json: &Value, call_id: &str) -> Value {
    match args_json {
        Value::Object(map) => {
            let mut with_meta = map.clone();
            with_meta.insert(
                INTERNAL_TOOL_CALL_ID_FIELD.to_string(),
                Value::String(call_id.to_string()),
            );
            Value::Object(with_meta)
        }
        _ => args_json.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::with_internal_tool_call_id;

    #[test]
    fn with_internal_tool_call_id_injects_metadata() {
        let args = json!({ "path": "README.md" });
        let with_meta = with_internal_tool_call_id(&args, "call-1");
        assert_eq!(
            with_meta
                .get("__youclaw_call_id")
                .and_then(|value| value.as_str()),
            Some("call-1")
        );
        assert_eq!(
            with_meta.get("path").and_then(|value| value.as_str()),
            Some("README.md")
        );
    }
}
