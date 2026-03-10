pub mod agent;
pub mod errors;
pub mod filesystem;
pub mod models;
pub mod storage;
pub mod ws;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aquaregia::{GenerateTextRequest, LlmClient};
use serde::Serialize;
use tokio::sync::{broadcast, oneshot};
use tokio_util::sync::CancellationToken;

pub use errors::{AppError, AppResult};
pub use models::*;
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

    pub fn emit_run_event(
        &self,
        run_id: &str,
        name: &str,
        payload: impl Serialize,
    ) -> AppResult<()> {
        let _ = run_id;
        let payload = serde_json::to_value(payload)?;
        let envelope = WsEnvelope::event(name, payload)?;
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
    pub storage: StorageService,
    pub ws_hub: WsHub,
    pub approvals: ApprovalService,
    active_runs: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl BackendState {
    pub fn new(base_dir: PathBuf) -> AppResult<Self> {
        let storage = StorageService::new(base_dir)?;
        let ws_hub = WsHub::new();
        let approvals = ApprovalService::new(storage.clone());
        Ok(Self {
            storage,
            ws_hub,
            approvals,
            active_runs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn bootstrap(&self) -> AppResult<BootstrapPayload> {
        self.storage.load_bootstrap()
    }

    pub fn get_provider_profile(&self, provider_id: &str) -> AppResult<Option<ProviderProfile>> {
        Ok(self
            .storage
            .list_provider_profiles()?
            .into_iter()
            .find(|profile| profile.id == provider_id))
    }

    pub async fn create_provider(&self, req: CreateProviderRequest) -> AppResult<ProviderProfile> {
        validate_provider_request(&req.profile_name, &req.base_url, &req.api_key, &req.model)?;
        test_provider_connection(&req.base_url, &req.api_key, &req.model).await?;
        let profile = new_provider_profile(req);
        let mut profiles = self.storage.list_provider_profiles()?;
        profiles.push(profile.clone());
        self.storage.save_provider_profiles(&profiles)?;
        self.publish_providers_changed()?;
        Ok(profile)
    }

    pub async fn update_provider(&self, req: UpdateProviderRequest) -> AppResult<ProviderProfile> {
        validate_provider_request(&req.profile_name, &req.base_url, &req.api_key, &req.model)?;
        test_provider_connection(&req.base_url, &req.api_key, &req.model).await?;
        let mut profiles = self.storage.list_provider_profiles()?;
        let index = profiles
            .iter()
            .position(|profile| profile.id == req.id)
            .ok_or_else(|| AppError::NotFound(format!("provider profile `{}`", req.id)))?;
        let profile = update_provider_profile(&profiles[index], req);
        profiles[index] = profile.clone();
        self.storage.save_provider_profiles(&profiles)?;
        self.publish_providers_changed()?;
        Ok(profile)
    }

    pub fn create_session(&self, provider_profile_id: Option<String>) -> AppResult<ChatSession> {
        if let Some(profile_id) = provider_profile_id.as_deref() {
            if self.get_provider_profile(profile_id)?.is_none() {
                return Err(AppError::NotFound(format!(
                    "provider profile `{profile_id}`"
                )));
            }
        }
        let session = new_chat_session(provider_profile_id);
        self.storage.insert_session(&session)?;
        self.storage.set_last_opened_session_id(Some(&session.id))?;
        self.publish_sessions_changed()?;
        Ok(session)
    }

    pub fn bind_session_provider(
        &self,
        session_id: &str,
        provider_profile_id: &str,
    ) -> AppResult<()> {
        if self.get_provider_profile(provider_profile_id)?.is_none() {
            return Err(AppError::NotFound(format!(
                "provider profile `{provider_profile_id}`"
            )));
        }
        self.storage
            .update_session_provider(session_id, provider_profile_id)?;
        self.publish_sessions_changed()?;
        Ok(())
    }

    pub fn delete_session(&self, session_id: &str) -> AppResult<()> {
        self.storage.delete_session(session_id)?;
        self.publish_sessions_changed()?;
        Ok(())
    }

    pub fn publish_providers_changed(&self) -> AppResult<()> {
        self.ws_hub.emit(
            "providers.changed",
            ProvidersChangedPayload {
                provider_profiles: self.storage.list_provider_profiles()?,
            },
        )
    }

    pub fn publish_sessions_changed(&self) -> AppResult<()> {
        self.ws_hub
            .emit("sessions.changed", self.storage.sessions_payload()?)
    }

    pub fn register_run(&self, run_id: String) {
        if let Ok(mut runs) = self.active_runs.lock() {
            runs.insert(run_id, CancellationToken::new());
        }
    }

    pub fn unregister_run(&self, run_id: &str) {
        if let Ok(mut runs) = self.active_runs.lock() {
            runs.remove(run_id);
        }
    }

    pub fn get_run_token(&self, run_id: &str) -> Option<CancellationToken> {
        self.active_runs
            .lock()
            .ok()
            .and_then(|runs| runs.get(run_id).cloned())
    }

    pub fn cancel_run(&self, run_id: &str) -> AppResult<bool> {
        let maybe = self
            .active_runs
            .lock()
            .map_err(|_| AppError::Storage("run token lock poisoned".to_string()))?
            .get(run_id)
            .cloned();
        if let Some(token) = maybe {
            token.cancel();
            return Ok(true);
        }
        Ok(false)
    }
}

fn validate_provider_request(
    name: &str,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> AppResult<()> {
    if name.trim().is_empty()
        || base_url.trim().is_empty()
        || api_key.trim().is_empty()
        || model.trim().is_empty()
    {
        return Err(AppError::Validation(
            "provider fields cannot be empty".to_string(),
        ));
    }
    Ok(())
}

async fn test_provider_connection(base_url: &str, api_key: &str, model: &str) -> AppResult<()> {
    let client = LlmClient::openai_compatible(base_url.to_string())
        .api_key(api_key.to_string())
        .build()?;
    client
        .generate(
            GenerateTextRequest::builder(model.to_string())
                .user_prompt("Reply with OK.")
                .max_output_tokens(8)
                .build()?,
        )
        .await?;
    Ok(())
}
