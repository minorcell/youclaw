//! 文件系统工具共享上下文与复用辅助函数。
//!
//! 该模块只负责：
//! - 统一上下文载体（会话、审批、存储、事件）；
//! - 路径校验、忽略策略、diff/截断等通用 helper；
//! - 审批等待等跨工具复用逻辑。
//!
//! 具体工具实现放在各自模块中（如 `list_directory.rs` / `write_file.rs`）。

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};

use glob::Pattern;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use uuid::Uuid;

use crate::backend::errors::{AppError, AppResult};

use super::tool_runtime::ToolRuntimeContext;

/// `read_text_file` head/tail line limit.
pub const MAX_HEAD_TAIL_LIMIT: usize = 1000;
/// Maximum number of paths allowed in a single `read_files` call.
pub const MAX_BATCH_READ_FILES: usize = 32;
/// Maximum number of matches returned by `search_files`.
pub const MAX_SEARCH_RESULTS: usize = 500;

/// Prevent oversized tool outputs from bloating UI/model context.
pub(crate) const MAX_TOOL_OUTPUT_CHARS: usize = 24_000;
/// Preview diff max lines for approval payloads.
const MAX_DIFF_PREVIEW_LINES: usize = 240;
/// Optional extra allowed roots env variable.
const FS_ALLOWED_ROOTS_ENV: &str = "YOUCLAW_FS_ALLOWED_ROOTS";
/// Tool-phase ignored directories to reduce noisy traversal.
const TOOL_IGNORED_DIRS: [&str; 13] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".cache",
    "coverage",
    "vendor",
    "out",
    "tmp",
    ".idea",
];
/// Tool-phase ignored files to reduce noisy traversal.
const TOOL_IGNORED_FILES: [&str; 4] = [".DS_Store", "Thumbs.db", ".env", ".env.local"];

#[derive(Debug, Clone)]
pub struct FileEdit {
    pub old_text: String,
    pub new_text: String,
}

/// Shared runtime context for filesystem tools.
#[derive(Clone)]
pub struct FilesystemToolContext {
    pub runtime: ToolRuntimeContext,
    /// Workspace root; relative paths are resolved from this root.
    pub workspace_root: PathBuf,
}

impl Deref for FilesystemToolContext {
    type Target = ToolRuntimeContext;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl FilesystemToolContext {
    /// List direct children of a directory.
    pub fn list_directory(
        &self,
        tool_name: &str,
        input_path: &str,
        tool_call_id: Option<&str>,
    ) -> AppResult<Value> {
        super::list_directory::execute_list_directory(self, tool_name, input_path, tool_call_id)
    }

    /// Read file as UTF-8 text. Supports optional `head` or `tail` line limits.
    pub fn read_text_file(
        &self,
        tool_name: &str,
        input_path: &str,
        head: Option<usize>,
        tail: Option<usize>,
        tool_call_id: Option<&str>,
    ) -> AppResult<Value> {
        super::read_text_file::execute_read_text_file(
            self,
            tool_name,
            input_path,
            head,
            tail,
            tool_call_id,
        )
    }

    /// Read multiple files in one tool call.
    ///
    /// Per-file failures are included in the response and do not abort the batch.
    pub fn read_files(
        &self,
        tool_name: &str,
        paths: &[String],
        tool_call_id: Option<&str>,
    ) -> AppResult<Value> {
        super::read_files::execute_read_files(self, tool_name, paths, tool_call_id)
    }

    /// Search files recursively from a root path using glob patterns.
    pub fn search_files(
        &self,
        tool_name: &str,
        input_path: &str,
        pattern: &str,
        exclude_patterns: &[String],
        tool_call_id: Option<&str>,
    ) -> AppResult<Value> {
        super::search_files::execute_search_files(
            self,
            tool_name,
            input_path,
            pattern,
            exclude_patterns,
            tool_call_id,
        )
    }

    /// Create or overwrite a file with approval protection.
    ///
    /// Memory files (`MEMORY.md` and `memory/*.md`) bypass approval.
    pub async fn write_file(
        &self,
        tool_name: &str,
        input_path: &str,
        content: &str,
        tool_call_id: Option<&str>,
    ) -> AppResult<Value> {
        super::write_file::execute_write_file(self, tool_name, input_path, content, tool_call_id)
            .await
    }

    /// Apply ordered text edits and optionally write them back with approval.
    pub async fn edit_file(
        &self,
        tool_name: &str,
        input_path: &str,
        edits: &[FileEdit],
        dry_run: bool,
        tool_call_id: Option<&str>,
    ) -> AppResult<Value> {
        super::edit_file::execute_edit_file(
            self,
            tool_name,
            input_path,
            edits,
            dry_run,
            tool_call_id,
        )
        .await
    }
}

/// 遍历/列举类工具使用的执行期忽略策略。
///
/// 规则：
/// - 固定忽略常见噪音目录与文件；
/// - 当目标路径位于工作区内时，附加应用工作区根 `.gitignore`。
pub(crate) struct ToolIgnorePolicy {
    workspace_root: PathBuf,
    root_gitignore: Option<Gitignore>,
}

impl ToolIgnorePolicy {
    /// 构建工具执行期忽略策略。
    pub(crate) fn new(workspace_root: &Path) -> Self {
        let normalized_root = normalize_path(workspace_root);
        let root_gitignore = build_workspace_gitignore_matcher(workspace_root);
        Self {
            workspace_root: normalized_root,
            root_gitignore,
        }
    }

