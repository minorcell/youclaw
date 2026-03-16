//! `edit_file` tool.

use std::fs;

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::new_tool_approval;

use super::filesystem_context::{
    apply_ordered_edits, await_approval, build_mutation_preview, create_unified_diff,
    read_text_if_exists, truncate, validate_path, write_file_content_atomic, FileEdit,
    FilesystemToolContext, MAX_TOOL_OUTPUT_CHARS,
};

pub const EDIT_FILE_TOOL_NAME: &str = "edit_file";

#[derive(Debug, Deserialize)]
struct EditItemArgs {
    #[serde(rename = "oldText")]
    old_text: String,
    #[serde(rename = "newText")]
    new_text: String,
}

#[derive(Debug, Deserialize)]
struct EditFileArgs {
    path: String,
    edits: Vec<EditItemArgs>,
    #[serde(default, rename = "dryRun")]
    dry_run: Option<bool>,
    #[serde(default, rename = "__youclaw_call_id")]
    tool_call_id: Option<String>,
}

/// 执行 `edit_file` 工具的核心逻辑。
///
/// 支持 `dry_run` 预览；非 dry-run 需要人工审批后落盘。
pub(crate) async fn execute_edit_file(
    context: &FilesystemToolContext,
    tool_name: &str,
    input_path: &str,
    edits: &[FileEdit],
    dry_run: bool,
    tool_call_id: Option<&str>,
) -> AppResult<Value> {
    let tool_call = context.claim_tool_call(tool_name, tool_call_id)?;
    if edits.is_empty() {
        return Err(AppError::Validation("`edits` cannot be empty".to_string()));
    }

    let resolved = validate_path(input_path, &context.workspace_root)?;
    let metadata = fs::metadata(&resolved)?;
    if !metadata.is_file() {
        return Err(AppError::Validation(format!(
            "`{}` is not a file",
            resolved.display()
        )));
    }

    let previous = read_text_if_exists(&resolved)?;
    let next = apply_ordered_edits(&previous, edits)?;
    let diff = create_unified_diff(&previous, &next, &resolved);

    if dry_run {
        context.storage.record_file_operation(
            &context.session_id,
            &context.turn_id,
            Some(&tool_call.call_id),
            "edit_file",
            &resolved.to_string_lossy(),
            "dry_run",
            None,
        )?;
        return Ok(json!({
            "action": "edit_file",
            "path": resolved.to_string_lossy(),
            "dry_run": true,
            "diff": truncate(&diff, MAX_TOOL_OUTPUT_CHARS),
        }));
    }

    if context.should_skip_mutation_approval() {
        write_file_content_atomic(&resolved, &next)?;
        context.storage.record_file_operation(
            &context.session_id,
            &context.turn_id,
            Some(&tool_call.call_id),
            "edit_file",
            &resolved.to_string_lossy(),
            "ok",
            Some(next.len()),
        )?;
        return Ok(json!({
            "action": "edit_file",
            "path": resolved.to_string_lossy(),
            "dry_run": false,
            "bytes_written": next.len(),
            "diff": truncate(&diff, MAX_TOOL_OUTPUT_CHARS),
            "approval_bypassed": true,
        }));
    }

    let preview_json = build_mutation_preview(&resolved, &previous, &next);
    let approval = new_tool_approval(
        context.session_id.clone(),
        context.turn_id.clone(),
        tool_call.call_id.clone(),
        "edit_file",
        resolved.to_string_lossy().to_string(),
        preview_json,
    );
    let decision = await_approval(context, &tool_call, &approval).await?;

    if !decision {
        context.storage.record_file_operation(
            &context.session_id,
            &context.turn_id,
            Some(&tool_call.call_id),
            "edit_file",
            &resolved.to_string_lossy(),
            "rejected",
            None,
        )?;
        return Err(AppError::Cancelled(format!(
            "edit rejected for `{}`",
            resolved.display()
        )));
    }

    write_file_content_atomic(&resolved, &next)?;
    context.storage.record_file_operation(
        &context.session_id,
        &context.turn_id,
        Some(&tool_call.call_id),
        "edit_file",
        &resolved.to_string_lossy(),
        "ok",
        Some(next.len()),
    )?;

    Ok(json!({
        "action": "edit_file",
        "path": resolved.to_string_lossy(),
        "dry_run": false,
        "bytes_written": next.len(),
        "diff": truncate(&diff, MAX_TOOL_OUTPUT_CHARS),
    }))
}

pub fn build_edit_file_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(EDIT_FILE_TOOL_NAME)
        .description(format!(
            "Apply ordered text edits and return a diff. If dryRun=true, only preview the diff. Paths can be absolute or relative to workspace root `{workspace_root}`."
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": format!("Target file path. Accepts absolute path or path relative to workspace root `{workspace_root}`.")
                },
                "edits": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "oldText": {
                                "type": "string"
                            },
                            "newText": {
                                "type": "string"
                            }
                        },
                        "required": ["oldText", "newText"]
                    },
                    "description": "Ordered list of replacements."
                },
                "dryRun": {
                    "type": ["boolean", "null"],
                    "default": false,
                    "description": "If true, return preview diff without writing."
                }
            },
            "required": ["path", "edits"]
        }))
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let args = serde_json::from_value::<EditFileArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                let edits = args
                    .edits
                    .into_iter()
                    .map(|item| FileEdit {
                        old_text: item.old_text,
                        new_text: item.new_text,
                    })
                    .collect::<Vec<_>>();
                context
                    .edit_file(
                        EDIT_FILE_TOOL_NAME,
                        &args.path,
                        &edits,
                        args.dry_run.unwrap_or(false),
                        args.tool_call_id.as_deref(),
                    )
                    .await
                    .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}
