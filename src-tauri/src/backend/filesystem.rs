use std::collections::VecDeque;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aquaregia::tool::{tool, Tool, ToolExecError};
use aquaregia::ToolCall;
use serde::Deserialize;
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::{new_tool_approval, ToolRequestedPayload};
use crate::backend::{ApprovalService, StorageService, WsHub};

const MAX_READ_LIMIT: usize = 300;
const MAX_TOOL_OUTPUT_CHARS: usize = 24_000;
const APPROVAL_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Deserialize)]
struct FilesystemToolRawInput {
    action: String,
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
    content: Option<String>,
}

#[derive(Debug, Clone)]
enum FilesystemToolInput {
    ListDir {
        path: String,
    },
    ReadFile {
        path: String,
        offset: Option<usize>,
        limit: Option<usize>,
    },
    WriteFile {
        path: String,
        content: String,
    },
}

impl TryFrom<Value> for FilesystemToolInput {
    type Error = AppError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let raw = serde_json::from_value::<FilesystemToolRawInput>(value)
            .map_err(|err| AppError::Validation(format!("invalid filesystem tool args: {err}")))?;

        match raw.action.as_str() {
            "list_dir" => Ok(Self::ListDir { path: raw.path }),
            "read_file" => Ok(Self::ReadFile {
                path: raw.path,
                offset: raw.offset,
                limit: raw.limit,
            }),
            "write_file" => Ok(Self::WriteFile {
                path: raw.path,
                content: raw.content.ok_or_else(|| {
                    AppError::Validation("`content` is required for write_file".to_string())
                })?,
            }),
            other => Err(AppError::Validation(format!(
                "unsupported filesystem action `{other}`"
            ))),
        }
    }
}

#[derive(Clone)]
pub struct FilesystemToolContext {
    pub session_id: String,
    pub run_id: String,
    pub current_step: Arc<AtomicU8>,
    pub tool_calls: Arc<Mutex<VecDeque<ToolCall>>>,
    pub cancellation_token: CancellationToken,
    pub approvals: ApprovalService,
    pub storage: StorageService,
    pub hub: WsHub,
}

impl FilesystemToolContext {
    async fn execute(&self, input: FilesystemToolInput) -> Result<Value, ToolExecError> {
        match input {
            FilesystemToolInput::ListDir { path } => self.list_dir(&path),
            FilesystemToolInput::ReadFile {
                path,
                offset,
                limit,
            } => self.read_file(&path, offset.unwrap_or(0), limit.unwrap_or(200)),
            FilesystemToolInput::WriteFile { path, content } => {
                self.write_file(&path, &content).await
            }
        }
        .map_err(|err| ToolExecError::Execution(err.message()))
    }

    fn list_dir(&self, input_path: &str) -> AppResult<Value> {
        let resolved = resolve_path(input_path)?;
        let tool_call = self.claim_tool_call("list_dir", input_path);
        let entries = fs::read_dir(&resolved)?
            .map(|entry| {
                let entry = entry?;
                let metadata = entry.metadata()?;
                Ok(json!({
                    "name": entry.file_name().to_string_lossy().to_string(),
                    "path": entry.path().to_string_lossy().to_string(),
                    "is_dir": metadata.is_dir(),
                    "size_bytes": if metadata.is_file() { Some(metadata.len()) } else { None },
                }))
            })
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        self.storage.record_file_operation(
            &self.session_id,
            &self.run_id,
            Some(&tool_call.call_id),
            "list_dir",
            &resolved.to_string_lossy(),
            "ok",
            None,
        )?;
        Ok(json!({
            "action": "list_dir",
            "path": resolved.to_string_lossy(),
            "entries": entries,
        }))
    }

    fn read_file(&self, input_path: &str, offset: usize, limit: usize) -> AppResult<Value> {
        if !(1..=MAX_READ_LIMIT).contains(&limit) {
            return Err(AppError::Validation(format!(
                "`limit` must be within 1..={MAX_READ_LIMIT}`"
            )));
        }
        let resolved = resolve_path(input_path)?;
        let tool_call = self.claim_tool_call("read_file", input_path);
        let bytes = fs::read(&resolved)?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        let lines = text.lines().collect::<Vec<_>>();
        let start = offset.min(lines.len());
        let end = start.saturating_add(limit).min(lines.len());
        let content = lines[start..end]
            .iter()
            .enumerate()
            .map(|(idx, line)| format!("{}\t{}", start + idx + 1, line))
            .collect::<Vec<_>>()
            .join("\n");
        self.storage.record_file_operation(
            &self.session_id,
            &self.run_id,
            Some(&tool_call.call_id),
            "read_file",
            &resolved.to_string_lossy(),
            "ok",
            None,
        )?;
        Ok(json!({
            "action": "read_file",
            "path": resolved.to_string_lossy(),
            "line_start": start + 1,
            "line_end": end,
            "total_lines": lines.len(),
            "content": truncate(&content, MAX_TOOL_OUTPUT_CHARS),
        }))
    }