    /// 判断目标路径在工具执行阶段是否应被忽略。
    pub(crate) fn should_ignore_path(&self, path: &Path, is_dir: bool) -> bool {
        let normalized_path = normalize_path(path);
        let relative = normalized_path
            .strip_prefix(&self.workspace_root)
            .ok()
            .map(Path::to_path_buf);
        let subject = relative.as_deref().unwrap_or(normalized_path.as_path());

        if contains_tool_ignored_dirs(subject) {
            return true;
        }

        if let Some(file_name) = subject.file_name().and_then(|name| name.to_str()) {
            if TOOL_IGNORED_FILES.contains(&file_name) {
                return true;
            }
        }

        if let (Some(rel), Some(matcher)) = (relative.as_deref(), &self.root_gitignore) {
            if matcher.matched_path_or_any_parents(rel, is_dir).is_ignore() {
                return true;
            }
        }

        false
    }
}

/// 读取文本文件；文件不存在时返回空字符串而非报错。
pub(crate) fn read_text_if_exists(path: &Path) -> AppResult<String> {
    match fs::read(path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err.into()),
    }
}

/// 以“临时文件 + rename”方式原子写入文本内容。
pub(crate) fn write_file_content_atomic(path: &Path, content: &str) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::Validation(format!(
            "cannot write file without parent directory: `{}`",
            path.display()
        ))
    })?;

    if !parent.exists() {
        return Err(AppError::Validation(format!(
            "Parent directory does not exist: {}",
            parent.display()
        )));
    }

    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(content.as_bytes())?;
            return Ok(());
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err.into()),
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let temp_path = path.with_file_name(format!(".{file_name}.{}.tmp", Uuid::new_v4()));

    match fs::File::create(&temp_path).and_then(|mut file| file.write_all(content.as_bytes())) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&temp_path);
            return Err(err.into());
        }
    }

    if let Err(err) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(err.into());
    }

    Ok(())
}

/// 生成写入/编辑审批预览内容。
pub(crate) fn build_mutation_preview(path: &Path, previous: &str, next: &str) -> Value {
    json!({
        "path": path.to_string_lossy(),
        "diff": build_diff_preview(previous, next),
        "old_excerpt": truncate(previous, 4000),
        "new_excerpt": truncate(next, 4000),
    })
}

/// 在允许根目录集合中解析并校验请求路径。
///
/// 安全保证：
/// - 阻止路径穿越到允许目录之外；
/// - 阻止已存在路径通过符号链接逃逸；
/// - 对不存在目标，校验其父目录是否仍在允许范围内。
pub(crate) fn validate_path(requested_path: &str, workspace_root: &Path) -> AppResult<PathBuf> {
    let trimmed = requested_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("path cannot be empty".to_string()));
    }
    if trimmed.contains('\0') {
        return Err(AppError::Validation("path contains null byte".to_string()));
    }

    let allowed_dirs = resolve_allowed_directories(workspace_root);
    let expanded = expand_home_path(trimmed);

    let absolute = if expanded.is_absolute() {
        normalize_path(&expanded)
    } else {
        resolve_relative_path_against_allowed_directories(&expanded, &allowed_dirs)
    };

    if !is_path_within_allowed_dirs(&absolute, &allowed_dirs) {
        return Err(AppError::Validation(format!(
            "access denied - path outside allowed directories: {} not in {}",
            absolute.display(),
            format_allowed_dirs(&allowed_dirs)
        )));
    }

    match fs::canonicalize(&absolute) {
        Ok(real_path) => {
            let normalized_real = normalize_path(&real_path);
            if !is_path_within_allowed_dirs(&normalized_real, &allowed_dirs) {
                return Err(AppError::Validation(format!(
                    "access denied - symlink target outside allowed directories: {} not in {}",
                    normalized_real.display(),
                    format_allowed_dirs(&allowed_dirs)
                )));
            }
            Ok(normalized_real)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let parent_dir = absolute.parent().ok_or_else(|| {
                AppError::Validation(format!(
                    "invalid path without parent: {}",
                    absolute.display()
                ))
            })?;

            let real_parent = fs::canonicalize(parent_dir).map_err(|_| {
                AppError::Validation(format!(
                    "Parent directory does not exist: {}",
                    parent_dir.display()
                ))
            })?;
            let normalized_parent = normalize_path(&real_parent);

            if !is_path_within_allowed_dirs(&normalized_parent, &allowed_dirs) {
                return Err(AppError::Validation(format!(
                    "access denied - parent directory outside allowed directories: {} not in {}",
                    normalized_parent.display(),
                    format_allowed_dirs(&allowed_dirs)
                )));
            }

            Ok(absolute)
        }
        Err(err) => Err(err.into()),
    }
}

