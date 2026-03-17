//! `memory_get` 工具定义。
//!
//! 按路径和行区间读取记忆文件，适合和 `memory_search` 配合使用。

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::models::requests::MemoryGetRequest;
use crate::backend::services::MemoryService;

#[derive(Debug, Deserialize)]
struct MemoryGetToolArgs {
    /// memory 文件路径（仅允许 MEMORY.md / memory/*.md）
    path: String,
    /// 起始行（0-based）
    #[serde(default)]
    from: Option<u32>,
    /// 最大返回行数
    #[serde(default)]
    lines: Option<u32>,
}

/// 构建 `memory_get` 工具。
///
/// 用于按路径 + 行区间读取记忆文件片段，便于模型增量获取上下文。
pub fn build_memory_get_tool(memory: MemoryService) -> Tool {
    tool("memory_get")
        .description("读取记忆文件片段。参数：path，可选 from/lines。")
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string", "description": "记忆文件路径（白名单内）。" },
                "from": { "type": ["integer", "null"], "minimum": 0, "description": "起始偏移行（0-based，可选）。" },
                "lines": { "type": ["integer", "null"], "minimum": 1, "maximum": 2000, "description": "最大返回行数（可选）。" }
            },
            "required": ["path"]
        }))
        .execute_raw(move |value| {
            let memory = memory.clone();
            async move {
                let args = serde_json::from_value::<MemoryGetToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let payload = memory
                    .get(MemoryGetRequest {
                        path: args.path,
                        from: args.from,
                        lines: args.lines,
                    })
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                serde_json::to_value(payload)
                    .map_err(|err| ToolExecError::Execution(format!("serialize failed: {err}")))
            }
        })
}
