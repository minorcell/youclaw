use std::cmp::Ordering;
use std::collections::HashSet;

use uuid::Uuid;

use super::*;
use crate::backend::models::domain::MemoryRecord;

const DEFAULT_MEMORY_LIST_LIMIT: u32 = 64;
const MAX_MEMORY_LIST_LIMIT: u32 = 200;

impl StorageService {
    pub fn list_memory_entries(&self, limit: Option<u32>) -> AppResult<Vec<MemoryRecordSummary>> {
        let limit = limit
            .unwrap_or(DEFAULT_MEMORY_LIST_LIMIT)
            .clamp(1, MAX_MEMORY_LIST_LIMIT);
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, content, updated_at
             FROM memory_entries
             ORDER BY updated_at DESC, created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let content = row.get::<_, String>(2)?;
            Ok(MemoryRecordSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                preview: preview_text(&content, 120),
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_memory_entry(&self, id: &str) -> AppResult<MemoryRecord> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, title, content, created_at, updated_at
             FROM memory_entries
             WHERE id = ?1",
            [id],
            |row| {
                Ok(MemoryRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("memory entry `{id}`")))
    }

    pub fn upsert_memory_entry(
        &self,
        id: Option<&str>,
        title: &str,
        content: &str,
    ) -> AppResult<(MemoryRecord, bool)> {
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;
        let now = now_timestamp();
        let entry_id = id
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let created_at = tx
            .query_row(
                "SELECT created_at FROM memory_entries WHERE id = ?1",
                [entry_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_else(|| now.clone());
        let created = created_at == now;

        tx.execute(
            "INSERT INTO memory_entries (id, title, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                updated_at = excluded.updated_at",
            params![entry_id, title, content, created_at, now],
        )?;
        tx.execute("DELETE FROM memory_entries_fts WHERE id = ?1", [entry_id.as_str()])?;
        tx.execute(
            "INSERT INTO memory_entries_fts (id, title, content) VALUES (?1, ?2, ?3)",
            params![entry_id, title, content],
        )?;
        tx.commit()?;

        Ok((
            MemoryRecord {
                id: entry_id,
                title: title.to_string(),
                content: content.to_string(),
                created_at,
                updated_at: now,
            },
            created,
        ))
    }

    pub fn delete_memory_entry(&self, id: &str) -> AppResult<bool> {
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM memory_entries_fts WHERE id = ?1", [id])?;
        let deleted = tx.execute("DELETE FROM memory_entries WHERE id = ?1", [id])? > 0;
        tx.commit()?;
        Ok(deleted)
    }

    pub fn search_memory_entries(
        &self,
        query: &str,
        max_results: u32,
        min_score: f32,
    ) -> AppResult<Vec<MemorySystemSearchHit>> {
        let query = query.trim();
        let normalized_query = build_fts_query(query)?;
        let max_results = max_results.clamp(1, 100);
        let min_score = min_score.clamp(0.0, 1.0);

        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                e.id,
                e.title,
                snippet(memory_entries_fts, 2, '[', ']', '...', 24) AS snippet,
                bm25(memory_entries_fts) AS rank
             FROM memory_entries_fts
             JOIN memory_entries e ON e.id = memory_entries_fts.id
             WHERE memory_entries_fts MATCH ?1
             ORDER BY rank ASC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![normalized_query, max_results as i64], |row| {
            let rank = row.get::<_, f64>(3).unwrap_or(1000.0).abs() as f32;
            let score = 1.0 / (1.0 + rank);
            Ok(MemorySystemSearchHit {
                id: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get::<_, String>(2).unwrap_or_default(),
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

        if !contains_cjk(query) {
            return Ok(hits);
        }

        let substring_terms = build_substring_terms(query);
        if substring_terms.is_empty() {
            return Ok(hits);
        }

        let fallback_hits =
            self.search_memory_entries_with_substring(&substring_terms, max_results, min_score)?;
        Ok(merge_search_hits(hits, fallback_hits, max_results as usize))
    }

    fn search_memory_entries_with_substring(
        &self,
        terms: &[String],
        max_results: u32,
        min_score: f32,
    ) -> AppResult<Vec<MemorySystemSearchHit>> {
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let scan_limit = ((max_results as i64) * 120).clamp(300, 2_000);
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, content
             FROM memory_entries
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![scan_limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut hits = Vec::new();
        for row in rows {
            let (id, title, content) = row?;
            let combined = format!("{title}\n{content}");
            let scored = score_content_by_terms(&combined, terms);
            if scored.score < min_score {
                continue;
            }
            hits.push(MemorySystemSearchHit {
                id,
                title,
                snippet: build_snippet(&combined, scored.first_match_byte),
                score: scored.score,
            });
        }

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.title.cmp(&b.title))
                .then_with(|| a.id.cmp(&b.id))
        });
        hits.truncate(max_results as usize);
        Ok(hits)
    }
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
    primary: Vec<MemorySystemSearchHit>,
    fallback: Vec<MemorySystemSearchHit>,
    max_results: usize,
) -> Vec<MemorySystemSearchHit> {
    let mut merged = std::collections::HashMap::<String, MemorySystemSearchHit>::new();
    for hit in primary.into_iter().chain(fallback.into_iter()) {
        if let Some(existing) = merged.get_mut(&hit.id) {
            if hit.score > existing.score {
                existing.score = hit.score;
            }
            if existing.snippet.is_empty() && !hit.snippet.is_empty() {
                existing.snippet = hit.snippet;
            }
            if existing.title.is_empty() && !hit.title.is_empty() {
                existing.title = hit.title;
            }
            continue;
        }
        merged.insert(hit.id.clone(), hit);
    }

    let mut hits = merged.into_values().collect::<Vec<_>>();
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
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

    let score = matched as f32 / terms.len() as f32;

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

fn preview_text(content: &str, max_chars: usize) -> String {
    let normalized = content.replace('\n', " ").trim().to_string();
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut preview = normalized.chars().take(max_chars.saturating_sub(3)).collect::<String>();
    preview.push_str("...");
    preview
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
    fn upsert_and_search_memory_entries() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");

        let (entry, created) = storage
            .upsert_memory_entry(None, "习惯总结", "今天继续学习习惯总结，保持长期复盘。")
            .expect("upsert");
        assert!(created);

        let hits = storage
            .search_memory_entries("学习 习惯 总结", 6, 0.2)
            .expect("search");
        assert_eq!(hits[0].id, entry.id);
        assert_eq!(hits[0].title, "习惯总结");
    }

    #[test]
    fn list_memory_entries_orders_by_updated_at() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");

        let (first, _) = storage
            .upsert_memory_entry(None, "A", "alpha")
            .expect("first");
        let _ = storage
            .upsert_memory_entry(Some(first.id.as_str()), "A2", "alpha2")
            .expect("update");
        let _ = storage
            .upsert_memory_entry(None, "B", "beta")
            .expect("second");

        let entries = storage.list_memory_entries(Some(10)).expect("list");
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn delete_memory_entry_removes_record() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");
        let (entry, _) = storage
            .upsert_memory_entry(None, "A", "alpha")
            .expect("create");

        assert!(storage.delete_memory_entry(&entry.id).expect("delete"));
        assert!(!storage.delete_memory_entry(&entry.id).expect("delete again"));
    }
}
