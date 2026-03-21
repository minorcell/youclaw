//! `read_text_file` tool.

use std::fs;

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::backend::errors::{AppError, AppResult};

use super::filesystem_context::{
    head_lines, tail_lines, truncate, validate_path, FilesystemToolContext, MAX_HEAD_TAIL_LIMIT,
    MAX_TOOL_OUTPUT_CHARS,
};

pub const READ_TEXT_FILE_TOOL_NAME: &str = "read_text_file";

#[derive(Debug, Deserialize)]
struct ReadTextFileArgs {
    path: String,
    #[serde(default)]
    head: Option<usize>,
    #[serde(default)]
    tail: Option<usize>,
    #[serde(default, rename = "__youclaw_call_id")]
    tool_call_id: Option<String>,
}

/// 执行 `read_text_file` 工具的核心逻辑。
pub(crate) fn execute_read_text_file(
    context: &FilesystemToolContext,
    tool_name: &str,
    input_path: &str,
    head: Option<usize>,
    tail: Option<usize>,
    tool_call_id: Option<&str>,
) -> AppResult<Value> {
    let tool_call = context.claim_tool_call(tool_name, tool_call_id)?;
    if head.is_some() && tail.is_some() {
        return Err(AppError::Validation(
            "cannot set both `head` and `tail`".to_string(),
        ));
    }
    if let Some(value) = head.or(tail) {
        if value == 0 || value > MAX_HEAD_TAIL_LIMIT {
            return Err(AppError::Validation(format!(
                "line limit must be in 1..={MAX_HEAD_TAIL_LIMIT}"
            )));
        }
    }

    let resolved = validate_path(input_path, &context.workspace_root)?;
    let metadata = fs::metadata(&resolved)?;
    if !metadata.is_file() {
        return Err(AppError::Validation(format!(
            "`{}` is not a file",
            resolved.display()
        )));
    }

    let bytes = fs::read(&resolved)?;
    let full_text = String::from_utf8_lossy(&bytes).to_string();
    let selected = if let Some(lines) = head {
        head_lines(&full_text, lines)
    } else if let Some(lines) = tail {
        tail_lines(&full_text, lines)
    } else {
        full_text.clone()
    };

    context.storage.record_file_operation(
        &context.session_id,
        &context.turn_id,
        Some(&tool_call.call_id),
        "read_text_file",
        &resolved.to_string_lossy(),
        "ok",
        None,
    )?;

    let total_lines = full_text.lines().count();
    let truncated = selected.chars().count() > MAX_TOOL_OUTPUT_CHARS;

    Ok(json!({
        "total_lines": total_lines,
        "truncated": truncated,
        "content": truncate(&selected, MAX_TOOL_OUTPUT_CHARS),
    }))
}

pub fn build_read_text_file_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(READ_TEXT_FILE_TOOL_NAME)
        .description(format!(
            "Read text file content. Supports optional head/tail line limits. Paths can be absolute or relative to workspace root `{workspace_root}`."
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": format!("File path. Accepts absolute path or path relative to workspace root `{workspace_root}`.")
                },
                "head": {
                    "type": ["integer", "null"],
                    "minimum": 1,
                    "maximum": MAX_HEAD_TAIL_LIMIT,
                    "description": format!("Read first N lines (1..={MAX_HEAD_TAIL_LIMIT}).")
                },
                "tail": {
                    "type": ["integer", "null"],
                    "minimum": 1,
                    "maximum": MAX_HEAD_TAIL_LIMIT,
                    "description": format!("Read last N lines (1..={MAX_HEAD_TAIL_LIMIT}).")
                }
            },
            "required": ["path"]
        }))
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let args = serde_json::from_value::<ReadTextFileArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                context
                    .read_text_file(
                        READ_TEXT_FILE_TOOL_NAME,
                        &args.path,
                        args.head,
                        args.tail,
                        args.tool_call_id.as_deref(),
                    )
                    .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}
