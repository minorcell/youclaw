use crate::backend::errors::AppResult;
use crate::backend::models::requests::{
    AgentConfigUpdateRequest, MemoryGetRequest, MemorySearchRequest, WorkspaceFileReadRequest,
    WorkspaceFileWriteRequest,
};
use crate::backend::models::WsEnvelope;
use crate::backend::BackendState;

pub(super) fn try_handle(
    state: &BackendState,
    envelope: &WsEnvelope,
) -> AppResult<Option<WsEnvelope>> {
    let response = match envelope.name.as_str() {
        "agent.config.get" => WsEnvelope::response_ok(
            envelope.id.clone(),
            envelope.name.clone(),
            state.runtime_service().get_agent_config()?,
        )?,
        "agent.config.update" => {
            let req = serde_json::from_value::<AgentConfigUpdateRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.runtime_service().update_agent_config(req)?,
            )?
        }
        "agent.workspace.files.list" => WsEnvelope::response_ok(
            envelope.id.clone(),
            envelope.name.clone(),
            state.workspace_service().list_files()?,
        )?,
        "agent.workspace.files.read" => {
            let req = serde_json::from_value::<WorkspaceFileReadRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.workspace_service().read_file(req)?,
            )?
        }
        "agent.workspace.files.write" => {
            let req =
                serde_json::from_value::<WorkspaceFileWriteRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.workspace_service().write_file(req)?,
            )?
        }
        "agent.memory.search" => {
            let req = serde_json::from_value::<MemorySearchRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.memory_service().search(req)?,
            )?
        }
        "agent.memory.get" => {
            let req = serde_json::from_value::<MemoryGetRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.memory_service().get(req)?,
            )?
        }
        "agent.memory.reindex" => WsEnvelope::response_ok(
            envelope.id.clone(),
            envelope.name.clone(),
            state.memory_service().reindex()?,
        )?,
        _ => return Ok(None),
    };
    Ok(Some(response))
}
