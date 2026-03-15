use super::super::*;
use crate::backend::memory_manager::is_memory_related_workspace_path;

impl BackendState {
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
        if is_memory_related_workspace_path(&req.path) {
            let changed = vec![req.path.clone()];
            let _ = self.sync_memory_paths(&changed);
        }
        Ok(WorkspaceFileWritePayload {
            path: req.path,
            written: true,
        })
    }
}
