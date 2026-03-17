use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{SecondsFormat, Utc};

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::responses::WorkspaceFileInfo;

const BOOTSTRAP_CONTEXT_FILES: [&str; 7] = [
    "AGENTS.md",
    "SOUL.md",
    "TOOLS.md",
    "IDENTITY.md",
    "USER.md",
    "HEARTBEAT.md",
    "BOOTSTRAP.md",
];

const TOP_LEVEL_TEMPLATE_FILES: [&str; 8] = [
    "AGENTS.md",
    "SOUL.md",
    "TOOLS.md",
    "IDENTITY.md",
    "USER.md",
    "HEARTBEAT.md",
    "MEMORY.md",
    "BOOTSTRAP.md",
];

const REQUIRED_CONTEXT_FILES: [&str; 2] = ["AGENTS.md", "SOUL.md"];
const BOOTSTRAP_PER_FILE_MAX_CHARS: usize = 20_000;
const BOOTSTRAP_TOTAL_MAX_CHARS: usize = 150_000;

const BOOTSTRAP_COMPLETED_FILE: &str = ".bootstrap_completed";

const ZH_AGENTS_TEMPLATE: &str = include_str!("prompts/templates/AGENTS.md");
const ZH_SOUL_TEMPLATE: &str = include_str!("prompts/templates/SOUL.md");
const ZH_TOOLS_TEMPLATE: &str = include_str!("prompts/templates/TOOLS.md");
const ZH_IDENTITY_TEMPLATE: &str = include_str!("prompts/templates/IDENTITY.md");
const ZH_USER_TEMPLATE: &str = include_str!("prompts/templates/USER.md");
const ZH_HEARTBEAT_TEMPLATE: &str = include_str!("prompts/templates/HEARTBEAT.md");
const ZH_MEMORY_TEMPLATE: &str = include_str!("prompts/templates/MEMORY.md");
const ZH_BOOTSTRAP_TEMPLATE: &str = include_str!("prompts/templates/BOOTSTRAP.md");

#[derive(Clone, Debug)]
pub struct AgentWorkspace {
    root: PathBuf,
}

