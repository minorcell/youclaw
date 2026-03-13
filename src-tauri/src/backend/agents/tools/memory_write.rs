//! `memory_write` 工具定义。
//!
//! 只允许改写记忆相关文件，写入后自动触发记忆索引刷新。

use aquaregia::tool::{tool, Tool, ToolExecError};
use chrono::Local;
use serde::Deserialize;
use serde_json::json;

use crate::backend::BackendState;

#[derive(Debug, Deserialize)]
struct MemoryWriteToolArgs {
    /// 目标路径（仅允许 MEMORY.md / PROFILE.md / memory/*.md / memory/today）
    path: String,
    /// 写入内容
    content: String,
    /// 是否追加写入；默认 false（覆盖）
    #[serde(default)]
    append: Option<bool>,
}

/// `memory/today` → `memory/YYYY-MM-DD.md`（使用本地时间）
fn resolve_path(path: &str) -> String {
    if path.trim_end_matches('/') == "memory/today" || path.trim() == "today" {
        let today = Local::now().format("%Y-%m-%d");
        format!("memory/{today}.md")
    } else {
        path.to_string()
    }
}

/// 追加模式下，在内容前自动插入系统时间戳行（`<!-- ts: HH:MM +0800 -->`）。
fn stamp_content(content: &str) -> String {
    let ts = Local::now().format("%H:%M %z");
    format!("<!-- ts: {ts} -->\n{content}")
}

/// 构建 `memory_write` 工具。
///
/// 该工具仅允许写入记忆相关文件，写完后会触发内存索引重建。
/// 时间信息由系统自动注入，模型无需在 path 或 content 中自行填写日期/时间。
pub fn build_memory_write_tool(state: BackendState) -> Tool {
    tool("memory_write")
        .description(
            "写入记忆文件（MEMORY.md / PROFILE.md / memory/*.md）。\
             path 填 \"memory/today\" 可自动写入今日笔记文件；\
             append=true 时系统会自动在内容前插入当前时间戳，无需手动填写时间。",
        )
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "记忆文件路径。可填 \"memory/today\" 自动解析为今日日期文件。"
                },
                "content": { "type": "string", "description": "要写入的内容。" },
                "append": {
                    "type": ["boolean", "null"],
                    "description": "是否追加写入（默认 false 覆盖）。追加时系统自动在内容前插入时间戳。"
                }
            },
            "required": ["path", "content"]
        }))
        .execute_raw(move |value| {
            let state = state.clone();
            async move {
                let args = serde_json::from_value::<MemoryWriteToolArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let append = args.append.unwrap_or(false);
                let resolved_path = resolve_path(&args.path);
                // 追加时自动注入系统时间戳，覆盖写入时不修改内容
                let content = if append {
                    stamp_content(&args.content)
                } else {
                    args.content.clone()
                };
                let written_path = state
                    .workspace
                    .write_memory_file(&resolved_path, &content, append)
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                let relative = state
                    .workspace
                    .relative_path(written_path.as_path())
                    .unwrap_or(resolved_path.clone());
                let reindex = state
                    .reindex_memory()
                    .map_err(|err| ToolExecError::Execution(format!("写入成功，但索引更新失败: {err}")))?;
                Ok(json!({
                    "action": "memory_write",
                    "path": relative,
                    "append": append,
                    "bytes_written": content.len(),
                    "indexed_chunks": reindex.indexed_chunks,
                }))
            }
        })
}
