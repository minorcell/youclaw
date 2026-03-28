//! `memory_system_search` tool definition.

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::models::requests::MemorySystemSearchRequest;
use crate::backend::services::MemoryService;

#[derive(Debug, Deserialize)]
struct MemorySystemSearchToolArgs {
    query: String,
    #[serde(default, rename = "maxResults")]
    max_results: Option<u32>,
    #[serde(default, rename = "minScore")]
    min_score: Option<f32>,
}

pub fn build_memory_system_search_tool(memory: MemoryService) -> Tool {
    tool("memory_system_search")
        .description(
            "Search your long-term memory. Use this before answering questions about anything you might have been told before — people, events, preferences, or past conversations.",
        )
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": { "type": "string", "description": "Search query." },
                "maxResults": { "type": ["integer", "null"], "minimum": 1, "maximum": 100, "description": "Maximum number of results." },
                "minScore": { "type": ["number", "null"], "minimum": 0, "maximum": 1, "description": "Minimum match score." }
            },
            "required": ["query"]
        }))
        .execute_raw(move |value| {
            let memory = memory.clone();
            async move {
                let args = serde_json::from_value::<MemorySystemSearchToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let payload = memory
                    .search(MemorySystemSearchRequest {
                        query: args.query,
                        max_results: args.max_results,
                        min_score: args.min_score,
                    })
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                serde_json::to_value(payload)
                    .map_err(|err| ToolExecError::Execution(format!("serialize failed: {err}")))
            }
        })
}
