use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::domain::now_timestamp;
use crate::backend::models::responses::{
    MemoryGetPayload, MemoryReindexPayload, MemorySearchPayload,
};
use crate::backend::storage::{MemoryChunkInput, MemorySourceFileInput, StorageService};

const MEMORY_CHUNK_TOKENS: usize = 400;
const MEMORY_CHUNK_OVERLAP: usize = 80;
const DEFAULT_MAX_RESULTS: u32 = 6;
const DEFAULT_MIN_SCORE: f32 = 0.35;
const DEFAULT_READ_LINES: u32 = 120;

#[derive(Debug, Clone)]
pub struct MemoryManagerStatus {
    pub provider: String,
    pub mode: String,
}

pub trait MemorySearchManager {
    fn search(
        &self,
        query: &str,
        max_results: Option<u32>,
        min_score: Option<f32>,
    ) -> AppResult<MemorySearchPayload>;
    fn read_file(
        &self,
        path: &str,
        from: Option<u32>,
        lines: Option<u32>,
    ) -> AppResult<MemoryGetPayload>;
    fn sync(
        &self,
        force: bool,
        changed_paths: Option<&[String]>,
    ) -> AppResult<MemoryReindexPayload>;
    fn status(&self) -> MemoryManagerStatus;
}

#[derive(Clone)]
pub struct BuiltinFtsMemoryManager {
    storage: StorageService,
    workspace_root: PathBuf,
}

#[derive(Debug, Clone)]
struct MemoryFileSnapshot {
    path: String,
    content: String,
    file_hash: String,
    file_size: u64,
    mtime_ms: i64,
    source: String,
}

impl BuiltinFtsMemoryManager {
    pub fn new(storage: StorageService, workspace_root: PathBuf) -> Self {
        Self {
            storage,
            workspace_root,
        }
    }
}

impl MemorySearchManager for BuiltinFtsMemoryManager {
    fn search(
        &self,
        query: &str,
        max_results: Option<u32>,
        min_score: Option<f32>,
    ) -> AppResult<MemorySearchPayload> {
        let query = query.trim();
        if query.is_empty() {
            return Err(AppError::Validation(
                "memory query cannot be empty".to_string(),
            ));
        }

        let max_results = max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        let min_score = min_score.unwrap_or(DEFAULT_MIN_SCORE);
        let status = self.status();

        match self.storage.memory_search(query, max_results, min_score) {
            Ok(results) => Ok(MemorySearchPayload {
                results,
                provider: status.provider,
                mode: status.mode,
                disabled: None,
                unavailable: None,
                error: None,
                warning: None,
                action: None,
            }),
            Err(err) => Ok(unavailable_search_payload(err.to_string(), self.status())),
        }
    }

    fn read_file(
        &self,
        path: &str,
        from: Option<u32>,
        lines: Option<u32>,
    ) -> AppResult<MemoryGetPayload> {
        let resolved = path.trim().to_string();
        let full_path = resolve_memory_path(&self.workspace_root, &resolved)?;
        let text = match fs::read_to_string(&full_path) {
            Ok(value) => value,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(MemoryGetPayload {
                    path: resolved,
                    text: String::new(),
                    disabled: None,
                    error: None,
                });
            }
            Err(err) => {
                return Ok(MemoryGetPayload {
                    path: resolved,
                    text: String::new(),
                    disabled: Some(true),
                    error: Some(err.to_string()),
                });
            }
        };

