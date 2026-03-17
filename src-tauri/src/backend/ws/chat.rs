use crate::backend::agents;
use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::requests::{
    ChatTurnCancelRequest, ChatTurnStartRequest, ToolApprovalResolveRequest, TurnStepsListRequest,
};
use crate::backend::models::responses::TurnStepsListPayload;
use crate::backend::models::WsEnvelope;
use crate::backend::BackendState;

pub(super) async fn try_handle(
    state: &BackendState,
    envelope: &WsEnvelope,
) -> AppResult<Option<WsEnvelope>> {
    let response = match envelope.name.as_str() {
        "chat.turn.start" => {
            let req = serde_json::from_value::<ChatTurnStartRequest>(envelope.payload.clone())?;
            if req.text.trim().is_empty() {
                return Err(AppError::Validation(
                    "message text cannot be empty".to_string(),
                ));
            }
            let turn_id = agents::start_turn(state.clone(), req.session_id, req.text)?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "turn_id": turn_id }),
            )?
        }
        "chat.turn.cancel" => {
            let req = serde_json::from_value::<ChatTurnCancelRequest>(envelope.payload.clone())?;
            let cancelled = state.cancel_turn(&req.turn_id)?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "cancelled": cancelled }),
            )?
        }
        "chat.turn.steps.list" => {
            let req = serde_json::from_value::<TurnStepsListRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                TurnStepsListPayload {
                    turn_id: req.turn_id.clone(),
                    steps: state.storage.list_turn_steps(&req.turn_id)?,
                },
            )?
        }
        "tool_approvals.resolve" => {
            let req =
                serde_json::from_value::<ToolApprovalResolveRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.approvals.resolve(&req.approval_id, req.approved)?,
            )?
        }
        _ => return Ok(None),
    };
    Ok(Some(response))
}
