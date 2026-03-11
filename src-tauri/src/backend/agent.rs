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
use serde_json::{json, Value};
use tiktoken_rs::{get_bpe_from_model, CoreBPE};

use crate::backend::agents::tools::{
    build_filesystem_list_dir_tool, build_filesystem_read_file_tool,
    build_filesystem_write_file_tool, build_memory_get_tool, build_memory_search_tool,
    build_memory_write_tool, FilesystemToolContext, FILESYSTEM_LIST_DIR_TOOL_NAME,
    FILESYSTEM_READ_FILE_TOOL_NAME, FILESYSTEM_WRITE_FILE_TOOL_NAME,
};
use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::{
    now_timestamp, record_from_message, title_from_first_prompt, AgentMemoryCompactedPayload,
    ChatMessage, ChatRun, ReasoningFinishedPayload, ReasoningStartedPayload, ReasoningTokenPayload,
    RunCancelledPayload, RunFailedPayload, RunFinishedPayload, RunStartedPayload,
    StepFinishedPayload, StepStartedPayload, TokenPayload, ToolFinishedPayload,
    ToolRequestedPayload,
};
use crate::backend::{normalize_openai_compatible_endpoint, BackendState};

const MAX_OUTPUT_TOKENS: u32 = 1400;
const SUMMARY_CHAR_LIMIT: usize = 16_000;
const SUMMARY_MARKER: &str = "[previous-summary]";
const STEP_SUMMARY_MARKER: &str = "[step-summary]";

