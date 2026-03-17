mod client;

pub(crate) use client::{
    normalize_openai_compatible_endpoint, resolve_provider_api_key, validate_provider_api_key_input,
};
