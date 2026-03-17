//! 工具结果统一后处理器（`result-process`）。
//!
//! 该模块只负责**系统级输出预算治理**，不承担具体工具语义上的过滤逻辑。
//! 路径忽略、目录跳过等策略应在工具执行期处理（例如 `list_directory/search_files`）。
//!
//! 当前职责：
//! - 对工具输出施加全局字符预算；
//! - 超预算时返回截断预览，并附带 XML `system-hint` 供模型感知。

use serde_json::{json, Value};

/// 工具结果全局字符预算上限。
const GLOBAL_TOOL_RESULT_MAX_CHARS: usize = 24_000;
/// 超预算时返回给模型的预览字符上限。
const MAX_TRUNCATED_PREVIEW_CHARS: usize = 8_000;

/// 统一工具结果后处理器。
#[derive(Clone, Default)]
pub struct ToolResultProcessor;

impl ToolResultProcessor {
    /// 创建处理器实例。
    pub fn new() -> Self {
        Self
    }

    /// 对工具输出执行统一后处理。
    ///
    /// 若输出未超预算，原样返回；超预算则返回预算拦截结果。
    pub fn process(&self, tool_name: &str, output_json: Value) -> Value {
        self.enforce_size_budget(tool_name, output_json)
    }

    /// 执行全局输出预算拦截。
    fn enforce_size_budget(&self, tool_name: &str, output: Value) -> Value {
        let serialized = serde_json::to_string(&output).unwrap_or_default();
        let original_chars = serialized.chars().count();

        if original_chars <= GLOBAL_TOOL_RESULT_MAX_CHARS {
            return output;
        }

        let preview = truncate_chars(&serialized, MAX_TRUNCATED_PREVIEW_CHARS);
        let hint_xml = build_xml_system_hint(
            "tool-output-truncated",
            tool_name,
            &format!(
                "Tool output exceeded global budget ({} chars > {} chars). Output was replaced with a preview.",
                original_chars, GLOBAL_TOOL_RESULT_MAX_CHARS
            ),
        );

        json!({
            "warning": "tool_output_truncated",
            "tool_name": tool_name,
            "original_chars": original_chars,
            "max_chars": GLOBAL_TOOL_RESULT_MAX_CHARS,
            "preview": preview,
            "system_hint_xml": hint_xml,
        })
    }
}

/// 生成 XML 形式的 `system-hint`。
fn build_xml_system_hint(kind: &str, tool_name: &str, message: &str) -> String {
    format!(
        "<system-hint type=\"{kind}\" tool=\"{tool_name}\"><message>{}</message></system-hint>",
        escape_xml(message)
    )
}

/// 转义 XML 文本节点所需字符。
fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// 按字符数截断文本，避免 UTF-8 字节边界被破坏。
fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }

    let mut output = value.chars().take(limit).collect::<String>();
    output.push_str("\n...[truncated]");
    output
}

#[cfg(test)]
mod tests {
    use super::ToolResultProcessor;
    use serde_json::json;

    #[test]
    fn keeps_non_oversized_output_unchanged() {
        let processor = ToolResultProcessor::new();
        let output = json!({
            "action": "list_directory",
            "entries": [
                {"name": "src"},
                {"name": "Cargo.toml"}
            ]
        });

        let processed = processor.process("list_directory", output.clone());
        assert_eq!(processed, output);
    }

    #[test]
    fn wraps_large_output_with_xml_hint() {
        let processor = ToolResultProcessor::new();

        let huge = "x".repeat(30_000);
        let output = json!({
            "action": "read_text_file",
            "content": huge,
        });

        let processed = processor.process("read_text_file", output);
        assert_eq!(
            processed.get("warning").and_then(|value| value.as_str()),
            Some("tool_output_truncated")
        );
        assert!(processed
            .get("system_hint_xml")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .contains("<system-hint"));
    }
}
