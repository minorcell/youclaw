use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension};
use serde_json::Value;

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::{
    flatten_provider_profiles, message_from_record, migrate_provider_accounts_from_legacy,
    normalize_provider_accounts, now_timestamp, AgentActiveHoursConfig, AgentConfigPayload,
    AgentConfigUpdateRequest, BootstrapPayload, ChatMessage, ChatRun, ChatSession,
    MemoryReindexPayload, MemorySearchHit, MemorySearchPayload, ProviderAccount, ProviderProfile,
    SessionsChangedPayload, StoredProviders, ToolApproval, UsageLogDetailPayload,
    UsageLogDetailRequest, UsageLogItem, UsageLogsListRequest, UsageLogsPayload,
    UsageModelStatsItem, UsageModelStatsPayload, UsagePage, UsageProviderStatsItem,
    UsageProviderStatsPayload, UsageSettingsPayload, UsageStatsListRequest, UsageSummaryPayload,
    UsageSummaryRequest, UsageToolLogItem, UsageToolStatsItem, UsageToolStatsPayload,
    USAGE_RANGE_24H, USAGE_RANGE_30D, USAGE_RANGE_7D, USAGE_RANGE_ALL,
};

#[derive(Clone)]
pub struct StorageService {
    inner: Arc<StorageInner>,
}

struct StorageInner {
    base_dir: PathBuf,
    db_path: PathBuf,
    providers_path: PathBuf,
    providers_lock: Mutex<()>,
}

const DEFAULT_USAGE_PAGE_SIZE: u32 = 20;
const MAX_USAGE_PAGE_SIZE: u32 = 100;
const DEFAULT_USAGE_DETAIL_LOGGING_ENABLED: bool = true;

#[derive(Debug, Clone)]
pub struct MemoryChunkInput {
    pub id: String,
    pub path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub heading: Option<String>,
    pub content: String,
}

impl StorageService {
    pub fn new(base_dir: PathBuf) -> AppResult<Self> {
        let inner = Arc::new(StorageInner {
            db_path: base_dir.join("app.sqlite"),
            providers_path: base_dir.join("providers.json"),
            base_dir,
            providers_lock: Mutex::new(()),
        });
        let this = Self { inner };
        this.initialize()?;
        Ok(this)
    }

