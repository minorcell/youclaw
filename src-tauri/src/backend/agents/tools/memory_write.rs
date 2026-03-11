//! `memory_write` 工具定义。
//!
//! 只允许改写记忆相关文件，写入后自动触发记忆索引刷新。

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use crate::backend::BackendState;

#[derive(Debug, Deserialize)]
struct MemoryWriteToolArgs {
    /// 目标路径（仅允许 MEMORY.md / PROFILE.md / memory/*.md）
    path: String,
    /// 写入内容
    content: String,
    /// 是否追加写入；默认 false（覆盖）
    #[serde(default)]
    append: Option<bool>,
}

/// 构建 `memory_write` 工具。
///
/// 该工具仅允许写入记忆相关文件，写完后会触发内存索引重建。
pub fn build_memory_write_tool(state: BackendState) -> Tool {
    tool("memory_write")
        .description("写入记忆文件（MEMORY.md / PROFILE.md / memory/*.md）。")
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string", "description": "记忆文件路径（白名单内）。" },
                "content": { "type": "string", "description": "要写入的内容。" },
                "append": { "type": ["boolean", "null"], "description": "是否追加写入（默认 false 覆盖）。" }
            },
            "required": ["path", "content"]
        }))
        .execute_raw(move |value| {
            let state = state.clone();
            async move {
                let args = serde_json::from_value::<MemoryWriteToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let append = args.append.unwrap_or(false);
                let written_path = state
                    .workspace
                    .write_memory_file(&args.path, &args.content, append)
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                let relative = state
                    .workspace
                    .relative_path(written_path.as_path())
                    .unwrap_or(args.path.clone());
                let _ = state.reindex_memory();
                Ok(json!({
                    "action": "memory_write",
                    "path": relative,
                    "append": append,
                    "bytes_written": args.content.len(),
                }))
            }
        })
}
