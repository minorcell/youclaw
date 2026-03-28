use crate::backend::agents::workspace::AgentWorkspace;
use crate::backend::models::domain::AgentConfigPayload;
use crate::backend::models::requests::AgentConfigUpdateRequest;
use crate::backend::models::responses::BootstrapPayload;
use crate::backend::{AppResult, StorageService};

#[derive(Clone)]
pub(crate) struct AgentRuntimeService {
    storage: StorageService,
    workspace: AgentWorkspace,
}

impl AgentRuntimeService {
    pub fn new(storage: StorageService, workspace: AgentWorkspace) -> Self {
        Self { storage, workspace }
    }

    pub fn bootstrap(&self) -> AppResult<BootstrapPayload> {
        let mut payload = self.storage.load_bootstrap()?;
        payload.agent_config = self.storage.get_agent_config()?;
        Ok(payload)
    }

    pub fn get_agent_config(&self) -> AppResult<AgentConfigPayload> {
        self.storage.get_agent_config()
    }

    pub fn update_agent_config(
        &self,
        req: AgentConfigUpdateRequest,
    ) -> AppResult<AgentConfigPayload> {
        let updated = self.storage.update_agent_config(req)?;
        self.workspace.install_templates()?;
        Ok(updated)
    }
}