impl AgentWorkspace {
    pub fn new(base_dir: &Path) -> Self {
        Self {
            root: base_dir.join("workspace"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn memory_dir(&self) -> PathBuf {
        self.root.join("memory")
    }

    pub fn ensure_layout(&self) -> AppResult<()> {
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(self.memory_dir())?;
        Ok(())
    }

    pub fn install_templates(
        &self,
        _language: &str,
        skip_existing: bool,
    ) -> AppResult<Vec<String>> {
        self.ensure_layout()?;

        let templates = templates_for_language();
        let mut copied = Vec::new();

        for (name, content) in templates {
            let target = self.root.join(name);
            if skip_existing && target.exists() {
                continue;
            }
            fs::write(&target, content)?;
            copied.push(name.to_string());
        }

        Ok(copied)
    }

    pub fn list_files(&self) -> AppResult<Vec<WorkspaceFileInfo>> {
        self.ensure_layout()?;

        let mut files = Vec::new();

        for name in TOP_LEVEL_TEMPLATE_FILES {
            let path = self.root.join(name);
            if path.is_file() {
                files.push(self.file_info_from_path(&path)?);
            }
        }

        let memory_dir = self.memory_dir();
        if memory_dir.is_dir() {
            let mut entries = fs::read_dir(memory_dir)?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md")
                })
                .collect::<Vec<_>>();
            entries.sort();
            for path in entries {
                files.push(self.file_info_from_path(&path)?);
            }
        }

        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }

    pub fn read_workspace_file(&self, relative_path: &str) -> AppResult<String> {
        let path = self.resolve_workspace_file(relative_path)?;
        fs::read_to_string(path).map_err(Into::into)
    }

    pub fn write_workspace_file(&self, relative_path: &str, content: &str) -> AppResult<PathBuf> {
        let path = self.resolve_workspace_file(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(path)
    }

    pub fn build_system_prompt(&self) -> AppResult<String> {
        self.ensure_layout()?;
        for required in REQUIRED_CONTEXT_FILES {
            let path = self.root.join(required);
            if !path.exists() {
                return Err(AppError::Validation(format!(
                    "required prompt file `{required}` is missing"
                )));
            }
        }

        let mut prompt = vec![
            "You are a personal assistant running inside YouClaw.".to_string(),
            "".to_string(),
            "## Memory Recall".to_string(),
            "Before answering anything about prior work, decisions, dates, people, preferences, or todos: run memory_search on MEMORY.md + memory/*.md; then use memory_get to pull only the needed lines.".to_string(),
            "If memory_search returns unavailable/disabled, explicitly tell the user memory retrieval is unavailable.".to_string(),
            "".to_string(),
            "## Workspace".to_string(),
            format!("Your working directory is: {}", self.root.to_string_lossy()),
            "memory/*.md daily files are not auto injected; read them on demand via memory_search/memory_get.".to_string(),
            "".to_string(),
            "## Project Context".to_string(),
        ];

        let mut remaining_budget = BOOTSTRAP_TOTAL_MAX_CHARS;
        let mut truncated_files = Vec::new();
        for path in self.collect_bootstrap_prompt_files() {
            if remaining_budget == 0 {
                break;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let content = fs::read_to_string(&path).map_err(|err| {
                AppError::Io(format!("failed to read prompt file `{name}`: {err}"))
            })?;
            let normalized = strip_frontmatter(content.trim());
            if normalized.is_empty() {
                continue;
            }

            let (per_file_trimmed, per_file_truncated) =
                truncate_with_marker(&normalized, BOOTSTRAP_PER_FILE_MAX_CHARS);
            let (final_content, total_truncated) =
                truncate_with_marker(&per_file_trimmed, remaining_budget);
            if final_content.trim().is_empty() {
                continue;
            }
            remaining_budget = remaining_budget.saturating_sub(final_content.chars().count());
            if per_file_truncated || total_truncated {
                truncated_files.push(name.to_string());
            }
            prompt.push(format!("# {name}\n\n{final_content}"));
            prompt.push(String::new());
        }

        if !truncated_files.is_empty() {
            prompt.push(format!(
                "Warning: bootstrap context truncated for: {}",
                truncated_files.join(", ")
            ));
        }

        Ok(prompt.join("\n"))
    }

    pub fn build_bootstrap_guidance(&self, _language: &str) -> String {
        "# 引导模式已激活\n\n请先读取工作区中的 BOOTSTRAP.md 并按其执行。若用户明确要求跳过引导，再直接回答原问题。".to_string()
    }

    fn collect_bootstrap_prompt_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for name in BOOTSTRAP_CONTEXT_FILES {
            let path = self.root.join(name);
            if path.is_file() {
                files.push(path);
            }
        }

        let memory = self.root.join("MEMORY.md");
        if memory.is_file() {
            files.push(memory);
        }
        files
    }

    pub fn should_bootstrap(&self) -> bool {
        self.root.join("BOOTSTRAP.md").is_file()
            && !self.root.join(BOOTSTRAP_COMPLETED_FILE).is_file()
    }

    pub fn mark_bootstrap_completed(&self) -> AppResult<()> {
        fs::write(self.root.join(BOOTSTRAP_COMPLETED_FILE), b"ok")?;
        Ok(())
    }

    pub fn relative_path(&self, path: &Path) -> AppResult<String> {
        let canonical_root = self.root.canonicalize()?;
        let canonical = path.canonicalize()?;
        if !canonical.starts_with(&canonical_root) {
            return Err(AppError::Validation(
                "path is outside workspace".to_string(),
            ));
        }
        let rel = canonical
            .strip_prefix(&canonical_root)
            .map_err(|_| AppError::Validation("failed to resolve relative path".to_string()))?;
        Ok(rel.to_string_lossy().replace('\\', "/"))
    }

    fn file_info_from_path(&self, path: &Path) -> AppResult<WorkspaceFileInfo> {
        let metadata = fs::metadata(path)?;
        let modified_at = metadata
            .modified()
            .ok()
            .map(|ts| chrono::DateTime::<Utc>::from(ts).to_rfc3339_opts(SecondsFormat::Nanos, true))
            .unwrap_or_else(|| Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true));

        Ok(WorkspaceFileInfo {
            path: self.relative_path(path)?,
            size: metadata.len(),
            modified_at,
        })
    }

    pub fn resolve_workspace_file(&self, relative_path: &str) -> AppResult<PathBuf> {
        self.ensure_layout()?;
        let rel = normalize_rel_path(relative_path)?;

        if !is_allowed_workspace_path(&rel) {
            return Err(AppError::Validation(
                "workspace path is not allowed".to_string(),
            ));
        }

        Ok(self.root.join(rel))
    }
}

fn templates_for_language() -> [(&'static str, &'static str); 8] {
    [
        ("AGENTS.md", ZH_AGENTS_TEMPLATE),
        ("SOUL.md", ZH_SOUL_TEMPLATE),
        ("TOOLS.md", ZH_TOOLS_TEMPLATE),
        ("IDENTITY.md", ZH_IDENTITY_TEMPLATE),
        ("USER.md", ZH_USER_TEMPLATE),
        ("HEARTBEAT.md", ZH_HEARTBEAT_TEMPLATE),
        ("MEMORY.md", ZH_MEMORY_TEMPLATE),
        ("BOOTSTRAP.md", ZH_BOOTSTRAP_TEMPLATE),
    ]
}

fn strip_frontmatter(content: &str) -> String {
    if !content.starts_with("---") {
        return content.trim().to_string();
    }

    let mut parts = content.splitn(3, "---");
    let _ = parts.next();
    let second = parts.next();
    let third = parts.next();

    if second.is_some() {
        third.unwrap_or_default().trim().to_string()
    } else {
        content.trim().to_string()
    }
}

fn truncate_with_marker(content: &str, max_chars: usize) -> (String, bool) {
    if content.chars().count() <= max_chars {
        return (content.to_string(), false);
    }
    const MARKER: &str = "\n...[truncated]";
    let marker_chars = MARKER.chars().count();
    if max_chars <= marker_chars {
        return (MARKER.chars().take(max_chars).collect::<String>(), true);
    }
    let content_budget = max_chars - marker_chars;
    let mut trimmed = content.chars().take(content_budget).collect::<String>();
    trimmed.push_str(MARKER);
    (trimmed, true)
}

fn normalize_rel_path(relative_path: &str) -> AppResult<PathBuf> {
    let raw = relative_path.trim();
    if raw.is_empty() {
        return Err(AppError::Validation("path is empty".to_string()));
    }

    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(AppError::Validation(
            "absolute path is not allowed".to_string(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::Validation(
                    "parent/root path components are not allowed".to_string(),
                ));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(AppError::Validation("path is empty".to_string()));
    }

    Ok(normalized)
}

fn is_allowed_workspace_path(path: &Path) -> bool {
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return false;
    }

    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    match components.as_slice() {
        [top] => TOP_LEVEL_TEMPLATE_FILES.contains(&top.as_str()),
        [dir, _file] if dir == "memory" => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::AgentWorkspace;

    #[test]
    fn installs_templates_when_missing() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        let copied = workspace.install_templates("en", true).expect("install");
        assert_eq!(copied.len(), 8);

        let root = workspace.root().to_path_buf();
        for name in [
            "AGENTS.md",
            "SOUL.md",
            "TOOLS.md",
            "IDENTITY.md",
            "USER.md",
            "HEARTBEAT.md",
            "MEMORY.md",
            "BOOTSTRAP.md",
        ] {
            assert!(root.join(name).is_file(), "missing {name}");
        }
        let agents = fs::read_to_string(root.join("AGENTS.md")).expect("read agents");
        assert!(agents.contains("## 记忆"));
    }

    #[test]
    fn build_system_prompt_errors_when_required_file_missing() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file("USER.md", "only user")
            .expect("write user");

        assert!(workspace.build_system_prompt().is_err());
    }

