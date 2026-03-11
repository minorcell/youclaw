//! `memory_get` 工具定义。
//!
//! 按路径和行区间读取记忆文件，适合和 `memory_search` 配合使用。

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::models::MemoryGetRequest;
use crate::backend::BackendState;

#[derive(Debug, Deserialize)]
struct MemoryGetToolArgs {
    /// memory 文件路径（仅允许 MEMORY.md / PROFILE.md / memory/*.md）
    path: String,
    /// 起始行偏移（0-based）
    #[serde(default)]
    offset: Option<u32>,
    /// 最大返回行数
    #[serde(default)]
    limit: Option<u32>,
}

/// 构建 `memory_get` 工具。
///
/// 用于按路径 + 行区间读取记忆文件片段，便于模型增量获取上下文。
pub fn build_memory_get_tool(state: BackendState) -> Tool {
    tool("memory_get")
        .description("读取记忆文件片段。参数：path，可选 offset/limit。")
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string", "description": "记忆文件路径（白名单内）。" },
                "offset": { "type": ["integer", "null"], "minimum": 0, "description": "起始偏移行（0-based，可选）。" },
                "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": 1000, "description": "最大返回行数（可选）。" }
            },
            "required": ["path"]
        }))
        .execute_raw(move |value| {
            let state = state.clone();
            async move {
                let args = serde_json::from_value::<MemoryGetToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let payload = state
                    .memory_get(MemoryGetRequest {
                        path: args.path,
                        offset: args.offset,
                        limit: args.limit,
                    })
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                serde_json::to_value(payload)
                    .map_err(|err| ToolExecError::Execution(format!("serialize failed: {err}")))
            }
        })
}