        let line_start = from.unwrap_or(0) as usize;
        let line_count = lines.unwrap_or(DEFAULT_READ_LINES).clamp(1, 2000) as usize;
        let body = slice_lines(&text, line_start, line_count);
        Ok(MemoryGetPayload {
            path: resolved,
            text: body,
            disabled: None,
            error: None,
        })
    }

    fn sync(
        &self,
        force: bool,
        changed_paths: Option<&[String]>,
    ) -> AppResult<MemoryReindexPayload> {
        let snapshots = collect_memory_source_snapshots(&self.workspace_root)?;
        let existing = self.storage.list_memory_source_files()?;
        let changed_filter = normalize_changed_paths(changed_paths, &self.workspace_root)?;

        let tracked_paths = snapshots
            .iter()
            .map(|item| item.path.clone())
            .collect::<HashSet<_>>();
        let scanned = if let Some(filter) = changed_filter.as_ref() {
            filter
                .iter()
                .filter(|path| tracked_paths.contains(path.as_str()))
                .count() as u32
        } else {
            snapshots.len() as u32
        };

        let mut updated_files = Vec::new();
        let mut updated_chunks = Vec::new();

        for snapshot in &snapshots {
            if let Some(filter) = changed_filter.as_ref() {
                if !filter.contains(snapshot.path.as_str()) {
                    continue;
                }
            }

            let changed = force
                || existing
                    .get(snapshot.path.as_str())
                    .map(|item| {
                        item.file_hash != snapshot.file_hash
                            || item.file_size != snapshot.file_size
                            || item.mtime_ms != snapshot.mtime_ms
                    })
                    .unwrap_or(true);
            if !changed {
                continue;
            }

            updated_files.push(MemorySourceFileInput {
                path: snapshot.path.clone(),
                file_hash: snapshot.file_hash.clone(),
                file_size: snapshot.file_size,
                mtime_ms: snapshot.mtime_ms,
                indexed_at: now_timestamp(),
                source: snapshot.source.clone(),
            });
            updated_chunks.extend(chunk_markdown_memory_file(snapshot));
        }

        let mut deleted_paths = Vec::new();
        for (path, previous) in &existing {
            let should_consider = changed_filter
                .as_ref()
                .map(|filter| filter.contains(path.as_str()))
                .unwrap_or(true);
            if !should_consider {
                continue;
            }
            if !tracked_paths.contains(path.as_str()) && previous.source == "memory" {
                deleted_paths.push(path.to_string());
            }
        }

        self.storage
            .sync_memory_chunks(&updated_files, &deleted_paths, &updated_chunks, scanned)
    }

    fn status(&self) -> MemoryManagerStatus {
        MemoryManagerStatus {
            provider: "builtin".to_string(),
            mode: "fts".to_string(),
        }
    }
}

pub fn is_memory_related_workspace_path(path: &str) -> bool {
    let trimmed = path.trim();
    trimmed == "MEMORY.md" || trimmed.starts_with("memory/")
}

pub fn resolve_relative_memory_path_from_absolute(
    absolute_path: &Path,
    workspace_root: &Path,
) -> Option<String> {
    let rel = absolute_path.strip_prefix(workspace_root).ok()?;
    let rel = normalize_rel_path_lossy(rel)?;
    if is_allowed_memory_path(Path::new(rel.as_str())) {
        Some(rel)
    } else {
        None
    }
}

fn unavailable_search_payload(message: String, status: MemoryManagerStatus) -> MemorySearchPayload {
    let lower = message.to_lowercase();
    let is_quota_error = lower.contains("quota") || lower.contains("429");
    MemorySearchPayload {
        results: Vec::new(),
        provider: status.provider,
        mode: status.mode,
        disabled: Some(true),
        unavailable: Some(true),
        error: Some(message),
        warning: Some(if is_quota_error {
            "Memory search is unavailable because backend quota is exhausted.".to_string()
        } else {
            "Memory search is unavailable due to backend error.".to_string()
        }),
        action: Some(if is_quota_error {
            "Retry after quota recovers, then run memory_search again.".to_string()
        } else {
            "Check memory backend and retry memory_search.".to_string()
        }),
    }
}

fn collect_memory_source_snapshots(workspace_root: &Path) -> AppResult<Vec<MemoryFileSnapshot>> {
    let mut files = Vec::new();

    for top in ["MEMORY.md"] {
        let path = workspace_root.join(top);
        if path.is_file() {
            files.push(path);
        }
    }

    let memory_dir = workspace_root.join("memory");
    if memory_dir.is_dir() {
        let mut entries = fs::read_dir(&memory_dir)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
            .collect::<Vec<_>>();
        entries.sort();
        files.extend(entries);
    }

    files.sort();

    let mut snapshots = Vec::new();
    let mut seen = HashSet::new();
    for file in files {
        let content = fs::read_to_string(&file)?;
        let rel = normalize_relative_from_workspace(workspace_root, &file)?;
        if !seen.insert(rel.clone()) {
            continue;
        }
        let metadata = fs::metadata(&file)?;
        let mtime_ms = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis() as i64)
            .unwrap_or_default();
        let file_hash = sha256_hex(&content);
        snapshots.push(MemoryFileSnapshot {
            path: rel,
            content,
            file_hash,
            file_size: metadata.len(),
            mtime_ms,
            source: "memory".to_string(),
        });
    }
    Ok(snapshots)
}

