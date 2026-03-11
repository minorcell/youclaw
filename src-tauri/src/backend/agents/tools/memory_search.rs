//! `memory_search` 工具定义。
//!
//! 面向记忆索引做语义检索，适合“先搜再读”的调用模式。

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::BackendState;

#[derive(Debug, Deserialize)]
struct MemorySearchToolArgs {
    /// 搜索关键词
    query: String,
    /// 返回条数上限
    #[serde(default)]
    max_results: Option<u32>,
    /// 最低相似度阈值
    #[serde(default)]
    min_score: Option<f32>,
}

/// 构建 `memory_search` 工具。
///
/// 用于在 MEMORY.md / PROFILE.md / memory/*.md 的索引中做语义检索。
pub fn build_memory_search_tool(state: BackendState) -> Tool {
    tool("memory_search")
        .description("搜索长期记忆和用户资料。参数：query，可选 max_results / min_score。")
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": { "type": "string", "description": "搜索关键词。" },
                "max_results": { "type": ["integer", "null"], "minimum": 1, "maximum": 100, "description": "返回条数上限（可选）。" },
                "min_score": { "type": ["number", "null"], "minimum": 0, "maximum": 1, "description": "最低相似度阈值（可选）。" }
            },
            "required": ["query"]
        }))
        .execute_raw(move |value| {
            let state = state.clone();
            async move {
                let args = serde_json::from_value::<MemorySearchToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let payload = state
                    .storage
                    .memory_search(
                        args.query.trim(),
                        args.max_results.unwrap_or(8),
                        args.min_score.unwrap_or(0.05),
                    )
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                serde_json::to_value(payload)
                    .map_err(|err| ToolExecError::Execution(format!("serialize failed: {err}")))
            }
        })
}
