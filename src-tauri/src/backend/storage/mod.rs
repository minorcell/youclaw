mod memory;
mod providers;
mod schema;
mod sessions;
mod usage;

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
    MemoryReindexPayload, MemorySearchHit, MemorySearchPayload, MessageRole, ProviderAccount,
    ProviderProfile, RunStatus, SessionsChangedPayload, StoredProviders, ToolApproval,
    UsageLogDetailPayload, UsageLogDetailRequest, UsageLogItem, UsageLogsListRequest,
    UsageLogsPayload, UsageModelStatsItem, UsageModelStatsPayload, UsagePage,
    UsageProviderStatsItem, UsageProviderStatsPayload, UsageSettingsPayload, UsageStatsListRequest,
    UsageSummaryPayload, UsageSummaryRequest, UsageToolLogItem, UsageToolStatsItem,
    UsageToolStatsPayload, USAGE_RANGE_24H, USAGE_RANGE_30D, USAGE_RANGE_7D, USAGE_RANGE_ALL,
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
}

fn normalize_language(value: String) -> String {
    let _ = value;
    "zh".to_string()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{memory::build_fts_query, normalize_language, StorageService};
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
