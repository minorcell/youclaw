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

    pub fn get_provider_account(&self, provider_id: &str) -> AppResult<Option<ProviderAccount>> {
        Ok(self
            .storage
            .list_provider_accounts()?
            .into_iter()
            .find(|provider| provider.id == provider_id))
    }

    pub async fn create_provider(&self, req: CreateProviderRequest) -> AppResult<ProviderAccount> {
        validate_provider_account_request(&req.profile_name, &req.base_url, &req.api_key)?;
        let account = new_provider_account(req);
        let mut accounts = self.storage.list_provider_accounts()?;
        accounts.push(account.clone());
        self.storage.save_provider_accounts(&accounts)?;
        self.publish_providers_changed()?;
        Ok(account)
    }

    pub async fn update_provider(&self, req: UpdateProviderRequest) -> AppResult<ProviderAccount> {
        validate_provider_account_request(&req.profile_name, &req.base_url, &req.api_key)?;
        let mut accounts = self.storage.list_provider_accounts()?;
        let index = accounts
            .iter()
            .position(|provider| provider.id == req.id)
            .ok_or_else(|| AppError::NotFound(format!("provider account `{}`", req.id)))?;
        let account = update_provider_account(&accounts[index], req);
        accounts[index] = account.clone();
        self.storage.save_provider_accounts(&accounts)?;
        self.publish_providers_changed()?;
        Ok(account)
    }

    pub async fn create_provider_model(
        &self,
        req: CreateProviderModelRequest,
    ) -> AppResult<ProviderModel> {
        validate_provider_model_request(&req.model_name, &req.model)?;
        let mut accounts = self.storage.list_provider_accounts()?;
        let index = accounts
            .iter()
            .position(|provider| provider.id == req.provider_id)
            .ok_or_else(|| AppError::NotFound(format!("provider account `{}`", req.provider_id)))?;
        let account = &accounts[index];
        test_provider_connection(&account.base_url, &account.api_key, &req.model).await?;
        let model = new_provider_model(req);
        accounts[index].models.push(model.clone());
        accounts[index].updated_at = now_timestamp();
        self.storage.save_provider_accounts(&accounts)?;
        self.publish_providers_changed()?;
        Ok(model)
    }

    pub async fn update_provider_model(
        &self,
        req: UpdateProviderModelRequest,
    ) -> AppResult<ProviderModel> {
        validate_provider_model_request(&req.model_name, &req.model)?;
        let mut accounts = self.storage.list_provider_accounts()?;
        let mut found: Option<(usize, usize)> = None;
        for (account_index, account) in accounts.iter().enumerate() {
            if let Some(model_index) = account.models.iter().position(|model| model.id == req.id) {
                found = Some((account_index, model_index));
                break;
            }
        }
        let (account_index, model_index) =
            found.ok_or_else(|| AppError::NotFound(format!("provider model `{}`", req.id)))?;
        let account = &accounts[account_index];
        test_provider_connection(&account.base_url, &account.api_key, &req.model).await?;
        let model = update_provider_model(&account.models[model_index], req);
        accounts[account_index].models[model_index] = model.clone();
        accounts[account_index].updated_at = now_timestamp();
        self.storage.save_provider_accounts(&accounts)?;
        self.publish_providers_changed()?;
        Ok(model)
    }

    pub fn delete_provider_model(&self, model_id: &str) -> AppResult<()> {
        let mut accounts = self.storage.list_provider_accounts()?;
        let mut found: Option<(usize, usize)> = None;
        for (account_index, account) in accounts.iter().enumerate() {
            if let Some(model_index) = account.models.iter().position(|model| model.id == model_id)
            {
                found = Some((account_index, model_index));
                break;
            }
        }
        let (account_index, model_index) =
            found.ok_or_else(|| AppError::NotFound(format!("provider model `{model_id}`")))?;
        if accounts[account_index].models.len() <= 1 {
            return Err(AppError::Validation(
                "provider account must keep at least one model".to_string(),
            ));
        }
        accounts[account_index].models.remove(model_index);
        accounts[account_index].updated_at = now_timestamp();
        self.storage.save_provider_accounts(&accounts)?;
        self.storage.clear_session_provider_binding(model_id)?;
        self.publish_providers_changed()?;
        self.publish_sessions_changed()?;
        Ok(())
    }

    pub async fn test_provider_model(&self, req: TestProviderModelRequest) -> AppResult<()> {
        if req.model.trim().is_empty() {
            return Err(AppError::Validation("model cannot be empty".to_string()));
        }
        let provider = self
            .get_provider_account(&req.provider_id)?
            .ok_or_else(|| AppError::NotFound(format!("provider account `{}`", req.provider_id)))?;
        test_provider_connection(&provider.base_url, &provider.api_key, &req.model).await?;
        Ok(())
    }

    pub fn list_provider_snapshot(&self) -> AppResult<ProvidersChangedPayload> {
        let provider_accounts = self.storage.list_provider_accounts()?;
        let provider_profiles = flatten_provider_profiles(&provider_accounts);
        Ok(ProvidersChangedPayload {
            provider_profiles,
            provider_accounts,
        })
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
        self.ws_hub
            .emit("providers.changed", self.list_provider_snapshot()?)
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

fn validate_provider_account_request(name: &str, base_url: &str, api_key: &str) -> AppResult<()> {
    if name.trim().is_empty() || base_url.trim().is_empty() || api_key.trim().is_empty() {
        return Err(AppError::Validation(
            "provider fields cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_provider_model_request(model_name: &str, model: &str) -> AppResult<()> {
    if model_name.trim().is_empty() || model.trim().is_empty() {
        return Err(AppError::Validation(
            "provider model fields cannot be empty".to_string(),
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
