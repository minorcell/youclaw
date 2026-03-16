use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::now_timestamp;

const MIN_CONTEXT_WINDOW_TOKENS: u32 = 75_000;
const MAX_CONTEXT_WINDOW_TOKENS: u32 = 200_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub id: String,
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model_name: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModel {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<ProviderModel>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProviderRequest {
    pub profile_name: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProviderRequest {
    pub id: String,
    pub profile_name: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProviderModelRequest {
    pub provider_id: String,
    pub model_name: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProviderModelRequest {
    pub id: String,
    pub model_name: String,
    pub model: String,
    // `None` => keep unchanged; `Some(None)` => clear; `Some(Some(v))` => set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<Option<u32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProviderModelRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestProviderModelRequest {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredProviders {
    #[serde(default)]
    pub accounts: Vec<ProviderAccount>,
}

pub fn flatten_provider_profiles(accounts: &[ProviderAccount]) -> Vec<ProviderProfile> {
    let mut profiles = Vec::new();
    for account in accounts {
        for model in &account.models {
            let model_name = if model.name.trim().is_empty() {
                model.model.clone()
            } else {
                model.name.clone()
            };
            profiles.push(ProviderProfile {
                id: model.id.clone(),
                provider_id: account.id.clone(),
                model_name,
                name: account.name.clone(),
                base_url: account.base_url.clone(),
                api_key: account.api_key.clone(),
                model: model.model.clone(),
                context_window_tokens: model.context_window_tokens,
                created_at: model.created_at.clone(),
                updated_at: model.updated_at.clone(),
            });
        }
    }
    profiles.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    profiles
}

pub fn new_provider_account(req: CreateProviderRequest) -> ProviderAccount {
    let now = now_timestamp();
    ProviderAccount {
        id: Uuid::new_v4().to_string(),
        name: req.profile_name,
        base_url: req.base_url,
        api_key: req.api_key,
        models: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    }
}

pub fn update_provider_account(
    existing: &ProviderAccount,
    req: UpdateProviderRequest,
) -> ProviderAccount {
    ProviderAccount {
        id: existing.id.clone(),
        name: req.profile_name,
        base_url: req.base_url,
        api_key: req.api_key,
        models: existing.models.clone(),
        created_at: existing.created_at.clone(),
        updated_at: now_timestamp(),
    }
}

pub fn new_provider_model(req: CreateProviderModelRequest) -> ProviderModel {
    let now = now_timestamp();
    ProviderModel {
        id: Uuid::new_v4().to_string(),
        provider_id: req.provider_id,
        name: req.model_name,
        model: req.model,
        context_window_tokens: normalize_context_window_tokens(req.context_window_tokens),
        created_at: now.clone(),
        updated_at: now,
    }
}

pub fn update_provider_model(
    existing: &ProviderModel,
    req: UpdateProviderModelRequest,
) -> ProviderModel {
    let UpdateProviderModelRequest {
        id: _,
        model_name,
        model,
        context_window_tokens,
    } = req;
    let context_window_tokens = context_window_tokens
        .map(normalize_context_window_tokens)
        .unwrap_or(existing.context_window_tokens);
    ProviderModel {
        id: existing.id.clone(),
        provider_id: existing.provider_id.clone(),
        name: model_name,
        model,
        context_window_tokens,
        created_at: existing.created_at.clone(),
        updated_at: now_timestamp(),
    }
}

fn normalize_context_window_tokens(value: Option<u32>) -> Option<u32> {
    let value = value?;
    if value == 0 {
        return None;
    }
    Some(value.clamp(MIN_CONTEXT_WINDOW_TOKENS, MAX_CONTEXT_WINDOW_TOKENS))
}
