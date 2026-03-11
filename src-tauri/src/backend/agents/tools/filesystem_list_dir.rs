//! `filesystem_list_dir` 工具定义。
//!
//! 该文件只负责“列目录”能力，避免与读写文件逻辑耦合。

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::json;

use super::filesystem_context::FilesystemToolContext;

/// 工具名常量：列目录。
pub const FILESYSTEM_LIST_DIR_TOOL_NAME: &str = "filesystem_list_dir";

#[derive(Debug, Deserialize)]
struct ListDirArgs {
    /// 文件夹路径（绝对路径，或相对 workspace_root）
    path: String,
}

/// 构建 `filesystem_list_dir` 工具。
///
/// 该工具只负责目录枚举，不混入读写逻辑，方便权限边界和审计归类。
pub fn build_filesystem_list_dir_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(FILESYSTEM_LIST_DIR_TOOL_NAME)
        .description(format!(
            "列出目录内容。路径可为绝对路径，或相对 workspace 根目录 `{workspace_root}`。"
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": format!("目录路径。可传绝对路径，或相对 workspace 根目录 `{workspace_root}`。")
                }
            },
            "required": ["path"]
        }))
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let args = serde_json::from_value::<ListDirArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                context
                    .list_dir(FILESYSTEM_LIST_DIR_TOOL_NAME, &args.path)
                    .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}
