use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use aquaregia::tool::{Tool, ToolExecError};
use aquaregia::{
    AgentStep, ContentPart, ErrorCode, FinishReason, GenerateTextRequest, LlmClient, Message,
    MessageRole, ReasoningPart, StreamEvent, ToolCall, ToolResult, Usage,
};
use futures_util::StreamExt;
use serde_json::json;

use crate::backend::errors::{AppError, AppResult};
use crate::backend::filesystem::{build_filesystem_tool, FilesystemToolContext};
use crate::backend::models::{
    title_from_first_prompt, ChatRun, RunCancelledPayload, RunFailedPayload, RunFinishedPayload,
    ReasoningFinishedPayload, ReasoningStartedPayload, ReasoningTokenPayload, RunStartedPayload,
    StepFinishedPayload, StepStartedPayload, TokenPayload, ToolFinishedPayload, ToolRequestedPayload,
};
use crate::backend::BackendState;

const SYSTEM_PROMPT: &str = r#"
你是本地桌面 Agent 的 MVP 版本。
- 你可以聊天，也可以使用 filesystem 工具读取目录、读取文件、写入文件。
- 修改文件前，优先读取或列目录确认上下文。
- 仅在确实需要落盘时调用 write_file。
- 相对路径按用户 home 目录解析。
- 写入需要用户审批；如果被拒绝，请解释原因并给出下一步建议。
- 使用中文回答，输出简洁。
"#;

const MAX_STEPS: u8 = 8;
const MAX_OUTPUT_TOKENS: u32 = 1400;

pub fn spawn_run(state: BackendState, run: ChatRun) {
    tokio::spawn(async move {
        let run_id = run.id.clone();
        let session_id = run.session_id.clone();
        let result = execute_run(state.clone(), run).await;
        if let Err(err) = result {
            let _ = state
                .storage
                .update_run(&run_id, err_status(&err), None, Some(&err.message()));
            let _ = state.ws_hub.emit_run_event(
                &run_id,
                if matches!(err, AppError::Cancelled(_)) {
                    "chat.run.cancelled"
                } else {
                    "chat.run.failed"
                },
                if matches!(err, AppError::Cancelled(_)) {
                    RunCancelledPayload {
                        session_id,
                        run_id: run_id.clone(),
                    }
                    .into_payload()
                } else {
                    RunFailedPayload {
                        session_id,
                        run_id: run_id.clone(),
                        error: err.message(),
                    }
                    .into_payload()
                },
            );
        }
        state.unregister_run(&run_id);
    });
}

