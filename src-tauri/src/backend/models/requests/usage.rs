use serde::{Deserialize, Serialize};

use crate::backend::models::USAGE_RANGE_7D;

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
