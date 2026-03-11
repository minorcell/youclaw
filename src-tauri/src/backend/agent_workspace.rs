use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{SecondsFormat, Utc};

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::WorkspaceFileInfo;

const FILE_ORDER: [(&str, bool); 4] = [
    ("AGENTS.md", true),
    ("SOUL.md", true),
    ("PROFILE.md", false),
    ("MEMORY.md", false),
];

const TOP_LEVEL_TEMPLATE_FILES: [&str; 5] = [
    "AGENTS.md",
    "SOUL.md",
    "PROFILE.md",
    "MEMORY.md",
    "BOOTSTRAP.md",
];

const BOOTSTRAP_COMPLETED_FILE: &str = ".bootstrap_completed";

const ZH_AGENTS_TEMPLATE: &str = include_str!("prompts/templates/AGENTS.md");
const ZH_SOUL_TEMPLATE: &str = include_str!("prompts/templates/SOUL.md");
const ZH_PROFILE_TEMPLATE: &str = include_str!("prompts/templates/PROFILE.md");
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

    pub fn read_memory_file(&self, relative_path: &str) -> AppResult<String> {
        let path = self.resolve_memory_file(relative_path)?;
        fs::read_to_string(path).map_err(Into::into)
    }

    pub fn write_memory_file(
        &self,
        relative_path: &str,
        content: &str,
        append: bool,
    ) -> AppResult<PathBuf> {
        let path = self.resolve_memory_file(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        if append {
            let mut new_content = String::new();
            if path.exists() {
                new_content.push_str(&fs::read_to_string(&path)?);
                if !new_content.ends_with('\n') {
                    new_content.push('\n');
                }
            }
            new_content.push_str(content);
            fs::write(&path, new_content)?;
        } else {
            fs::write(&path, content)?;
        }

        Ok(path)
    }

    pub fn build_system_prompt(&self) -> AppResult<String> {
        self.ensure_layout()?;
        let mut parts = Vec::<String>::new();

        for (name, required) in FILE_ORDER {
            let path = self.root.join(name);
            if !path.exists() {
                if required {
                    return Err(AppError::Validation(format!(
                        "required prompt file `{name}` is missing"
                    )));
                }
                continue;
            }

            let content = fs::read_to_string(&path).map_err(|err| {
                AppError::Io(format!(
                    "failed to read required prompt file `{name}`: {err}"
                ))
            })?;
            let normalized = strip_frontmatter(content.trim());
            if normalized.is_empty() {
                if required {
                    return Err(AppError::Validation(format!(
                        "required prompt file `{name}` is empty"
                    )));
                }
                continue;
            }
            parts.push(format!("# {name}\n\n{normalized}"));
        }

        if parts.is_empty() {
            Err(AppError::Validation(
                "no prompt sections available to build system prompt".to_string(),
            ))
        } else {
            Ok(parts.join("\n\n"))
        }
    }

    pub fn build_bootstrap_guidance(&self, _language: &str) -> String {
        "# 引导模式已激活\n\n请先读取工作区中的 BOOTSTRAP.md 并按其执行。若用户明确要求跳过引导，再直接回答原问题。".to_string()
    }

    pub fn should_bootstrap(&self) -> bool {
        self.root.join("BOOTSTRAP.md").is_file()
            && !self.root.join(BOOTSTRAP_COMPLETED_FILE).is_file()
    }

    pub fn mark_bootstrap_completed(&self) -> AppResult<()> {
        fs::write(self.root.join(BOOTSTRAP_COMPLETED_FILE), b"ok")?;
        Ok(())
    }

    pub fn collect_memory_source_files(&self) -> AppResult<Vec<PathBuf>> {
        self.ensure_layout()?;

        let mut files = Vec::new();
        for top in ["MEMORY.md", "PROFILE.md"] {
            let path = self.root.join(top);
            if path.is_file() {
                files.push(path);
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
            files.extend(entries);
        }

        Ok(files)
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

    pub fn resolve_memory_file(&self, relative_path: &str) -> AppResult<PathBuf> {
        self.ensure_layout()?;
        let rel = normalize_rel_path(relative_path)?;

        if !is_allowed_memory_path(&rel) {
            return Err(AppError::Validation(
                "memory path is not allowed".to_string(),
            ));
        }

        Ok(self.root.join(rel))
    }
}

fn templates_for_language() -> [(&'static str, &'static str); 5] {
    [
        ("AGENTS.md", ZH_AGENTS_TEMPLATE),
        ("SOUL.md", ZH_SOUL_TEMPLATE),
        ("PROFILE.md", ZH_PROFILE_TEMPLATE),
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

fn is_allowed_memory_path(path: &Path) -> bool {
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return false;
    }

    match components.as_slice() {
        [top] => top == "MEMORY.md" || top == "PROFILE.md",
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
        assert_eq!(copied.len(), 5);

        let root = workspace.root().to_path_buf();
        for name in [
            "AGENTS.md",
            "SOUL.md",
            "PROFILE.md",
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
            .write_workspace_file("PROFILE.md", "only profile")
            .expect("write profile");

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
            .write_workspace_file("PROFILE.md", "C")
            .expect("write profile");

        let prompt = workspace.build_system_prompt().expect("prompt");
        let agents_index = prompt.find("# AGENTS.md").expect("agents section");
        let soul_index = prompt.find("# SOUL.md").expect("soul section");
        let profile_index = prompt.find("# PROFILE.md").expect("profile section");
        assert!(agents_index < soul_index && soul_index < profile_index);
        assert!(!prompt.contains("name: a"));
        assert!(prompt.contains("\n\nA"));
        assert!(prompt.contains("\n\nB"));
        assert!(prompt.contains("\n\nC"));
    }
}