    fn initialize(&self) -> AppResult<()> {
        fs::create_dir_all(&self.inner.base_dir)?;
        if !self.inner.providers_path.exists() {
            fs::write(
                &self.inner.providers_path,
                serde_json::to_vec_pretty(&StoredProviders::default())?,
            )?;
        }
        let conn = self.open_connection()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT
            );
            CREATE TABLE IF NOT EXISTS chat_sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                provider_profile_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_run_at TEXT
            );
            CREATE TABLE IF NOT EXISTS chat_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                parts_json TEXT NOT NULL,
                run_id TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chat_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                status TEXT NOT NULL,
                user_message TEXT NOT NULL,
                output_text TEXT NOT NULL,
                created_at TEXT NOT NULL,
                finished_at TEXT,
                error_message TEXT
            );
            CREATE TABLE IF NOT EXISTS tool_approvals (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                call_id TEXT NOT NULL,
                action TEXT NOT NULL,
                path TEXT NOT NULL,
                preview_json TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                resolved_at TEXT
            );
            CREATE TABLE IF NOT EXISTS file_operations (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                call_id TEXT,
                action TEXT NOT NULL,
                path TEXT NOT NULL,
                status TEXT NOT NULL,
                bytes_written INTEGER,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS run_usage_metrics (
                run_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                provider_profile_id TEXT,
                provider_id TEXT,
                provider_name TEXT,
                model_id TEXT,
                model_name TEXT,
                model TEXT,
                status TEXT NOT NULL,
                user_message TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                duration_ms INTEGER,
                detail_logged INTEGER NOT NULL DEFAULT 1,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                input_no_cache_tokens INTEGER NOT NULL DEFAULT 0,
                input_cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                input_cache_write_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                output_text_tokens INTEGER NOT NULL DEFAULT 0,
                reasoning_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS run_tool_metrics (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                tool_action TEXT,
                status TEXT NOT NULL,
                duration_ms INTEGER,
                is_error INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                max_steps INTEGER NOT NULL DEFAULT 8,
                max_input_tokens INTEGER NOT NULL DEFAULT 32768,
                compact_ratio REAL NOT NULL DEFAULT 0.7,
                keep_recent INTEGER NOT NULL DEFAULT 8,
                language TEXT NOT NULL DEFAULT 'zh',
                heartbeat_enabled INTEGER NOT NULL DEFAULT 0,
                heartbeat_every TEXT NOT NULL DEFAULT '30m',
                heartbeat_target TEXT NOT NULL DEFAULT 'main',
                heartbeat_active_start TEXT,
                heartbeat_active_end TEXT,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS session_memory_state (
                session_id TEXT PRIMARY KEY,
                compressed_summary TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS message_marks (
                message_id TEXT NOT NULL,
                mark TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (message_id, mark)
            );
            CREATE TABLE IF NOT EXISTS memory_chunks (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                line_start INTEGER NOT NULL,
                line_end INTEGER NOT NULL,
                heading TEXT,
                content TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS memory_chunks_fts USING fts5(
                id UNINDEXED,
                path UNINDEXED,
                heading,
                content
            );
            CREATE INDEX IF NOT EXISTS idx_run_usage_metrics_started_at
            ON run_usage_metrics (started_at DESC, run_id DESC);
            CREATE INDEX IF NOT EXISTS idx_run_usage_metrics_model
            ON run_usage_metrics (model_id, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_run_usage_metrics_status
            ON run_usage_metrics (status, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_run_tool_metrics_run
            ON run_tool_metrics (run_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_run_tool_metrics_tool
            ON run_tool_metrics (tool_name, tool_action, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_message_marks_mark
            ON message_marks (mark, message_id);
            CREATE INDEX IF NOT EXISTS idx_memory_chunks_path
            ON memory_chunks (path);
            ",
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO agent_settings (
                id, max_steps, max_input_tokens, compact_ratio, keep_recent,
                language, heartbeat_enabled, heartbeat_every, heartbeat_target,
                heartbeat_active_start, heartbeat_active_end, updated_at
             ) VALUES (1, 8, 32768, 0.7, 8, 'zh', 0, '30m', 'main', NULL, NULL, ?1)",
            [now_timestamp()],
        )?;
        Ok(())
    }

    fn open_connection(&self) -> AppResult<Connection> {
        let conn = Connection::open(&self.inner.db_path)?;
        conn.busy_timeout(Duration::from_secs(3))?;
        Ok(conn)
    }

    pub fn base_dir(&self) -> &PathBuf {
        &self.inner.base_dir
    }

    pub fn load_bootstrap(&self) -> AppResult<BootstrapPayload> {
        let provider_accounts = self.list_provider_accounts()?;
        Ok(BootstrapPayload {
            provider_profiles: flatten_provider_profiles(&provider_accounts),
            provider_accounts,
            sessions: self.list_sessions()?,
            messages: self.list_messages()?,
            approvals: self.list_approvals()?,
            runs: self.list_runs()?,
            last_opened_session_id: self.get_last_opened_session_id()?,
            agent_config: self.get_agent_config()?,
            workspace_files: Vec::new(),
        })
    }

    pub fn list_provider_profiles(&self) -> AppResult<Vec<ProviderProfile>> {
        Ok(flatten_provider_profiles(&self.list_provider_accounts()?))
    }

    pub fn list_provider_accounts(&self) -> AppResult<Vec<ProviderAccount>> {
        let _guard = self
            .inner
            .providers_lock
            .lock()
            .map_err(|_| AppError::Storage("provider lock poisoned".to_string()))?;
        self.load_provider_accounts_unlocked()
    }

    pub fn save_provider_accounts(&self, accounts: &[ProviderAccount]) -> AppResult<()> {
        let _guard = self
            .inner
            .providers_lock
            .lock()
            .map_err(|_| AppError::Storage("provider lock poisoned".to_string()))?;
        self.write_provider_accounts_unlocked(accounts)
    }

    fn load_provider_accounts_unlocked(&self) -> AppResult<Vec<ProviderAccount>> {
        let raw = fs::read_to_string(&self.inner.providers_path)?;
        let parsed = if raw.trim().is_empty() {
            StoredProviders::default()
        } else {
            serde_json::from_str::<StoredProviders>(&raw)?
        };

        let mut should_persist = false;
        let mut accounts = if parsed.accounts.is_empty() && !parsed.profiles.is_empty() {
            should_persist = true;
            migrate_provider_accounts_from_legacy(parsed.profiles)
        } else {
            parsed.accounts
        };
        if normalize_provider_accounts(&mut accounts) {
            should_persist = true;
        }
        if should_persist {
            self.write_provider_accounts_unlocked(&accounts)?;
        }
        Ok(accounts)
    }

    fn write_provider_accounts_unlocked(&self, accounts: &[ProviderAccount]) -> AppResult<()> {
        let body = serde_json::to_vec_pretty(&StoredProviders {
            accounts: accounts.to_vec(),
            profiles: Vec::new(),
        })?;
        fs::write(&self.inner.providers_path, body)?;
        Ok(())
    }

    pub fn list_sessions(&self) -> AppResult<Vec<ChatSession>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, provider_profile_id, created_at, updated_at, last_run_at
             FROM chat_sessions
             ORDER BY COALESCE(last_run_at, updated_at) DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ChatSession {
                id: row.get(0)?,
                title: row.get(1)?,
                provider_profile_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                last_run_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_session(&self, session_id: &str) -> AppResult<ChatSession> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, title, provider_profile_id, created_at, updated_at, last_run_at
             FROM chat_sessions WHERE id = ?1",
            [session_id],
            |row| {
                Ok(ChatSession {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    provider_profile_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    last_run_at: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("session `{session_id}`")))
    }

    pub fn insert_session(&self, session: &ChatSession) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO chat_sessions (id, title, provider_profile_id, created_at, updated_at, last_run_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session.id,
                session.title,
                session.provider_profile_id,
                session.created_at,
                session.updated_at,
                session.last_run_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_session_provider(
        &self,
        session_id: &str,
        provider_profile_id: &str,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "UPDATE chat_sessions SET provider_profile_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![session_id, provider_profile_id, now_timestamp()],
        )?;
        Ok(())
    }

    pub fn clear_session_provider_binding(&self, provider_profile_id: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "UPDATE chat_sessions SET provider_profile_id = NULL, updated_at = ?2 WHERE provider_profile_id = ?1",
            params![provider_profile_id, now_timestamp()],
        )?;
        Ok(())
    }

    pub fn touch_session_for_run(&self, session_id: &str, title: Option<&str>) -> AppResult<()> {
        let conn = self.open_connection()?;
        let now = now_timestamp();
        if let Some(title) = title {
            conn.execute(
                "UPDATE chat_sessions SET title = ?2, updated_at = ?3, last_run_at = ?3 WHERE id = ?1",
                params![session_id, title, now],
            )?;
        } else {
            conn.execute(
                "UPDATE chat_sessions SET updated_at = ?2, last_run_at = ?2 WHERE id = ?1",
                params![session_id, now],
            )?;
        }
        Ok(())
    }

    pub fn delete_session(&self, session_id: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "DELETE FROM message_marks
             WHERE message_id IN (SELECT id FROM chat_messages WHERE session_id = ?1)",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM session_memory_state WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM chat_messages WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM tool_approvals WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute("DELETE FROM chat_runs WHERE session_id = ?1", [session_id])?;
        conn.execute(
            "DELETE FROM run_usage_metrics WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM run_tool_metrics WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM file_operations WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute("DELETE FROM chat_sessions WHERE id = ?1", [session_id])?;

        if self.get_last_opened_session_id()?.as_deref() == Some(session_id) {
            let next = self
                .list_sessions()?
                .into_iter()
                .next()
                .map(|session| session.id);
            self.set_last_opened_session_id(next.as_deref())?;
        }
        Ok(())
    }

    pub fn list_messages(&self) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, parts_json, run_id, created_at
             FROM chat_messages
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let parts_json = row.get::<_, String>(3)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                run_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_messages_for_session(&self, session_id: &str) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, parts_json, run_id, created_at
             FROM chat_messages
             WHERE session_id = ?1
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([session_id], |row| {
            let parts_json = row.get::<_, String>(3)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                run_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_active_messages_for_session(
        &self,
        session_id: &str,
    ) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.parts_json, m.run_id, m.created_at
             FROM chat_messages m
             LEFT JOIN message_marks mm
               ON mm.message_id = m.id AND mm.mark = 'compressed'
             WHERE m.session_id = ?1
               AND mm.message_id IS NULL
             ORDER BY m.created_at ASC, m.rowid ASC",
        )?;
        let rows = stmt.query_map([session_id], |row| {
            let parts_json = row.get::<_, String>(3)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                run_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn clear_session_messages(&self, session_id: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "DELETE FROM message_marks
             WHERE message_id IN (SELECT id FROM chat_messages WHERE session_id = ?1)",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM chat_messages WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM session_memory_state WHERE session_id = ?1",
            [session_id],
        )?;
        Ok(())
    }

    pub fn mark_messages(&self, message_ids: &[String], mark: &str) -> AppResult<u32> {
        if message_ids.is_empty() {
            return Ok(0);
        }
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;
        let mut changed = 0u32;
        for message_id in message_ids {
            changed += tx.execute(
                "INSERT OR IGNORE INTO message_marks (message_id, mark, created_at)
                 VALUES (?1, ?2, ?3)",
                params![message_id, mark, now_timestamp()],
            )? as u32;
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn count_marked_messages(&self, session_id: &str, mark: &str) -> AppResult<u32> {
        let conn = self.open_connection()?;
        let count = conn.query_row(
            "SELECT COUNT(*)
             FROM message_marks mm
             JOIN chat_messages m ON m.id = mm.message_id
             WHERE m.session_id = ?1 AND mm.mark = ?2",
            params![session_id, mark],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(count.max(0) as u32)
    }

    pub fn get_session_compressed_summary(&self, session_id: &str) -> AppResult<String> {
        let conn = self.open_connection()?;
        let summary = conn
            .query_row(
                "SELECT compressed_summary FROM session_memory_state WHERE session_id = ?1",
                [session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_default();
        Ok(summary)
    }

    pub fn upsert_session_compressed_summary(
        &self,
        session_id: &str,
        compressed_summary: &str,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO session_memory_state (session_id, compressed_summary, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET
               compressed_summary = excluded.compressed_summary,
               updated_at = excluded.updated_at",
            params![session_id, compressed_summary, now_timestamp()],
        )?;
        Ok(())
    }

    pub fn list_messages_for_run(&self, run_id: &str) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, parts_json, run_id, created_at
             FROM chat_messages
             WHERE run_id = ?1
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([run_id], |row| {
            let parts_json = row.get::<_, String>(3)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                run_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_messages(&self, messages: &[ChatMessage]) -> AppResult<()> {
        if messages.is_empty() {
            return Ok(());
        }
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;
        for message in messages {
            tx.execute(
                "INSERT INTO chat_messages (id, session_id, role, parts_json, run_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    message.id,
                    message.session_id,
                    message.role,
                    serde_json::to_string(&message.parts_json)?,
                    message.run_id,
                    message.created_at,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_message(&self, message: &ChatMessage) -> AppResult<()> {
        self.insert_messages(std::slice::from_ref(message))
    }

    pub fn list_message_objects_for_session(
        &self,
        session_id: &str,
    ) -> AppResult<Vec<aquaregia::Message>> {
        self.list_messages_for_session(session_id)?
            .iter()
            .map(message_from_record)
            .collect()
    }

    pub fn list_active_message_objects_for_session(
        &self,
        session_id: &str,
    ) -> AppResult<Vec<aquaregia::Message>> {
        self.list_active_messages_for_session(session_id)?
            .iter()
            .map(message_from_record)
            .collect()
    }

    pub fn list_runs(&self) -> AppResult<Vec<ChatRun>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, status, user_message, output_text, created_at, finished_at, error_message
             FROM chat_runs
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ChatRun {
                id: row.get(0)?,
                session_id: row.get(1)?,
                status: row.get(2)?,
                user_message: row.get(3)?,
                output_text: row.get(4)?,
                created_at: row.get(5)?,
                finished_at: row.get(6)?,
                error_message: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_run(&self, run: &ChatRun) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO chat_runs (id, session_id, status, user_message, output_text, created_at, finished_at, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run.id,
                run.session_id,
                run.status,
                run.user_message,
                run.output_text,
                run.created_at,
                run.finished_at,
                run.error_message,
            ],
        )?;
        Ok(())
    }

    pub fn update_run(
        &self,
        run_id: &str,
        status: &str,
        output_text: Option<&str>,
        error_message: Option<&str>,
    ) -> AppResult<ChatRun> {
        let conn = self.open_connection()?;
        let finished_at = now_timestamp();
        conn.execute(
            "UPDATE chat_runs
             SET status = ?2, output_text = COALESCE(?3, output_text), error_message = ?4, finished_at = ?5
             WHERE id = ?1",
            params![run_id, status, output_text, error_message, finished_at],
        )?;
        conn.query_row(
            "SELECT id, session_id, status, user_message, output_text, created_at, finished_at, error_message
             FROM chat_runs WHERE id = ?1",
            [run_id],
            |row| {
                Ok(ChatRun {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    status: row.get(2)?,
                    user_message: row.get(3)?,
                    output_text: row.get(4)?,
                    created_at: row.get(5)?,
                    finished_at: row.get(6)?,
                    error_message: row.get(7)?,
                })
            },
        )
        .map_err(Into::into)
    }

    pub fn get_agent_config(&self) -> AppResult<AgentConfigPayload> {
        let conn = self.open_connection()?;
        let config = conn.query_row(
            "SELECT
                max_steps, max_input_tokens, compact_ratio, keep_recent, language,
                heartbeat_enabled, heartbeat_every, heartbeat_target,
                heartbeat_active_start, heartbeat_active_end
             FROM agent_settings
             WHERE id = 1",
            [],
            |row| {
                let max_steps = row.get::<_, i64>(0)?.clamp(1, 32) as u8;
                let max_input_tokens = row.get::<_, i64>(1)?.clamp(1000, 1_000_000) as u32;
                let compact_ratio = row.get::<_, f64>(2)?.clamp(0.1, 0.95) as f32;
                let keep_recent = row.get::<_, i64>(3)?.clamp(1, 128) as u32;
                let language = normalize_language(row.get::<_, String>(4)?);
                let heartbeat_enabled = row.get::<_, i64>(5)? != 0;
                let heartbeat_every = row.get::<_, String>(6)?;
                let heartbeat_target = row.get::<_, String>(7)?;
                let active_start = row.get::<_, Option<String>>(8)?;
                let active_end = row.get::<_, Option<String>>(9)?;
                Ok(AgentConfigPayload {
                    max_steps,
                    max_input_tokens,
                    compact_ratio,
                    keep_recent,
                    language,
                    heartbeat: crate::backend::models::AgentHeartbeatConfig {
                        enabled: heartbeat_enabled,
                        every: heartbeat_every,
                        target: heartbeat_target,
                        active_hours: match (active_start, active_end) {
                            (Some(start), Some(end)) => Some(AgentActiveHoursConfig { start, end }),
                            _ => None,
                        },
                    },
                })
            },
        )?;
        Ok(config)
    }

    pub fn update_agent_config(
        &self,
        req: AgentConfigUpdateRequest,
    ) -> AppResult<AgentConfigPayload> {
        let mut current = self.get_agent_config()?;

        if let Some(value) = req.max_steps {
            current.max_steps = value.clamp(1, 32);
        }
        if let Some(value) = req.max_input_tokens {
            current.max_input_tokens = value.clamp(1000, 1_000_000);
        }
        if let Some(value) = req.compact_ratio {
            current.compact_ratio = value.clamp(0.1, 0.95);
        }
        if let Some(value) = req.keep_recent {
            current.keep_recent = value.clamp(1, 128);
        }
        if let Some(value) = req.language {
            current.language = normalize_language(value);
        }
        if let Some(value) = req.heartbeat {
            current.heartbeat.enabled = value.enabled;
            if !value.every.trim().is_empty() {
                current.heartbeat.every = value.every;
            }
            if !value.target.trim().is_empty() {
                current.heartbeat.target = value.target;
            }
            current.heartbeat.active_hours = value.active_hours;
        }

        let conn = self.open_connection()?;
        conn.execute(
            "UPDATE agent_settings
             SET max_steps = ?2,
                 max_input_tokens = ?3,
                 compact_ratio = ?4,
                 keep_recent = ?5,
                 language = ?6,
                 heartbeat_enabled = ?7,
                 heartbeat_every = ?8,
                 heartbeat_target = ?9,
                 heartbeat_active_start = ?10,
                 heartbeat_active_end = ?11,
                 updated_at = ?12
             WHERE id = ?1",
            params![
                1i64,
                current.max_steps as i64,
                current.max_input_tokens as i64,
                current.compact_ratio as f64,
                current.keep_recent as i64,
                current.language,
                if current.heartbeat.enabled {
                    1i64
                } else {
                    0i64
                },
                current.heartbeat.every,
                current.heartbeat.target,
                current
                    .heartbeat
                    .active_hours
                    .as_ref()
                    .map(|item| item.start.clone()),
                current
                    .heartbeat
                    .active_hours
                    .as_ref()
                    .map(|item| item.end.clone()),
                now_timestamp(),
            ],
        )?;

        self.get_agent_config()
    }

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

    pub fn get_usage_detail_logging_enabled(&self) -> AppResult<bool> {
        let conn = self.open_connection()?;
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'usage_detail_logging_enabled'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(match value.as_deref() {
            Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("on") => true,
            Some("0") | Some("false") | Some("FALSE") | Some("no") | Some("off") => false,
            Some(_) => DEFAULT_USAGE_DETAIL_LOGGING_ENABLED,
            None => DEFAULT_USAGE_DETAIL_LOGGING_ENABLED,
        })
    }

    pub fn set_usage_detail_logging_enabled(
        &self,
        detail_logging_enabled: bool,
    ) -> AppResult<UsageSettingsPayload> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('usage_detail_logging_enabled', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [if detail_logging_enabled { "1" } else { "0" }],
        )?;
        Ok(UsageSettingsPayload {
            detail_logging_enabled,
        })
    }

    pub fn usage_settings_payload(&self) -> AppResult<UsageSettingsPayload> {
        Ok(UsageSettingsPayload {
            detail_logging_enabled: self.get_usage_detail_logging_enabled()?,
        })
    }

    pub fn insert_run_usage_metric_start(
        &self,
        run: &ChatRun,
        provider: Option<&ProviderProfile>,
        detail_logged: bool,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO run_usage_metrics (
                run_id, session_id, provider_profile_id, provider_id, provider_name,
                model_id, model_name, model, status, user_message, started_at, detail_logged
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(run_id) DO UPDATE SET
                session_id = excluded.session_id,
                provider_profile_id = excluded.provider_profile_id,
                provider_id = excluded.provider_id,
                provider_name = excluded.provider_name,
                model_id = excluded.model_id,
                model_name = excluded.model_name,
                model = excluded.model,
                status = excluded.status,
                user_message = excluded.user_message,
                started_at = excluded.started_at,
                detail_logged = excluded.detail_logged",
            params![
                run.id,
                run.session_id,
                provider.map(|item| item.id.clone()),
                provider.map(|item| item.provider_id.clone()),
                provider.map(|item| item.name.clone()),
                provider.map(|item| item.id.clone()),
                provider.map(|item| item.model_name.clone()),
                provider.map(|item| item.model.clone()),
                run.status,
                run.user_message,
                run.created_at,
                if detail_logged { 1i64 } else { 0i64 },
            ],
        )?;
        Ok(())
    }

    pub fn update_run_usage_metric(
        &self,
        run: &ChatRun,
        usage: Option<&aquaregia::Usage>,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        let started_at = conn
            .query_row(
                "SELECT started_at FROM run_usage_metrics WHERE run_id = ?1",
                [run.id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        let duration_ms = calculate_duration_ms(started_at.as_deref(), run.finished_at.as_deref());
        let usage_value = usage.cloned().unwrap_or_default();
        let changed = conn.execute(
            "UPDATE run_usage_metrics
             SET status = ?2,
                 finished_at = ?3,
                 duration_ms = ?4,
                 input_tokens = ?5,
                 input_no_cache_tokens = ?6,
                 input_cache_read_tokens = ?7,
                 input_cache_write_tokens = ?8,
                 output_tokens = ?9,
                 output_text_tokens = ?10,
                 reasoning_tokens = ?11,
                 total_tokens = ?12
             WHERE run_id = ?1",
            params![
                run.id,
                run.status,
                run.finished_at,
                duration_ms,
                usage_value.input_tokens as i64,
                usage_value.input_no_cache_tokens as i64,
                usage_value.input_cache_read_tokens as i64,
                usage_value.input_cache_write_tokens as i64,
                usage_value.output_tokens as i64,
                usage_value.output_text_tokens as i64,
                usage_value.reasoning_tokens as i64,
                usage_value.total_tokens as i64,
            ],
        )?;

        if changed == 0 {
            let detail_logged = self.get_usage_detail_logging_enabled()?;
            conn.execute(
                "INSERT INTO run_usage_metrics (
                    run_id, session_id, status, user_message, started_at,
                    finished_at, duration_ms, detail_logged,
                    input_tokens, input_no_cache_tokens, input_cache_read_tokens, input_cache_write_tokens,
                    output_tokens, output_text_tokens, reasoning_tokens, total_tokens
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    run.id,
                    run.session_id,
                    run.status,
                    run.user_message,
                    run.created_at,
                    run.finished_at,
                    duration_ms,
                    if detail_logged { 1i64 } else { 0i64 },
                    usage_value.input_tokens as i64,
                    usage_value.input_no_cache_tokens as i64,
                    usage_value.input_cache_read_tokens as i64,
                    usage_value.input_cache_write_tokens as i64,
                    usage_value.output_tokens as i64,
                    usage_value.output_text_tokens as i64,
                    usage_value.reasoning_tokens as i64,
                    usage_value.total_tokens as i64,
                ],
            )?;
        }
        Ok(())
    }

    pub fn record_run_tool_metric(
        &self,
        run_id: &str,
        session_id: &str,
        tool_name: &str,
        tool_action: Option<&str>,
        status: &str,
        duration_ms: Option<u64>,
        is_error: bool,
    ) -> AppResult<()> {
        if !self.is_run_detail_logging_enabled(run_id)? {
            return Ok(());
        }

        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO run_tool_metrics (id, run_id, session_id, tool_name, tool_action, status, duration_ms, is_error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                run_id,
                session_id,
                tool_name,
                tool_action,
                status,
                duration_ms.map(|value| value as i64),
                if is_error { 1i64 } else { 0i64 },
                now_timestamp(),
            ],
        )?;
        Ok(())
    }

    pub fn usage_summary(&self, req: UsageSummaryRequest) -> AppResult<UsageSummaryPayload> {
        let conn = self.open_connection()?;
        let (where_clause, params) = build_run_usage_where_clause(&req.range, None, None, None)?;
        let sql = format!(
            "SELECT
                COUNT(*) AS total_requests,
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(reasoning_tokens), 0),
                COALESCE(SUM(total_tokens), 0),
                COALESCE(SUM(input_cache_read_tokens), 0),
                COALESCE(SUM(input_cache_write_tokens), 0)
             FROM run_usage_metrics{where_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        stmt.query_row(params_from_iter(params), |row| {
            Ok(UsageSummaryPayload {
                range: req.range.clone(),
                total_requests: row.get::<_, i64>(0)? as u64,
                input_tokens: row.get::<_, i64>(1)? as u64,
                output_tokens: row.get::<_, i64>(2)? as u64,
                reasoning_tokens: row.get::<_, i64>(3)? as u64,
                total_tokens: row.get::<_, i64>(4)? as u64,
                input_cache_read_tokens: row.get::<_, i64>(5)? as u64,
                input_cache_write_tokens: row.get::<_, i64>(6)? as u64,
            })
        })
        .map_err(Into::into)
    }

    pub fn list_usage_logs(&self, req: UsageLogsListRequest) -> AppResult<UsageLogsPayload> {
        let conn = self.open_connection()?;
        let (page, page_size, offset) = normalize_usage_pagination(req.page, req.page_size);
        let (where_clause, base_params) = build_run_usage_where_clause(
            &req.range,
            req.model_id.as_deref(),
            req.status.as_deref(),
            req.detail_logged,
        )?;

        let count_sql = format!("SELECT COUNT(*) FROM run_usage_metrics{where_clause}");
        let total = conn.query_row(&count_sql, params_from_iter(base_params.clone()), |row| {
            row.get::<_, i64>(0)
        })? as u64;

        let mut list_params = base_params;
        list_params.push(SqlValue::Integer(page_size as i64));
        list_params.push(SqlValue::Integer(offset as i64));
        let list_sql = format!(
            "SELECT
                run_id, session_id, status, user_message,
                provider_id, provider_name, model_id, model_name, model,
                started_at, finished_at, duration_ms, detail_logged,
                input_tokens, output_tokens, reasoning_tokens, total_tokens,
                input_cache_read_tokens, input_cache_write_tokens
             FROM run_usage_metrics{where_clause}
             ORDER BY started_at DESC, run_id DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt.query_map(params_from_iter(list_params), |row| {
            let detail_logged = row.get::<_, i64>(12)? != 0;
            let duration_ms = row.get::<_, Option<i64>>(11)?.and_then(|value| {
                if value >= 0 {
                    Some(value as u64)
                } else {
                    None
                }
            });
            Ok(UsageLogItem {
                run_id: row.get(0)?,
                session_id: row.get(1)?,
                status: row.get(2)?,
                user_message: row.get(3)?,
                provider_id: row.get(4)?,
                provider_name: row.get(5)?,
                model_id: row.get(6)?,
                model_name: row.get(7)?,
                model: row.get(8)?,
                started_at: row.get(9)?,
                finished_at: row.get(10)?,
                duration_ms,
                detail_logged,
                input_tokens: row.get::<_, i64>(13)? as u64,
                output_tokens: row.get::<_, i64>(14)? as u64,
                reasoning_tokens: row.get::<_, i64>(15)? as u64,
                total_tokens: row.get::<_, i64>(16)? as u64,
                input_cache_read_tokens: row.get::<_, i64>(17)? as u64,
                input_cache_write_tokens: row.get::<_, i64>(18)? as u64,
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;

        let has_more = (offset as u64 + page_size as u64) < total;
        Ok(UsageLogsPayload {
            page: UsagePage {
                page,
                page_size,
                total,
                has_more,
            },
            items,
        })
    }

    pub fn list_usage_provider_stats(
        &self,
        req: UsageStatsListRequest,
    ) -> AppResult<UsageProviderStatsPayload> {
        let conn = self.open_connection()?;
        let (page, page_size, offset) = normalize_usage_pagination(req.page, req.page_size);
        let (where_clause, base_params) =
            build_run_usage_where_clause(&req.range, None, None, None)?;

        let count_sql = format!(
            "SELECT COUNT(*) FROM (
                SELECT 1 FROM run_usage_metrics{where_clause}
                GROUP BY provider_id, provider_name
            )"
        );
        let total = conn.query_row(&count_sql, params_from_iter(base_params.clone()), |row| {
            row.get::<_, i64>(0)
        })? as u64;

        let mut list_params = base_params;
        list_params.push(SqlValue::Integer(page_size as i64));
        list_params.push(SqlValue::Integer(offset as i64));
        let list_sql = format!(
            "SELECT
                provider_id, provider_name,
                COUNT(*) AS request_count,
                SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed_count,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed_count,
                SUM(CASE WHEN status = 'cancelled' THEN 1 ELSE 0 END) AS cancelled_count,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COALESCE(SUM(input_cache_read_tokens), 0) AS input_cache_read_tokens,
                COALESCE(SUM(input_cache_write_tokens), 0) AS input_cache_write_tokens
             FROM run_usage_metrics{where_clause}
             GROUP BY provider_id, provider_name
             ORDER BY request_count DESC, total_tokens DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt.query_map(params_from_iter(list_params), |row| {
            Ok(UsageProviderStatsItem {
                provider_id: row.get(0)?,
                provider_name: row.get(1)?,
                request_count: row.get::<_, i64>(2)? as u64,
                completed_count: row.get::<_, i64>(3)? as u64,
                failed_count: row.get::<_, i64>(4)? as u64,
                cancelled_count: row.get::<_, i64>(5)? as u64,
                input_tokens: row.get::<_, i64>(6)? as u64,
                output_tokens: row.get::<_, i64>(7)? as u64,
                total_tokens: row.get::<_, i64>(8)? as u64,
                input_cache_read_tokens: row.get::<_, i64>(9)? as u64,
                input_cache_write_tokens: row.get::<_, i64>(10)? as u64,
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        let has_more = (offset as u64 + page_size as u64) < total;
        Ok(UsageProviderStatsPayload {
            page: UsagePage {
                page,
                page_size,
                total,
                has_more,
            },
            items,
        })
    }

    pub fn list_usage_model_stats(
        &self,
        req: UsageStatsListRequest,
    ) -> AppResult<UsageModelStatsPayload> {
        let conn = self.open_connection()?;
        let (page, page_size, offset) = normalize_usage_pagination(req.page, req.page_size);
        let (where_clause, base_params) =
            build_run_usage_where_clause(&req.range, None, None, None)?;

        let count_sql = format!(
            "SELECT COUNT(*) FROM (
                SELECT 1 FROM run_usage_metrics{where_clause}
                GROUP BY model_id, model_name, model, provider_id, provider_name
            )"
        );
        let total = conn.query_row(&count_sql, params_from_iter(base_params.clone()), |row| {
            row.get::<_, i64>(0)
        })? as u64;

        let mut list_params = base_params;
        list_params.push(SqlValue::Integer(page_size as i64));
        list_params.push(SqlValue::Integer(offset as i64));
        let list_sql = format!(
            "SELECT
                model_id, model_name, model, provider_id, provider_name,
                COUNT(*) AS request_count,
                SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed_count,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed_count,
                SUM(CASE WHEN status = 'cancelled' THEN 1 ELSE 0 END) AS cancelled_count,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COALESCE(SUM(input_cache_read_tokens), 0) AS input_cache_read_tokens,
                COALESCE(SUM(input_cache_write_tokens), 0) AS input_cache_write_tokens,
                AVG(duration_ms) AS avg_duration_ms
             FROM run_usage_metrics{where_clause}
             GROUP BY model_id, model_name, model, provider_id, provider_name
             ORDER BY request_count DESC, total_tokens DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt.query_map(params_from_iter(list_params), |row| {
            Ok(UsageModelStatsItem {
                model_id: row.get(0)?,
                model_name: row.get(1)?,
                model: row.get(2)?,
                provider_id: row.get(3)?,
                provider_name: row.get(4)?,
                request_count: row.get::<_, i64>(5)? as u64,
                completed_count: row.get::<_, i64>(6)? as u64,
                failed_count: row.get::<_, i64>(7)? as u64,
                cancelled_count: row.get::<_, i64>(8)? as u64,
                input_tokens: row.get::<_, i64>(9)? as u64,
                output_tokens: row.get::<_, i64>(10)? as u64,
                total_tokens: row.get::<_, i64>(11)? as u64,
                input_cache_read_tokens: row.get::<_, i64>(12)? as u64,
                input_cache_write_tokens: row.get::<_, i64>(13)? as u64,
                avg_duration_ms: row
                    .get::<_, Option<f64>>(14)?
                    .map(|value| value.max(0.0).round() as u64),
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        let has_more = (offset as u64 + page_size as u64) < total;
        Ok(UsageModelStatsPayload {
            page: UsagePage {
                page,
                page_size,
                total,
                has_more,
            },
            items,
        })
    }

    pub fn list_usage_tool_stats(
        &self,
        req: UsageStatsListRequest,
    ) -> AppResult<UsageToolStatsPayload> {
        let conn = self.open_connection()?;
        let (page, page_size, offset) = normalize_usage_pagination(req.page, req.page_size);
        let (where_clause, base_params) =
            build_usage_range_where_clause(&req.range, "r.started_at")?;

        let count_sql = format!(
            "SELECT COUNT(*) FROM (
                SELECT 1
                FROM run_tool_metrics t
                LEFT JOIN run_usage_metrics r ON r.run_id = t.run_id
                {where_clause}
                GROUP BY t.tool_name, t.tool_action
            )"
        );
        let total = conn.query_row(&count_sql, params_from_iter(base_params.clone()), |row| {
            row.get::<_, i64>(0)
        })? as u64;

        let mut list_params = base_params;
        list_params.push(SqlValue::Integer(page_size as i64));
        list_params.push(SqlValue::Integer(offset as i64));
        let list_sql = format!(
            "SELECT
                t.tool_name,
                t.tool_action,
                COUNT(*) AS call_count,
                SUM(CASE WHEN t.is_error = 0 THEN 1 ELSE 0 END) AS success_count,
                SUM(CASE WHEN t.is_error = 1 THEN 1 ELSE 0 END) AS error_count,
                AVG(t.duration_ms) AS avg_duration_ms
             FROM run_tool_metrics t
             LEFT JOIN run_usage_metrics r ON r.run_id = t.run_id
             {where_clause}
             GROUP BY t.tool_name, t.tool_action
             ORDER BY call_count DESC, error_count DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt.query_map(params_from_iter(list_params), |row| {
            Ok(UsageToolStatsItem {
                tool_name: row.get(0)?,
                tool_action: row.get(1)?,
                call_count: row.get::<_, i64>(2)? as u64,
                success_count: row.get::<_, i64>(3)? as u64,
                error_count: row.get::<_, i64>(4)? as u64,
                avg_duration_ms: row
                    .get::<_, Option<f64>>(5)?
                    .map(|value| value.max(0.0).round() as u64),
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        let has_more = (offset as u64 + page_size as u64) < total;
        Ok(UsageToolStatsPayload {
            page: UsagePage {
                page,
                page_size,
                total,
                has_more,
            },
            items,
        })
    }

    pub fn usage_log_detail(&self, req: UsageLogDetailRequest) -> AppResult<UsageLogDetailPayload> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, session_id, tool_name, tool_action, status, duration_ms, is_error, created_at
             FROM run_tool_metrics
             WHERE run_id = ?1
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([req.run_id.as_str()], |row| {
            let duration_ms = row.get::<_, Option<i64>>(6)?.and_then(|value| {
                if value >= 0 {
                    Some(value as u64)
                } else {
                    None
                }
            });
            Ok(UsageToolLogItem {
                id: row.get(0)?,
                run_id: row.get(1)?,
                session_id: row.get(2)?,
                tool_name: row.get(3)?,
                tool_action: row.get(4)?,
                status: row.get(5)?,
                duration_ms,
                is_error: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
            })
        })?;
        let tools = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(UsageLogDetailPayload {
            run_id: req.run_id,
            tools,
        })
    }

    fn is_run_detail_logging_enabled(&self, run_id: &str) -> AppResult<bool> {
        let conn = self.open_connection()?;
        let value = conn
            .query_row(
                "SELECT detail_logged FROM run_usage_metrics WHERE run_id = ?1",
                [run_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if let Some(flag) = value {
            return Ok(flag != 0);
        }
        self.get_usage_detail_logging_enabled()
    }

    pub fn list_approvals(&self) -> AppResult<Vec<ToolApproval>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, run_id, call_id, action, path, preview_json, status, created_at, resolved_at
             FROM tool_approvals
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let preview_json = row.get::<_, String>(6)?;
            Ok(ToolApproval {
                id: row.get(0)?,
                session_id: row.get(1)?,
                run_id: row.get(2)?,
                call_id: row.get(3)?,
                action: row.get(4)?,
                path: row.get(5)?,
                preview_json: serde_json::from_str(&preview_json).unwrap_or(Value::Null),
                status: row.get(7)?,
                created_at: row.get(8)?,
                resolved_at: row.get(9)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_approval(&self, approval: &ToolApproval) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO tool_approvals (id, session_id, run_id, call_id, action, path, preview_json, status, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                approval.id,
                approval.session_id,
                approval.run_id,
                approval.call_id,
                approval.action,
                approval.path,
                serde_json::to_string(&approval.preview_json)?,
                approval.status,
                approval.created_at,
                approval.resolved_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_approval_status(
        &self,
        approval_id: &str,
        status: &str,
    ) -> AppResult<ToolApproval> {
        let conn = self.open_connection()?;
        let resolved_at = if status == "pending" {
            None
        } else {
            Some(now_timestamp())
        };
        conn.execute(
            "UPDATE tool_approvals SET status = ?2, resolved_at = ?3 WHERE id = ?1",
            params![approval_id, status, resolved_at],
        )?;
        conn.query_row(
            "SELECT id, session_id, run_id, call_id, action, path, preview_json, status, created_at, resolved_at
             FROM tool_approvals WHERE id = ?1",
            [approval_id],
            |row| {
                let preview_json = row.get::<_, String>(6)?;
                Ok(ToolApproval {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    run_id: row.get(2)?,
                    call_id: row.get(3)?,
                    action: row.get(4)?,
                    path: row.get(5)?,
                    preview_json: serde_json::from_str(&preview_json).unwrap_or(Value::Null),
                    status: row.get(7)?,
                    created_at: row.get(8)?,
                    resolved_at: row.get(9)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("approval `{approval_id}`")))
    }

    pub fn record_file_operation(
        &self,
        session_id: &str,
        run_id: &str,
        call_id: Option<&str>,
        action: &str,
        path: &str,
        status: &str,
        bytes_written: Option<usize>,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO file_operations (id, session_id, run_id, call_id, action, path, status, bytes_written, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                session_id,
                run_id,
                call_id,
                action,
                path,
                status,
                bytes_written.map(|value| value as i64),
                now_timestamp(),
            ],
        )?;
        Ok(())
    }

    pub fn set_last_opened_session_id(&self, session_id: Option<&str>) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('last_opened_session_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [session_id],
        )?;
        Ok(())
    }

    pub fn get_last_opened_session_id(&self) -> AppResult<Option<String>> {
        let conn = self.open_connection()?;
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'last_opened_session_id'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        Ok(value.flatten())
    }

    pub fn sessions_payload(&self) -> AppResult<SessionsChangedPayload> {
        Ok(SessionsChangedPayload {
            sessions: self.list_sessions()?,
            last_opened_session_id: self.get_last_opened_session_id()?,
        })
    }
}

fn normalize_usage_pagination(page: Option<u32>, page_size: Option<u32>) -> (u32, u32, u32) {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size
        .unwrap_or(DEFAULT_USAGE_PAGE_SIZE)
        .clamp(1, MAX_USAGE_PAGE_SIZE);
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    (page, page_size, offset)
}

fn usage_range_start(range: &str) -> AppResult<Option<String>> {
    let now = Utc::now();
    let start = match range {
        USAGE_RANGE_24H => Some(now - ChronoDuration::hours(24)),
        USAGE_RANGE_7D => Some(now - ChronoDuration::days(7)),
        USAGE_RANGE_30D => Some(now - ChronoDuration::days(30)),
        USAGE_RANGE_ALL | "" => None,
        other => {
            return Err(AppError::Validation(format!(
                "unsupported usage range `{other}`"
            )))
        }
    };
    Ok(start.map(|value| value.to_rfc3339_opts(SecondsFormat::Nanos, true)))
}

fn build_run_usage_where_clause(
    range: &str,
    model_id: Option<&str>,
    status: Option<&str>,
    detail_logged: Option<bool>,
) -> AppResult<(String, Vec<SqlValue>)> {
    let mut clauses = Vec::<String>::new();
    let mut params = Vec::<SqlValue>::new();

    if let Some(start_at) = usage_range_start(range)? {
        clauses.push("started_at >= ?".to_string());
        params.push(SqlValue::Text(start_at));
    }
    if let Some(model_id) = model_id.filter(|value| !value.trim().is_empty()) {
        clauses.push("model_id = ?".to_string());
        params.push(SqlValue::Text(model_id.to_string()));
    }
    if let Some(status) = status.filter(|value| !value.trim().is_empty()) {
        clauses.push("status = ?".to_string());
        params.push(SqlValue::Text(status.to_string()));
    }
    if let Some(detail_logged) = detail_logged {
        clauses.push("detail_logged = ?".to_string());
        params.push(SqlValue::Integer(if detail_logged { 1 } else { 0 }));
    }

    if clauses.is_empty() {
        return Ok((String::new(), params));
    }
    Ok((format!(" WHERE {}", clauses.join(" AND ")), params))
}

fn build_usage_range_where_clause(
    range: &str,
    started_at_column: &str,
) -> AppResult<(String, Vec<SqlValue>)> {
    if let Some(start_at) = usage_range_start(range)? {
        return Ok((
            format!(" WHERE {started_at_column} >= ?"),
            vec![SqlValue::Text(start_at)],
        ));
    }
    Ok((String::new(), Vec::new()))
}

fn calculate_duration_ms(started_at: Option<&str>, finished_at: Option<&str>) -> Option<i64> {
    let started_at = started_at?;
    let finished_at = finished_at?;
    let start = chrono::DateTime::parse_from_rfc3339(started_at).ok()?;
    let end = chrono::DateTime::parse_from_rfc3339(finished_at).ok()?;
    let duration = end.signed_duration_since(start).num_milliseconds();
    Some(duration.max(0))
}

fn normalize_language(value: String) -> String {
    let _ = value;
    "zh".to_string()
}

fn build_fts_query(query: &str) -> AppResult<String> {
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

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{build_fts_query, normalize_language, StorageService};
    use crate::backend::models::{
        new_chat_session, new_provider_account, new_provider_model, CreateProviderModelRequest,
        CreateProviderRequest,
    };

    #[test]
    fn persists_provider_profiles() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");
        let mut account = new_provider_account(CreateProviderRequest {
            profile_name: "Local".to_string(),
            base_url: "https://example.com".to_string(),
            api_key: "sk-test".to_string(),
        });
        let model = new_provider_model(CreateProviderModelRequest {
            provider_id: account.id.clone(),
            model_name: "gpt-test".to_string(),
            model: "gpt-test".to_string(),
        });
        account.models.push(model);
        storage
            .save_provider_accounts(std::slice::from_ref(&account))
            .expect("save provider");

        let sessions = storage.list_provider_profiles().expect("list providers");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Local");
    }

    #[test]
    fn creates_and_loads_sessions() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");
        let session = new_chat_session(None);
        storage.insert_session(&session).expect("insert session");
        storage
            .set_last_opened_session_id(Some(&session.id))
            .expect("set last open");

        let loaded = storage.list_sessions().expect("list sessions");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, session.id);
    }

    #[test]
    fn handles_null_last_opened_session_id() {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");

        storage
            .set_last_opened_session_id(None)
            .expect("set null last open");

        let loaded = storage
            .get_last_opened_session_id()
            .expect("load last opened session id");
        assert!(loaded.is_none());
    }

    #[test]
    fn language_is_zh_only() {
        assert_eq!(normalize_language("zh".to_string()), "zh");
        assert_eq!(normalize_language("en".to_string()), "zh");
        assert_eq!(normalize_language(" anything ".to_string()), "zh");
    }

    #[test]
    fn fts_query_requires_non_empty_input() {
        assert!(build_fts_query("   ").is_err());
        assert_eq!(
            build_fts_query("memory project").expect("query"),
            "\"memory\" AND \"project\""
        );
    }
}
