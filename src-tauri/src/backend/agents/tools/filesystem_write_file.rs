//! `filesystem_write_file` 工具定义。
//!
//! 写文件属于高风险操作，真正执行前会走审批流程（见 `filesystem_context.rs`）。

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use super::filesystem_context::FilesystemToolContext;

/// 工具名常量：写文件（需审批）。
pub const FILESYSTEM_WRITE_FILE_TOOL_NAME: &str = "filesystem_write_file";

#[derive(Debug, Deserialize)]
struct WriteFileArgs {
    /// 文件路径（绝对路径，或相对 workspace_root）
    path: String,
    /// 要写入的完整内容（覆盖写）
    content: String,
}

/// 构建 `filesystem_write_file` 工具。
///
/// 该工具会先触发审批，再执行写入，确保写操作可控。
pub fn build_filesystem_write_file_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(FILESYSTEM_WRITE_FILE_TOOL_NAME)
        .description(format!(
            "写入文件（覆盖写，需用户审批）。路径可为绝对路径，或相对 workspace 根目录 `{workspace_root}`。"
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": format!("文件路径。可传绝对路径，或相对 workspace 根目录 `{workspace_root}`。")
                },
                "content": {
                    "type": "string",
                    "description": "要写入的完整文件内容（覆盖写）。"
                }
            },
            "required": ["path", "content"]
        }))
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let args = serde_json::from_value::<WriteFileArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                context
                    .write_file(FILESYSTEM_WRITE_FILE_TOOL_NAME, &args.path, &args.content)
                    .await
                    .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}
