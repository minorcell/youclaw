use crate::backend::errors::AppResult;
use crate::backend::models::WsEnvelope;
use crate::backend::BackendState;

pub(super) fn try_handle(
    state: &BackendState,
    envelope: &WsEnvelope,
) -> AppResult<Option<WsEnvelope>> {
    if envelope.name != "bootstrap.get" {
        return Ok(None);
    }
    Ok(Some(WsEnvelope::response_ok(
        envelope.id.clone(),
        envelope.name.clone(),
        state.runtime_service().bootstrap()?,
    )?))
}
