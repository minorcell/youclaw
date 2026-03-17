use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use aquaregia::{
    AgentStep, ErrorCode, FinishReason, GenerateTextRequest, LlmClient, ToolCall, Usage,
};

use crate::backend::agents::context_compactor::{
    compact_in_memory_messages, maybe_compact_session_context,
};
use crate::backend::agents::message_builder::{
    build_turn_messages, inject_bootstrap_guidance, make_assistant_message,
};
use crate::backend::agents::stream_collector::collect_step_stream;
use crate::backend::agents::token_estimator::estimate_tokens_for_messages;
use crate::backend::agents::tool_dispatcher::handle_tool_calls;
use crate::backend::agents::tools::{
    build_bash_exec_tool, build_filesystem_tools, build_memory_get_tool, build_memory_search_tool,
    BashToolContext, FilesystemToolContext, ToolRuntimeContext,
};
use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::domain::{
    record_from_message, title_from_first_prompt, ChatMessage, ChatTurn, TurnStatus,
};
use crate::backend::models::events::{
    StepFinishedPayload, StepStartedPayload, TurnFinishedPayload,
};
use crate::backend::providers::{normalize_openai_compatible_endpoint, resolve_provider_api_key};
use crate::backend::BackendState;

pub(crate) const MAX_OUTPUT_TOKENS: u32 = 1400;
pub(crate) const MIN_MAX_STEPS: u8 = 8;
pub(crate) const MAX_MAX_STEPS: u8 = 128;
const MIN_CONTEXT_WINDOW_TOKENS: u32 = 75_000;
const MAX_CONTEXT_WINDOW_TOKENS: u32 = 200_000;

