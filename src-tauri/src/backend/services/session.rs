use super::{publish_sessions_changed, ProviderService};
use crate::backend::models::domain::{new_chat_session, ChatSession, SessionApprovalMode};
use crate::backend::models::responses::{ArchivedSessionsPayload, SessionsChangedPayload};
use crate::backend::{AppError, AppResult, StorageService, WsHub};

const SESSION_TITLE_MAX_CHARS: usize = 48;

#[derive(Clone)]
pub(crate) struct SessionService {
    storage: StorageService,
    hub: WsHub,
    providers: ProviderService,
}

impl SessionService {
    pub fn new(storage: StorageService, hub: WsHub, providers: ProviderService) -> Self {
        Self {
            storage,
            hub,
            providers,
        }
    }

    pub fn list(&self) -> AppResult<SessionsChangedPayload> {
        self.storage.sessions_payload()
    }

    pub fn list_archived(&self) -> AppResult<ArchivedSessionsPayload> {
        self.storage.archived_sessions_payload()
    }

    pub fn create(&self, provider_profile_id: Option<String>) -> AppResult<ChatSession> {
        if let Some(profile_id) = provider_profile_id.as_deref() {
            if self.providers.get_profile(profile_id)?.is_none() {
                return Err(AppError::NotFound(format!(
                    "provider profile `{profile_id}`"
                )));
            }
        }
        if let Some(existing_session) = self.storage.find_latest_empty_session()? {
            if let Some(profile_id) = provider_profile_id.as_deref() {
                if existing_session.provider_profile_id.as_deref() != Some(profile_id) {
                    self.storage
                        .update_session_provider(&existing_session.id, profile_id)?;
                }
            }
            self.storage
                .set_last_opened_session_id(Some(&existing_session.id))?;
            self.publish_changed()?;
            return self.storage.get_session(&existing_session.id);
        }
        let session = new_chat_session(provider_profile_id);
        self.storage.insert_session(&session)?;
        self.storage.set_last_opened_session_id(Some(&session.id))?;
        self.publish_changed()?;
        Ok(session)
    }

    pub fn bind_provider(&self, session_id: &str, provider_profile_id: &str) -> AppResult<()> {
        if self.providers.get_profile(provider_profile_id)?.is_none() {
            return Err(AppError::NotFound(format!(
                "provider profile `{provider_profile_id}`"
            )));
        }
        self.storage
            .update_session_provider(session_id, provider_profile_id)?;
        self.publish_changed()?;
        Ok(())
    }

    pub fn update_approval_mode(
        &self,
        session_id: &str,
        approval_mode: SessionApprovalMode,
    ) -> AppResult<()> {
        self.storage
            .update_session_approval_mode(session_id, approval_mode)?;
        self.publish_changed()?;
        Ok(())
    }

    pub fn delete(&self, session_id: &str) -> AppResult<()> {
        self.storage.delete_session(session_id)?;
        self.publish_changed()?;
        Ok(())
    }

    pub fn restore(&self, session_id: &str) -> AppResult<()> {
        self.storage.restore_session(session_id)?;
        self.publish_changed()?;
        Ok(())
    }

    pub fn purge(&self, session_id: &str) -> AppResult<()> {
        self.storage.purge_session(session_id)?;
        self.publish_changed()?;
        Ok(())
    }

    pub fn rename(&self, session_id: &str, title: &str) -> AppResult<()> {
        let next_title = title.trim();
        if next_title.is_empty() {
            return Err(AppError::Validation(
                "session title cannot be empty".to_string(),
            ));
        }
        let normalized_title = next_title
            .chars()
            .take(SESSION_TITLE_MAX_CHARS)
            .collect::<String>();
        self.storage
            .update_session_title(session_id, normalized_title.as_str())?;
        self.publish_changed()?;
        Ok(())
    }

    pub fn publish_changed(&self) -> AppResult<()> {
        publish_sessions_changed(&self.storage, &self.hub)
    }
}
