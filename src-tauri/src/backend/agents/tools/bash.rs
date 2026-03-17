//! `bash_exec` tool.

use std::env;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use aquaregia::tool::{tool, Tool, ToolExecError};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

use crate::backend::errors::{AppError, AppResult};

use super::filesystem_context::validate_path;
use super::tool_runtime::{
    ToolApprovalMode, ToolApprovalOutcome, ToolApprovalRequest, ToolRuntimeContext,
};

pub const BASH_EXEC_TOOL_NAME: &str = "bash_exec";

const DEFAULT_TIMEOUT_MS: u64 = 20_000;
const MAX_TIMEOUT_MS: u64 = 120_000;
const MAX_COMMAND_CHARS: usize = 4_000;
const MAX_CAPTURE_BYTES: usize = 16_000;
const TRUNCATED_SUFFIX: &str = "\n...[truncated]";

#[derive(Clone)]
pub struct BashToolContext {
    pub runtime: ToolRuntimeContext,
    pub workspace_root: PathBuf,
}

impl Deref for BashToolContext {
    type Target = ToolRuntimeContext;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

#[derive(Debug, Deserialize)]
struct BashExecArgs {
    command: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default, rename = "__youclaw_call_id")]
    tool_call_id: Option<String>,
}

#[derive(Debug)]
struct CapturedOutput {
    text: String,
    bytes: usize,
    truncated: bool,
}

#[derive(Debug)]
struct ShellExecutionResult {
    exit_code: Option<i32>,
    signal: Option<i32>,
    duration_ms: u64,
    stdout: CapturedOutput,
    stderr: CapturedOutput,
    timed_out: bool,
    cancelled: bool,
}

pub fn build_bash_exec_tool(context: BashToolContext) -> Tool {
    let workspace_root = context.workspace_root.to_string_lossy().to_string();
    tool(BASH_EXEC_TOOL_NAME)
        .description(format!(
            "Run a short non-interactive bash command inside the workspace. In default mode it requires approval; in full_access mode it runs directly. Timeout/output limits still apply. Paths in `cwd` must stay within workspace root `{workspace_root}`."
        ))
        .raw_schema(json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Bash command to execute. Keep it short and non-interactive."
                },
                "cwd": {
                    "type": ["string", "null"],
                    "description": format!("Working directory. Accepts absolute path or path relative to workspace root `{workspace_root}`.")
                },
                "timeout_ms": {
                    "type": ["integer", "null"],
                    "minimum": 1000,
                    "maximum": MAX_TIMEOUT_MS,
                    "description": "Execution timeout in milliseconds."
                }
            },
            "required": ["command"]
        }))
        .execute_raw(move |value| {
            let context = context.clone();
            async move {
                let args = serde_json::from_value::<BashExecArgs>(value)
                    .map_err(|err| ToolExecError::Execution(format!("invalid args: {err}")))?;
                execute_bash_exec(
                    &context,
                    &args.command,
                    args.cwd.as_deref(),
                    args.timeout_ms,
                    args.tool_call_id.as_deref(),
                )
                .await
                .map_err(|err| ToolExecError::Execution(err.message()))
            }
        })
}

