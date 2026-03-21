//! `write_file` tool.

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::backend::errors::{AppError, AppResult};

use super::filesystem_context::{
    build_mutation_preview, read_text_if_exists, validate_path, write_file_content_atomic,
    FilesystemToolContext,
};
use super::tool_runtime::{ToolApprovalMode, ToolApprovalOutcome, ToolApprovalRequest};

pub const WRITE_FILE_TOOL_NAME: &str = "write_file";

#[derive(Debug, Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
    #[serde(default, rename = "__youclaw_call_id")]
    tool_call_id: Option<String>,
}

/// 执行 `write_file` 工具的核心逻辑。
///
/// 只负责提供审批规格，是否需要等待审批由 runtime 统一决定。
pub(crate) async fn execute_write_file(
    context: &FilesystemToolContext,
    tool_name: &str,
    input_path: &str,
    content: &str,
    tool_call_id: Option<&str>,
) -> AppResult<Value> {
    let tool_call = context.claim_tool_call(tool_name, tool_call_id)?;
    let resolved = validate_path(input_path, &context.workspace_root)?;

    let previous = read_text_if_exists(&resolved)?;
    let approval = context
        .runtime
        .authorize_tool_call(
            &tool_call,
            ToolApprovalRequest {
                mode: ToolApprovalMode::Default,
                action: "write_file".to_string(),
                subject: resolved.to_string_lossy().to_string(),
                preview_json: build_mutation_preview(&resolved, &previous, content),
            },
        )
        .await?;

    if approval == ToolApprovalOutcome::Rejected {
        context.storage.record_file_operation(
            &context.session_id,
            &context.turn_id,
            Some(&tool_call.call_id),
            "write_file",
            &resolved.to_string_lossy(),
            "rejected",
            None,
        )?;
        return Err(AppError::Cancelled(format!(
            "write rejected for `{}`",
            resolved.display()
        )));
    }

    write_file_content_atomic(&resolved, content)?;
    context.storage.record_file_operation(
        &context.session_id,
        &context.turn_id,
        Some(&tool_call.call_id),
        "write_file",
        &resolved.to_string_lossy(),
        "ok",
        Some(content.len()),
    )?;

    Ok(json!({
        "bytes_written": content.len(),
    }))
}

pub fn build_write_file_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(WRITE_FILE_TOOL_NAME)
        .description(format!(
            "Create or overwrite a UTF-8 file with atomic replace semantics. Writes require approval in default mode and run directly in full_access mode. Paths can be absolute or relative to workspace root `{workspace_root}`."
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": format!("Target file path. Accepts absolute path or path relative to workspace root `{workspace_root}`.")
                },
                "content": {
                    "type": "string",
                    "description": "Full UTF-8 content to write."
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
                    .write_file(
                        WRITE_FILE_TOOL_NAME,
                        &args.path,
                        &args.content,
                        args.tool_call_id.as_deref(),
                    )
                    .await
                    .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}
