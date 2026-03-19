use crate::backend::models::domain::MemoryRecord;
use crate::backend::models::requests::{
    MemorySystemDeleteRequest, MemorySystemGetRequest, MemorySystemListRequest,
    MemorySystemSearchRequest, MemorySystemUpsertRequest,
};
use crate::backend::models::responses::{
    MemorySystemDeletePayload, MemorySystemGetPayload, MemorySystemListPayload,
    MemorySystemSearchPayload, MemorySystemWritePayload,
};
use crate::backend::{AppError, AppResult, StorageService};

const DEFAULT_MAX_RESULTS: u32 = 6;
const DEFAULT_MIN_SCORE: f32 = 0.35;
const MAX_TITLE_CHARS: usize = 120;
const MAX_CONTENT_CHARS: usize = 24_000;

#[derive(Clone)]
pub(crate) struct MemoryService {
    storage: StorageService,
}

impl MemoryService {
    pub fn new(storage: StorageService) -> Self {
        Self { storage }
    }

    pub fn list(&self, req: MemorySystemListRequest) -> AppResult<MemorySystemListPayload> {
        Ok(MemorySystemListPayload {
            entries: self.storage.list_memory_entries(req.limit)?,
        })
    }

    pub fn search(&self, req: MemorySystemSearchRequest) -> AppResult<MemorySystemSearchPayload> {
        let query = req.query.trim();
        if query.is_empty() {
            return Err(AppError::Validation(
                "memory query cannot be empty".to_string(),
            ));
        }
        Ok(MemorySystemSearchPayload {
            results: self.storage.search_memory_entries(
                query,
                req.max_results.unwrap_or(DEFAULT_MAX_RESULTS),
                req.min_score.unwrap_or(DEFAULT_MIN_SCORE),
            )?,
        })
    }

    pub fn get(&self, req: MemorySystemGetRequest) -> AppResult<MemorySystemGetPayload> {
        Ok(MemorySystemGetPayload {
            entry: self.storage.get_memory_entry(req.id.trim())?,
        })
    }

    pub fn upsert(&self, req: MemorySystemUpsertRequest) -> AppResult<MemorySystemWritePayload> {
        let title = normalize_memory_title(&req.title)?;
        let content = normalize_memory_content(&req.content)?;
        let id = req.id.as_deref().map(str::trim).filter(|value| !value.is_empty());
        let (entry, created) = self.storage.upsert_memory_entry(id, &title, &content)?;
        Ok(MemorySystemWritePayload { entry, created })
    }

    pub fn update_existing(
        &self,
        id: &str,
        title: &str,
        content: &str,
    ) -> AppResult<MemorySystemWritePayload> {
        let entry_id = id.trim();
        if entry_id.is_empty() {
            return Err(AppError::Validation("memory id cannot be empty".to_string()));
        }
        let _existing: MemoryRecord = self.storage.get_memory_entry(entry_id)?;
        self.upsert(MemorySystemUpsertRequest {
            id: Some(entry_id.to_string()),
            title: title.to_string(),
            content: content.to_string(),
        })
    }

    pub fn delete(&self, req: MemorySystemDeleteRequest) -> AppResult<MemorySystemDeletePayload> {
        Ok(MemorySystemDeletePayload {
            deleted: self.storage.delete_memory_entry(req.id.trim())?,
        })
    }
}

fn normalize_memory_title(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("memory title cannot be empty".to_string()));
    }
    Ok(trimmed.chars().take(MAX_TITLE_CHARS).collect::<String>())
}

fn normalize_memory_content(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "memory content cannot be empty".to_string(),
        ));
    }
    Ok(trimmed.chars().take(MAX_CONTENT_CHARS).collect::<String>())
}
