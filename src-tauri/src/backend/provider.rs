use crate::backend::errors::{AppError, AppResult};

const API_KEY_ENV_PREFIX: &str = "env:";

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

pub(crate) fn validate_provider_api_key_input(api_key: &str) -> AppResult<()> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("api key cannot be empty".to_string()));
    }
    if let Some(env_name) = parse_env_api_key_name(trimmed) {
        if env_name.is_empty() {
            return Err(AppError::Validation(
                "environment variable name cannot be empty".to_string(),
            ));
        }
        if !is_valid_env_var_name(&env_name) {
            return Err(AppError::Validation(format!(
                "invalid environment variable name `{env_name}`"
            )));
        }
    }
    Ok(())
}

pub(crate) fn resolve_provider_api_key(api_key: &str) -> AppResult<String> {
    validate_provider_api_key_input(api_key)?;
    if let Some(env_name) = parse_env_api_key_name(api_key.trim()) {
        let value = std::env::var(&env_name).map_err(|_| {
            AppError::Validation(format!("environment variable `{env_name}` is not set"))
        })?;
        if value.trim().is_empty() {
            return Err(AppError::Validation(format!(
                "environment variable `{env_name}` is empty"
            )));
        }
        return Ok(value);
    }
    Ok(api_key.trim().to_string())
}

fn parse_env_api_key_name(api_key: &str) -> Option<String> {
    let (prefix, rest) = api_key.split_at(API_KEY_ENV_PREFIX.len().min(api_key.len()));
    if !prefix.eq_ignore_ascii_case(API_KEY_ENV_PREFIX) {
        return None;
    }
    let env_name = rest.trim().trim_end_matches([';', '；']).trim();
    Some(env_name.to_string())
}

fn is_valid_env_var_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_openai_compatible_endpoint, resolve_provider_api_key,
        validate_provider_api_key_input,
    };

    #[test]
    fn normalize_endpoint_keeps_regular_base_url() {
        let (base, path) = normalize_openai_compatible_endpoint("https://api.minimaxi.com/v1");
        assert_eq!(base, "https://api.minimaxi.com/v1");
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

    #[test]
    fn api_key_validation_accepts_env_reference() {
        validate_provider_api_key_input("env:MINIMAX_API_KEY").expect("valid env reference");
        validate_provider_api_key_input("env:MINIMAX_API_KEY;").expect("valid env reference");
    }

    #[test]
    fn api_key_validation_rejects_empty_env_name() {
        let err = validate_provider_api_key_input("env: ").expect_err("should fail");
        assert!(err.message().contains("environment variable name"));
    }

    #[test]
    fn resolve_api_key_reads_env_reference() {
        let expected = std::env::var("PATH").expect("path");
        let resolved = resolve_provider_api_key("env:PATH").expect("resolve");
        assert_eq!(resolved, expected);
    }
}
