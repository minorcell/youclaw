//! `write_file` tool.

use std::fs;

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::backend::errors::{AppError, AppResult};
use crate::backend::memory_manager::{
    resolve_relative_memory_path_from_absolute, BuiltinFtsMemoryManager, MemorySearchManager,
};
use crate::backend::models::new_tool_approval;

use super::filesystem_context::{
    build_mutation_preview, read_text_if_exists, validate_path, write_file_content_atomic,
    FilesystemToolContext,
};
use super::tool_runtime::await_approval;

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
/// - 记忆路径：自动放行并触发增量索引；
/// - 普通路径：进入审批流，审批通过后执行原子写入。
pub(crate) async fn execute_write_file(
    context: &FilesystemToolContext,
    tool_name: &str,
    input_path: &str,
    content: &str,
    tool_call_id: Option<&str>,
) -> AppResult<Value> {
    let tool_call = context.claim_tool_call(tool_name, tool_call_id)?;
    let resolved = validate_path(input_path, &context.workspace_root)?;

    if let Some(rel) =
        resolve_relative_memory_path_from_absolute(&resolved, &context.workspace_root)
    {
        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent)?;
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
        let manager =
            BuiltinFtsMemoryManager::new(context.storage.clone(), context.workspace_root.clone());
        let changed_paths = vec![rel];
        let sync = manager.sync(false, Some(&changed_paths))?;
        return Ok(json!({
            "action": "write_file",
            "path": resolved.to_string_lossy(),
            "bytes_written": content.len(),
            "approval_bypassed": true,
            "memory_sync": sync,
        }));
    }

    if context.should_skip_mutation_approval() {
        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent)?;
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
        return Ok(json!({
            "action": "write_file",
            "path": resolved.to_string_lossy(),
            "bytes_written": content.len(),
            "approval_bypassed": true,
        }));
    }

    let previous = read_text_if_exists(&resolved)?;
    let preview_json = build_mutation_preview(&resolved, &previous, content);
    let approval = new_tool_approval(
        context.session_id.clone(),
        context.turn_id.clone(),
        tool_call.call_id.clone(),
        "write_file",
        resolved.to_string_lossy().to_string(),
        preview_json,
    );
    let decision = await_approval(&context.runtime, &tool_call, &approval).await?;

    if !decision {
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
        "action": "write_file",
        "path": resolved.to_string_lossy(),
        "bytes_written": content.len(),
    }))
}

pub fn build_write_file_tool(context: FilesystemToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(WRITE_FILE_TOOL_NAME)
        .description(format!(
            "Create or overwrite a UTF-8 file with atomic replace semantics. Memory paths are auto-approved; other writes require approval. Paths can be absolute or relative to workspace root `{workspace_root}`."
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