pub(super) async fn execute_turn(state: BackendState, turn: ChatTurn) -> AppResult<()> {
    let provider_service = state.provider_service();
    let runtime_service = state.runtime_service();
    let session = state.storage.get_session(&turn.session_id)?;
    let provider_id = session
        .provider_profile_id
        .clone()
        .ok_or_else(|| AppError::Validation("session has no bound provider profile".to_string()))?;
    let provider = provider_service
        .get_profile(&provider_id)?
        .ok_or_else(|| AppError::NotFound(format!("provider profile `{provider_id}`")))?;
    let config = runtime_service.get_agent_config()?;
    let max_steps = clamp_max_steps(config.max_steps);
    let context_window_tokens =
        resolve_context_window_tokens(config.max_input_tokens, provider.context_window_tokens);
    let compact_threshold = ((context_window_tokens as f32) * config.compact_ratio)
        .round()
        .max(1.0) as usize;

    let resolved_api_key = resolve_provider_api_key(&provider.api_key)?;
    let (normalized_base_url, chat_path) = normalize_openai_compatible_endpoint(&provider.base_url);
    let builder = LlmClient::openai_compatible(normalized_base_url)
        .api_key(resolved_api_key)
        .think_tag_parsing(true);
    let client = if let Some(path) = chat_path {
        builder.chat_completions_path(path).build()?
    } else {
        builder.build()?
    };

    let _ = maybe_compact_session_context(
        &state,
        &turn.session_id,
        &provider.model,
        compact_threshold,
        false,
    )?;
    let mut messages = build_turn_messages(&state, &turn.session_id)?;
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
    let tool_calls = Arc::new(Mutex::new(HashMap::<String, ToolCall>::new()));
    let token = state
        .get_turn_token(&turn.id)
        .ok_or_else(|| AppError::Cancelled("turn token missing".to_string()))?;

    let tool_runtime = ToolRuntimeContext {
        session_id: turn.session_id.clone(),
        turn_id: turn.id.clone(),
        current_step: Arc::clone(&current_step),
        tool_calls: Arc::clone(&tool_calls),
        cancellation_token: token.clone(),
        approvals: state.approvals.clone(),
        approval_mode: session.approval_mode,
        storage: state.storage.clone(),
        hub: state.ws_hub.clone(),
    };
    let filesystem_context = FilesystemToolContext {
        runtime: tool_runtime.clone(),
        workspace_root: state.workspace.root().to_path_buf(),
    };
    let mut tools = build_filesystem_tools(filesystem_context);
    tools.push(build_bash_exec_tool(BashToolContext {
        runtime: tool_runtime,
        workspace_root: state.workspace.root().to_path_buf(),
    }));
    tools.push(build_memory_search_tool(state.memory_service()));
    tools.push(build_memory_get_tool(state.memory_service()));

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
    let mut new_persisted_messages = Vec::<ChatMessage>::new();
    let mut final_output = String::new();
    let mut completed_step_count = 0u32;
    let mut finished = false;

    for step in 1..=max_steps {
        current_step.store(step, Ordering::Relaxed);

        let estimated_tokens = estimate_tokens_for_messages(&messages, &provider.model);
        if estimated_tokens > compact_threshold {
            if step == 1 {
                if maybe_compact_session_context(
                    &state,
                    &turn.session_id,
                    &provider.model,
                    compact_threshold,
                    true,
                )?
                .is_some()
                {
                    messages = build_turn_messages(&state, &turn.session_id)?;
                    if let Some(guidance) = bootstrap_guidance.as_deref() {
                        inject_bootstrap_guidance(&mut messages, guidance);
                    }
                }
            } else {
                let _ = compact_in_memory_messages(&mut messages);
            }
        }

        state.ws_hub.emit_turn_event(
            &turn.id,
            "chat.step.started",
            StepStartedPayload {
                session_id: turn.session_id.clone(),
                turn_id: turn.id.clone(),
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

        let stream = match client.stream(request).await {
            Ok(stream) => stream,
            Err(err) if err.code == ErrorCode::Cancelled => {
                return Err(AppError::Cancelled(err.message));
            }
            Err(err) => return Err(err.into()),
        };

        let step_output = collect_step_stream(&state, &turn, step, stream, &tool_calls).await?;
        usage_total += step_output.usage.clone();

        let assistant_message = make_assistant_message(
            &step_output.reasoning_parts,
            &step_output.text,
            &step_output.tool_calls,
        )?;
        let persisted_assistant =
            record_from_message(&turn.session_id, &turn.id, &assistant_message)?;
        messages.push(assistant_message);
        new_persisted_messages.push(persisted_assistant);

        if step_output.tool_calls.is_empty() {
            let step_state = AgentStep {
                step,
                output_text: step_output.text.clone(),
                reasoning_text: step_output.reasoning_text.clone(),
                reasoning_parts: step_output.reasoning_parts.clone(),
                finish_reason: FinishReason::Stop,
                usage: step_output.usage,
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
            };
            emit_step_finished(&state, &turn, &step_state)?;
            completed_step_count += 1;
            let _ = state.storage.update_turn_usage_metric(
                &turn,
                Some(&usage_total),
                Some(completed_step_count),
            );
            final_output = step_output.text;
            finished = true;
            break;
        }

        let tool_results = handle_tool_calls(
            &state,
            &turn,
            step,
            &step_output.tool_calls,
            &tool_map,
            &mut messages,
            &mut new_persisted_messages,
        )
        .await?;

        let step_state = AgentStep {
            step,
            output_text: step_output.text,
            reasoning_text: step_output.reasoning_text,
            reasoning_parts: step_output.reasoning_parts,
            finish_reason: FinishReason::ToolCalls,
            usage: step_output.usage,
            tool_calls: step_output.tool_calls,
            tool_results,
        };
        emit_step_finished(&state, &turn, &step_state)?;
        completed_step_count += 1;
        let _ = state.storage.update_turn_usage_metric(
            &turn,
            Some(&usage_total),
            Some(completed_step_count),
        );
    }

    if !finished {
        return Err(AppError::Agent(format!(
            "agent reached max_steps ({max_steps}) without final answer"
        )));
    }

    finalize_turn(
        &state,
        &turn,
        &session.title,
        &new_persisted_messages,
        &final_output,
        &usage_total,
        completed_step_count,
    )
}

pub(crate) fn clamp_max_steps(value: u8) -> u8 {
    value.clamp(MIN_MAX_STEPS, MAX_MAX_STEPS)
}

pub(crate) fn resolve_context_window_tokens(user_default: u32, model_override: Option<u32>) -> u32 {
    model_override
        .unwrap_or(user_default)
        .clamp(MIN_CONTEXT_WINDOW_TOKENS, MAX_CONTEXT_WINDOW_TOKENS)
}

pub(super) fn err_status(err: &AppError) -> TurnStatus {
    match err {
        AppError::Cancelled(_) => TurnStatus::Cancelled,
        _ => TurnStatus::Failed,
    }
}

fn finalize_turn(
    state: &BackendState,
    turn: &ChatTurn,
    session_title: &str,
    new_persisted_messages: &[ChatMessage],
    final_output: &str,
    usage_total: &Usage,
    step_count: u32,
) -> AppResult<()> {
    state.storage.insert_messages(new_persisted_messages)?;
    let turn_messages = state.storage.list_messages_for_turn(&turn.id)?;
    let finished_turn =
        state
            .storage
            .update_turn(&turn.id, TurnStatus::Completed, Some(final_output), None)?;
    state
        .storage
        .update_turn_usage_metric(&finished_turn, Some(usage_total), Some(step_count))?;
    let title = if session_title == "New chat" {
        Some(title_from_first_prompt(&turn.user_message))
    } else {
        None
    };
    state
        .storage
        .touch_session_for_turn(&turn.session_id, title.as_deref())?;
    state.session_service().publish_changed()?;
    state.ws_hub.emit_turn_event(
        &turn.id,
        "chat.turn.finished",
        TurnFinishedPayload {
            session_id: turn.session_id.clone(),
            turn: finished_turn,
            new_messages: turn_messages,
            usage_total: usage_total.clone(),
        },
    )?;
    Ok(())
}

fn emit_step_finished(state: &BackendState, turn: &ChatTurn, step: &AgentStep) -> AppResult<()> {
    state
        .storage
        .insert_turn_step(&turn.id, &turn.session_id, step)?;
    state.ws_hub.emit_turn_event(
        &turn.id,
        "chat.step.finished",
        StepFinishedPayload {
            session_id: turn.session_id.clone(),
            turn_id: turn.id.clone(),
            step: step.clone(),
        },
    )
}