    async fn write_file(&self, input_path: &str, content: &str) -> AppResult<Value> {
        let resolved = resolve_path(input_path)?;
        let tool_call = self.claim_tool_call("write_file", input_path);
        let previous = fs::read_to_string(&resolved).unwrap_or_default();
        let preview_json = json!({
            "path": resolved.to_string_lossy(),
            "diff": build_diff_preview(&previous, content),
            "old_excerpt": truncate(&previous, 4000),
            "new_excerpt": truncate(content, 4000),
        });
        let approval = new_tool_approval(
            self.session_id.clone(),
            self.run_id.clone(),
            tool_call.call_id.clone(),
            "write_file",
            resolved.to_string_lossy().to_string(),
            preview_json,
        );
        let receiver = self.approvals.register_pending(approval.clone())?;
        self.hub.emit_run_event(
            &self.run_id,
            "chat.tool.requested",
            ToolRequestedPayload {
                session_id: self.session_id.clone(),
                run_id: self.run_id.clone(),
                step: self.current_step.load(Ordering::Relaxed),
                state: "awaiting_approval".to_string(),
                tool_call: tool_call.clone(),
                approval: Some(approval.clone()),
            },
        )?;

        let decision = match wait_for_approval(receiver, &self.cancellation_token).await {
            Ok(value) => value,
            Err(ApprovalWaitError::Cancelled) => {
                let _ = self.approvals.mark_status(&approval.id, "cancelled");
                return Err(AppError::Cancelled(
                    "run cancelled while waiting for approval".to_string(),
                ));
            }
            Err(ApprovalWaitError::TimedOut) => {
                let _ = self.approvals.mark_status(&approval.id, "timed_out");
                return Err(AppError::Cancelled("approval timed out".to_string()));
            }
            Err(ApprovalWaitError::ChannelClosed) => {
                let _ = self.approvals.mark_status(&approval.id, "cancelled");
                return Err(AppError::Cancelled("approval channel closed".to_string()));
            }
        };
        if !decision {
            self.storage.record_file_operation(
                &self.session_id,
                &self.run_id,
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

        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&resolved, content.as_bytes())?;
        self.storage.record_file_operation(
            &self.session_id,
            &self.run_id,
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

    fn claim_tool_call(&self, action: &str, input_path: &str) -> ToolCall {
        if let Ok(mut queue) = self.tool_calls.lock() {
            if let Some(call) = queue.pop_front() {
                return call;
            }
        }
        ToolCall {
            call_id: format!("fallback-{action}-{}", input_path),
            tool_name: "filesystem".to_string(),
            args_json: json!({ "action": action, "path": input_path }),
        }
    }
}

pub fn build_filesystem_tool(context: FilesystemToolContext) -> Tool {
    tool("filesystem")
        .description("Access local files with action=list_dir|read_file|write_file. Relative paths resolve from the user's home directory. write_file requires explicit user approval.")
        .raw_schema(filesystem_tool_schema())
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let input = FilesystemToolInput::try_from(value)
                    .map_err(|err| ToolExecError::Execution(err.message()))?;
                context.execute(input).await
            }
        })
}

fn filesystem_tool_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "action": {
                "type": "string",
                "enum": ["list_dir", "read_file", "write_file"],
                "description": "Which filesystem operation to execute."
            },
            "path": {
                "type": "string",
                "description": "Absolute path, or relative to the user's home directory."
            },
            "offset": {
                "type": ["integer", "null"],
                "minimum": 0,
                "description": "Optional start line for read_file."
            },
            "limit": {
                "type": ["integer", "null"],
                "minimum": 1,
                "maximum": MAX_READ_LIMIT,
                "description": "Optional max line count for read_file."
            },
            "content": {
                "type": ["string", "null"],
                "description": "Required only for write_file."
            }
        },
        "required": ["action", "path"]
    })
}

fn resolve_path(input_path: &str) -> AppResult<PathBuf> {
    let path = Path::new(input_path);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let home = dirs::home_dir()
            .ok_or_else(|| AppError::Io("could not resolve user home directory".to_string()))?;
        home.join(path)
    };
    Ok(normalize_path(&joined))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out = text.chars().take(limit).collect::<String>();
    out.push_str("\n...[truncated]");
    out
}

fn build_diff_preview(previous: &str, next: &str) -> String {
    let diff = TextDiff::from_lines(previous, next);
    let mut lines = Vec::new();
    for change in diff.iter_all_changes().take(240) {
        let sign = match change.tag() {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };
        let line = format!("{sign} {}", change.to_string().trim_end_matches('\n'));
        lines.push(line);
    }
    if diff.iter_all_changes().count() > 240 {
        lines.push("... [diff truncated]".to_string());
    }
    lines.join("\n")
}

async fn wait_for_approval(
    receiver: oneshot::Receiver<bool>,
    cancellation_token: &CancellationToken,
) -> Result<bool, ApprovalWaitError> {
    tokio::select! {
        _ = cancellation_token.cancelled() => Err(ApprovalWaitError::Cancelled),
        result = timeout(Duration::from_secs(APPROVAL_TIMEOUT_SECS), receiver) => {
            match result {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(_)) => Err(ApprovalWaitError::ChannelClosed),
                Err(_) => Err(ApprovalWaitError::TimedOut),
            }
        }
    }
}

enum ApprovalWaitError {
    Cancelled,
    TimedOut,
    ChannelClosed,
}

#[cfg(test)]
mod tests {
    use super::{build_diff_preview, filesystem_tool_schema, resolve_path};

    #[test]
    fn diff_preview_marks_insertions() {
        let diff = build_diff_preview("alpha\n", "alpha\nbeta\n");
        assert!(diff.contains("+ beta"));
    }

    #[test]
    fn relative_paths_resolve_without_error() {
        let path = resolve_path("Desktop/example.txt").expect("resolved path");
        assert!(path.is_absolute());
    }

    #[test]
    fn tool_schema_has_object_root() {
        let schema = filesystem_tool_schema();
        assert_eq!(
            schema.get("type").and_then(|value| value.as_str()),
            Some("object")
        );
    }
}