/// 解析工具允许访问的根目录集合（含环境变量扩展）。
pub(crate) fn resolve_allowed_directories(workspace_root: &Path) -> Vec<PathBuf> {
    let mut dedup = HashSet::<String>::new();
    let mut roots = Vec::<PathBuf>::new();

    push_root_variants(workspace_root, &mut roots, &mut dedup);

    if let Ok(raw) = std::env::var(FS_ALLOWED_ROOTS_ENV) {
        for value in raw
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            let expanded = expand_home_path(value);
            push_root_variants(&expanded, &mut roots, &mut dedup);
        }
    }

    if roots.is_empty() {
        roots.push(normalize_path(workspace_root));
    }

    roots
}

fn push_root_variants(root: &Path, roots: &mut Vec<PathBuf>, dedup: &mut HashSet<String>) {
    let absolute = if root.is_absolute() {
        normalize_path(root)
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        normalize_path(&cwd.join(root))
    };
    push_unique_path(absolute.clone(), roots, dedup);

    if let Ok(real) = fs::canonicalize(&absolute) {
        push_unique_path(normalize_path(&real), roots, dedup);
    }
}

fn push_unique_path(path: PathBuf, roots: &mut Vec<PathBuf>, dedup: &mut HashSet<String>) {
    let key = path.to_string_lossy().to_string();
    if dedup.insert(key) {
        roots.push(path);
    }
}

fn expand_home_path(raw_path: &str) -> PathBuf {
    if raw_path == "~" || raw_path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            if raw_path == "~" {
                return home;
            }
            return home.join(raw_path.trim_start_matches("~/"));
        }
    }
    PathBuf::from(raw_path)
}

fn resolve_relative_path_against_allowed_directories(
    relative_path: &Path,
    allowed_directories: &[PathBuf],
) -> PathBuf {
    if allowed_directories.is_empty() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        return normalize_path(&cwd.join(relative_path));
    }

    for allowed_dir in allowed_directories {
        let candidate = normalize_path(&allowed_dir.join(relative_path));
        if is_path_within_allowed_dirs(&candidate, allowed_directories) {
            return candidate;
        }
    }

    normalize_path(&allowed_directories[0].join(relative_path))
}

/// 判断路径是否位于允许目录集合之内。
pub(crate) fn is_path_within_allowed_dirs(path: &Path, allowed_directories: &[PathBuf]) -> bool {
    let normalized_path = normalize_path(path);
    allowed_directories.iter().any(|allowed_dir| {
        let normalized_dir = normalize_path(allowed_dir);
        normalized_path == normalized_dir || normalized_path.starts_with(&normalized_dir)
    })
}

fn format_allowed_dirs(allowed_dirs: &[PathBuf]) -> String {
    allowed_dirs
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_workspace_gitignore_matcher(workspace_root: &Path) -> Option<Gitignore> {
    let mut builder = GitignoreBuilder::new(workspace_root);
    let gitignore_path = workspace_root.join(".gitignore");
    if gitignore_path.is_file() {
        let _ = builder.add(gitignore_path);
    }
    builder.build().ok()
}

fn contains_tool_ignored_dirs(path: &Path) -> bool {
    path.components().any(|component| {
        if let Component::Normal(value) = component {
            let segment = value.to_string_lossy();
            TOOL_IGNORED_DIRS.contains(&segment.as_ref())
        } else {
            false
        }
    })
}

/// Normalize lexical `.` and `..` components.
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

/// 按字符数裁剪文本，防止单次工具输出过大。
pub(crate) fn truncate(text: &str, limit: usize) -> String {
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

    for (index, change) in diff.iter_all_changes().enumerate() {
        if index >= MAX_DIFF_PREVIEW_LINES {
            lines.push("... [diff truncated]".to_string());
            break;
        }
        let sign = match change.tag() {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };
        lines.push(format!(
            "{sign} {}",
            change.to_string().trim_end_matches('\n')
        ));
    }

    lines.join("\n")
}

/// 生成统一 diff 文本，供 `edit_file` 预览/返回使用。
pub(crate) fn create_unified_diff(previous: &str, next: &str, file_path: &Path) -> String {
    let normalized_previous = normalize_line_endings(previous);
    let normalized_next = normalize_line_endings(next);
    let diff = TextDiff::from_lines(&normalized_previous, &normalized_next);

    let mut output = String::new();
    let path = file_path.to_string_lossy();
    output.push_str(&format!("--- {path}\n"));
    output.push_str(&format!("+++ {path}\n"));

    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };
        output.push(sign);
        output.push_str(&change.to_string());
    }

    output
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// 读取前 N 行文本。
pub(crate) fn head_lines(text: &str, num_lines: usize) -> String {
    if num_lines == 0 {
        return String::new();
    }
    text.lines().take(num_lines).collect::<Vec<_>>().join("\n")
}

