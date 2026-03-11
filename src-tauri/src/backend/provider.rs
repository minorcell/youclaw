pub(crate) fn normalize_openai_compatible_endpoint(base_url: &str) -> (String, Option<String>) {
    const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

    let trimmed = base_url.trim().trim_end_matches('/');
    if let Some(prefix) = trimmed.strip_suffix(CHAT_COMPLETIONS_PATH) {
        if !prefix.is_empty() {
            return (prefix.to_string(), Some(CHAT_COMPLETIONS_PATH.to_string()));
        }
    }
    (trimmed.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::normalize_openai_compatible_endpoint;

    #[test]
    fn normalize_endpoint_keeps_regular_base_url() {
        let (base, path) = normalize_openai_compatible_endpoint("https://api.deepseek.com");
        assert_eq!(base, "https://api.deepseek.com");
        assert!(path.is_none());
    }

    #[test]
    fn normalize_endpoint_splits_full_chat_completions_url() {
        let (base, path) = normalize_openai_compatible_endpoint(
            "https://open.bigmodel.cn/api/paas/v4/chat/completions",
        );
        assert_eq!(base, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(path.as_deref(), Some("/chat/completions"));
    }
}
