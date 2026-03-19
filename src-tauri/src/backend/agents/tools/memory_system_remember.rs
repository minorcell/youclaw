//! `memory_system_remember` tool definition.

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::models::requests::MemorySystemUpsertRequest;
use crate::backend::services::MemoryService;

#[derive(Debug, Deserialize)]
struct MemorySystemRememberToolArgs {
    title: String,
    content: String,
}

pub fn build_memory_system_remember_tool(memory: MemoryService) -> Tool {
    tool("memory_system_remember")
        .description(
            "Save something to long-term memory so you can remember it in future conversations. Call this proactively when something worth keeping comes up — things the person told you, patterns you noticed, or anything you'd want to carry forward.",
        )
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "title": { "type": "string", "description": "Short stable label for the memory entry." },
                "content": { "type": "string", "description": "Long-term memory content to store." }
            },
            "required": ["title", "content"]
        }))
        .execute_raw(move |value| {
            let memory = memory.clone();
            async move {
                let args = serde_json::from_value::<MemorySystemRememberToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let payload = memory
                    .upsert(MemorySystemUpsertRequest {
                        id: None,
                        title: args.title,
                        content: args.content,
                    })
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                serde_json::to_value(payload)
                    .map_err(|err| ToolExecError::Execution(format!("serialize failed: {err}")))
            }
        })
}