async fn execute_bash_exec(
    context: &BashToolContext,
    command: &str,
    cwd: Option<&str>,
    timeout_ms: Option<u64>,
    tool_call_id: Option<&str>,
) -> AppResult<Value> {
    validate_command(command)?;
    ensure_command_is_foreground(command)?;

    let tool_call = context.claim_tool_call(BASH_EXEC_TOOL_NAME, tool_call_id)?;
    let resolved_cwd = resolve_command_cwd(context, cwd)?;
    let timeout_ms = timeout_ms
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1000, MAX_TIMEOUT_MS);

    let approval = context
        .runtime
        .authorize_tool_call(
            &tool_call,
            ToolApprovalRequest {
                mode: ToolApprovalMode::Default,
                action: "bash_exec".to_string(),
                subject: summarize_command(command),
                preview_json: json!({
                    "kind": "command",
                    "command": command,
                    "cwd": resolved_cwd.to_string_lossy(),
                    "timeout_ms": timeout_ms,
                    "risk_flags": detect_risk_flags(command),
                    "description": "Shell execution requires approval in default mode."
                }),
            },
        )
        .await?;

    if approval == ToolApprovalOutcome::Rejected {
        context.storage.record_shell_execution(
            &context.session_id,
            &context.turn_id,
            Some(&tool_call.call_id),
            command,
            &resolved_cwd.to_string_lossy(),
            "rejected",
            None,
            None,
            None,
            None,
            None,
        )?;
        return Err(AppError::Cancelled("shell command rejected".to_string()));
    }

    let execution = run_shell_command(context, command, &resolved_cwd, timeout_ms).await?;
    let status = if execution.cancelled {
        "cancelled"
    } else if execution.timed_out {
        "timed_out"
    } else if execution.exit_code == Some(0) {
        "ok"
    } else {
        "error"
    };
    context.storage.record_shell_execution(
        &context.session_id,
        &context.turn_id,
        Some(&tool_call.call_id),
        command,
        &resolved_cwd.to_string_lossy(),
        status,
        execution.exit_code,
        execution.signal,
        Some(execution.duration_ms),
        Some(execution.stdout.bytes),
        Some(execution.stderr.bytes),
    )?;

    if execution.cancelled {
        return Err(AppError::Cancelled("shell command cancelled".to_string()));
    }

    let mut result = json!({
        "action": "exec",
        "command": command,
        "cwd": resolved_cwd.to_string_lossy(),
        "exit_code": execution.exit_code,
        "signal": execution.signal,
        "duration_ms": execution.duration_ms,
        "timed_out": execution.timed_out,
        "stdout": execution.stdout.text,
        "stderr": execution.stderr.text,
        "stdout_truncated": execution.stdout.truncated,
        "stderr_truncated": execution.stderr.truncated,
    });

    if approval.approval_bypassed() {
        result["approval_bypassed"] = Value::Bool(true);
    }

    Ok(result)
}

async fn run_shell_command(
    context: &BashToolContext,
    command: &str,
    cwd: &Path,
    timeout_ms: u64,
) -> AppResult<ShellExecutionResult> {
    let started = Instant::now();
    let mut child = spawn_shell_child(command, cwd)?;
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_task = tokio::spawn(async move { read_stream(stdout).await });
    let stderr_task = tokio::spawn(async move { read_stream(stderr).await });

    let timed_out;
    let cancelled;
    let status = {
        let mut wait_future = Box::pin(child.wait());
        tokio::select! {
            result = &mut wait_future => {
                timed_out = false;
                cancelled = false;
                result?
            }
            _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
                drop(wait_future);
                timed_out = true;
                cancelled = false;
                kill_process_tree(&mut child, pid).await?;
                child.wait().await?
            }
            _ = context.cancellation_token.cancelled() => {
                drop(wait_future);
                timed_out = false;
                cancelled = true;
                kill_process_tree(&mut child, pid).await?;
                child.wait().await?
            }
        }
    };

    let stdout = stdout_task
        .await
        .map_err(|err| AppError::Agent(format!("stdout task failed: {err}")))??;
    let stderr = stderr_task
        .await
        .map_err(|err| AppError::Agent(format!("stderr task failed: {err}")))??;
    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;

    Ok(ShellExecutionResult {
        exit_code: status.code(),
        signal: exit_status_signal(&status),
        duration_ms,
        stdout,
        stderr,
        timed_out,
        cancelled,
    })
}

fn validate_command(command: &str) -> AppResult<()> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "shell command cannot be empty".to_string(),
        ));
    }
    if trimmed.contains('\0') {
        return Err(AppError::Validation(
            "shell command contains null byte".to_string(),
        ));
    }
    if trimmed.chars().count() > MAX_COMMAND_CHARS {
        return Err(AppError::Validation(format!(
            "shell command too long (max {MAX_COMMAND_CHARS} chars)"
        )));
    }
    if trimmed.split_whitespace().any(|part| part == "sudo") {
        return Err(AppError::Validation(
            "sudo is not allowed in shell commands".to_string(),
        ));
    }
    Ok(())
}

