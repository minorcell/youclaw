//! `list_directory` tool.

use std::fs;

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::backend::errors::{AppError, AppResult};

use super::filesystem_context::{
    truncate, validate_path, FilesystemToolContext, ToolIgnorePolicy, MAX_TOOL_OUTPUT_CHARS,
};

pub const LIST_DIRECTORY_TOOL_NAME: &str = "list_directory";

#[derive(Debug, Deserialize)]
struct ListDirectoryArgs {
    path: String,
    #[serde(default, rename = "__youclaw_call_id")]
    tool_call_id: Option<String>,
}

/// 执行 `list_directory` 工具的核心逻辑。
///
/// 该实现负责：
/// - 目录路径校验；
/// - 工具执行期忽略策略（含 `.gitignore` 与常见噪音目录）；
/// - 统一的审计记录。
pub(crate) fn execute_list_directory(
    context: &FilesystemToolContext,
    tool_name: &str,
    input_path: &str,
    tool_call_id: Option<&str>,
) -> AppResult<Value> {
    let tool_call = context.claim_tool_call(tool_name, tool_call_id)?;
    let resolved = validate_path(input_path, &context.workspace_root)?;
    let metadata = fs::metadata(&resolved)?;
    if !metadata.is_dir() {
        return Err(AppError::Validation(format!(
            "`{}` is not a directory",
            resolved.display()
        )));
    }

    let ignore_policy = ToolIgnorePolicy::new(&context.workspace_root);
    let mut entries = Vec::<Value>::new();
    let mut filtered_ignored_entries = 0usize;
    for entry in fs::read_dir(&resolved)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let is_dir = metadata.is_dir();
        if ignore_policy.should_ignore_path(&entry.path(), is_dir) {
            filtered_ignored_entries += 1;
            continue;
        }
        entries.push(json!({
            "name": entry.file_name().to_string_lossy().to_string(),
            "path": entry.path().to_string_lossy().to_string(),
            "kind": if is_dir { "dir" } else { "file" },
            "label": if is_dir { "[DIR]" } else { "[FILE]" },
            "size_bytes": if metadata.is_file() { Some(metadata.len()) } else { None },
        }));
    }

    entries.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .cmp(&right.get("name").and_then(Value::as_str))
    });

    let formatted = entries
        .iter()
        .map(|item| {
            let label = item
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("[FILE]");
            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
            format!("{label} {name}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    context.storage.record_file_operation(
        &context.session_id,
        &context.turn_id,
        Some(&tool_call.call_id),
        "list_directory",
        &resolved.to_string_lossy(),
        "ok",
        None,
    )?;

    Ok(json!({
        "action": "list_directory",
        "path": resolved.to_string_lossy(),
        "entries": entries,
        "filtered_ignored_entries": filtered_ignored_entries,
        "formatted": truncate(&formatted, MAX_TOOL_OUTPUT_CHARS),
    }))
}

pub fn build_list_directory_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(LIST_DIRECTORY_TOOL_NAME)
        .description(format!(
            "List direct children of a directory using [DIR]/[FILE] labels. Paths can be absolute or relative to workspace root `{workspace_root}`."
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": format!("Directory path. Accepts absolute path or path relative to workspace root `{workspace_root}`.")
                }
            },
            "required": ["path"]
        }))
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let args = serde_json::from_value::<ListDirectoryArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                context
                    .list_directory(
                        LIST_DIRECTORY_TOOL_NAME,
                        &args.path,
                        args.tool_call_id.as_deref(),
                    )
                    .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}
