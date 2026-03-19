//! `memory_system_get` tool definition.

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::models::requests::MemorySystemGetRequest;
use crate::backend::services::MemoryService;

#[derive(Debug, Deserialize)]
struct MemorySystemGetToolArgs {
    #[serde(rename = "memoryId")]
    memory_id: String,
}

pub fn build_memory_system_get_tool(memory: MemoryService) -> Tool {
    tool("memory_system_get")
        .description("Read a specific memory entry by id. Use this to retrieve the full content of something you found in search.")
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "memoryId": { "type": "string", "description": "Target memory entry id." }
            },
            "required": ["memoryId"]
        }))
        .execute_raw(move |value| {
            let memory = memory.clone();
            async move {
                let args = serde_json::from_value::<MemorySystemGetToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let payload = memory
                    .get(MemorySystemGetRequest {
                        id: args.memory_id,
                    })
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                serde_json::to_value(payload)
                    .map_err(|err| ToolExecError::Execution(format!("serialize failed: {err}")))
            }
        })
}
