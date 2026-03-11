use super::super::*;
use std::fs;

impl BackendState {
    pub fn memory_search(&self, req: MemorySearchRequest) -> AppResult<MemorySearchPayload> {
        self.storage.memory_search(
            req.query.trim(),
            req.max_results.unwrap_or(8),
            req.min_score.unwrap_or(0.05),
        )
    }

    pub fn memory_get(&self, req: MemoryGetRequest) -> AppResult<MemoryGetPayload> {
        let full = self.workspace.read_memory_file(&req.path)?;
        let lines = full.lines().collect::<Vec<_>>();
        let total_lines = lines.len() as u32;
        let offset = req.offset.unwrap_or(0) as usize;
        let limit = req.limit.unwrap_or(120).clamp(1, 1000) as usize;
        let start = offset.min(lines.len());
        let end = start.saturating_add(limit).min(lines.len());
        let content = lines[start..end].join("\n");

        Ok(MemoryGetPayload {
            path: req.path,
            line_start: start as u32 + 1,
            line_end: end as u32,
            total_lines,
            content,
        })
    }

    pub fn reindex_memory(&self) -> AppResult<MemoryReindexPayload> {
        let files = self.workspace.collect_memory_source_files()?;
        let mut chunks = Vec::<MemoryChunkInput>::new();

        for path in &files {
            let content = fs::read_to_string(path)?;
            let relative_path = self.workspace.relative_path(path)?;
            chunks.extend(chunk_markdown_memory_file(&relative_path, &content));
        }

        self.storage
            .rebuild_memory_chunks(&chunks, files.len() as u32)
    }
}

fn chunk_markdown_memory_file(path: &str, content: &str) -> Vec<MemoryChunkInput> {
    let lines = content.lines().collect::<Vec<_>>();
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
    for (heading, section_start, section_end) in sections {
        let mut cursor = section_start;
        while cursor < section_end {
            let chunk_end = cursor.saturating_add(MEMORY_CHUNK_WINDOW).min(section_end);
            let body = lines[cursor..chunk_end].join("\n");
            if !body.trim().is_empty() {
                let line_start = cursor as u32 + 1;
                let line_end = chunk_end as u32;
                chunks.push(MemoryChunkInput {
                    id: format!("{path}:{line_start}:{line_end}"),
                    path: path.to_string(),
                    line_start,
                    line_end,
                    heading: heading.clone().filter(|item| !item.is_empty()),
                    content: body,
                });
            }
            cursor = chunk_end;
        }
    }

    chunks
}
