mod agent_runtime;
mod memory;
mod provider;
mod session;
mod workspace;

use crate::backend::models::domain::flatten_provider_profiles;
use crate::backend::models::responses::ProvidersChangedPayload;
use crate::backend::{AppResult, StorageService, WsHub};

pub(crate) use agent_runtime::AgentRuntimeService;
pub(crate) use memory::MemoryService;
pub(crate) use provider::ProviderService;
pub(crate) use session::SessionService;
pub(crate) use workspace::WorkspaceService;

fn list_provider_snapshot(storage: &StorageService) -> AppResult<ProvidersChangedPayload> {
    let provider_accounts = storage.list_provider_accounts()?;
    let provider_profiles = flatten_provider_profiles(&provider_accounts);
    Ok(ProvidersChangedPayload {
        provider_profiles,
        provider_accounts,
    })
}

fn publish_providers_changed(storage: &StorageService, hub: &WsHub) -> AppResult<()> {
    hub.emit("providers.changed", list_provider_snapshot(storage)?)
}

fn publish_sessions_changed(storage: &StorageService, hub: &WsHub) -> AppResult<()> {
    hub.emit("sessions.changed", storage.sessions_payload()?)
}
