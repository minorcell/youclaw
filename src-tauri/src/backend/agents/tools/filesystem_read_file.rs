//! `filesystem_read_file` 工具定义。
//!
//! 支持 offset/limit 的按行切片读取，避免一次性返回过大文件内容。

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use super::filesystem_context::{FilesystemToolContext, MAX_READ_LIMIT};

/// 工具名常量：读文件。
pub const FILESYSTEM_READ_FILE_TOOL_NAME: &str = "filesystem_read_file";

#[derive(Debug, Deserialize)]
struct ReadFileArgs {
    /// 文件路径（绝对路径，或相对 workspace_root）
    path: String,
    /// 起始偏移行（0-based）
    #[serde(default)]
    offset: Option<usize>,
    /// 返回行数上限
    #[serde(default)]
    limit: Option<usize>,
}

/// 构建 `filesystem_read_file` 工具。
///
/// 支持按行切片读取，默认读取 200 行，用于控制上下文体积。
pub fn build_filesystem_read_file_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(FILESYSTEM_READ_FILE_TOOL_NAME)
        .description(format!(
            "读取文件内容（支持 offset/limit）。路径可为绝对路径，或相对 workspace 根目录 `{workspace_root}`。"
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": format!("文件路径。可传绝对路径，或相对 workspace 根目录 `{workspace_root}`。")
                },
                "offset": {
                    "type": ["integer", "null"],
                    "minimum": 0,
                    "description": "起始偏移行（0-based，可选）。"
                },
                "limit": {
                    "type": ["integer", "null"],
                    "minimum": 1,
                    "maximum": MAX_READ_LIMIT,
                    "description": format!("最大返回行数（可选，最大 {MAX_READ_LIMIT} 行）。")
                }
            },
            "required": ["path"]
        }))
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let args = serde_json::from_value::<ReadFileArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                context
                    .read_file(
                        FILESYSTEM_READ_FILE_TOOL_NAME,
                        &args.path,
                        args.offset.unwrap_or(0),
                        args.limit.unwrap_or(200),
                    )
                    .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}