fn normalize_changed_paths(
    changed_paths: Option<&[String]>,
    workspace_root: &Path,
) -> AppResult<Option<HashSet<String>>> {
    let Some(paths) = changed_paths else {
        return Ok(None);
    };
    if paths.is_empty() {
        return Ok(Some(HashSet::new()));
    }

    let mut normalized = HashSet::new();
    for raw in paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let candidate = if Path::new(trimmed).is_absolute() {
            match resolve_relative_memory_path_from_absolute(Path::new(trimmed), workspace_root) {
                Some(rel) => rel,
                None => continue,
            }
        } else {
            let rel = normalize_rel_path(Path::new(trimmed))?;
            if !is_allowed_memory_path(&rel) {
                continue;
            }
            rel.to_string_lossy().replace('\\', "/")
        };
        normalized.insert(candidate);
    }
    Ok(Some(normalized))
}

fn chunk_markdown_memory_file(snapshot: &MemoryFileSnapshot) -> Vec<MemoryChunkInput> {
    let lines = snapshot.content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Vec::new();
    }

    let heading_positions = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                Some((index, trimmed.trim_start_matches('#').trim().to_string()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let mut sections = Vec::<(Option<String>, usize, usize)>::new();
    if heading_positions.is_empty() {
        sections.push((None, 0, lines.len()));
    } else {
        for (index, (start, heading)) in heading_positions.iter().enumerate() {
            let end = heading_positions
                .get(index + 1)
                .map(|entry| entry.0)
                .unwrap_or(lines.len());
            sections.push((Some(heading.clone()), *start, end));
        }
    }

    let mut chunks = Vec::new();
    let mut chunk_index = 0usize;
    for (heading, section_start, section_end) in sections {
        let mut cursor = section_start;
        while cursor < section_end {
            let mut chunk_end = cursor;
            let mut token_count = 0usize;
            while chunk_end < section_end {
                let next = estimate_line_tokens(lines[chunk_end]);
                if chunk_end > cursor && token_count + next > MEMORY_CHUNK_TOKENS {
                    break;
                }
                token_count += next;
                chunk_end += 1;
                if token_count >= MEMORY_CHUNK_TOKENS {
                    break;
                }
            }
            if chunk_end <= cursor {
                chunk_end = (cursor + 1).min(section_end);
            }

            let body = lines[cursor..chunk_end].join("\n");
            if !body.trim().is_empty() {
                let line_start = cursor as u32 + 1;
                let line_end = chunk_end as u32;
                let hash_short = &snapshot.file_hash.chars().take(12).collect::<String>();
                chunks.push(MemoryChunkInput {
                    id: format!(
                        "{}:{}:{}:{}:{}",
                        snapshot.path, line_start, line_end, hash_short, chunk_index
                    ),
                    path: snapshot.path.clone(),
                    line_start,
                    line_end,
                    heading: heading.clone().filter(|item| !item.is_empty()),
                    content: body,
                    file_hash: snapshot.file_hash.clone(),
                    source: snapshot.source.clone(),
                });
                chunk_index += 1;
            }

            if chunk_end >= section_end {
                break;
            }
            cursor = compute_overlap_start(&lines, cursor, chunk_end);
        }
    }
    chunks
}

fn compute_overlap_start(lines: &[&str], current_start: usize, chunk_end: usize) -> usize {
    let mut cursor = chunk_end;
    let mut overlap_tokens = 0usize;
    while cursor > current_start {
        let line_tokens = estimate_line_tokens(lines[cursor - 1]);
        if overlap_tokens > 0 && overlap_tokens + line_tokens > MEMORY_CHUNK_OVERLAP {
            break;
        }
        overlap_tokens += line_tokens;
        cursor -= 1;
        if overlap_tokens >= MEMORY_CHUNK_OVERLAP {
            break;
        }
    }
    if cursor <= current_start {
        chunk_end
            .saturating_sub(1)
            .max(current_start.saturating_add(1))
    } else {
        cursor
    }
}

fn estimate_line_tokens(line: &str) -> usize {
    let chars = line.chars().count();
    ((chars + 3) / 4).max(1)
}

fn slice_lines(content: &str, from: usize, lines: usize) -> String {
    let collected = content.lines().collect::<Vec<_>>();
    if collected.is_empty() {
        return String::new();
    }
    let start = from.min(collected.len());
    let end = start.saturating_add(lines).min(collected.len());
    collected[start..end].join("\n")
}

fn resolve_memory_path(workspace_root: &Path, relative_path: &str) -> AppResult<PathBuf> {
    let normalized = normalize_rel_path(Path::new(relative_path))?;
    if !is_allowed_memory_path(&normalized) {
        return Err(AppError::Validation(
            "memory path is not allowed".to_string(),
        ));
    }
    Ok(workspace_root.join(normalized))
}

fn normalize_relative_from_workspace(workspace_root: &Path, path: &Path) -> AppResult<String> {
    let canonical_root = workspace_root.canonicalize()?;
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&canonical_root) {
        return Err(AppError::Validation(
            "path is outside workspace".to_string(),
        ));
    }
    let rel = canonical
        .strip_prefix(&canonical_root)
        .map_err(|_| AppError::Validation("failed to resolve relative path".to_string()))?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn normalize_rel_path_lossy(path: &Path) -> Option<String> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if normalized.as_os_str().is_empty() {
        return None;
    }
    Some(normalized.to_string_lossy().replace('\\', "/"))
}

