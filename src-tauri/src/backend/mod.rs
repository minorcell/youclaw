pub mod agent;
pub mod agent_workspace;
pub mod agents;
pub mod errors;
pub mod models;
pub mod storage;
pub mod ws;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aquaregia::{GenerateTextRequest, LlmClient};
use serde::Serialize;
use tokio::sync::{broadcast, oneshot};
use tokio_util::sync::CancellationToken;

use agent_workspace::AgentWorkspace;
pub use errors::{AppError, AppResult};
pub use models::*;
use storage::MemoryChunkInput;
pub use storage::StorageService;

const MEMORY_CHUNK_WINDOW: usize = 24;

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
    pub workspace: AgentWorkspace,
    pub ws_hub: WsHub,
    pub approvals: ApprovalService,
    active_runs: Arc<Mutex<HashMap<String, CancellationToken>>>,
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
            active_runs: Arc::new(Mutex::new(HashMap::new())),
        };
        let _ = state.reindex_memory();
        Ok(state)
    }

    pub fn bootstrap(&self) -> AppResult<BootstrapPayload> {
        let mut payload = self.storage.load_bootstrap()?;
        payload.agent_config = self.storage.get_agent_config()?;
        payload.workspace_files = self.workspace.list_files()?;
        Ok(payload)
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

    pub fn get_agent_config(&self) -> AppResult<AgentConfigPayload> {
        self.storage.get_agent_config()
    }

    pub fn update_agent_config(
        &self,
        req: AgentConfigUpdateRequest,
    ) -> AppResult<AgentConfigPayload> {
        let updated = self.storage.update_agent_config(req)?;
        self.workspace.install_templates(&updated.language, true)?;
        Ok(updated)
    }

    pub fn list_workspace_files(&self) -> AppResult<WorkspaceFilesPayload> {
        Ok(WorkspaceFilesPayload {
            files: self.workspace.list_files()?,
        })
    }

    pub fn read_workspace_file(
        &self,
        req: WorkspaceFileReadRequest,
    ) -> AppResult<WorkspaceFileReadPayload> {
        Ok(WorkspaceFileReadPayload {
            path: req.path.clone(),
            content: self.workspace.read_workspace_file(&req.path)?,
        })
    }

    pub fn write_workspace_file(
        &self,
        req: WorkspaceFileWriteRequest,
    ) -> AppResult<WorkspaceFileWritePayload> {
        self.workspace
            .write_workspace_file(&req.path, &req.content)?;
        if is_memory_related_path(&req.path) {
            let _ = self.reindex_memory();
        }
        Ok(WorkspaceFileWritePayload {
            path: req.path,
            written: true,
        })
    }

    pub fn memory_search(&self, req: MemorySearchRequest) -> AppResult<MemorySearchPayload> {
        self.storage.memory_search(
            req.query.trim(),
            req.max_results.unwrap_or(8),
            req.min_score.unwrap_or(0.05),
        )
    }

    pub fn memory_get(&self, req: MemoryGetRequest) -> AppResult<MemoryGetPayload> {
        let full = self.workspace.read_memory_file(&req.path)?;
        let lines = full.lines().collect::<Vec<_>>();
        let total_lines = lines.len() as u32;
        let offset = req.offset.unwrap_or(0) as usize;
        let limit = req.limit.unwrap_or(120).clamp(1, 1000) as usize;
        let start = offset.min(lines.len());
        let end = start.saturating_add(limit).min(lines.len());
        let content = lines[start..end].join("\n");

        Ok(MemoryGetPayload {
            path: req.path,
            line_start: start as u32 + 1,
            line_end: end as u32,
            total_lines,
            content,
        })
    }

    pub fn reindex_memory(&self) -> AppResult<MemoryReindexPayload> {
        let files = self.workspace.collect_memory_source_files()?;
        let mut chunks = Vec::<MemoryChunkInput>::new();

        for path in &files {
            let content = fs::read_to_string(path)?;
            let relative_path = self.workspace.relative_path(path)?;
            chunks.extend(chunk_markdown_memory_file(&relative_path, &content));
        }

        self.storage
            .rebuild_memory_chunks(&chunks, files.len() as u32)
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

pub(crate) fn normalize_openai_compatible_endpoint(base_url: &str) -> (String, Option<String>) {
    const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

    let trimmed = base_url.trim().trim_end_matches('/');
    if let Some(prefix) = trimmed.strip_suffix(CHAT_COMPLETIONS_PATH) {
        if !prefix.is_empty() {
            return (prefix.to_string(), Some(CHAT_COMPLETIONS_PATH.to_string()));
        }
    }
    (trimmed.to_string(), None)
}

async fn test_provider_connection(base_url: &str, api_key: &str, model: &str) -> AppResult<()> {
    let (normalized_base_url, chat_path) = normalize_openai_compatible_endpoint(base_url);
    let builder = LlmClient::openai_compatible(normalized_base_url).api_key(api_key.to_string());
    let client = if let Some(path) = chat_path {
        builder.chat_completions_path(path).build()?
    } else {
        builder.build()?
    };
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

fn is_memory_related_path(path: &str) -> bool {
    path == "MEMORY.md" || path == "PROFILE.md" || path.starts_with("memory/")
}

fn chunk_markdown_memory_file(path: &str, content: &str) -> Vec<MemoryChunkInput> {
    let lines = content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Vec::new();
    }

    let heading_positions = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                Some((index, trimmed.trim_start_matches('#').trim().to_string()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let mut sections = Vec::<(Option<String>, usize, usize)>::new();
    if heading_positions.is_empty() {
        sections.push((None, 0, lines.len()));
    } else {
        for (index, (start, heading)) in heading_positions.iter().enumerate() {
            let end = heading_positions
                .get(index + 1)
                .map(|entry| entry.0)
                .unwrap_or(lines.len());
            sections.push((Some(heading.clone()), *start, end));
        }
    }

    let mut chunks = Vec::new();
    for (heading, section_start, section_end) in sections {
        let mut cursor = section_start;
        while cursor < section_end {
            let chunk_end = cursor.saturating_add(MEMORY_CHUNK_WINDOW).min(section_end);
            let body = lines[cursor..chunk_end].join("\n");
            if !body.trim().is_empty() {
                let line_start = cursor as u32 + 1;
                let line_end = chunk_end as u32;
                chunks.push(MemoryChunkInput {
                    id: format!("{path}:{line_start}:{line_end}"),
                    path: path.to_string(),
                    line_start,
                    line_end,
                    heading: heading.clone().filter(|item| !item.is_empty()),
                    content: body,
                });
            }
            cursor = chunk_end;
        }
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::normalize_openai_compatible_endpoint;

    #[test]
    fn normalize_endpoint_keeps_regular_base_url() {
        let (base, path) = normalize_openai_compatible_endpoint("https://api.deepseek.com");
        assert_eq!(base, "https://api.deepseek.com");
        assert!(path.is_none());
    }

    #[test]
    fn normalize_endpoint_splits_full_chat_completions_url() {
        let (base, path) = normalize_openai_compatible_endpoint(
            "https://open.bigmodel.cn/api/paas/v4/chat/completions",
        );
        assert_eq!(base, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(path.as_deref(), Some("/chat/completions"));
    }
}
