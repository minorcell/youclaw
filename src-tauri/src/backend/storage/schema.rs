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
                last_turn_at TEXT
            );
            CREATE TABLE IF NOT EXISTS chat_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                parts_json TEXT NOT NULL,
                turn_id TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chat_turns (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                status TEXT NOT NULL,
                user_message TEXT NOT NULL,
                output_text TEXT NOT NULL,
                created_at TEXT NOT NULL,
                finished_at TEXT,
                error_message TEXT
            );
            CREATE TABLE IF NOT EXISTS chat_steps (
                turn_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                step INTEGER NOT NULL,
                output_text TEXT NOT NULL,
                reasoning_text TEXT NOT NULL,
                reasoning_parts_json TEXT NOT NULL,
                finish_reason_json TEXT NOT NULL,
                usage_json TEXT NOT NULL,
                tool_calls_json TEXT NOT NULL,
                tool_results_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (turn_id, step)
            );
            CREATE TABLE IF NOT EXISTS tool_approvals (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                turn_id TEXT NOT NULL,
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
                turn_id TEXT NOT NULL,
                call_id TEXT,
                action TEXT NOT NULL,
                path TEXT NOT NULL,
                status TEXT NOT NULL,
                bytes_written INTEGER,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS turn_usage_metrics (
                turn_id TEXT PRIMARY KEY,
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
                step_count INTEGER NOT NULL DEFAULT 0,
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
            CREATE TABLE IF NOT EXISTS turn_tool_metrics (
                id TEXT PRIMARY KEY,
                turn_id TEXT NOT NULL,
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
                file_hash TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT 'memory',
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS memory_source_files (
                path TEXT PRIMARY KEY,
                file_hash TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                mtime_ms INTEGER NOT NULL,
                indexed_at TEXT NOT NULL,
                source TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS memory_chunks_fts USING fts5(
                id UNINDEXED,
                path UNINDEXED,
                heading,
                content
            );
            CREATE INDEX IF NOT EXISTS idx_turn_usage_metrics_started_at
            ON turn_usage_metrics (started_at DESC, turn_id DESC);
            CREATE INDEX IF NOT EXISTS idx_turn_usage_metrics_model
            ON turn_usage_metrics (model_id, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_turn_usage_metrics_status
            ON turn_usage_metrics (status, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_turn_tool_metrics_turn
            ON turn_tool_metrics (turn_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_turn_tool_metrics_tool
            ON turn_tool_metrics (tool_name, tool_action, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_chat_steps_turn
            ON chat_steps (turn_id, step);
            CREATE INDEX IF NOT EXISTS idx_message_marks_mark
            ON message_marks (mark, message_id);
            CREATE INDEX IF NOT EXISTS idx_memory_chunks_path
            ON memory_chunks (path);
            CREATE INDEX IF NOT EXISTS idx_memory_source_files_source
            ON memory_source_files (source);
            ",
        )?;
        ensure_memory_schema_fields(&conn)?;
        conn.execute(
            "INSERT OR IGNORE INTO agent_settings (
                id, max_steps, max_input_tokens, compact_ratio, keep_recent,
                language, updated_at
             ) VALUES (1, 8, 32768, 0.7, 8, 'zh', ?1)",
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

fn ensure_memory_schema_fields(conn: &Connection) -> AppResult<()> {
    ensure_table_column(
        conn,
        "memory_chunks",
        "file_hash",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_table_column(
        conn,
        "memory_chunks",
        "source",
        "TEXT NOT NULL DEFAULT 'memory'",
    )?;
    Ok(())
}

fn ensure_table_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> AppResult<()> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&pragma)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name = row.get::<_, String>(1)?;
        if name == column {
            return Ok(());
        }
    }

    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    conn.execute(&sql, [])?;
    Ok(())
}