pub fn spawn_run(state: BackendState, run: ChatRun) {
    tokio::spawn(async move {
        let run_id = run.id.clone();
        let session_id = run.session_id.clone();
        let result = execute_run(state.clone(), run).await;
        if let Err(err) = result {
            if let Ok(updated_run) =
                state
                    .storage
                    .update_run(&run_id, err_status(&err), None, Some(&err.message()))
            {
                let _ = state.storage.update_run_usage_metric(&updated_run, None);
            }
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
    let config = state.storage.get_agent_config()?;
    let max_steps = config.max_steps.max(1);
    let compact_threshold = ((config.max_input_tokens as f32) * config.compact_ratio)
        .round()
        .max(1.0) as usize;
    let keep_recent = config.keep_recent as usize;

    let (normalized_base_url, chat_path) = normalize_openai_compatible_endpoint(&provider.base_url);
    let builder =
        LlmClient::openai_compatible(normalized_base_url).api_key(provider.api_key.clone());
    let client = if let Some(path) = chat_path {
        builder.chat_completions_path(path).build()?
    } else {
        builder.build()?
    };

    let _ = maybe_compact_session_context(&state, &run.session_id, &provider.model, false)?;
    let mut messages = build_run_messages(&state, &run.session_id)?;
    let bootstrap_guidance = if state.workspace.should_bootstrap() {
        let guidance = state.workspace.build_bootstrap_guidance(&config.language);
        state.workspace.mark_bootstrap_completed()?;
        Some(guidance)
    } else {
        None
    };
    if let Some(guidance) = bootstrap_guidance.as_deref() {
        inject_bootstrap_guidance(&mut messages, guidance);
    }

    let current_step = Arc::new(AtomicU8::new(0));
    let tool_calls = Arc::new(Mutex::new(VecDeque::new()));
    let token = state
        .get_run_token(&run.id)
        .ok_or_else(|| AppError::Cancelled("run token missing".to_string()))?;

    let filesystem_context = FilesystemToolContext {
        session_id: run.session_id.clone(),
        run_id: run.id.clone(),
        workspace_root: state.workspace.root().to_path_buf(),
        current_step: Arc::clone(&current_step),
        tool_calls: Arc::clone(&tool_calls),
        cancellation_token: token.clone(),
        approvals: state.approvals.clone(),
        storage: state.storage.clone(),
        hub: state.ws_hub.clone(),
    };
    let filesystem_list_dir_tool = build_filesystem_list_dir_tool(filesystem_context.clone());
    let filesystem_read_file_tool = build_filesystem_read_file_tool(filesystem_context.clone());
    let filesystem_write_file_tool = build_filesystem_write_file_tool(filesystem_context.clone());
    let memory_search_tool = build_memory_search_tool(state.clone());
    let memory_get_tool = build_memory_get_tool(state.clone());
    let memory_write_tool = build_memory_write_tool(state.clone());
    let tools = vec![
        filesystem_list_dir_tool,
        filesystem_read_file_tool,
        filesystem_write_file_tool,
        memory_search_tool,
        memory_get_tool,
        memory_write_tool,
    ];
    let tool_map = tools
        .iter()
        .cloned()
        .map(|item| (item.descriptor.name.clone(), item))
        .collect::<HashMap<_, _>>();
    let tool_descriptors = tools
        .iter()
        .map(|item| item.descriptor.clone())
        .collect::<Vec<_>>();

    let mut usage_total = Usage::default();
    let mut step_results = Vec::new();
    let mut new_persisted_messages = Vec::<ChatMessage>::new();
    let mut final_output = String::new();
    let mut finished = false;

    for step in 1..=max_steps {
        current_step.store(step, Ordering::Relaxed);

        let estimated_tokens = estimate_tokens_for_messages(&messages, &provider.model);
        if estimated_tokens > compact_threshold {
            if step == 1 {
                if maybe_compact_session_context(&state, &run.session_id, &provider.model, true)?
                    .is_some()
                {
                    messages = build_run_messages(&state, &run.session_id)?;
                    if let Some(guidance) = bootstrap_guidance.as_deref() {
                        inject_bootstrap_guidance(&mut messages, guidance);
                    }
                }
            } else {
                let _ = compact_in_memory_messages(&mut messages, keep_recent);
            }
        }

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
            .tools(tool_descriptors.clone())
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
                    if matches!(
                        call.tool_name.as_str(),
                        FILESYSTEM_LIST_DIR_TOOL_NAME
                            | FILESYSTEM_READ_FILE_TOOL_NAME
                            | FILESYSTEM_WRITE_FILE_TOOL_NAME
                    ) {
                        if let Ok(mut queue) = tool_calls.lock() {
                            queue.push_back(call.clone());
                        }
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
        let persisted_assistant =
            record_from_message(&run.session_id, &run.id, &assistant_message)?;
        messages.push(assistant_message);
        new_persisted_messages.push(persisted_assistant);

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
            let selected_tool = tool_map.get(&tool_call.tool_name);
            let (tool_result, duration_ms) = if let Some(tool) = selected_tool {
                execute_tool_call(tool, tool_call).await
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
            let tool_action = tool_call
                .args_json
                .get("action")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
                .or_else(|| filesystem_tool_action(&tool_call.tool_name).map(ToOwned::to_owned));
            let _ = state.storage.record_run_tool_metric(
                &run.id,
                &run.session_id,
                &tool_call.tool_name,
                tool_action.as_deref(),
                if tool_result.is_error { "error" } else { "ok" },
                Some(duration_ms),
                tool_result.is_error,
            );

            if tool_call.tool_name == "memory_write" {
                let status = if tool_result.is_error { "error" } else { "ok" };
                let path = tool_result
                    .output_json
                    .get("path")
                    .and_then(|value| value.as_str())
                    .or_else(|| {
                        tool_call
                            .args_json
                            .get("path")
                            .and_then(|value| value.as_str())
                    })
                    .unwrap_or("memory/unknown.md");
                let bytes_written = tool_result
                    .output_json
                    .get("bytes_written")
                    .and_then(|value| value.as_u64())
                    .map(|value| value as usize);
                let _ = state.storage.record_file_operation(
                    &run.session_id,
                    &run.id,
                    Some(&tool_call.call_id),
                    "memory_write",
                    path,
                    status,
                    bytes_written,
                );
            }

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
            let tool_message = Message::tool_result(tool_result.clone());
            let persisted_tool_message =
                record_from_message(&run.session_id, &run.id, &tool_message)?;
            messages.push(tool_message);
            new_persisted_messages.push(persisted_tool_message);
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
            "agent reached max_steps ({max_steps}) without final answer"
        )));
    }

    state.storage.insert_messages(&new_persisted_messages)?;
    let run_messages = state.storage.list_messages_for_run(&run.id)?;
    let finished_run = state
        .storage
        .update_run(&run.id, "completed", Some(&final_output), None)?;
    state
        .storage
        .update_run_usage_metric(&finished_run, Some(&usage_total))?;
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
            new_messages: run_messages,
            usage_total,
        },
    )?;
    let _ = step_results;
    Ok(())
}

fn build_run_messages(state: &BackendState, session_id: &str) -> AppResult<Vec<Message>> {
    let mut messages = Vec::new();
    messages.push(Message::system_text(state.workspace.build_system_prompt()?));

    let compressed_summary = state.storage.get_session_compressed_summary(session_id)?;
    if !compressed_summary.trim().is_empty() {
        messages.push(Message::user_text(format!(
            "{SUMMARY_MARKER}\n{}",
            compressed_summary.trim()
        )));
    }

    messages.extend(
        state
            .storage
            .list_active_message_objects_for_session(session_id)?,
    );
    Ok(messages)
}

