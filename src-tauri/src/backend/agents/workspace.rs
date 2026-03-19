use std::fs;
use std::path::{Path, PathBuf};

use super::prompt;
#[cfg(test)]
use crate::backend::errors::AppError;
use crate::backend::errors::AppResult;
use crate::backend::models::domain::AgentProfile;

const AGENTS_FILE_NAME: &str = "AGENTS.md";

const ZH_AGENTS_TEMPLATE: &str = include_str!("prompts/templates/AGENTS.md");

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

        let target = self.root.join(AGENTS_FILE_NAME);
        fs::write(&target, ZH_AGENTS_TEMPLATE)?;
        Ok(vec![AGENTS_FILE_NAME.to_string()])
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
        let agents_path = self.root.join(AGENTS_FILE_NAME);
        prompt::build_system_prompt(&agents_path, project_workspace_root, profiles)
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

    matches!(components.as_slice(), [name] if name == AGENTS_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{AgentWorkspace, ZH_AGENTS_TEMPLATE};
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
    fn installs_agents_template_when_missing() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        let copied = workspace.install_templates().expect("install");
        assert_eq!(copied, vec!["AGENTS.md".to_string()]);
        assert!(dir.path().join("workspace").join("AGENTS.md").is_file());
    }

    #[test]
    fn build_system_prompt_errors_when_agents_missing() {
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
            .write_workspace_file("AGENTS.md", "---\nname: a\n---\nA")
            .expect("write agents");

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
        assert!(prompt.contains("# AGENTS.md"));
        assert!(prompt.contains("User data"));
        assert!(prompt.contains("Soul data"));
    }

    #[test]
    fn system_prompt_references_project_workspace_profiles_and_memory_system() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file("AGENTS.md", "A")
            .expect("write agents");

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
        assert!(prompt.contains("profile_update"));
        assert!(prompt.contains("memory_system_search"));
        assert!(prompt.contains("A user"));
        assert!(prompt.contains("A soul"));
    }

    #[test]
    fn system_prompt_truncates_large_profile_content() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file("AGENTS.md", "A")
            .expect("write agents");

        let prompt = workspace
            .build_system_prompt(
                dir.path(),
                &[
                    profile(ProfileTarget::User, &"x".repeat(20_000)),
                    profile(ProfileTarget::Soul, "ok"),
                ],
            )
            .expect("prompt");
        assert!(prompt.contains("...[truncated]"));
    }

    #[test]
    fn install_templates_overwrites_existing_agents_template() {
        let dir = tempdir().expect("tempdir");
        let workspace = AgentWorkspace::new(dir.path());
        workspace.ensure_layout().expect("layout");
        workspace
            .write_workspace_file("AGENTS.md", "A")
            .expect("write agents");
        let content =
            fs::read_to_string(dir.path().join("workspace").join("AGENTS.md")).expect("read agents");
        assert_eq!(content, "A");

        let copied = workspace.install_templates().expect("install");
        assert_eq!(copied, vec!["AGENTS.md".to_string()]);
        let content =
            fs::read_to_string(dir.path().join("workspace").join("AGENTS.md")).expect("read agents");
        assert_eq!(content, ZH_AGENTS_TEMPLATE);
    }
}
