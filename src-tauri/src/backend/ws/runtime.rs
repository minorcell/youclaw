use crate::backend::errors::AppResult;
use crate::backend::models::requests::{
    AgentConfigUpdateRequest, MemorySystemDeleteRequest, MemorySystemGetRequest,
    MemorySystemListRequest, MemorySystemSearchRequest, MemorySystemUpsertRequest,
    ProfileGetRequest, ProfileUpdateRequest,
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
        "agent.profile.get" => {
            let req = serde_json::from_value::<ProfileGetRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.profile_service().get(req)?,
            )?
        }
        "agent.profile.update" => {
            let req = serde_json::from_value::<ProfileUpdateRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.profile_service().update(req)?,
            )?
        }
        "agent.memory_system.list" => {
            let req = serde_json::from_value::<MemorySystemListRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.memory_service().list(req)?,
            )?
        }
        "agent.memory_system.search" => {
            let req =
                serde_json::from_value::<MemorySystemSearchRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.memory_service().search(req)?,
            )?
        }
        "agent.memory_system.get" => {
            let req = serde_json::from_value::<MemorySystemGetRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.memory_service().get(req)?,
            )?
        }
        "agent.memory_system.upsert" => {
            let req =
                serde_json::from_value::<MemorySystemUpsertRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.memory_service().upsert(req)?,
            )?
        }
        "agent.memory_system.delete" => {
            let req =
                serde_json::from_value::<MemorySystemDeleteRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.memory_service().delete(req)?,
            )?
        }
        _ => return Ok(None),
    };
    Ok(Some(response))
}
