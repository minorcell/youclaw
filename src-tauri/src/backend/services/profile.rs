use crate::backend::models::domain::{AgentProfile, ProfileTarget};
use crate::backend::models::requests::{ProfileGetRequest, ProfileUpdateRequest};
use crate::backend::models::responses::{ProfileGetPayload, ProfileWritePayload};
use crate::backend::{AppError, AppResult, StorageService};

const MAX_PROFILE_CHARS: usize = 12_000;

#[derive(Clone)]
pub(crate) struct ProfileService {
    storage: StorageService,
}

impl ProfileService {
    pub fn new(storage: StorageService) -> Self {
        Self { storage }
    }

    pub fn get(&self, req: ProfileGetRequest) -> AppResult<ProfileGetPayload> {
        let all_profiles = self.storage.list_profiles()?;
        let profiles = match req.target {
            Some(target) => vec![self.storage.get_profile(target)?],
            None => all_profiles.clone(),
        };
        let missing_targets = missing_profile_targets(&all_profiles);
        Ok(ProfileGetPayload {
            profiles,
            needs_onboarding: !missing_targets.is_empty(),
            missing_targets,
        })
    }

    pub fn update(&self, req: ProfileUpdateRequest) -> AppResult<ProfileWritePayload> {
        let content = normalize_profile_content(&req.content)?;
        let profile = self.storage.upsert_profile(req.target, &content)?;
        let missing_targets = missing_profile_targets(&self.storage.list_profiles()?);
        Ok(ProfileWritePayload {
            profile,
            needs_onboarding: !missing_targets.is_empty(),
            missing_targets,
        })
    }

    pub fn list_all(&self) -> AppResult<Vec<AgentProfile>> {
        self.storage.list_profiles()
    }

    pub fn missing_targets(&self) -> AppResult<Vec<ProfileTarget>> {
        Ok(missing_profile_targets(&self.storage.list_profiles()?))
    }
}

fn normalize_profile_content(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "profile content cannot be empty".to_string(),
        ));
    }
    Ok(trimmed.chars().take(MAX_PROFILE_CHARS).collect::<String>())
}

fn missing_profile_targets(profiles: &[AgentProfile]) -> Vec<ProfileTarget> {
    ProfileTarget::ALL
        .iter()
        .copied()
        .filter(|target| {
            profiles
                .iter()
                .find(|profile| profile.target == *target)
                .map(|profile| profile.content.trim().is_empty())
                .unwrap_or(true)
        })
        .collect()
}