    #[test]
    fn build_system_prompt_orders_and_strips_frontmatter() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file("AGENTS.md", "---\nname: a\n---\nA")
            .expect("write agents");
        workspace
            .write_workspace_file("SOUL.md", "---\nname: b\n---\nB")
            .expect("write soul");
        workspace
            .write_workspace_file("TOOLS.md", "C")
            .expect("write tools");
        workspace
            .write_workspace_file("USER.md", "D")
            .expect("write user");
        workspace
            .write_workspace_file("MEMORY.md", "E")
            .expect("write memory");

        let prompt = workspace.build_system_prompt().expect("prompt");
        let agents_index = prompt.find("# AGENTS.md").expect("agents section");
        let soul_index = prompt.find("# SOUL.md").expect("soul section");
        let tools_index = prompt.find("# TOOLS.md").expect("tools section");
        assert!(agents_index < soul_index && soul_index < tools_index);
        assert!(!prompt.contains("name: a"));
        assert!(prompt.contains("\n\nA"));
        assert!(prompt.contains("\n\nB"));
        assert!(prompt.contains("\n\nC"));
        assert!(prompt.contains("\n\nD"));
        assert!(prompt.contains("\n\nE"));
    }

    #[test]
    fn system_prompt_does_not_auto_inject_daily_memory_files() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file("AGENTS.md", "A")
            .expect("write agents");
        workspace
            .write_workspace_file("SOUL.md", "B")
            .expect("write soul");
        workspace
            .write_workspace_file("MEMORY.md", "C")
            .expect("write memory");
        workspace
            .write_workspace_file("memory/2026-03-15.md", "DAILY_CONTENT_MARKER")
            .expect("write daily");

        let prompt = workspace.build_system_prompt().expect("prompt");
        assert!(!prompt.contains("DAILY_CONTENT_MARKER"));
        assert!(prompt.contains("# MEMORY.md"));
    }

    #[test]
    fn system_prompt_truncates_large_bootstrap_files() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file("AGENTS.md", "A")
            .expect("write agents");
        workspace
            .write_workspace_file("SOUL.md", "B")
            .expect("write soul");
        workspace
            .write_workspace_file("TOOLS.md", &"x".repeat(30_000))
            .expect("write tools");

        let prompt = workspace.build_system_prompt().expect("prompt");
        assert!(prompt.contains("...[truncated]"));
    }
}
