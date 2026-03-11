//! 文件系统工具共享上下文与公共逻辑。
//!
//! 说明：
//! - `filesystem_list_dir` / `filesystem_read_file` / `filesystem_write_file` 三个工具复用本文件；
//! - 工具定义放在独立文件，本文件只承载共用能力（路径解析、审批等待、审计日志等）；
//! - 这样可以保持“一个工具一个文件”，同时避免重复实现。

use std::collections::VecDeque;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aquaregia::ToolCall;
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::{new_tool_approval, ToolRequestedPayload};
use crate::backend::{ApprovalService, StorageService, WsHub};

/// `read_file` 的最大读取行数，避免单次输出过大。
pub const MAX_READ_LIMIT: usize = 300;
/// 工具输出的字符上限，防止消息过大导致前端或模型上下文膨胀。
const MAX_TOOL_OUTPUT_CHARS: usize = 24_000;
/// 写文件审批超时时间（秒）。
const APPROVAL_TIMEOUT_SECS: u64 = 600;

/// 文件系统工具共享上下文。
///
/// 三个工具（list/read/write）共用同一份运行上下文，
/// 以便共享审批状态、审计日志和 call_id 映射队列。
#[derive(Clone)]
pub struct FilesystemToolContext {
    /// 会话 ID，用于记录审计日志与事件。
    pub session_id: String,
    /// Turn ID，用于关联当前这轮对话执行流。
    pub turn_id: String,
    /// 工作区根目录，相对路径统一基于该目录解析。
    pub workspace_root: PathBuf,
    /// 当前步骤序号（step），用于事件中标记执行进度。
    pub current_step: Arc<AtomicU8>,
    /// 工具调用队列，用于将流式事件中的 call_id 绑定到实际工具执行。
    pub tool_calls: Arc<Mutex<VecDeque<ToolCall>>>,
    /// 运行取消令牌，用于审批等待期间快速中断。
    pub cancellation_token: CancellationToken,
    /// 审批服务（写文件需要）。
    pub approvals: ApprovalService,
    /// 存储服务（审计日志、数据库写入）。
    pub storage: StorageService,
    /// WebSocket 事件中心（向前端发工具事件）。
    pub hub: WsHub,
}

impl FilesystemToolContext {
    /// 列出目录内容。
    ///
    /// - 相对路径基于 workspace_root
    /// - 记录 turn 级别文件操作审计日志
    pub fn list_dir(&self, tool_name: &str, input_path: &str) -> AppResult<Value> {
        let resolved = resolve_path(input_path, &self.workspace_root)?;
        let tool_call = self.claim_tool_call(tool_name, "list_dir", input_path);
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
            &self.turn_id,
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

    /// 读取文件内容（按行切片）。
    ///
    /// - `offset` 为起始行偏移（0-based）
    /// - `limit` 为最大返回行数，受 MAX_READ_LIMIT 约束
    pub fn read_file(
        &self,
        tool_name: &str,
        input_path: &str,
        offset: usize,
        limit: usize,
    ) -> AppResult<Value> {
        if !(1..=MAX_READ_LIMIT).contains(&limit) {
            return Err(AppError::Validation(format!(
                "`limit` 必须位于 1..={MAX_READ_LIMIT}"
            )));
        }
        let resolved = resolve_path(input_path, &self.workspace_root)?;
        let tool_call = self.claim_tool_call(tool_name, "read_file", input_path);
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
            &self.turn_id,
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

    /// 写入文件（覆盖写），并通过审批流程保护写操作。
    ///
    /// - 先生成 diff 预览并发起审批事件
    /// - 审批通过后执行写入并记录审计日志
    pub async fn write_file(
        &self,
        tool_name: &str,
        input_path: &str,
        content: &str,
    ) -> AppResult<Value> {
        let resolved = resolve_path(input_path, &self.workspace_root)?;
        let tool_call = self.claim_tool_call(tool_name, "write_file", input_path);
        let previous = fs::read_to_string(&resolved).unwrap_or_default();
        let preview_json = json!({
            "path": resolved.to_string_lossy(),
            "diff": build_diff_preview(&previous, content),
            "old_excerpt": truncate(&previous, 4000),
            "new_excerpt": truncate(content, 4000),
        });
        let approval = new_tool_approval(
            self.session_id.clone(),
            self.turn_id.clone(),
            tool_call.call_id.clone(),
            "write_file",
            resolved.to_string_lossy().to_string(),
            preview_json,
        );
        let receiver = self.approvals.register_pending(approval.clone())?;
        self.hub.emit_turn_event(
            &self.turn_id,
            "chat.step.tool.requested",
            ToolRequestedPayload {
                session_id: self.session_id.clone(),
                turn_id: self.turn_id.clone(),
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
                    "turn cancelled while waiting for approval".to_string(),
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
                &self.turn_id,
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
            &self.turn_id,
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

    /// 从 ToolCall 队列中获取当前调用信息。
    ///
    /// 因为工具执行函数拿不到 call_id，运行时会在事件流中
    /// 先把 ToolCall 压队列，再由这里按顺序弹出绑定。
    fn claim_tool_call(&self, tool_name: &str, action: &str, input_path: &str) -> ToolCall {
        if let Ok(mut queue) = self.tool_calls.lock() {
            if let Some(call) = queue.pop_front() {
                return call;
            }
        }
        ToolCall {
            call_id: format!("fallback-{action}-{}", input_path),
            tool_name: tool_name.to_string(),
            args_json: json!({ "path": input_path }),
        }
    }
}

/// 将输入路径解析成绝对路径。
///
/// - 绝对路径：原样使用
/// - 相对路径：拼接到 workspace_root
/// - 最后统一做规范化，去掉 `.` / `..`
fn resolve_path(input_path: &str, workspace_root: &Path) -> AppResult<PathBuf> {
    let path = Path::new(input_path);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };
    Ok(normalize_path(&joined))
}

/// 归一化路径中的 `.` / `..` 片段。
///
/// 这里不做权限判断，仅做语义归一化；
/// 具体可写范围由上层 workspace / 审批策略控制。
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

/// 对长文本做截断，避免工具输出过大。
///
/// 按字符数截断而非字节数，避免多字节字符被切断。
fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out = text.chars().take(limit).collect::<String>();
    out.push_str("\n...[truncated]");
    out
}

/// 生成可读 diff 预览，供写文件审批弹窗展示。
///
/// 只保留前 240 行变化，保证审批弹窗可读且性能稳定。
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

/// 等待审批结果，支持超时与取消信号。
///
/// 返回值：
/// - `Ok(true)`：审批通过
/// - `Ok(false)`：审批拒绝
/// - `Err(...)`：超时/取消/通道关闭
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
    use super::{build_diff_preview, resolve_path};
    use std::path::Path;

    #[test]
    fn diff_preview_marks_insertions() {
        let diff = build_diff_preview("alpha\n", "alpha\nbeta\n");
        assert!(diff.contains("+ beta"));
    }

    #[test]
    fn relative_paths_resolve_without_error() {
        let workspace_root = Path::new("/tmp/youclaw-workspace");
        let path = resolve_path("Desktop/example.txt", workspace_root).expect("resolved path");
        assert!(path.is_absolute());
        assert!(path.starts_with(workspace_root));
    }

    #[test]
    fn absolute_paths_are_preserved() {
        let workspace_root = Path::new("/tmp/youclaw-workspace");
        let absolute = Path::new("/tmp/absolute/example.txt");
        let path = resolve_path(absolute.to_string_lossy().as_ref(), workspace_root)
            .expect("resolved path");
        assert_eq!(path, absolute);
    }
}
