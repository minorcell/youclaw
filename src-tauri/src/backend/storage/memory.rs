use super::*;

impl StorageService {
    pub fn rebuild_memory_chunks(
        &self,
        chunks: &[MemoryChunkInput],
        files_indexed: u32,
    ) -> AppResult<MemoryReindexPayload> {
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM memory_chunks", [])?;
        tx.execute("DELETE FROM memory_chunks_fts", [])?;

        for chunk in chunks {
            tx.execute(
                "INSERT INTO memory_chunks (
                    id, path, line_start, line_end, heading, content, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    chunk.id,
                    chunk.path,
                    chunk.line_start as i64,
                    chunk.line_end as i64,
                    chunk.heading,
                    chunk.content,
                    now_timestamp(),
                ],
            )?;
            tx.execute(
                "INSERT INTO memory_chunks_fts (id, path, heading, content)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    chunk.id,
                    chunk.path,
                    chunk.heading.as_deref().unwrap_or_default(),
                    chunk.content
                ],
            )?;
        }

        tx.commit()?;

        Ok(MemoryReindexPayload {
            indexed_chunks: chunks.len() as u32,
            files_indexed,
        })
    }

    pub fn memory_search(
        &self,
        query: &str,
        max_results: u32,
        min_score: f32,
    ) -> AppResult<MemorySearchPayload> {
        let normalized_query = build_fts_query(query)?;
        let max_results = max_results.clamp(1, 100);
        let min_score = min_score.clamp(0.0, 1.0);

        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                c.path,
                c.line_start,
                c.line_end,
                snippet(memory_chunks_fts, 3, '[', ']', '...', 24) AS snippet,
                bm25(memory_chunks_fts) AS rank
             FROM memory_chunks_fts
             JOIN memory_chunks c ON c.id = memory_chunks_fts.id
             WHERE memory_chunks_fts MATCH ?1
             ORDER BY rank ASC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![normalized_query, max_results as i64], |row| {
            let rank = row.get::<_, f64>(4).unwrap_or(1000.0).abs() as f32;
            let score = 1.0 / (1.0 + rank);
            Ok(MemorySearchHit {
                path: row.get(0)?,
                line_start: row.get::<_, i64>(1)?.max(0) as u32,
                line_end: row.get::<_, i64>(2)?.max(0) as u32,
                snippet: row.get::<_, String>(3).unwrap_or_default(),
                score,
            })
        })?;

        let mut hits = Vec::new();
        for row in rows {
            let hit = row?;
            if hit.score >= min_score {
                hits.push(hit);
            }
        }

        Ok(MemorySearchPayload {
            query: query.to_string(),
            hits,
        })
    }
}

pub(super) fn build_fts_query(query: &str) -> AppResult<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "memory query cannot be empty".to_string(),
        ));
    }

    let mut terms = trimmed
        .split_whitespace()
        .filter_map(|term| {
            let cleaned = term
                .trim_matches(|ch: char| ch.is_ascii_punctuation())
                .trim_matches('"')
                .trim();
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned.to_string())
            }
        })
        .collect::<Vec<_>>();

    if terms.is_empty() {
        terms.push(trimmed.replace('"', ""));
    }

    let query = terms
        .into_iter()
        .map(|term| {
            let escaped = term.replace('"', "\"\"");
            format!("\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(" AND ");

    Ok(query)
}
