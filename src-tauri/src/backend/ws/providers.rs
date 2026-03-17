use crate::backend::errors::AppResult;
use crate::backend::models::requests::{
    CreateProviderModelRequest, CreateProviderRequest, DeleteProviderModelRequest,
    TestProviderModelRequest, UpdateProviderModelRequest, UpdateProviderRequest,
};
use crate::backend::models::WsEnvelope;
use crate::backend::BackendState;

pub(super) async fn try_handle(
    state: &BackendState,
    envelope: &WsEnvelope,
) -> AppResult<Option<WsEnvelope>> {
    let response = match envelope.name.as_str() {
        "providers.list" => WsEnvelope::response_ok(
            envelope.id.clone(),
            envelope.name.clone(),
            state.provider_service().list_snapshot()?,
        )?,
        "providers.create" => {
            let req = serde_json::from_value::<CreateProviderRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.provider_service().create(req).await?,
            )?
        }
        "providers.update" => {
            let req = serde_json::from_value::<UpdateProviderRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.provider_service().update(req).await?,
            )?
        }
        "providers.models.create" => {
            let req =
                serde_json::from_value::<CreateProviderModelRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.provider_service().create_model(req).await?,
            )?
        }
        "providers.models.update" => {
            let req =
                serde_json::from_value::<UpdateProviderModelRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.provider_service().update_model(req).await?,
            )?
        }
        "providers.models.delete" => {
            let req =
                serde_json::from_value::<DeleteProviderModelRequest>(envelope.payload.clone())?;
            state.provider_service().delete_model(&req.id)?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "deleted": true }),
            )?
        }
        "providers.models.test" => {
            let req = serde_json::from_value::<TestProviderModelRequest>(envelope.payload.clone())?;
            state.provider_service().test_model(req).await?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                serde_json::json!({ "ok": true }),
            )?
        }
        _ => return Ok(None),
    };
    Ok(Some(response))
}
