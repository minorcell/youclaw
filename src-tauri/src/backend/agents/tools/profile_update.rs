//! `profile_update` tool definition.

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::models::domain::ProfileTarget;
use crate::backend::models::requests::ProfileUpdateRequest;
use crate::backend::services::ProfileService;

#[derive(Debug, Deserialize)]
struct ProfileUpdateToolArgs {
    target: ProfileTarget,
    content: String,
}

pub fn build_profile_update_tool(profiles: ProfileService) -> Tool {
    tool("profile_update")
        .description(
            "Replace the persisted user profile or agent soul profile after the user confirms stable long-term preferences, collaboration style, or operating principles.",
        )
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "target": {
                    "type": "string",
                    "enum": ["user", "soul"],
                    "description": "Which profile to replace."
                },
                "content": {
                    "type": "string",
                    "description": "The full new profile content to store."
                }
            },
            "required": ["target", "content"]
        }))
        .execute_raw(move |value| {
            let profiles = profiles.clone();
            async move {
                let args = serde_json::from_value::<ProfileUpdateToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let payload = profiles
                    .update(ProfileUpdateRequest {
                        target: args.target,
                        content: args.content,
                    })
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                serde_json::to_value(payload)
                    .map_err(|err| ToolExecError::Execution(format!("serialize failed: {err}")))
            }
        })
}