/// 读取后 N 行文本。
pub(crate) fn tail_lines(text: &str, num_lines: usize) -> String {
    if num_lines == 0 {
        return String::new();
    }
    let lines = text.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(num_lines);
    lines[start..].join("\n")
}

fn leading_whitespace(value: &str) -> &str {
    let end = value
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(index, _)| index)
        .unwrap_or(value.len());
    &value[..end]
}

/// 依序应用文本替换编辑。
pub(crate) fn apply_ordered_edits(content: &str, edits: &[FileEdit]) -> AppResult<String> {
    let mut modified_content = normalize_line_endings(content);

    for edit in edits {
        if edit.old_text.is_empty() {
            return Err(AppError::Validation(
                "`old_text` cannot be empty".to_string(),
            ));
        }

        let normalized_old = normalize_line_endings(&edit.old_text);
        let normalized_new = normalize_line_endings(&edit.new_text);

        if modified_content.contains(&normalized_old) {
            modified_content = modified_content.replacen(&normalized_old, &normalized_new, 1);
            continue;
        }

        let old_lines = normalized_old.split('\n').collect::<Vec<_>>();
        let mut content_lines = modified_content
            .split('\n')
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        if old_lines.len() > content_lines.len() {
            return Err(AppError::Validation(format!(
                "Could not find exact match for edit:\n{}",
                edit.old_text
            )));
        }

        let mut match_index = None;
        for index in 0..=(content_lines.len() - old_lines.len()) {
            let is_match = old_lines.iter().enumerate().all(|(line_index, old_line)| {
                let current_line = content_lines
                    .get(index + line_index)
                    .map(String::as_str)
                    .unwrap_or("");
                old_line.trim() == current_line.trim()
            });
            if is_match {
                match_index = Some(index);
                break;
            }
        }

        let Some(index) = match_index else {
            return Err(AppError::Validation(format!(
                "Could not find exact match for edit:\n{}",
                edit.old_text
            )));
        };

        let original_indent = content_lines
            .get(index)
            .map(String::as_str)
            .map(leading_whitespace)
            .unwrap_or("")
            .to_string();

        let new_lines = normalized_new
            .split('\n')
            .enumerate()
            .map(|(line_index, line)| {
                if line_index == 0 {
                    return format!("{original_indent}{}", line.trim_start());
                }

                let old_indent = old_lines
                    .get(line_index)
                    .map(|value| leading_whitespace(value))
                    .unwrap_or("");
                let new_indent = leading_whitespace(line);

                if !old_indent.is_empty() && !new_indent.is_empty() {
                    let relative_indent =
                        (new_indent.len() as isize - old_indent.len() as isize).max(0) as usize;
                    return format!(
                        "{original_indent}{}{trimmed}",
                        " ".repeat(relative_indent),
                        trimmed = line.trim_start()
                    );
                }

                line.to_string()
            })
            .collect::<Vec<_>>();

        content_lines.splice(index..index + old_lines.len(), new_lines);
        modified_content = content_lines.join("\n");
    }

    Ok(modified_content)
}

