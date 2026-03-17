use std::path::PathBuf;

use crate::backend::agents::memory::{BuiltinFtsMemoryManager, MemorySearchManager};
use crate::backend::models::requests::{MemoryGetRequest, MemorySearchRequest};
use crate::backend::models::responses::{
    MemoryGetPayload, MemoryReindexPayload, MemorySearchPayload,
};
use crate::backend::{AppResult, StorageService};

#[derive(Clone)]
pub(crate) struct MemoryService {
    storage: StorageService,
    workspace_root: PathBuf,
}

impl MemoryService {
    pub fn new(storage: StorageService, workspace_root: PathBuf) -> Self {
        Self {
            storage,
            workspace_root,
        }
    }

    fn manager(&self) -> BuiltinFtsMemoryManager {
        BuiltinFtsMemoryManager::new(self.storage.clone(), self.workspace_root.clone())
    }

    pub fn search(&self, req: MemorySearchRequest) -> AppResult<MemorySearchPayload> {
        self.manager()
            .search(&req.query, req.max_results, req.min_score)
    }

    pub fn get(&self, req: MemoryGetRequest) -> AppResult<MemoryGetPayload> {
        self.manager().read_file(&req.path, req.from, req.lines)
    }

    pub fn reindex(&self) -> AppResult<MemoryReindexPayload> {
        self.manager().sync(true, None)
    }

    pub fn sync_paths(&self, changed_paths: &[String]) -> AppResult<MemoryReindexPayload> {
        self.manager().sync(false, Some(changed_paths))
    }
}
