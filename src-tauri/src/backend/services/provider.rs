use aquaregia::{GenerateTextRequest, LlmClient};

use super::{list_provider_snapshot, publish_providers_changed, publish_sessions_changed};
use crate::backend::models::domain::{
    new_provider_account, update_provider_account, update_provider_model, ProviderAccount,
    ProviderModel, ProviderProfile,
};
use crate::backend::models::requests::{
    CreateProviderModelRequest, CreateProviderRequest, TestProviderModelRequest,
    UpdateProviderModelRequest, UpdateProviderRequest,
};
use crate::backend::models::responses::ProvidersChangedPayload;
use crate::backend::providers::{
    normalize_openai_compatible_endpoint, resolve_provider_api_key, validate_provider_api_key_input,
};
use crate::backend::{now_timestamp, AppError, AppResult, StorageService, WsHub};

#[derive(Clone)]
pub(crate) struct ProviderService {
    storage: StorageService,
    hub: WsHub,
}

impl ProviderService {
    pub fn new(storage: StorageService, hub: WsHub) -> Self {
        Self { storage, hub }
    }

    pub fn get_profile(&self, provider_id: &str) -> AppResult<Option<ProviderProfile>> {
        Ok(self
            .storage
            .list_provider_profiles()?
            .into_iter()
            .find(|profile| profile.id == provider_id))
    }

    pub fn get_account(&self, provider_id: &str) -> AppResult<Option<ProviderAccount>> {
        Ok(self
            .storage
            .list_provider_accounts()?
            .into_iter()
            .find(|provider| provider.id == provider_id))
    }

    pub async fn create(&self, req: CreateProviderRequest) -> AppResult<ProviderAccount> {
        validate_provider_account_request(&req.profile_name, &req.base_url, &req.api_key)?;
        let account = new_provider_account(req);
        let mut accounts = self.storage.list_provider_accounts()?;
        accounts.push(account.clone());
        self.storage.save_provider_accounts(&accounts)?;
        self.publish_changed()?;
        Ok(account)
    }

    pub async fn update(&self, req: UpdateProviderRequest) -> AppResult<ProviderAccount> {
        validate_provider_account_request(&req.profile_name, &req.base_url, &req.api_key)?;
        let mut accounts = self.storage.list_provider_accounts()?;
        let index = accounts
            .iter()
            .position(|provider| provider.id == req.id)
            .ok_or_else(|| AppError::NotFound(format!("provider account `{}`", req.id)))?;
        let account = update_provider_account(&accounts[index], req);
        accounts[index] = account.clone();
        self.storage.save_provider_accounts(&accounts)?;
        self.publish_changed()?;
        Ok(account)
    }

    pub async fn create_model(&self, req: CreateProviderModelRequest) -> AppResult<ProviderModel> {
        validate_provider_model_request(&req.model_name, &req.model)?;
        let mut accounts = self.storage.list_provider_accounts()?;
        let index = accounts
            .iter()
            .position(|provider| provider.id == req.provider_id)
            .ok_or_else(|| AppError::NotFound(format!("provider account `{}`", req.provider_id)))?;
        let account = &accounts[index];
        test_provider_connection(&account.base_url, &account.api_key, &req.model).await?;
        let model = crate::backend::models::domain::new_provider_model(req);
        accounts[index].models.push(model.clone());
        accounts[index].updated_at = now_timestamp();
        self.storage.save_provider_accounts(&accounts)?;
        self.publish_changed()?;
        Ok(model)
    }

    pub async fn update_model(&self, req: UpdateProviderModelRequest) -> AppResult<ProviderModel> {
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
        self.publish_changed()?;
        Ok(model)
    }

    pub fn delete_model(&self, model_id: &str) -> AppResult<()> {
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
        self.publish_changed()?;
        publish_sessions_changed(&self.storage, &self.hub)?;
        Ok(())
    }

    pub async fn test_model(&self, req: TestProviderModelRequest) -> AppResult<()> {
        if req.model.trim().is_empty() {
            return Err(AppError::Validation("model cannot be empty".to_string()));
        }
        let provider = self
            .get_account(&req.provider_id)?
            .ok_or_else(|| AppError::NotFound(format!("provider account `{}`", req.provider_id)))?;
        test_provider_connection(&provider.base_url, &provider.api_key, &req.model).await?;
        Ok(())
    }

    pub fn list_snapshot(&self) -> AppResult<ProvidersChangedPayload> {
        list_provider_snapshot(&self.storage)
    }

    pub fn publish_changed(&self) -> AppResult<()> {
        publish_providers_changed(&self.storage, &self.hub)
    }
}

fn validate_provider_account_request(name: &str, base_url: &str, api_key: &str) -> AppResult<()> {
    if name.trim().is_empty() || base_url.trim().is_empty() {
        return Err(AppError::Validation(
            "provider fields cannot be empty".to_string(),
        ));
    }
    validate_provider_api_key_input(api_key)?;
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
    let resolved_api_key = resolve_provider_api_key(api_key)?;
    let (normalized_base_url, chat_path) = normalize_openai_compatible_endpoint(base_url);
    let builder = LlmClient::openai_compatible(normalized_base_url)
        .api_key(resolved_api_key)
        .think_tag_parsing(true);
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
