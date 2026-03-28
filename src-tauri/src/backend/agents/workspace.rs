use std::fs;
use std::path::{Path, PathBuf};

use super::prompt;
#[cfg(test)]
use crate::backend::errors::AppError;
use crate::backend::errors::AppResult;
use crate::backend::models::domain::AgentProfile;

const LEGACY_PROMPT_FILE_NAME: &str = "AGENTS.md";
const SYSTEM_PROMPT_FILE_NAME: &str = "SYSTEM_PROMPT.md";

const ZH_SYSTEM_PROMPT_TEMPLATE: &str = include_str!("prompts/templates/SYSTEM_PROMPT.md");

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

    pub fn ensure_layout(&self) -> AppResult<()> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    pub fn install_templates(&self) -> AppResult<Vec<String>> {
        self.ensure_layout()?;

        let target = self.root.join(SYSTEM_PROMPT_FILE_NAME);
        fs::write(&target, ZH_SYSTEM_PROMPT_TEMPLATE)?;

        let legacy_target = self.root.join(LEGACY_PROMPT_FILE_NAME);
        if legacy_target.exists() {
            fs::remove_file(legacy_target)?;
        }

        Ok(vec![SYSTEM_PROMPT_FILE_NAME.to_string()])
    }

    #[cfg(test)]
    pub fn write_workspace_file(&self, relative_path: &str, content: &str) -> AppResult<PathBuf> {
        let path = self.resolve_workspace_file(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(path)
    }

    pub fn build_system_prompt(
        &self,
        project_workspace_root: &Path,
        profiles: &[AgentProfile],
    ) -> AppResult<String> {
        self.ensure_layout()?;
        let system_prompt_path = self.root.join(SYSTEM_PROMPT_FILE_NAME);
        prompt::build_system_prompt(&system_prompt_path, project_workspace_root, profiles)
    }

    #[cfg(test)]
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

#[cfg(test)]
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
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
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

#[cfg(test)]
fn is_allowed_workspace_path(path: &Path) -> bool {
    let components = path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    matches!(components.as_slice(), [name] if name == SYSTEM_PROMPT_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        AgentWorkspace, LEGACY_PROMPT_FILE_NAME, SYSTEM_PROMPT_FILE_NAME, ZH_SYSTEM_PROMPT_TEMPLATE,
    };
    use crate::backend::models::domain::{AgentProfile, ProfileTarget};

    fn profile(target: ProfileTarget, content: &str) -> AgentProfile {
        AgentProfile {
            target,
            content: content.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn installs_system_prompt_template_when_missing() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        let copied = workspace.install_templates().expect("install");
        assert_eq!(copied, vec![SYSTEM_PROMPT_FILE_NAME.to_string()]);
        assert!(
            dir.path()
                .join("workspace")
                .join(SYSTEM_PROMPT_FILE_NAME)
                .is_file()
        );
    }

    #[test]
    fn build_system_prompt_errors_when_prompt_file_missing() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");

        assert!(workspace.build_system_prompt(dir.path(), &[]).is_err());
    }

    #[test]
    fn build_system_prompt_strips_frontmatter_and_injects_profiles() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file(SYSTEM_PROMPT_FILE_NAME, "---\nname: a\n---\nA")
            .expect("write system prompt");

        let prompt = workspace
            .build_system_prompt(
                dir.path(),
                &[
                    profile(ProfileTarget::User, "User data"),
                    profile(ProfileTarget::Soul, "Soul data"),
                ],
            )
            .expect("prompt");
        assert!(!prompt.contains("name: a"));
        assert!(prompt.starts_with("A\n\n## 你的一些记忆："));
        assert!(prompt.contains("## 你的一些记忆："));
        assert!(prompt.contains("User data"));
        assert!(prompt.contains("Soul data"));
    }

    #[test]
    fn system_prompt_references_project_workspace_profiles_and_memory_system() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file(SYSTEM_PROMPT_FILE_NAME, "A")
            .expect("write system prompt");

        let project_dir = dir.path().join("project");
        fs::create_dir_all(&project_dir).expect("project dir");

        let prompt = workspace
            .build_system_prompt(
                &project_dir,
                &[
                    profile(ProfileTarget::User, "A user"),
                    profile(ProfileTarget::Soul, "A soul"),
                ],
            )
            .expect("prompt");
        assert!(prompt.contains(project_dir.to_string_lossy().as_ref()));
        assert!(prompt.contains("## 活动区域"));
        assert!(prompt.contains("### User Profile"));
        assert!(prompt.contains("### Agent Soul"));
        assert!(prompt.contains("A user"));
        assert!(prompt.contains("A soul"));
    }

    #[test]
    fn system_prompt_truncates_large_profile_content() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file(SYSTEM_PROMPT_FILE_NAME, "A")
            .expect("write system prompt");

        let prompt = workspace
            .build_system_prompt(
                dir.path(),
                &[
                    profile(ProfileTarget::User, &"x".repeat(20_000)),
                    profile(ProfileTarget::Soul, "ok"),
                ],
            )
            .expect("prompt");
        assert!(prompt.contains("...[已截断]"));
    }

    #[test]
    fn install_templates_overwrites_existing_system_prompt_template() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file(SYSTEM_PROMPT_FILE_NAME, "A")
            .expect("write system prompt");
        let content = fs::read_to_string(
            dir.path().join("workspace").join(SYSTEM_PROMPT_FILE_NAME),
        )
        .expect("read system prompt");
        assert_eq!(content, "A");

        let copied = workspace.install_templates().expect("install");
        assert_eq!(copied, vec![SYSTEM_PROMPT_FILE_NAME.to_string()]);
        let content = fs::read_to_string(
            dir.path().join("workspace").join(SYSTEM_PROMPT_FILE_NAME),
        )
        .expect("read system prompt");
        assert_eq!(content, ZH_SYSTEM_PROMPT_TEMPLATE);
    }

    #[test]
    fn install_templates_removes_legacy_agents_file() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        fs::write(
            dir.path().join("workspace").join(LEGACY_PROMPT_FILE_NAME),
            "legacy",
        )
        .expect("write legacy prompt");

        workspace.install_templates().expect("install");

        assert!(
            !dir.path()
                .join("workspace")
                .join(LEGACY_PROMPT_FILE_NAME)
                .exists()
        );
        assert!(
            dir.path()
                .join("workspace")
                .join(SYSTEM_PROMPT_FILE_NAME)
                .is_file()
        );
    }
}