fn ensure_command_is_foreground(command: &str) -> AppResult<()> {
    if has_standalone_ampersand(command)
        || ["nohup", "disown", "setsid"]
            .iter()
            .any(|token| command.split_whitespace().any(|part| part == *token))
    {
        return Err(AppError::Validation(
            "background or detached shell commands are not allowed".to_string(),
        ));
    }
    Ok(())
}

fn resolve_command_cwd(context: &BashToolContext, cwd: Option<&str>) -> AppResult<PathBuf> {
    match cwd {
        Some(value) => validate_path(value, &context.workspace_root),
        None => Ok(context.workspace_root.clone()),
    }
}

fn build_command(command: &str, cwd: &Path) -> AppResult<Command> {
    let mut shell = Command::new("/bin/bash");
    shell
        .arg("-lc")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("LANG", "C.UTF-8")
        .env("TERM", "dumb")
        .env("PWD", cwd.as_os_str());

    for key in ["PATH", "HOME", "TMPDIR", "TMP", "TEMP", "USER"] {
        if let Ok(value) = env::var(key) {
            shell.env(key, value);
        }
    }

    #[cfg(unix)]
    unsafe {
        shell.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    Ok(shell)
}

fn spawn_shell_child(command: &str, cwd: &Path) -> AppResult<Child> {
    build_command(command, cwd)?
        .spawn()
        .map_err(|err| AppError::Agent(format!("failed to spawn bash: {err}")))
}

async fn read_stream(
    stream: Option<impl tokio::io::AsyncRead + Unpin>,
) -> AppResult<CapturedOutput> {
    let Some(mut stream) = stream else {
        return Ok(CapturedOutput {
            text: String::new(),
            bytes: 0,
            truncated: false,
        });
    };

    let mut total_bytes = 0usize;
    let mut captured = Vec::<u8>::new();
    let mut truncated = false;
    let mut buffer = [0u8; 4096];

    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|err| AppError::Agent(format!("failed to read shell output: {err}")))?;
        if read == 0 {
            break;
        }
        total_bytes += read;
        let remaining = MAX_CAPTURE_BYTES.saturating_sub(captured.len());
        if remaining > 0 {
            let to_take = read.min(remaining);
            captured.extend_from_slice(&buffer[..to_take]);
            if to_take < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }

    let mut text = String::from_utf8_lossy(&captured).to_string();
    if truncated {
        text.push_str(TRUNCATED_SUFFIX);
    }
    Ok(CapturedOutput {
        text,
        bytes: total_bytes,
        truncated,
    })
}

async fn kill_process_tree(child: &mut Child, pid: Option<u32>) -> AppResult<()> {
    #[cfg(unix)]
    {
        if let Some(pid) = pid {
            let rc = unsafe { libc::killpg(pid as i32, libc::SIGKILL) };
            if rc == 0 {
                return Ok(());
            }
        }
    }

    child
        .start_kill()
        .map_err(|err| AppError::Agent(format!("failed to kill shell command: {err}")))?;
    Ok(())
}

