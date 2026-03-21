use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SessionContextSummary {
    #[serde(default)]
    pub current_goal: String,
    #[serde(default)]
    pub progress: String,
    #[serde(default)]
    pub user_preferences: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub important_facts: Vec<String>,
    #[serde(default)]
    pub files_changed: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
    #[serde(default)]
    pub pending_actions: Vec<String>,
}

impl SessionContextSummary {
    pub fn is_empty(&self) -> bool {
        self.current_goal.trim().is_empty()
            && self.progress.trim().is_empty()
            && self.user_preferences.is_empty()
            && self.constraints.is_empty()
            && self.important_facts.is_empty()
            && self.files_changed.is_empty()
            && self.decisions.is_empty()
            && self.open_questions.is_empty()
            && self.pending_actions.is_empty()
    }

    pub fn normalize(self) -> Self {
        Self {
            current_goal: self.current_goal.trim().to_string(),
            progress: self.progress.trim().to_string(),
            user_preferences: normalize_lines(self.user_preferences),
            constraints: normalize_lines(self.constraints),
            important_facts: normalize_lines(self.important_facts),
            files_changed: normalize_lines(self.files_changed),
            decisions: normalize_lines(self.decisions),
            open_questions: normalize_lines(self.open_questions),
            pending_actions: normalize_lines(self.pending_actions),
        }
    }

    pub fn render_for_prompt(&self) -> String {
        let mut sections = Vec::<String>::new();

        push_text_section(&mut sections, "当前目标", &self.current_goal);
        push_text_section(&mut sections, "当前进展", &self.progress);
        push_list_section(&mut sections, "用户偏好", &self.user_preferences);
        push_list_section(&mut sections, "约束条件", &self.constraints);
        push_list_section(&mut sections, "重要事实", &self.important_facts);
        push_list_section(&mut sections, "已修改文件", &self.files_changed);
        push_list_section(&mut sections, "已做决策", &self.decisions);
        push_list_section(&mut sections, "待确认问题", &self.open_questions);
        push_list_section(&mut sections, "待执行事项", &self.pending_actions);

        sections.join("\n\n")
    }
}

fn normalize_lines(items: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::<String>::new();

    for item in items {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if normalized.iter().any(|existing| existing == trimmed) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }

    normalized
}

fn push_text_section(sections: &mut Vec<String>, title: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }

    sections.push(format!("## {title}\n{trimmed}"));
}

fn push_list_section(sections: &mut Vec<String>, title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }

    let body = items
        .iter()
        .map(|item| {
            if item.starts_with("- ") {
                item.to_string()
            } else {
                format!("- {item}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    sections.push(format!("## {title}\n{body}"));
}

#[cfg(test)]
mod tests {
    use super::SessionContextSummary;

    #[test]
    fn normalize_trims_and_deduplicates_list_items() {
        let summary = SessionContextSummary {
            user_preferences: vec![
                "  keep it short  ".to_string(),
                "".to_string(),
                "keep it short".to_string(),
            ],
            ..SessionContextSummary::default()
        }
        .normalize();

        assert_eq!(summary.user_preferences, vec!["keep it short".to_string()]);
    }

    #[test]
    fn render_for_prompt_omits_empty_sections() {
        let summary = SessionContextSummary {
            current_goal: "Ship the refactor.".to_string(),
            pending_actions: vec!["- Update tests".to_string()],
            ..SessionContextSummary::default()
        };

        let rendered = summary.render_for_prompt();
        assert!(rendered.contains("## 当前目标"));
        assert!(rendered.contains("Ship the refactor."));
        assert!(rendered.contains("## 待执行事项"));
        assert!(!rendered.contains("## 已查看文件"));
    }
}
