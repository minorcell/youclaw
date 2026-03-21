//! `search_files` tool.

use std::fs;

use aquaregia::tool::{tool, Tool, ToolExecError};
use glob::Pattern;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::backend::errors::{AppError, AppResult};

use super::filesystem_context::{
    resolve_allowed_directories, search_directory_recursive, validate_path, FilesystemToolContext,
    ToolIgnorePolicy,
};

pub const SEARCH_FILES_TOOL_NAME: &str = "search_files";

#[derive(Debug, Deserialize)]
struct SearchFilesArgs {
    path: String,
    pattern: String,
    #[serde(default, rename = "excludePatterns")]
    exclude_patterns: Vec<String>,
    #[serde(default, rename = "__youclaw_call_id")]
    tool_call_id: Option<String>,
}

/// 执行 `search_files` 工具的核心逻辑。
pub(crate) fn execute_search_files(
    context: &FilesystemToolContext,
    tool_name: &str,
    input_path: &str,
    pattern: &str,
    exclude_patterns: &[String],
    tool_call_id: Option<&str>,
) -> AppResult<Value> {
    let tool_call = context.claim_tool_call(tool_name, tool_call_id)?;
    if pattern.trim().is_empty() {
        return Err(AppError::Validation(
            "`pattern` cannot be empty".to_string(),
        ));
    }

    let resolved = validate_path(input_path, &context.workspace_root)?;
    let metadata = fs::metadata(&resolved)?;
    if !metadata.is_dir() {
        return Err(AppError::Validation(format!(
            "`{}` is not a directory",
            resolved.display()
        )));
    }

    let include = Pattern::new(pattern)
        .map_err(|err| AppError::Validation(format!("invalid glob pattern: {err}")))?;
    let excludes = exclude_patterns
        .iter()
        .map(|raw| {
            Pattern::new(raw).map_err(|err| {
                AppError::Validation(format!("invalid exclude pattern `{raw}`: {err}"))
            })
        })
        .collect::<AppResult<Vec<_>>>()?;

    let allowed_dirs = resolve_allowed_directories(&context.workspace_root);
    let ignore_policy = ToolIgnorePolicy::new(&context.workspace_root);
    let mut matches = Vec::<String>::new();
    let mut filtered_ignored_paths = 0usize;
    search_directory_recursive(
        &resolved,
        &resolved,
        &allowed_dirs,
        &ignore_policy,
        &include,
        &excludes,
        &mut matches,
        &mut filtered_ignored_paths,
    )?;

    context.storage.record_file_operation(
        &context.session_id,
        &context.turn_id,
        Some(&tool_call.call_id),
        "search_files",
        &resolved.to_string_lossy(),
        "ok",
        None,
    )?;

    Ok(json!({
        "count": matches.len(),
        "filtered_ignored_paths": filtered_ignored_paths,
        "matches": matches,
    }))
}

pub fn build_search_files_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(SEARCH_FILES_TOOL_NAME)
        .description(format!(
            "Recursively search files/directories by glob pattern under a root directory. Paths can be absolute or relative to workspace root `{workspace_root}`."
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": format!("Root directory path. Accepts absolute path or path relative to workspace root `{workspace_root}`.")
                },
                "pattern": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Glob pattern to match, e.g. `**/*.rs`."
                },
                "excludePatterns": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "default": [],
                    "description": "Glob patterns to exclude."
                }
            },
            "required": ["path", "pattern"]
        }))
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let args = serde_json::from_value::<SearchFilesArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                context
                    .search_files(
                        SEARCH_FILES_TOOL_NAME,
                        &args.path,
                        &args.pattern,
                        &args.exclude_patterns,
                        args.tool_call_id.as_deref(),
                    )
                    .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}
