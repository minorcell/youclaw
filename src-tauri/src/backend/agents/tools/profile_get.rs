//! `profile_get` tool definition.

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::models::domain::ProfileTarget;
use crate::backend::models::requests::ProfileGetRequest;
use crate::backend::services::ProfileService;

#[derive(Debug, Deserialize)]
struct ProfileGetToolArgs {
    #[serde(default)]
    target: Option<ProfileTarget>,
}

pub fn build_profile_get_tool(profiles: ProfileService) -> Tool {
    tool("profile_get")
        .description(
            "Read the persisted user profile and agent soul profile. These profiles are injected into every conversation and should be treated separately from searchable long-term memory.",
        )
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "target": {
                    "type": ["string", "null"],
                    "enum": ["user", "soul", null],
                    "description": "Optional single profile target to read. Omit to read both."
                }
            }
        }))
        .execute_raw(move |value| {
            let profiles = profiles.clone();
            async move {
                let args = serde_json::from_value::<ProfileGetToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let payload = profiles
                    .get(ProfileGetRequest {
                        target: args.target,
                    })
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                serde_json::to_value(payload)
                    .map_err(|err| ToolExecError::Execution(format!("serialize failed: {err}")))
            }
        })
}