async fn execute_run(state: BackendState, run: ChatRun) -> AppResult<()> {
    let session = state.storage.get_session(&run.session_id)?;
    let provider_id = session
        .provider_profile_id
        .clone()
        .ok_or_else(|| AppError::Validation("session has no bound provider profile".to_string()))?;
    let provider = state
        .get_provider_profile(&provider_id)?
        .ok_or_else(|| AppError::NotFound(format!("provider profile `{provider_id}`")))?;

    let client = LlmClient::openai_compatible(provider.base_url.clone())
        .api_key(provider.api_key.clone())
        .build()?;

    let mut messages = state
        .storage
        .list_message_objects_for_session(&run.session_id)?;
    if messages.is_empty()
        || !matches!(
            messages.first().map(Message::role),
            Some(aquaregia::MessageRole::System)
        )
    {
        messages.insert(0, Message::system_text(SYSTEM_PROMPT));
    }

    let current_step = Arc::new(AtomicU8::new(0));
    let tool_calls = Arc::new(Mutex::new(VecDeque::new()));
    let token = state
        .get_run_token(&run.id)
        .ok_or_else(|| AppError::Cancelled("run token missing".to_string()))?;

    let filesystem = build_filesystem_tool(FilesystemToolContext {
        session_id: run.session_id.clone(),
        run_id: run.id.clone(),
        current_step: Arc::clone(&current_step),
        tool_calls: Arc::clone(&tool_calls),
        cancellation_token: token.clone(),
        approvals: state.approvals.clone(),
        storage: state.storage.clone(),
        hub: state.ws_hub.clone(),
    });

    let mut usage_total = Usage::default();
    let mut step_results = Vec::new();
    let mut final_output = String::new();
    let mut finished = false;

    for step in 1..=MAX_STEPS {
        current_step.store(step, Ordering::Relaxed);
        state.ws_hub.emit_run_event(
            &run.id,
            "chat.step.started",
            StepStartedPayload {
                session_id: run.session_id.clone(),
                run_id: run.id.clone(),
                step,
            },
        )?;

        let request = GenerateTextRequest::builder(provider.model.clone())
            .messages(messages.clone())
            .temperature(0.2)
            .max_output_tokens(MAX_OUTPUT_TOKENS)
            .tools([filesystem.descriptor.clone()])
            .cancellation_token(token.clone())
            .build()
            .map_err(|err| AppError::Agent(err.to_string()))?;

        let mut stream = match client.stream(request).await {
            Ok(stream) => stream,
            Err(err) if err.code == ErrorCode::Cancelled => {
                return Err(AppError::Cancelled(err.message));
            }
            Err(err) => return Err(err.into()),
        };

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
                    state.ws_hub.emit_run_event(
                        &run.id,
                        "chat.reasoning.started",
                        ReasoningStartedPayload {
                            session_id: run.session_id.clone(),
                            run_id: run.id.clone(),
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
                    state.ws_hub.emit_run_event(
                        &run.id,
                        "chat.reasoning.token",
                        ReasoningTokenPayload {
                            session_id: run.session_id.clone(),
                            run_id: run.id.clone(),
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
                    state.ws_hub.emit_run_event(
                        &run.id,
                        "chat.reasoning.finished",
                        ReasoningFinishedPayload {
                            session_id: run.session_id.clone(),
                            run_id: run.id.clone(),
                            step,
                            block_id,
                            provider_metadata,
                        },
                    )?;
                }
                StreamEvent::TextDelta { text } => {
                    if !text.is_empty() {
                        step_text.push_str(&text);
                        state.ws_hub.emit_run_event(
                            &run.id,
                            "chat.token",
                            TokenPayload {
                                session_id: run.session_id.clone(),
                                run_id: run.id.clone(),
                                step,
                                text,
                            },
                        )?;
                    }
                }
                StreamEvent::ToolCallReady { call } => {
                    if let Ok(mut queue) = tool_calls.lock() {
                        queue.push_back(call.clone());
                    }
                    step_tool_calls.push(call.clone());
                    state.ws_hub.emit_run_event(
                        &run.id,
                        "chat.tool.requested",
                        ToolRequestedPayload {
                            session_id: run.session_id.clone(),
                            run_id: run.id.clone(),
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

        usage_total += step_usage.clone();

        let assistant_message =
            make_assistant_message(&step_reasoning_parts, &step_text, &step_tool_calls)?;
        messages.push(assistant_message);

        if step_tool_calls.is_empty() {
            let step_state = AgentStep {
                step,
                output_text: step_text.clone(),
                reasoning_text: step_reasoning_text.clone(),
                reasoning_parts: step_reasoning_parts.clone(),
                finish_reason: FinishReason::Stop,
                usage: step_usage,
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
            };
            state.ws_hub.emit_run_event(
                &run.id,
                "chat.step.finished",
                StepFinishedPayload {
                    session_id: run.session_id.clone(),
                    run_id: run.id.clone(),
                    step: step_state.clone(),
                },
            )?;
            step_results.push(step_state);
            final_output = step_text;
            finished = true;
            break;
        }

        let mut tool_results = Vec::new();
        for tool_call in &step_tool_calls {
            let (tool_result, duration_ms) = execute_tool_call(&filesystem, tool_call).await;
            state.ws_hub.emit_run_event(
                &run.id,
                "chat.tool.finished",
                ToolFinishedPayload {
                    session_id: run.session_id.clone(),
                    run_id: run.id.clone(),
                    step,
                    tool_call: tool_call.clone(),
                    tool_result: tool_result.clone(),
                    duration_ms,
                },
            )?;
            messages.push(Message::tool_result(tool_result.clone()));
            tool_results.push(tool_result);
        }

        let step_state = AgentStep {
            step,
            output_text: step_text,
            reasoning_text: step_reasoning_text,
            reasoning_parts: step_reasoning_parts,
            finish_reason: FinishReason::ToolCalls,
            usage: step_usage,
            tool_calls: step_tool_calls,
            tool_results,
        };
        state.ws_hub.emit_run_event(
            &run.id,
            "chat.step.finished",
            StepFinishedPayload {
                session_id: run.session_id.clone(),
                run_id: run.id.clone(),
                step: step_state.clone(),
            },
        )?;
        step_results.push(step_state);
    }

    if !finished {
        return Err(AppError::Agent(format!(
            "agent reached max_steps ({MAX_STEPS}) without final answer"
        )));
    }

    let persisted_messages =
        state
            .storage
            .replace_session_messages(&run.session_id, &run.id, &messages)?;
    let finished_run = state
        .storage
        .update_run(&run.id, "completed", Some(&final_output), None)?;
    let session_title = if session.title == "New chat" {
        Some(title_from_first_prompt(&run.user_message))
    } else {
        None
    };
    state
        .storage
        .touch_session_for_run(&run.session_id, session_title.as_deref())?;
    state.publish_sessions_changed()?;
    state.ws_hub.emit_run_event(
        &run.id,
        "chat.run.finished",
        RunFinishedPayload {
            session_id: run.session_id.clone(),
            run: finished_run,
            messages: persisted_messages,
            usage_total,
        },
    )?;
    let _ = step_results;
    Ok(())
}

fn make_assistant_message(
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

async fn execute_tool_call(tool: &Tool, call: &ToolCall) -> (ToolResult, u64) {
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
    let execution = tool.executor.execute(call.args_json.clone()).await;
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

fn err_status(err: &AppError) -> &str {
    match err {
        AppError::Cancelled(_) => "cancelled",
        _ => "failed",
    }
}

trait PayloadExt {
    fn into_payload(self) -> serde_json::Value;
}

impl PayloadExt for RunCancelledPayload {
    fn into_payload(self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

impl PayloadExt for RunFailedPayload {
    fn into_payload(self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

pub fn start_run(state: BackendState, session_id: String, text: String) -> AppResult<String> {
    let session = state.storage.get_session(&session_id)?;
    let title = if session.title == "New chat" {
        Some(title_from_first_prompt(&text))
    } else {
        None
    };
    state
        .storage
        .touch_session_for_run(&session_id, title.as_deref())?;
    state
        .storage
        .set_last_opened_session_id(Some(&session_id))?;

    let run = crate::backend::models::new_chat_run(session_id.clone(), text.clone());
    let user_message =
        crate::backend::models::new_user_chat_message(session_id.clone(), run.id.clone(), text);
    state.storage.insert_run(&run)?;
    state.storage.insert_message(&user_message)?;
    state.publish_sessions_changed()?;
    state.ws_hub.emit_run_event(
        &run.id,
        "chat.run.started",
        RunStartedPayload {
            session_id,
            run: run.clone(),
            user_message,
        },
    )?;
    state.register_run(run.id.clone());
    spawn_run(state, run.clone());
    Ok(run.id)
}
