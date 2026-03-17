use crate::backend::errors::AppResult;
use crate::backend::models::requests::{
    UsageLogDetailRequest, UsageLogsListRequest, UsageStatsListRequest, UsageSummaryRequest,
};
use crate::backend::models::WsEnvelope;
use crate::backend::BackendState;

pub(super) fn try_handle(
    state: &BackendState,
    envelope: &WsEnvelope,
) -> AppResult<Option<WsEnvelope>> {
    let response = match envelope.name.as_str() {
        "usage.summary.get" => {
            let req = serde_json::from_value::<UsageSummaryRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.storage.usage_summary(req)?,
            )?
        }
        "usage.logs.list" => {
            let req = serde_json::from_value::<UsageLogsListRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.storage.list_usage_logs(req)?,
            )?
        }
        "usage.logs.detail" => {
            let req = serde_json::from_value::<UsageLogDetailRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.storage.usage_log_detail(req)?,
            )?
        }
        "usage.stats.providers.list" => {
            let req = serde_json::from_value::<UsageStatsListRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.storage.list_usage_provider_stats(req)?,
            )?
        }
        "usage.stats.models.list" => {
            let req = serde_json::from_value::<UsageStatsListRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.storage.list_usage_model_stats(req)?,
            )?
        }
        "usage.stats.tools.list" => {
            let req = serde_json::from_value::<UsageStatsListRequest>(envelope.payload.clone())?;
            WsEnvelope::response_ok(
                envelope.id.clone(),
                envelope.name.clone(),
                state.storage.list_usage_tool_stats(req)?,
            )?
        }
        _ => return Ok(None),
    };
    Ok(Some(response))
}