fn normalize_rel_path(path: &Path) -> AppResult<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::Validation(
                    "parent/root path components are not allowed".to_string(),
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(AppError::Validation("path is empty".to_string()));
    }
    Ok(normalized)
}

fn is_allowed_memory_path(path: &Path) -> bool {
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return false;
    }
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    match components.as_slice() {
        [top] => top == "MEMORY.md",
        [dir, _file] if dir == "memory" => true,
        _ => false,
    }
}

fn sha256_hex(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::backend::storage::StorageService;

    use super::{
        chunk_markdown_memory_file, compute_overlap_start, BuiltinFtsMemoryManager,
        MemoryFileSnapshot, MemorySearchManager,
    };

    #[test]
    fn chunker_respects_overlap_progress() {
        let lines = vec!["a"; 100];
        let start = compute_overlap_start(&lines, 0, 10);
        assert!(start < 10);
    }

    #[test]
    fn chunker_emits_heading_chunks() {
        let snapshot = MemoryFileSnapshot {
            path: "memory/2026-03-15.md".to_string(),
            content: "# Title\none\ntwo".to_string(),
            file_hash: "abc".to_string(),
            file_size: 12,
            mtime_ms: 0,
            source: "memory".to_string(),
        };
        let chunks = chunk_markdown_memory_file(&snapshot);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].line_start, 1);
        assert_eq!(chunks[0].line_end, 3);
        assert_eq!(chunks[0].heading.as_deref(), Some("Title"));
    }

    #[test]
    fn sync_is_incremental_and_idempotent() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");
        let workspace_root = dir.path().join("workspace");
        fs::create_dir_all(workspace_root.join("memory")).expect("mkdir");
        fs::write(workspace_root.join("MEMORY.md"), "alpha\nbeta").expect("write memory");

        let manager = BuiltinFtsMemoryManager::new(storage, workspace_root.clone());
        let first = manager.sync(false, None).expect("first sync");
        assert_eq!(first.updated, 1);
        assert!(first.chunks_indexed > 0);

        let second = manager.sync(false, None).expect("second sync");
        assert_eq!(second.updated, 0);
        assert_eq!(second.deleted, 0);

        fs::write(workspace_root.join("MEMORY.md"), "alpha\nbeta\ngamma").expect("update memory");
        let third = manager.sync(false, None).expect("third sync");
        assert_eq!(third.updated, 1);
    }

    #[test]
    fn sync_detects_deleted_memory_files() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");
        let workspace_root = dir.path().join("workspace");
        fs::create_dir_all(workspace_root.join("memory")).expect("mkdir");
        let daily = workspace_root.join("memory/2026-03-15.md");
        fs::write(&daily, "project kickoff").expect("write daily");

        let manager = BuiltinFtsMemoryManager::new(storage, workspace_root.clone());
        let first = manager.sync(false, None).expect("first sync");
        assert_eq!(first.updated, 1);

        fs::remove_file(&daily).expect("remove daily");
        let second = manager.sync(false, None).expect("second sync");
        assert_eq!(second.deleted, 1);
    }

    #[test]
    fn memory_get_returns_empty_for_missing_file() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");
        let workspace_root = dir.path().join("workspace");
        fs::create_dir_all(workspace_root.join("memory")).expect("mkdir");
        let manager = BuiltinFtsMemoryManager::new(storage, workspace_root);

        let payload = manager
            .read_file("memory/2099-01-01.md", None, None)
            .expect("read missing");
        assert_eq!(payload.path, "memory/2099-01-01.md");
        assert!(payload.text.is_empty());
        assert!(payload.error.is_none());
    }
}
