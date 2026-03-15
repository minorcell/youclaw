use super::super::*;

use crate::backend::memory_manager::{BuiltinFtsMemoryManager, MemorySearchManager};

impl BackendState {
    fn memory_manager(&self) -> BuiltinFtsMemoryManager {
        BuiltinFtsMemoryManager::new(self.storage.clone(), self.workspace.root().to_path_buf())
    }

    pub fn memory_search(&self, req: MemorySearchRequest) -> AppResult<MemorySearchPayload> {
        self.memory_manager()
            .search(&req.query, req.max_results, req.min_score)
    }

    pub fn memory_get(&self, req: MemoryGetRequest) -> AppResult<MemoryGetPayload> {
        self.memory_manager()
            .read_file(&req.path, req.from, req.lines)
    }

    pub fn reindex_memory(&self) -> AppResult<MemoryReindexPayload> {
        self.memory_manager().sync(true, None)
    }

    pub fn sync_memory_paths(&self, changed_paths: &[String]) -> AppResult<MemoryReindexPayload> {
        self.memory_manager().sync(false, Some(changed_paths))
    }
}
