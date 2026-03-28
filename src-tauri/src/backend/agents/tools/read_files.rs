//! `read_files` tool.

use std::fs;
use std::path::PathBuf;

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::backend::errors::{AppError, AppResult};

use super::filesystem_context::{
    truncate, validate_path, FilesystemToolContext, MAX_BATCH_READ_FILES, MAX_TOOL_OUTPUT_CHARS,
};

pub const READ_FILES_TOOL_NAME: &str = "read_files";

#[derive(Debug, Deserialize)]
struct ReadFilesArgs {
    paths: Vec<String>,
    #[serde(default, rename = "__youclaw_call_id")]
    tool_call_id: Option<String>,
}

/// 执行 `read_files` 工具的核心逻辑。
///
/// 批量读取时采用“单文件失败不影响整体”策略，错误以内联结果返回。
pub(crate) fn execute_read_files(
    context: &FilesystemToolContext,
    tool_name: &str,
    paths: &[String],
    tool_call_id: Option<&str>,
) -> AppResult<Value> {
    let tool_call = context.claim_tool_call(tool_name, tool_call_id)?;
    if paths.is_empty() {
        return Err(AppError::Validation("`paths` cannot be empty".to_string()));
    }
    if paths.len() > MAX_BATCH_READ_FILES {
        return Err(AppError::Validation(format!(
            "`paths` length must be <= {MAX_BATCH_READ_FILES}"
        )));
    }

    let mut results = Vec::<Value>::with_capacity(paths.len());
    for input_path in paths {
        match validate_path(input_path, &context.workspace_root).and_then(
            |resolved| -> AppResult<(PathBuf, String, usize)> {
                let metadata = fs::metadata(&resolved)?;
                if !metadata.is_file() {
                    return Err(AppError::Validation(format!(
                        "`{}` is not a file",
                        resolved.display()
                    )));
                }
                let bytes = fs::read(&resolved)?;
                let text = String::from_utf8_lossy(&bytes).to_string();
                let line_count = text.lines().count();
                Ok((resolved, text, line_count))
            },
        ) {
            Ok((resolved, content, line_count)) => {
                context.storage.record_file_operation(
                    &context.session_id,
                    &context.turn_id,
                    Some(&tool_call.call_id),
                    "read_text_file",
                    &resolved.to_string_lossy(),
                    "ok",
                    None,
                )?;

                let safe_content = truncate(&content, MAX_TOOL_OUTPUT_CHARS);
                results.push(json!({
                    "path": input_path,
                    "line_count": line_count,
                    "content": safe_content,
                }));
            }
            Err(err) => {
                context.storage.record_file_operation(
                    &context.session_id,
                    &context.turn_id,
                    Some(&tool_call.call_id),
                    "read_text_file",
                    input_path,
                    "error",
                    None,
                )?;

                let message = err.message();
                results.push(json!({
                    "path": input_path,
                    "error": message,
                }));
            }
        }
    }

    Ok(json!({
        "results": results,
    }))
}

pub fn build_read_files_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(READ_FILES_TOOL_NAME)
        .description(format!(
            "Read multiple text files in one call. Per-file errors are returned inline. Paths can be absolute or relative to workspace root `{workspace_root}`."
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "paths": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "minItems": 1,
                    "maxItems": MAX_BATCH_READ_FILES,
                    "description": format!("File paths to read, up to {MAX_BATCH_READ_FILES} paths.")
                }
            },
            "required": ["paths"]
        }))
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let args = serde_json::from_value::<ReadFilesArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                context
                    .read_files(
                        READ_FILES_TOOL_NAME,
                        &args.paths,
                        args.tool_call_id.as_deref(),
                    )
                    .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}
