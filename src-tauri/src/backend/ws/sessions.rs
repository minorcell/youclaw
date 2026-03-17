use crate::backend::errors::AppResult;
use crate::backend::models::requests::{
    BindSessionProviderRequest, CreateSessionRequest, DeleteSessionRequest, PurgeSessionRequest,
    RenameSessionRequest, RestoreSessionRequest, UpdateSessionApprovalModeRequest,
};
use crate::backend::models::WsEnvelope;
use crate::backend::BackendState;

pub(super) fn try_handle(
    state: &BackendState,
    envelope: &WsEnvelope,
) -> AppResult<Option<WsEnvelope>> {
    let response = match envelope.name.as_str() {
        "sessions.list" => WsEnvelope::response_ok(
            envelope.id.clone(),
            envelope.name.clone(),
            state.session_service().list()?,
        )?,
        "sessions.archived.list" => WsEnvelope::response_ok(
            envelope.id.clone(),
            envelope.name.clone(),
            state.session_service().list_archived()?,
        )?,
        "sessions.create" => {
            let req = serde_json::from_value::<CreateSessionRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.session_service().create(req.provider_profile_id)?,
            )?
        }
        "sessions.delete" => {
            let req = serde_json::from_value::<DeleteSessionRequest>(envelope.payload.clone())?;
            state.session_service().delete(&req.session_id)?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "archived": true }),
            )?
        }
        "sessions.restore" => {
            let req = serde_json::from_value::<RestoreSessionRequest>(envelope.payload.clone())?;
            state.session_service().restore(&req.session_id)?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "restored": true }),
            )?
        }
        "sessions.purge" => {
            let req = serde_json::from_value::<PurgeSessionRequest>(envelope.payload.clone())?;
            state.session_service().purge(&req.session_id)?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "purged": true }),
            )?
        }
        "sessions.rename" => {
            let req = serde_json::from_value::<RenameSessionRequest>(envelope.payload.clone())?;
            state
                .session_service()
                .rename(&req.session_id, &req.title)?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "renamed": true }),
            )?
        }
        "sessions.bind_provider" => {
            let req =
                serde_json::from_value::<BindSessionProviderRequest>(envelope.payload.clone())?;
            state
                .session_service()
                .bind_provider(&req.session_id, &req.provider_profile_id)?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "bound": true }),
            )?
        }
        "sessions.update_approval_mode" => {
            let req = serde_json::from_value::<UpdateSessionApprovalModeRequest>(
                envelope.payload.clone(),
            )?;
            state
                .session_service()
                .update_approval_mode(&req.session_id, req.approval_mode)?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "updated": true }),
            )?
        }
        _ => return Ok(None),
    };
    Ok(Some(response))
}
