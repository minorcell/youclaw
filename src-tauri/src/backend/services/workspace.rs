use crate::backend::agents::memory::is_memory_related_workspace_path;
use crate::backend::agents::workspace::AgentWorkspace;
use crate::backend::models::requests::{WorkspaceFileReadRequest, WorkspaceFileWriteRequest};
use crate::backend::models::responses::{
    WorkspaceFileReadPayload, WorkspaceFileWritePayload, WorkspaceFilesPayload,
};
use crate::backend::AppResult;

use super::MemoryService;

#[derive(Clone)]
pub(crate) struct WorkspaceService {
    workspace: AgentWorkspace,
    memory: MemoryService,
}

impl WorkspaceService {
    pub fn new(workspace: AgentWorkspace, memory: MemoryService) -> Self {
        Self { workspace, memory }
    }

    pub fn list_files(&self) -> AppResult<WorkspaceFilesPayload> {
        Ok(WorkspaceFilesPayload {
            files: self.workspace.list_files()?,
        })
    }

    pub fn read_file(&self, req: WorkspaceFileReadRequest) -> AppResult<WorkspaceFileReadPayload> {
        Ok(WorkspaceFileReadPayload {
            path: req.path.clone(),
            content: self.workspace.read_workspace_file(&req.path)?,
        })
    }

    pub fn write_file(
        &self,
        req: WorkspaceFileWriteRequest,
    ) -> AppResult<WorkspaceFileWritePayload> {
        self.workspace
            .write_workspace_file(&req.path, &req.content)?;
        if is_memory_related_workspace_path(&req.path) {
            let changed = vec![req.path.clone()];
            let _ = self.memory.sync_paths(&changed);
        }
        Ok(WorkspaceFileWritePayload {
            path: req.path,
            written: true,
        })
    }
}
