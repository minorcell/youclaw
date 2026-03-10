use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::{
    flatten_provider_profiles, message_from_record, migrate_provider_accounts_from_legacy,
    normalize_provider_accounts, now_timestamp, record_from_message, BootstrapPayload, ChatMessage,
    ChatRun, ChatSession, ProviderAccount, ProviderProfile, SessionsChangedPayload,
    StoredProviders, ToolApproval,
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
            ",
        )?;
        Ok(())
    }

    fn open_connection(&self) -> AppResult<Connection> {
        let conn = Connection::open(&self.inner.db_path)?;
        conn.busy_timeout(Duration::from_secs(3))?;
        Ok(conn)
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
            "DELETE FROM chat_messages WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM tool_approvals WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute("DELETE FROM chat_runs WHERE session_id = ?1", [session_id])?;
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

    pub fn replace_session_messages(
        &self,
        session_id: &str,
        run_id: &str,
        messages: &[aquaregia::Message],
    ) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        conn.execute(
            "DELETE FROM chat_messages WHERE session_id = ?1",
            [session_id],
        )?;
        let tx = conn.unchecked_transaction()?;
        let mut persisted = Vec::with_capacity(messages.len());
        for message in messages {
            let record = record_from_message(session_id, run_id, message)?;
            tx.execute(
                "INSERT INTO chat_messages (id, session_id, role, parts_json, run_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    record.id,
                    record.session_id,
                    record.role,
                    serde_json::to_string(&record.parts_json)?,
                    record.run_id,
                    record.created_at,
                ],
            )?;
            persisted.push(record);
        }
        tx.commit()?;
        Ok(persisted)
    }

    pub fn insert_message(&self, message: &ChatMessage) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
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
        Ok(())
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

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::StorageService;
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
}
