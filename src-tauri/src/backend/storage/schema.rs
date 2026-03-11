use super::*;

impl StorageService {
    pub(super) fn initialize(&self) -> AppResult<()> {
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

    pub(super) fn open_connection(&self) -> AppResult<Connection> {
        let conn = Connection::open(&self.inner.db_path)?;
        conn.busy_timeout(Duration::from_secs(3))?;
        Ok(conn)
    }
}
