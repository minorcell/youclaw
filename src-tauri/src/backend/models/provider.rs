use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::now_timestamp;

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
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModel {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub model: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProviderModelRequest {
    pub id: String,
    pub model_name: String,
    pub model: String,
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
    #[serde(default)]
    pub profiles: Vec<LegacyProviderProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyProviderProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
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
                created_at: model.created_at.clone(),
                updated_at: model.updated_at.clone(),
            });
        }
    }
    profiles.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    profiles
}

pub fn migrate_provider_accounts_from_legacy(
    profiles: Vec<LegacyProviderProfile>,
) -> Vec<ProviderAccount> {
    let mut grouped: HashMap<(String, String, String), ProviderAccount> = HashMap::new();
    for profile in profiles {
        let key = (
            profile.name.clone(),
            profile.base_url.clone(),
            profile.api_key.clone(),
        );
        let created_at = profile.created_at.clone();
        let updated_at = profile.updated_at.clone();
        let account = grouped.entry(key).or_insert_with(|| ProviderAccount {
            id: Uuid::new_v4().to_string(),
            name: profile.name.clone(),
            base_url: profile.base_url.clone(),
            api_key: profile.api_key.clone(),
            models: Vec::new(),
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
        });
        account.models.push(ProviderModel {
            id: profile.id,
            provider_id: account.id.clone(),
            name: profile.model.clone(),
            model: profile.model,
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
        });
        if account.created_at > created_at {
            account.created_at = created_at;
        }
        if account.updated_at < updated_at {
            account.updated_at = updated_at;
        }
    }

    let mut accounts = grouped.into_values().collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    for account in &mut accounts {
        account
            .models
            .sort_by(|left, right| left.created_at.cmp(&right.created_at));
    }
    accounts
}

pub fn normalize_provider_accounts(accounts: &mut [ProviderAccount]) -> bool {
    let mut changed = false;
    for account in accounts {
        if account.id.trim().is_empty() {
            account.id = Uuid::new_v4().to_string();
            changed = true;
        }
        if account.created_at.trim().is_empty() {
            account.created_at = now_timestamp();
            changed = true;
        }
        if account.updated_at.trim().is_empty() {
            account.updated_at = account.created_at.clone();
            changed = true;
        }
        for model in &mut account.models {
            if model.id.trim().is_empty() {
                model.id = Uuid::new_v4().to_string();
                changed = true;
            }
            if model.provider_id != account.id {
                model.provider_id = account.id.clone();
                changed = true;
            }
            if model.name.trim().is_empty() {
                model.name = model.model.clone();
                changed = true;
            }
            if model.created_at.trim().is_empty() {
                model.created_at = now_timestamp();
                changed = true;
            }
            if model.updated_at.trim().is_empty() {
                model.updated_at = model.created_at.clone();
                changed = true;
            }
        }
    }
    changed
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
        created_at: now.clone(),
        updated_at: now,
    }
}

pub fn update_provider_model(
    existing: &ProviderModel,
    req: UpdateProviderModelRequest,
) -> ProviderModel {
    ProviderModel {
        id: existing.id.clone(),
        provider_id: existing.provider_id.clone(),
        name: req.model_name,
        model: req.model,
        created_at: existing.created_at.clone(),
        updated_at: now_timestamp(),
    }
}