fn inject_bootstrap_guidance(messages: &mut Vec<Message>, guidance: &str) {
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

fn maybe_compact_session_context(
    state: &BackendState,
    session_id: &str,
    model: &str,
    force: bool,
) -> AppResult<Option<AgentMemoryCompactedPayload>> {
    let config = state.storage.get_agent_config()?;
    let keep_recent = config.keep_recent as usize;
    let active_records = state.storage.list_active_messages_for_session(session_id)?;
    if active_records.len() <= keep_recent {
        return Ok(None);
    }

    if !force {
        let assembled = build_run_messages(state, session_id)?;
        let estimated = estimate_tokens_for_messages(&assembled, model);
        let threshold = ((config.max_input_tokens as f32) * config.compact_ratio)
            .round()
            .max(1.0) as usize;
        if estimated <= threshold {
            return Ok(None);
        }
    }

    let split_index = active_records.len().saturating_sub(keep_recent);
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

fn compact_in_memory_messages(messages: &mut Vec<Message>, keep_recent: usize) -> Option<String> {
    if messages.len() <= keep_recent + 2 {
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

    if messages.len() <= prefix_end + keep_recent + 1 {
        return None;
    }

    let split_index = messages.len().saturating_sub(keep_recent);
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

fn summarize_chat_records(records: &[ChatMessage]) -> String {
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

fn summarize_messages(messages: &[Message]) -> String {
    let mut lines = Vec::new();
    for message in messages {
        let content = extract_message_text(message);
        if content.trim().is_empty() {
            continue;
        }
        let role = match message.role() {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };
        lines.push(format!("- [{role}] {}", truncate(content.trim(), 220)));
    }
    truncate(&lines.join("\n"), SUMMARY_CHAR_LIMIT / 2)
}

fn merge_summaries(previous: &str, addition: &str) -> String {
    if previous.trim().is_empty() {
        return truncate(addition.trim(), SUMMARY_CHAR_LIMIT);
    }
    let merged = format!(
        "{previous}\n\n## Compressed at {}\n{addition}",
        now_timestamp()
    );
    truncate(&merged, SUMMARY_CHAR_LIMIT)
}

fn estimate_tokens_for_messages(messages: &[Message], model: &str) -> usize {
    let mut joined = String::new();
    for message in messages {
        let role = match message.role() {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };
        joined.push_str(role);
        joined.push(':');
        joined.push_str(&extract_message_text(message));
        joined.push('\n');
    }
    estimate_text_tokens(&joined, model)
}

fn estimate_text_tokens(text: &str, model: &str) -> usize {
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

fn message_contains_prefix(message: &Message, prefix: &str) -> bool {
    extract_message_text(message).starts_with(prefix)
}

fn extract_message_text(message: &Message) -> String {
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

fn extract_text_from_parts_value(parts_json: &Value) -> String {
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

fn truncate(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = input.chars().take(max_chars).collect::<String>();
    out.push('…');
    out
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

fn filesystem_tool_action(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        FILESYSTEM_LIST_DIR_TOOL_NAME => Some("list_dir"),
        FILESYSTEM_READ_FILE_TOOL_NAME => Some("read_file"),
        FILESYSTEM_WRITE_FILE_TOOL_NAME => Some("write_file"),
        _ => None,
    }
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
    let provider = if let Some(provider_id) = session.provider_profile_id.as_deref() {
        state.get_provider_profile(provider_id)?
    } else {
        None
    };
    let detail_logged = state.storage.get_usage_detail_logging_enabled()?;
    state
        .storage
        .insert_run_usage_metric_start(&run, provider.as_ref(), detail_logged)?;
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

#[cfg(test)]
mod tests {
    use aquaregia::Message;

    use super::{
        compact_in_memory_messages, extract_message_text, STEP_SUMMARY_MARKER, SUMMARY_MARKER,
    };

    #[test]
    fn in_memory_compaction_keeps_recent_messages() {
        let mut messages = vec![
            Message::system_text("system"),
            Message::user_text("u1"),
            Message::assistant_text("a1"),
            Message::user_text("u2"),
            Message::assistant_text("a2"),
            Message::user_text("u3"),
        ];

        let summary = compact_in_memory_messages(&mut messages, 2).expect("summary");
        assert!(!summary.is_empty());
        assert_eq!(messages.len(), 4);
        assert!(extract_message_text(&messages[1]).starts_with(STEP_SUMMARY_MARKER));
        assert_eq!(extract_message_text(&messages[2]), "a2");
        assert_eq!(extract_message_text(&messages[3]), "u3");
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

        compact_in_memory_messages(&mut messages, 1).expect("summary");
        assert!(extract_message_text(&messages[1]).starts_with(SUMMARY_MARKER));
        assert!(extract_message_text(&messages[2]).starts_with(STEP_SUMMARY_MARKER));
    }
}
