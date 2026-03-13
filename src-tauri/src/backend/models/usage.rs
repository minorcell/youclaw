use serde::{Deserialize, Serialize};

pub const USAGE_RANGE_24H: &str = "24h";
pub const USAGE_RANGE_7D: &str = "7d";
pub const USAGE_RANGE_30D: &str = "30d";
pub const USAGE_RANGE_ALL: &str = "all";

fn default_usage_range() -> String {
    USAGE_RANGE_7D.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummaryRequest {
    #[serde(default = "default_usage_range")]
    pub range: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLogsListRequest {
    #[serde(default = "default_usage_range")]
    pub range: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_logged: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStatsListRequest {
    #[serde(default = "default_usage_range")]
    pub range: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLogDetailRequest {
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsagePage {
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummaryPayload {
    pub range: String,
    pub total_turns: u64,
    pub total_steps: u64,
    pub avg_steps_per_turn: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub input_cache_read_tokens: u64,
    pub input_cache_write_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLogItem {
    pub turn_id: String,
    pub session_id: String,
    pub status: String,
    pub user_message: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub model: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub step_count: u32,
    pub detail_logged: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub input_cache_read_tokens: u64,
    pub input_cache_write_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLogsPayload {
    pub page: UsagePage,
    pub items: Vec<UsageLogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageProviderStatsItem {
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub turn_count: u64,
    pub completed_count: u64,
    pub failed_count: u64,
    pub cancelled_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub input_cache_read_tokens: u64,
    pub input_cache_write_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageProviderStatsPayload {
    pub page: UsagePage,
    pub items: Vec<UsageProviderStatsItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageModelStatsItem {
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub model: Option<String>,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub turn_count: u64,
    pub completed_count: u64,
    pub failed_count: u64,
    pub cancelled_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub input_cache_read_tokens: u64,
    pub input_cache_write_tokens: u64,
    pub avg_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageModelStatsPayload {
    pub page: UsagePage,
    pub items: Vec<UsageModelStatsItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageToolStatsItem {
    pub tool_name: String,
    pub tool_action: Option<String>,
    pub call_count: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub avg_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageToolStatsPayload {
    pub page: UsagePage,
    pub items: Vec<UsageToolStatsItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageToolLogItem {
    pub id: String,
    pub turn_id: String,
    pub session_id: String,
    pub tool_name: String,
    pub tool_action: Option<String>,
    pub status: String,
    pub duration_ms: Option<u64>,
    pub is_error: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLogDetailPayload {
    pub turn_id: String,
    pub tools: Vec<UsageToolLogItem>,
}
