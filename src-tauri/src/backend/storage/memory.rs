use std::collections::HashMap;

use rusqlite::Transaction;

use super::*;

impl StorageService {
    pub fn list_memory_source_files(&self) -> AppResult<HashMap<String, MemorySourceFileRecord>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT path, file_hash, file_size, mtime_ms, source
             FROM memory_source_files",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MemorySourceFileRecord {
                path: row.get::<_, String>(0)?,
                file_hash: row.get::<_, String>(1)?,
                file_size: row.get::<_, i64>(2)?.max(0) as u64,
                mtime_ms: row.get::<_, i64>(3)?,
                source: row.get::<_, String>(4)?,
            })
        })?;

        let mut map = HashMap::new();
        for row in rows {
            let entry = row?;
            map.insert(entry.path.clone(), entry);
        }
        Ok(map)
    }

    pub fn sync_memory_chunks(
        &self,
        updated_files: &[MemorySourceFileInput],
        deleted_paths: &[String],
        chunks: &[MemoryChunkInput],
        scanned_files: u32,
    ) -> AppResult<MemoryReindexPayload> {
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;

        for path in deleted_paths {
            delete_chunks_for_path(&tx, path)?;
            tx.execute(
                "DELETE FROM memory_source_files WHERE path = ?1",
                params![path],
            )?;
        }

        for file in updated_files {
            delete_chunks_for_path(&tx, &file.path)?;
        }

        for chunk in chunks {
            tx.execute(
                "INSERT INTO memory_chunks (
                    id, path, line_start, line_end, heading, content, file_hash, source, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    chunk.id,
                    chunk.path,
                    chunk.line_start as i64,
                    chunk.line_end as i64,
                    chunk.heading,
                    chunk.content,
                    chunk.file_hash,
                    chunk.source,
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

        for file in updated_files {
            tx.execute(
                "INSERT INTO memory_source_files (
                    path, file_hash, file_size, mtime_ms, indexed_at, source
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(path) DO UPDATE SET
                    file_hash = excluded.file_hash,
                    file_size = excluded.file_size,
                    mtime_ms = excluded.mtime_ms,
                    indexed_at = excluded.indexed_at,
                    source = excluded.source",
                params![
                    file.path,
                    file.file_hash,
                    file.file_size as i64,
                    file.mtime_ms,
                    file.indexed_at,
                    file.source,
                ],
            )?;
        }

        tx.commit()?;

        Ok(MemoryReindexPayload {
            scanned: scanned_files,
            updated: updated_files.len() as u32,
            deleted: deleted_paths.len() as u32,
            chunks_indexed: chunks.len() as u32,
        })
    }

    pub fn memory_search(
        &self,
        query: &str,
        max_results: u32,
        min_score: f32,
    ) -> AppResult<Vec<MemorySearchHit>> {
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
            let path = row.get::<_, String>(0)?;
            let start_line = row.get::<_, i64>(1)?.max(0) as u32;
            let end_line = row.get::<_, i64>(2)?.max(0) as u32;
            let citation = if start_line == end_line {
                Some(format!("{path}#L{start_line}"))
            } else {
                Some(format!("{path}#L{start_line}-L{end_line}"))
            };
            Ok(MemorySearchHit {
                path,
                start_line,
                end_line,
                snippet: row.get::<_, String>(3).unwrap_or_default(),
                score,
                citation,
            })
        })?;

        let mut hits = Vec::new();
        for row in rows {
            let hit = row?;
            if hit.score >= min_score {
                hits.push(hit);
            }
        }
        Ok(hits)
    }
}

fn delete_chunks_for_path(tx: &Transaction<'_>, path: &str) -> AppResult<()> {
    tx.execute(
        "DELETE FROM memory_chunks_fts
         WHERE id IN (SELECT id FROM memory_chunks WHERE path = ?1)",
        params![path],
    )?;
    tx.execute("DELETE FROM memory_chunks WHERE path = ?1", params![path])?;
    Ok(())
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
