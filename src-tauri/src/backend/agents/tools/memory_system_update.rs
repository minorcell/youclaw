//! `memory_system_update` tool definition.

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::services::MemoryService;

#[derive(Debug, Deserialize)]
struct MemorySystemUpdateToolArgs {
    #[serde(rename = "memoryId")]
    memory_id: String,
    title: String,
    content: String,
}

pub fn build_memory_system_update_tool(memory: MemoryService) -> Tool {
    tool("memory_system_update")
        .description(
            "Correct or update something you remembered wrong or incompletely. Use this when you realize an existing memory is outdated or no longer accurate.",
        )
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "memoryId": { "type": "string", "description": "Existing memory entry id." },
                "title": { "type": "string", "description": "Updated memory title." },
                "content": { "type": "string", "description": "Updated full memory content." }
            },
            "required": ["memoryId", "title", "content"]
        }))
        .execute_raw(move |value| {
            let memory = memory.clone();
            async move {
                let args = serde_json::from_value::<MemorySystemUpdateToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let payload = memory
                    .update_existing(&args.memory_id, &args.title, &args.content)
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                serde_json::to_value(payload)
                    .map_err(|err| ToolExecError::Execution(format!("serialize failed: {err}")))
            }
        })
}