/// 递归搜索目录并按 glob 规则聚合匹配结果。
pub(crate) fn search_directory_recursive(
    current_path: &Path,
    root_path: &Path,
    allowed_dirs: &[PathBuf],
    ignore_policy: &ToolIgnorePolicy,
    include_pattern: &Pattern,
    exclude_patterns: &[Pattern],
    results: &mut Vec<String>,
    filtered_ignored_paths: &mut usize,
) -> AppResult<()> {
    let entries = fs::read_dir(current_path)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if !is_path_within_allowed_dirs(&path, allowed_dirs) {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let is_dir = file_type.is_dir();
        if ignore_policy.should_ignore_path(&path, is_dir) {
            *filtered_ignored_paths += 1;
            continue;
        }

        let relative = path
            .strip_prefix(root_path)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");

        let excluded = exclude_patterns
            .iter()
            .any(|pattern| pattern.matches(&relative));
        if excluded {
            continue;
        }

        if include_pattern.matches(&relative) {
            results.push(path.to_string_lossy().to_string());
            if results.len() >= MAX_SEARCH_RESULTS {
                return Ok(());
            }
        }

        if is_dir {
            search_directory_recursive(
                &path,
                root_path,
                allowed_dirs,
                ignore_policy,
                include_pattern,
                exclude_patterns,
                results,
                filtered_ignored_paths,
            )?;
            if results.len() >= MAX_SEARCH_RESULTS {
                return Ok(());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_ordered_edits, build_diff_preview, validate_path, FileEdit, FilesystemToolContext,
    };
    use crate::backend::agents::tools::ToolRuntimeContext;
    use crate::backend::models::domain::{new_chat_session, SessionApprovalMode};
    use crate::backend::{ApprovalService, StorageService, WsHub};
    use aquaregia::ToolCall;
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU8;
    use std::sync::{Arc, Mutex};
    use tempfile::{tempdir, TempDir};
    use tokio::time::{sleep, Duration, Instant};
    use tokio_util::sync::CancellationToken;
    use uuid::Uuid;

    fn build_test_context(dir: &TempDir) -> (FilesystemToolContext, StorageService, PathBuf) {
        let storage = StorageService::new(dir.path().join("state")).expect("storage");
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace");
        std::fs::create_dir_all(workspace_root.join("memory")).expect("memory dir");
        let mut session = new_chat_session(None);
        session.id = "session-1".to_string();
        storage.insert_session(&session).expect("insert session");

        let context = FilesystemToolContext {
            runtime: ToolRuntimeContext {
                session_id: "session-1".to_string(),
                turn_id: "turn-1".to_string(),
                current_step: Arc::new(AtomicU8::new(1)),
                tool_calls: Arc::new(Mutex::new(std::collections::HashMap::new())),
                cancellation_token: CancellationToken::new(),
                approvals: ApprovalService::new(storage.clone()),
                approval_mode: SessionApprovalMode::Default,
                storage: storage.clone(),
                hub: WsHub::new(),
            },
            workspace_root: workspace_root.clone(),
        };

        (context, storage, workspace_root)
    }

    fn register_tool_call(
        context: &FilesystemToolContext,
        tool_name: &str,
        args_json: Value,
    ) -> String {
        let call_id = format!("test-{}", Uuid::new_v4());
        let call = ToolCall {
            call_id: call_id.clone(),
            tool_name: tool_name.to_string(),
            args_json,
        };
        let mut registry = context.tool_calls.lock().expect("tool call lock");
        registry.insert(call_id.clone(), call);
        call_id
    }

    async fn wait_for_pending_approval(storage: &StorageService, action: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let approvals = storage.list_approvals().expect("list approvals");
            if let Some(approval) = approvals
                .into_iter()
                .find(|item| item.status == "pending" && item.action == action)
            {
                return approval.id;
            }
            assert!(
                Instant::now() < deadline,
                "pending approval not found for {action}"
            );
            sleep(Duration::from_millis(10)).await;
        }
    }

    #[test]
    fn diff_preview_marks_insertions() {
        let diff = build_diff_preview("alpha\n", "alpha\nbeta\n");
        assert!(diff.contains("+ beta"));
    }

    #[test]
    fn validate_path_denies_relative_traversal_outside_workspace() {
        let dir = tempdir().expect("tempdir");
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace");

        let resolved = validate_path("../outside.txt", &workspace_root);
        assert!(resolved.is_err());
        assert!(resolved
            .err()
            .map(|value| value.message())
            .unwrap_or_default()
            .contains("outside allowed directories"));
    }

    #[cfg(unix)]
    #[test]
    fn validate_path_denies_symlink_target_outside_workspace() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("tempdir");
        let workspace_root = dir.path().join("workspace");
        let outside_root = dir.path().join("outside");
        std::fs::create_dir_all(&workspace_root).expect("workspace");
        std::fs::create_dir_all(&outside_root).expect("outside");
        std::fs::write(outside_root.join("secret.txt"), "secret").expect("write outside");

        let linked_path = workspace_root.join("linked.txt");
        symlink(outside_root.join("secret.txt"), &linked_path).expect("symlink");

        let resolved = validate_path("linked.txt", &workspace_root);
        assert!(resolved.is_err());
        assert!(resolved
            .err()
            .map(|value| value.message())
            .unwrap_or_default()
            .contains("symlink target outside allowed directories"));
    }

    #[test]
    fn apply_ordered_edits_replaces_in_order() {
        let original = "line1\nline2\nline3\n";
        let edits = vec![
            FileEdit {
                old_text: "line1".to_string(),
                new_text: "line1-updated".to_string(),
            },
            FileEdit {
                old_text: "line3".to_string(),
                new_text: "line3-updated".to_string(),
            },
        ];

        let next = apply_ordered_edits(original, &edits).expect("apply edits");
        assert_eq!(next, "line1-updated\nline2\nline3-updated\n");
    }

    #[test]
    fn claim_tool_call_requires_internal_id() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, _workspace_root) = build_test_context(&dir);

        let result = context.claim_tool_call("list_directory", None);
        assert!(result.is_err());
        assert!(result
            .err()
            .map(|value| value.message())
            .unwrap_or_default()
            .contains("missing internal"));
    }

    #[test]
    fn claim_tool_call_rejects_tool_name_mismatch() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, _workspace_root) = build_test_context(&dir);
        let tool_call_id = register_tool_call(
            &context,
            "read_files",
            json!({
                "paths": ["README.md"],
            }),
        );

        let result = context.claim_tool_call("list_directory", Some(tool_call_id.as_str()));
        assert!(result.is_err());
        assert!(result
            .err()
            .map(|value| value.message())
            .unwrap_or_default()
            .contains("binding mismatch"));
    }

    #[test]
    fn list_directory_formats_sorted_entries() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, workspace_root) = build_test_context(&dir);
        std::fs::create_dir_all(workspace_root.join("a-dir")).expect("dir");
        std::fs::write(workspace_root.join("b-file.txt"), "b").expect("file");
        let tool_call_id = register_tool_call(
            &context,
            "list_directory",
            json!({
                "path": ".",
            }),
        );

        let payload = context
            .list_directory("list_directory", ".", Some(tool_call_id.as_str()))
            .expect("list directory");
        let formatted = payload
            .get("formatted")
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        assert!(formatted.contains("[DIR] a-dir"));
        assert!(formatted.contains("[FILE] b-file.txt"));

        let dir_index = formatted.find("[DIR] a-dir").unwrap_or(usize::MAX);
        let file_index = formatted.find("[FILE] b-file.txt").unwrap_or(0);
        assert!(dir_index < file_index);
    }

    #[test]
    fn list_directory_ignores_common_noise_dirs() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, workspace_root) = build_test_context(&dir);
        std::fs::create_dir_all(workspace_root.join("node_modules")).expect("node_modules");
        std::fs::create_dir_all(workspace_root.join("src")).expect("src");
        let tool_call_id = register_tool_call(
            &context,
            "list_directory",
            json!({
                "path": ".",
            }),
        );

        let payload = context
            .list_directory("list_directory", ".", Some(tool_call_id.as_str()))
            .expect("list directory");
        let entries = payload
            .get("entries")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let names = entries
            .into_iter()
            .filter_map(|value| {
                value
                    .get("name")
                    .and_then(|name| name.as_str())
                    .map(ToOwned::to_owned)
            })
            .collect::<Vec<_>>();

        assert!(names.contains(&"src".to_string()));
        assert!(!names.contains(&"node_modules".to_string()));
        assert_eq!(
            payload
                .get("filtered_ignored_entries")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
    }

    #[test]
    fn read_text_file_supports_head_tail_and_rejects_conflict() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, workspace_root) = build_test_context(&dir);
        std::fs::write(workspace_root.join("notes.txt"), "line1\nline2\nline3\n").expect("write");
        let head_call_id = register_tool_call(
            &context,
            "read_text_file",
            json!({
                "path": "notes.txt",
                "head": 2,
                "tail": Value::Null,
            }),
        );
        let tail_call_id = register_tool_call(
            &context,
            "read_text_file",
            json!({
                "path": "notes.txt",
                "head": Value::Null,
                "tail": 2,
            }),
        );
        let conflict_call_id = register_tool_call(
            &context,
            "read_text_file",
            json!({
                "path": "notes.txt",
                "head": 1,
                "tail": 1,
            }),
        );

        let head = context
            .read_text_file(
                "read_text_file",
                "notes.txt",
                Some(2),
                None,
                Some(head_call_id.as_str()),
            )
            .expect("head");
        assert_eq!(
            head.get("content").and_then(|value| value.as_str()),
            Some("line1\nline2")
        );

        let tail = context
            .read_text_file(
                "read_text_file",
                "notes.txt",
                None,
                Some(2),
                Some(tail_call_id.as_str()),
            )
            .expect("tail");
        assert_eq!(
            tail.get("content").and_then(|value| value.as_str()),
            Some("line2\nline3")
        );

        let conflict = context.read_text_file(
            "read_text_file",
            "notes.txt",
            Some(1),
            Some(1),
            Some(conflict_call_id.as_str()),
        );
        assert!(conflict.is_err());
        assert!(conflict
            .err()
            .map(|value| value.message())
            .unwrap_or_default()
            .contains("cannot set both `head` and `tail`"));
    }

    #[test]
    fn read_files_inlines_errors_and_keeps_order() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, workspace_root) = build_test_context(&dir);
        std::fs::write(workspace_root.join("f1.txt"), "one").expect("f1");
        std::fs::write(workspace_root.join("f2.txt"), "two").expect("f2");
        let tool_call_id = register_tool_call(
            &context,
            "read_files",
            json!({
                "paths": [
                    "f1.txt",
                    "missing.txt",
                    "f2.txt",
                ],
            }),
        );

        let payload = context
            .read_files(
                "read_files",
                &[
                    "f1.txt".to_string(),
                    "missing.txt".to_string(),
                    "f2.txt".to_string(),
                ],
                Some(tool_call_id.as_str()),
            )
            .expect("read files");

        let merged = payload
            .get("merged")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        assert!(merged.contains("f1.txt:\none"));
        assert!(merged.contains("missing.txt: Error -"));
        assert!(merged.contains("f2.txt:\ntwo"));
    }

    #[test]
    fn search_files_honors_glob_and_excludes() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, workspace_root) = build_test_context(&dir);
        std::fs::create_dir_all(workspace_root.join("src")).expect("src");
        std::fs::create_dir_all(workspace_root.join("target")).expect("target");
        std::fs::write(workspace_root.join("src/lib.rs"), "lib").expect("src file");
        std::fs::write(workspace_root.join("target/skip.rs"), "skip").expect("target file");
        let tool_call_id = register_tool_call(
            &context,
            "search_files",
            json!({
                "path": ".",
                "pattern": "**/*.rs",
                "exclude_patterns": ["target/**"],
            }),
        );

        let payload = context
            .search_files(
                "search_files",
                ".",
                "**/*.rs",
                &["target/**".to_string()],
                Some(tool_call_id.as_str()),
            )
            .expect("search");

        let matches = payload
            .get("matches")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
            .collect::<Vec<_>>();

        assert!(matches
            .iter()
            .any(|path| path.ends_with("src/lib.rs") || path.ends_with("src\\lib.rs")));
        assert!(!matches
            .iter()
            .any(|path| path.ends_with("target/skip.rs") || path.ends_with("target\\skip.rs")));
    }

    #[test]
    fn search_files_ignores_common_dirs_and_gitignore_in_tool_phase() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, workspace_root) = build_test_context(&dir);
        std::fs::create_dir_all(workspace_root.join("src")).expect("src");
        std::fs::create_dir_all(workspace_root.join("node_modules/pkg")).expect("node_modules");
        std::fs::create_dir_all(workspace_root.join("dist")).expect("dist");
        std::fs::write(workspace_root.join(".gitignore"), "dist/\n").expect("gitignore");
        std::fs::write(workspace_root.join("src/keep.rs"), "keep").expect("keep");
        std::fs::write(workspace_root.join("node_modules/pkg/skip.rs"), "skip").expect("skip");
        std::fs::write(workspace_root.join("dist/generated.rs"), "generated").expect("generated");
        let tool_call_id = register_tool_call(
            &context,
            "search_files",
            json!({
                "path": ".",
                "pattern": "**/*.rs",
                "exclude_patterns": [],
            }),
        );

        let payload = context
            .search_files(
                "search_files",
                ".",
                "**/*.rs",
                &[],
                Some(tool_call_id.as_str()),
            )
            .expect("search");
        let matches = payload
            .get("matches")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
            .collect::<Vec<_>>();

        assert!(matches
            .iter()
            .any(|path| path.ends_with("src/keep.rs") || path.ends_with("src\\keep.rs")));
        assert!(!matches.iter().any(|path| path.contains("node_modules")));
        assert!(!matches
            .iter()
            .any(|path| path.contains("/dist/") || path.contains("\\dist\\")));
        assert!(
            payload
                .get("filtered_ignored_paths")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)
                >= 2
        );
    }

    #[tokio::test]
    async fn write_file_fails_when_parent_missing() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, _workspace_root) = build_test_context(&dir);
        let tool_call_id = register_tool_call(
            &context,
            "write_file",
            json!({
                "path": "missing/new.txt",
            }),
        );

        let result = context
            .write_file(
                "write_file",
                "missing/new.txt",
                "hello",
                Some(tool_call_id.as_str()),
            )
            .await;
        assert!(result.is_err());
        assert!(result
            .err()
            .map(|value| value.message())
            .unwrap_or_default()
            .contains("Parent directory does not exist"));
    }

    #[tokio::test]
    async fn write_file_waits_for_approval_and_applies_after_approve() {
        let dir = tempdir().expect("tempdir");
        let (context, storage, workspace_root) = build_test_context(&dir);
        let target = workspace_root.join("notes.txt");
        std::fs::write(&target, "before").expect("seed");
        let tool_call_id = register_tool_call(
            &context,
            "write_file",
            json!({
                "path": "notes.txt",
            }),
        );

        let task_context = context.clone();
        let task_tool_call_id = tool_call_id.clone();
        let handle = tokio::spawn(async move {
            task_context
                .write_file(
                    "write_file",
                    "notes.txt",
                    "after",
                    Some(task_tool_call_id.as_str()),
                )
                .await
        });

        let approval_id = wait_for_pending_approval(&storage, "write_file").await;
        context
            .approvals
            .resolve(&approval_id, true)
            .expect("approve write");

        let payload = handle.await.expect("join").expect("write after approve");
        assert_eq!(
            std::fs::read_to_string(&target).expect("read target"),
            "after"
        );
        assert_eq!(
            payload
                .get("approval_bypassed")
                .and_then(|value| value.as_bool()),
            None
        );
    }

    #[tokio::test]
    async fn edit_file_dry_run_does_not_write() {
        let dir = tempdir().expect("tempdir");
        let (context, _storage, workspace_root) = build_test_context(&dir);
        let target = workspace_root.join("edit.txt");
        std::fs::write(&target, "alpha\nbeta\n").expect("seed");
        let tool_call_id = register_tool_call(
            &context,
            "edit_file",
            json!({
                "path": "edit.txt",
                "edit_count": 1,
                "dry_run": true,
            }),
        );

        let payload = context
            .edit_file(
                "edit_file",
                "edit.txt",
                &[FileEdit {
                    old_text: "alpha".to_string(),
                    new_text: "alpha-updated".to_string(),
                }],
                true,
                Some(tool_call_id.as_str()),
            )
            .await
            .expect("dry run");

        assert_eq!(
            payload.get("dry_run").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(payload
            .get("diff")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .contains("---"));
        assert_eq!(
            std::fs::read_to_string(&target).expect("read"),
            "alpha\nbeta\n"
        );
    }

    #[tokio::test]
    async fn edit_file_rejected_keeps_original_content() {
        let dir = tempdir().expect("tempdir");
        let (context, storage, workspace_root) = build_test_context(&dir);
        let target = workspace_root.join("edit-reject.txt");
        std::fs::write(&target, "before\n").expect("seed");
        let tool_call_id = register_tool_call(
            &context,
            "edit_file",
            json!({
                "path": "edit-reject.txt",
                "edit_count": 1,
                "dry_run": false,
            }),
        );

        let task_context = context.clone();
        let task_tool_call_id = tool_call_id.clone();
        let handle = tokio::spawn(async move {
            task_context
                .edit_file(
                    "edit_file",
                    "edit-reject.txt",
                    &[FileEdit {
                        old_text: "before".to_string(),
                        new_text: "after".to_string(),
                    }],
                    false,
                    Some(task_tool_call_id.as_str()),
                )
                .await
        });

        let approval_id = wait_for_pending_approval(&storage, "edit_file").await;
        context
            .approvals
            .resolve(&approval_id, false)
            .expect("reject edit");

        let result = handle.await.expect("join");
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&target).expect("read"), "before\n");
    }

    #[tokio::test]
    async fn memory_paths_bypass_approval_and_trigger_sync() {
        let dir = tempdir().expect("tempdir");
        let (context, storage, _workspace_root) = build_test_context(&dir);
        let tool_call_id = register_tool_call(
            &context,
            "write_file",
            json!({
                "path": "MEMORY.md",
            }),
        );

        let payload = context
            .write_file(
                "write_file",
                "MEMORY.md",
                "hello memory",
                Some(tool_call_id.as_str()),
            )
            .await
            .expect("write memory");

        assert_eq!(
            payload
                .get("approval_bypassed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            payload
                .get("memory_sync")
                .and_then(|value| value.get("updated"))
                .and_then(|value| value.as_u64()),
            Some(1)
        );

        let hits = storage.memory_search("hello", 6, 0.01).expect("search");
        assert!(!hits.is_empty());
    }
}