fn summarize_command(command: &str) -> String {
    let trimmed = command.trim();
    if trimmed.chars().count() <= 96 {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(96).collect::<String>();
    out.push('…');
    out
}

fn detect_risk_flags(command: &str) -> Vec<String> {
    let mut flags = Vec::<String>::new();
    let lowered = command.to_lowercase();

    if lowered.contains("rm ") || lowered.contains(" rm") {
        flags.push("destructive_delete".to_string());
    }
    if lowered.contains("chmod ") || lowered.contains("chown ") || lowered.contains("mv ") {
        flags.push("filesystem_mutation".to_string());
    }
    if command.contains('>') || command.contains("tee ") {
        flags.push("shell_redirection".to_string());
    }
    if lowered.contains("curl ") || lowered.contains("wget ") || lowered.contains("ssh ") {
        flags.push("network_access".to_string());
    }
    if lowered.contains("git clean") || lowered.contains("find ") && lowered.contains("-delete") {
        flags.push("bulk_mutation".to_string());
    }

    flags
}

fn has_standalone_ampersand(command: &str) -> bool {
    let chars = command.chars().collect::<Vec<_>>();
    for (index, ch) in chars.iter().enumerate() {
        if *ch != '&' {
            continue;
        }
        let prev = index
            .checked_sub(1)
            .and_then(|item| chars.get(item))
            .copied();
        let next = chars.get(index + 1).copied();
        if prev == Some('&') || next == Some('&') {
            continue;
        }
        return true;
    }
    false
}

#[cfg(unix)]
fn exit_status_signal(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;

    status.signal()
}

#[cfg(not(unix))]
fn exit_status_signal(_: &std::process::ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::{detect_risk_flags, execute_bash_exec, has_standalone_ampersand, BashToolContext};
    use crate::backend::agents::tools::ToolRuntimeContext;
    use crate::backend::models::domain::{new_chat_session, SessionApprovalMode};
    use crate::backend::{ApprovalService, StorageService, WsHub};
    use aquaregia::ToolCall;
    use serde_json::json;
    use std::sync::atomic::AtomicU8;
    use std::sync::{Arc, Mutex};
    use tempfile::{tempdir, TempDir};
    use tokio_util::sync::CancellationToken;

    fn build_test_context(
        approval_mode: SessionApprovalMode,
    ) -> (BashToolContext, StorageService, TempDir) {
        let dir = tempdir().expect("tempdir");
        let storage = StorageService::new(dir.path().join("state")).expect("storage");
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace");

        let mut session = new_chat_session(None);
        session.id = "session-1".to_string();
        session.approval_mode = approval_mode;
        storage.insert_session(&session).expect("insert session");

        let context = BashToolContext {
            runtime: ToolRuntimeContext {
                session_id: session.id,
                turn_id: "turn-1".to_string(),
                current_step: Arc::new(AtomicU8::new(1)),
                tool_calls: Arc::new(Mutex::new(std::collections::HashMap::new())),
                cancellation_token: CancellationToken::new(),
                approvals: ApprovalService::new(storage.clone()),
                approval_mode,
                storage: storage.clone(),
                hub: WsHub::new(),
            },
            workspace_root,
        };

        (context, storage, dir)
    }

    fn register_tool_call(context: &BashToolContext, command: &str) -> String {
        let call_id = "test-bash-call".to_string();
        let call = ToolCall {
            call_id: call_id.clone(),
            tool_name: super::BASH_EXEC_TOOL_NAME.to_string(),
            args_json: json!({ "command": command }),
        };
        let mut registry = context.tool_calls.lock().expect("tool call lock");
        registry.insert(call_id.clone(), call);
        call_id
    }

    #[test]
    fn detects_background_operators() {
        assert!(has_standalone_ampersand("sleep 1 &"));
        assert!(!has_standalone_ampersand("cargo check && cargo test"));
    }

    #[test]
    fn detects_risk_flags() {
        let flags = detect_risk_flags("rm -rf dist && curl https://example.com");
        assert!(flags.iter().any(|flag| flag == "destructive_delete"));
        assert!(flags.iter().any(|flag| flag == "network_access"));
    }

    #[tokio::test]
    async fn full_access_bash_exec_skips_approval() {
        let (context, storage, _dir) = build_test_context(SessionApprovalMode::FullAccess);
        let call_id = register_tool_call(&context, "printf hello");

        let result = execute_bash_exec(
            &context,
            "printf hello",
            None,
            Some(super::DEFAULT_TIMEOUT_MS),
            Some(&call_id),
        )
        .await
        .expect("execute bash");

        assert_eq!(
            result.get("stdout").and_then(|value| value.as_str()),
            Some("hello")
        );
        assert_eq!(
            result
                .get("approval_bypassed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(storage.list_approvals().expect("list approvals").is_empty());
    }
}
