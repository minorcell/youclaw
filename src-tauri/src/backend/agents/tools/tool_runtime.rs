use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aquaregia::ToolCall;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::{SessionApprovalMode, ToolApproval, ToolRequestedPayload};
use crate::backend::{ApprovalService, StorageService, WsHub};

/// Approval timeout for privileged operations.
const APPROVAL_TIMEOUT_SECS: u64 = 600;
/// Internal field injected at runtime to bind tool call id safely.
pub(crate) const INTERNAL_TOOL_CALL_ID_FIELD: &str = "__youclaw_call_id";

/// Shared runtime context for privileged tools.
#[derive(Clone)]
pub struct ToolRuntimeContext {
    pub session_id: String,
    pub turn_id: String,
    pub current_step: Arc<AtomicU8>,
    pub tool_calls: Arc<Mutex<HashMap<String, ToolCall>>>,
    pub cancellation_token: CancellationToken,
    pub approvals: ApprovalService,
    pub approval_mode: SessionApprovalMode,
    pub storage: StorageService,
    pub hub: WsHub,
}

impl ToolRuntimeContext {
    pub fn should_skip_mutation_approval(&self) -> bool {
        matches!(self.approval_mode, SessionApprovalMode::FullAccess)
    }

    pub(crate) fn claim_tool_call(
        &self,
        tool_name: &str,
        tool_call_id: Option<&str>,
    ) -> AppResult<ToolCall> {
        let tool_call_id = tool_call_id.ok_or_else(|| {
            AppError::Agent(format!(
                "missing internal `{INTERNAL_TOOL_CALL_ID_FIELD}` for tool `{tool_name}`"
            ))
        })?;

        let mut registry = self
            .tool_calls
            .lock()
            .map_err(|_| AppError::Agent("tool call registry lock poisoned".to_string()))?;
        let call = registry.remove(tool_call_id).ok_or_else(|| {
            AppError::Agent(format!(
                "tool call binding not found: id=`{tool_call_id}`, tool=`{tool_name}`"
            ))
        })?;

        if call.tool_name != tool_name {
            return Err(AppError::Agent(format!(
                "tool call binding mismatch: id=`{tool_call_id}`, expected=`{tool_name}`, actual=`{}`",
                call.tool_name
            )));
        }

        Ok(call)
    }
}

pub(crate) async fn await_approval(
    context: &ToolRuntimeContext,
    tool_call: &ToolCall,
    approval: &ToolApproval,
) -> AppResult<bool> {
    let receiver = context.approvals.register_pending(approval.clone())?;

    context.hub.emit_turn_event(
        &context.turn_id,
        "chat.step.tool.requested",
        ToolRequestedPayload {
            session_id: context.session_id.clone(),
            turn_id: context.turn_id.clone(),
            step: context.current_step.load(Ordering::Relaxed),
            state: "awaiting_approval".to_string(),
            tool_call: tool_call.clone(),
            approval: Some(approval.clone()),
        },
    )?;

    match wait_for_approval(receiver, &context.cancellation_token).await {
        Ok(value) => Ok(value),
        Err(ApprovalWaitError::Cancelled) => {
            let _ = context.approvals.mark_status(&approval.id, "cancelled");
            Err(AppError::Cancelled(
                "turn cancelled while waiting for approval".to_string(),
            ))
        }
        Err(ApprovalWaitError::TimedOut) => {
            let _ = context.approvals.mark_status(&approval.id, "timed_out");
            Err(AppError::Cancelled("approval timed out".to_string()))
        }
        Err(ApprovalWaitError::ChannelClosed) => {
            let _ = context.approvals.mark_status(&approval.id, "cancelled");
            Err(AppError::Cancelled("approval channel closed".to_string()))
        }
    }
}

async fn wait_for_approval(
    receiver: oneshot::Receiver<bool>,
    cancellation_token: &CancellationToken,
) -> Result<bool, ApprovalWaitError> {
    tokio::select! {
        _ = cancellation_token.cancelled() => Err(ApprovalWaitError::Cancelled),
        result = timeout(Duration::from_secs(APPROVAL_TIMEOUT_SECS), receiver) => {
            match result {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(_)) => Err(ApprovalWaitError::ChannelClosed),
                Err(_) => Err(ApprovalWaitError::TimedOut),
            }
        }
    }
}

enum ApprovalWaitError {
    Cancelled,
    TimedOut,
    ChannelClosed,
}
