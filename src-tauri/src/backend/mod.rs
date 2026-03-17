pub mod agents;
pub mod errors;
pub mod models;
pub mod providers;
pub(crate) mod services;
pub mod storage;
pub mod ws;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::{broadcast, oneshot};
use tokio_util::sync::CancellationToken;

use agents::workspace::AgentWorkspace;
pub use errors::{AppError, AppResult};
use models::*;
use services::{
    AgentRuntimeService, MemoryService, ProviderService, SessionService, WorkspaceService,
};
pub use storage::StorageService;

#[derive(Clone)]
pub struct WsHub {
    sender: broadcast::Sender<WsEnvelope>,
}

impl WsHub {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(512);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsEnvelope> {
        self.sender.subscribe()
    }

    pub fn emit(&self, name: &str, payload: impl Serialize) -> AppResult<()> {
        let envelope = WsEnvelope::event(name, payload)?;
        let _ = self.sender.send(envelope);
        Ok(())
    }

    pub fn emit_turn_event(
        &self,
        turn_id: &str,
        name: &str,
        payload: impl Serialize,
    ) -> AppResult<()> {
        let envelope = WsEnvelope::event_for_turn(turn_id, name, payload)?;
        let _ = self.sender.send(envelope);
        Ok(())
    }
}

#[derive(Clone)]
pub struct ApprovalService {
    storage: StorageService,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
}

impl ApprovalService {
    pub fn new(storage: StorageService) -> Self {
        Self {
            storage,
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register_pending(&self, approval: ToolApproval) -> AppResult<oneshot::Receiver<bool>> {
        self.storage.insert_approval(&approval)?;
        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| AppError::Storage("approval lock poisoned".to_string()))?
            .insert(approval.id.clone(), sender);
        Ok(receiver)
    }

    pub fn resolve(&self, approval_id: &str, approved: bool) -> AppResult<ToolApproval> {
        let status = if approved { "approved" } else { "rejected" };
        let approval = self.storage.update_approval_status(approval_id, status)?;
        if let Some(sender) = self
            .pending
            .lock()
            .map_err(|_| AppError::Storage("approval lock poisoned".to_string()))?
            .remove(approval_id)
        {
            let _ = sender.send(approved);
        }
        Ok(approval)
    }

    pub fn mark_status(&self, approval_id: &str, status: &str) -> AppResult<ToolApproval> {
        self.pending
            .lock()
            .map_err(|_| AppError::Storage("approval lock poisoned".to_string()))?
            .remove(approval_id);
        self.storage.update_approval_status(approval_id, status)
    }
}

#[derive(Clone)]
pub struct BackendState {
    pub(crate) storage: StorageService,
    pub(crate) workspace: AgentWorkspace,
    pub(crate) ws_hub: WsHub,
    pub(crate) approvals: ApprovalService,
    active_turns: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl BackendState {
    pub fn new(base_dir: PathBuf) -> AppResult<Self> {
        let storage = StorageService::new(base_dir)?;
        let workspace = AgentWorkspace::new(storage.base_dir());
        workspace.ensure_layout()?;
        let config = storage.get_agent_config()?;
        workspace.install_templates(&config.language, true)?;
        let ws_hub = WsHub::new();
        let approvals = ApprovalService::new(storage.clone());
        let state = Self {
            storage,
            workspace,
            ws_hub,
            approvals,
            active_turns: Arc::new(Mutex::new(HashMap::new())),
        };
        let _ = state.memory_service().reindex();
        Ok(state)
    }

    pub(crate) fn runtime_service(&self) -> AgentRuntimeService {
        AgentRuntimeService::new(self.storage.clone(), self.workspace.clone())
    }

    pub(crate) fn provider_service(&self) -> ProviderService {
        ProviderService::new(self.storage.clone(), self.ws_hub.clone())
    }

    pub(crate) fn session_service(&self) -> SessionService {
        SessionService::new(
            self.storage.clone(),
            self.ws_hub.clone(),
            self.provider_service(),
        )
    }

    pub(crate) fn memory_service(&self) -> MemoryService {
        MemoryService::new(self.storage.clone(), self.workspace.root().to_path_buf())
    }

    pub(crate) fn workspace_service(&self) -> WorkspaceService {
        WorkspaceService::new(self.workspace.clone(), self.memory_service())
    }

    pub fn register_turn(&self, turn_id: String) {
        if let Ok(mut turns) = self.active_turns.lock() {
            turns.insert(turn_id, CancellationToken::new());
        }
    }

    pub fn unregister_turn(&self, turn_id: &str) {
        if let Ok(mut turns) = self.active_turns.lock() {
            turns.remove(turn_id);
        }
    }

    pub fn get_turn_token(&self, turn_id: &str) -> Option<CancellationToken> {
        self.active_turns
            .lock()
            .ok()
            .and_then(|turns| turns.get(turn_id).cloned())
    }

    pub fn cancel_turn(&self, turn_id: &str) -> AppResult<bool> {
        let maybe = self
            .active_turns
            .lock()
            .map_err(|_| AppError::Storage("turn token lock poisoned".to_string()))?
            .get(turn_id)
            .cloned();
        if let Some(token) = maybe {
            token.cancel();
            return Ok(true);
        }
        Ok(false)
    }
}
