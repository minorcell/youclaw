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
                workspace_path TEXT,
                approval_mode TEXT NOT NULL DEFAULT 'default',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_turn_at TEXT,
                archived_at TEXT
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
            CREATE TABLE IF NOT EXISTS shell_executions (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                turn_id TEXT NOT NULL,
                call_id TEXT,
                command TEXT NOT NULL,
                cwd TEXT NOT NULL,
                status TEXT NOT NULL,
                exit_code INTEGER,
                signal INTEGER,
                duration_ms INTEGER,
                stdout_bytes INTEGER,
                stderr_bytes INTEGER,
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
                call_id TEXT,
                tool_name TEXT NOT NULL,
                tool_action TEXT,
                args_json TEXT NOT NULL DEFAULT '{}',
                status TEXT NOT NULL,
                duration_ms INTEGER,
                is_error INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                max_steps INTEGER NOT NULL DEFAULT 64,
                max_input_tokens INTEGER NOT NULL DEFAULT 120000,
                compact_ratio REAL NOT NULL DEFAULT 0.8,
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
            CREATE TABLE IF NOT EXISTS workspace_roots (
                path TEXT PRIMARY KEY,
                last_used_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_profiles (
                target TEXT PRIMARY KEY,
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS memory_entries (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS memory_entries_fts USING fts5(
                id UNINDEXED,
                title,
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
            CREATE INDEX IF NOT EXISTS idx_shell_executions_turn
            ON shell_executions (turn_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_chat_steps_turn
            ON chat_steps (turn_id, step);
            CREATE INDEX IF NOT EXISTS idx_chat_sessions_archived_last_turn
            ON chat_sessions (archived_at, last_turn_at DESC, updated_at DESC, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_message_marks_mark
            ON message_marks (mark, message_id);
            CREATE INDEX IF NOT EXISTS idx_workspace_roots_last_used_at
            ON workspace_roots (last_used_at DESC);
            CREATE INDEX IF NOT EXISTS idx_memory_entries_updated_at
            ON memory_entries (updated_at DESC, created_at DESC);
            ",
        )?;
        ensure_chat_sessions_approval_mode_column(&conn)?;
        ensure_chat_sessions_workspace_path_column(&conn)?;
        ensure_turn_tool_metrics_detail_columns(&conn)?;
        conn.execute(
            "INSERT OR IGNORE INTO agent_settings (
                id, max_steps, max_input_tokens, compact_ratio, language, updated_at
             ) VALUES (1, 64, 120000, 0.8, 'zh', ?1)",
            [now_timestamp()],
        )?;
        seed_default_agent_profiles(&conn)?;
        Ok(())
    }

    pub(super) fn open_connection(&self) -> AppResult<Connection> {
        let conn = Connection::open(&self.inner.db_path)?;
        conn.busy_timeout(Duration::from_secs(3))?;
        Ok(conn)
    }
}

fn ensure_chat_sessions_approval_mode_column(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(chat_sessions)")?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let has_approval_mode = columns
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .any(|column| column == "approval_mode");

    if !has_approval_mode {
        conn.execute(
            "ALTER TABLE chat_sessions ADD COLUMN approval_mode TEXT NOT NULL DEFAULT 'default'",
            [],
        )?;
    }

    Ok(())
}

fn ensure_chat_sessions_workspace_path_column(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(chat_sessions)")?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let has_workspace_path = columns
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .any(|column| column == "workspace_path");

    if !has_workspace_path {
        conn.execute("ALTER TABLE chat_sessions ADD COLUMN workspace_path TEXT", [])?;
    }

    Ok(())
}

fn seed_default_agent_profiles(conn: &Connection) -> AppResult<()> {
    let now = now_timestamp();
    for target in ["user", "soul"] {
        conn.execute(
            "INSERT OR IGNORE INTO agent_profiles (target, content, created_at, updated_at)
             VALUES (?1, '', ?2, ?2)",
            params![target, now],
        )?;
    }
    Ok(())
}

fn ensure_turn_tool_metrics_detail_columns(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(turn_tool_metrics)")?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let existing = columns.collect::<Result<Vec<_>, _>>()?;

    if !existing.iter().any(|column| column == "call_id") {
        conn.execute("ALTER TABLE turn_tool_metrics ADD COLUMN call_id TEXT", [])?;
    }

    if !existing.iter().any(|column| column == "args_json") {
        conn.execute(
            "ALTER TABLE turn_tool_metrics ADD COLUMN args_json TEXT NOT NULL DEFAULT '{}'",
            [],
        )?;
    }

    Ok(())
}
