use std::fs;
use std::path::Path;

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::domain::{AgentProfile, ProfileTarget};

const DEFAULT_SYSTEM_PROMPT_FILE_NAME: &str = "SYSTEM_PROMPT.md";
const SYSTEM_PROMPT_MAX_CHARS: usize = 40_000;
const PROFILE_MAX_CHARS: usize = 12_000;
const TRUNCATION_MARKER: &str = "\n...[已截断]";

pub(crate) fn build_system_prompt(
    system_prompt_path: &Path,
    project_workspace_root: &Path,
    profiles: &[AgentProfile],
) -> AppResult<String> {
    let system_prompt_file_name = system_prompt_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(DEFAULT_SYSTEM_PROMPT_FILE_NAME);

    if !system_prompt_path.exists() {
        return Err(AppError::Validation(format!(
            "required prompt file `{system_prompt_file_name}` is missing"
        )));
    }

    let system_prompt_content = fs::read_to_string(system_prompt_path).map_err(|err| {
        AppError::Io(format!(
            "failed to read prompt file `{system_prompt_file_name}`: {err}"
        ))
    })?;
    let (system_prompt_content, system_prompt_truncated) = truncate_with_marker(
        &strip_frontmatter(system_prompt_content.trim()),
        SYSTEM_PROMPT_MAX_CHARS,
    );
    let (user_profile, user_truncated) = truncate_with_marker(
        &profile_content(profiles, ProfileTarget::User),
        PROFILE_MAX_CHARS,
    );
    let (soul_profile, soul_truncated) = truncate_with_marker(
        &profile_content(profiles, ProfileTarget::Soul),
        PROFILE_MAX_CHARS,
    );

    let mut prompt = vec![
        system_prompt_content,
        String::new(),
        "## 你的一些记忆：".to_string(),
        String::new(),
        format!("### {}", ProfileTarget::User.label()),
        user_profile,
        String::new(),
        format!("### {}", ProfileTarget::Soul.label()),
        soul_profile,
        String::new(),
        "## 活动区域".to_string(),
        format!(
            "你现在活动的目录是：{}",
            project_workspace_root.to_string_lossy()
        ),
    ];

    let mut truncated_sections = Vec::new();
    if system_prompt_truncated {
        truncated_sections.push(system_prompt_file_name.to_string());
    }
    if user_truncated {
        truncated_sections.push("用户画像(user_profile)".to_string());
    }
    if soul_truncated {
        truncated_sections.push("灵魂画像(soul_profile)".to_string());
    }
    if !truncated_sections.is_empty() {
        prompt.push(String::new());
        prompt.push(format!(
            "警告：以下内容已被截断：{}",
            truncated_sections.join(", ")
        ));
    }

    Ok(prompt.join("\n"))
}

fn profile_content(profiles: &[AgentProfile], target: ProfileTarget) -> String {
    profiles
        .iter()
        .find(|profile| profile.target == target)
        .map(|profile| profile.content.trim())
        .filter(|content| !content.is_empty())
        .unwrap_or("(尚未配置)")
        .to_string()
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
    let marker_chars = TRUNCATION_MARKER.chars().count();
    if max_chars <= marker_chars {
        return (
            TRUNCATION_MARKER
                .chars()
                .take(max_chars)
                .collect::<String>(),
            true,
        );
    }
    let content_budget = max_chars - marker_chars;
    let mut trimmed = content.chars().take(content_budget).collect::<String>();
    trimmed.push_str(TRUNCATION_MARKER);
    (trimmed, true)
}
