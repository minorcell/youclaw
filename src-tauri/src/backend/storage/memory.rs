use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

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
        let query = query.trim();
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

        if !contains_cjk(query) {
            return Ok(hits);
        }

        let substring_terms = build_substring_terms(query);
        if substring_terms.is_empty() {
            return Ok(hits);
        }

        let fallback_hits =
            self.memory_search_with_substring(&substring_terms, max_results, min_score)?;
        Ok(merge_search_hits(hits, fallback_hits, max_results as usize))
    }

    fn memory_search_with_substring(
        &self,
        terms: &[String],
        max_results: u32,
        min_score: f32,
    ) -> AppResult<Vec<MemorySearchHit>> {
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let scan_limit = ((max_results as i64) * 120).clamp(300, 2_000);
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                path,
                line_start,
                line_end,
                content
             FROM memory_chunks
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![scan_limit], |row| {
            let path = row.get::<_, String>(0)?;
            let start_line = row.get::<_, i64>(1)?.max(0) as u32;
            let end_line = row.get::<_, i64>(2)?.max(0) as u32;
            let content = row.get::<_, String>(3).unwrap_or_default();
            Ok((path, start_line, end_line, content))
        })?;

        let mut hits = Vec::new();
        for row in rows {
            let (path, start_line, end_line, content) = row?;
            let scored = score_content_by_terms(&content, terms);
            if scored.score < min_score {
                continue;
            }
            let citation = if start_line == end_line {
                Some(format!("{path}#L{start_line}"))
            } else {
                Some(format!("{path}#L{start_line}-L{end_line}"))
            };
            hits.push(MemorySearchHit {
                path,
                start_line,
                end_line,
                snippet: build_snippet(&content, scored.first_match_byte),
                score: scored.score,
                citation,
            });
        }

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.start_line.cmp(&b.start_line))
        });
        hits.truncate(max_results as usize);
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

    let mut terms = extract_query_terms(trimmed);

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
        .join(" OR ");

    Ok(query)
}

fn merge_search_hits(
    primary: Vec<MemorySearchHit>,
    fallback: Vec<MemorySearchHit>,
    max_results: usize,
) -> Vec<MemorySearchHit> {
    let mut merged = HashMap::<(String, u32, u32), MemorySearchHit>::new();
    for hit in primary.into_iter().chain(fallback.into_iter()) {
        let key = (hit.path.clone(), hit.start_line, hit.end_line);
        if let Some(existing) = merged.get_mut(&key) {
            if hit.score > existing.score {
                existing.score = hit.score;
            }
            if existing.snippet.is_empty() && !hit.snippet.is_empty() {
                existing.snippet = hit.snippet;
            }
            if existing.citation.is_none() {
                existing.citation = hit.citation;
            }
            continue;
        }
        merged.insert(key, hit);
    }

    let mut hits = merged.into_values().collect::<Vec<_>>();
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.start_line.cmp(&b.start_line))
    });
    hits.truncate(max_results);
    hits
}

fn extract_query_terms(query: &str) -> Vec<String> {
    query
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
        .collect::<Vec<_>>()
}

fn build_substring_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();

    for term in extract_query_terms(query) {
        if term.is_empty() {
            continue;
        }

        if contains_cjk(&term) {
            let compact = term
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>();
            if compact.is_empty() {
                continue;
            }

            if seen.insert(compact.clone()) {
                terms.push(compact.clone());
            }

            let chars = compact.chars().collect::<Vec<_>>();
            if chars.len() >= 2 {
                for window in chars.windows(2) {
                    let gram = window.iter().collect::<String>();
                    if seen.insert(gram.clone()) {
                        terms.push(gram);
                    }
                }
            }
        } else {
            let lowered = term.to_lowercase();
            if seen.insert(lowered.clone()) {
                terms.push(lowered);
            }
        }
    }

    terms
}

#[derive(Debug, Default)]
struct ScoredContentMatch {
    score: f32,
    first_match_byte: Option<usize>,
}

fn score_content_by_terms(content: &str, terms: &[String]) -> ScoredContentMatch {
    if content.is_empty() || terms.is_empty() {
        return ScoredContentMatch::default();
    }

    let mut matched = 0usize;
    let mut first_match = None;
    let lower_content = content.to_lowercase();

    for term in terms {
        if term.is_empty() {
            continue;
        }
        let found = if contains_cjk(term) {
            content.find(term)
        } else {
            lower_content.find(term)
        };
        if let Some(index) = found {
            matched += 1;
            if first_match.is_none_or(|prev| index < prev) {
                first_match = Some(index);
            }
        }
    }

    let score = if terms.is_empty() {
        0.0
    } else {
        matched as f32 / terms.len() as f32
    };

    ScoredContentMatch {
        score,
        first_match_byte: first_match,
    }
}

fn build_snippet(content: &str, first_match_byte: Option<usize>) -> String {
    const WINDOW: usize = 120;
    const CONTEXT_BEFORE: usize = 40;

    if content.is_empty() {
        return String::new();
    }

    let chars = content.chars().collect::<Vec<_>>();
    if chars.len() <= WINDOW {
        return content.replace('\n', " ").trim().to_string();
    }

    let center_char = first_match_byte
        .map(|byte_index| content[..byte_index].chars().count())
        .unwrap_or(0);
    let start = center_char.saturating_sub(CONTEXT_BEFORE);
    let end = (start + WINDOW).min(chars.len());

    let mut snippet = chars[start..end].iter().collect::<String>();
    snippet = snippet.replace('\n', " ").trim().to_string();

    if start > 0 {
        snippet.insert_str(0, "...");
    }
    if end < chars.len() {
        snippet.push_str("...");
    }
    snippet
}

fn contains_cjk(value: &str) -> bool {
    value.chars().any(is_cjk_char)
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}'
    )
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn builds_substring_terms_for_cjk_queries() {
        let terms = build_substring_terms("学习习惯总结");
        assert!(terms.contains(&"学习习惯总结".to_string()));
        assert!(terms.contains(&"学习".to_string()));
        assert!(terms.contains(&"习惯".to_string()));
        assert!(terms.contains(&"总结".to_string()));
    }

    #[test]
    fn memory_search_falls_back_to_substring_for_cjk() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");

        storage
            .sync_memory_chunks(
                &[MemorySourceFileInput {
                    path: "MEMORY.md".to_string(),
                    file_hash: "hash-1".to_string(),
                    file_size: 42,
                    mtime_ms: 1,
                    indexed_at: now_timestamp(),
                    source: "memory".to_string(),
                }],
                &[],
                &[MemoryChunkInput {
                    id: "chunk-1".to_string(),
                    path: "MEMORY.md".to_string(),
                    line_start: 1,
                    line_end: 3,
                    heading: None,
                    content: "今天继续学习习惯总结，保持长期复盘。".to_string(),
                    file_hash: "hash-1".to_string(),
                    source: "memory".to_string(),
                }],
                1,
            )
            .expect("sync");

        let hits = storage
            .memory_search("学习 习惯 总结", 6, 0.2)
            .expect("search");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].path, "MEMORY.md");
    }
}
