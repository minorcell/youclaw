use aquaregia::ErrorCode;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("validation error: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("agent error: {0}")]
    Agent(String),
    #[error("cancelled: {0}")]
    Cancelled(String),
    #[error("websocket error: {0}")]
    Ws(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "validation_error",
            Self::NotFound(_) => "not_found",
            Self::Storage(_) => "storage_error",
            Self::Io(_) => "io_error",
            Self::Provider(_) => "provider_error",
            Self::Agent(_) => "agent_error",
            Self::Cancelled(_) => "cancelled",
            Self::Ws(_) => "ws_error",
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<aquaregia::Error> for AppError {
    fn from(value: aquaregia::Error) -> Self {
        match value.code {
            ErrorCode::Cancelled => Self::Cancelled(value.message),
            ErrorCode::AuthFailed | ErrorCode::RateLimited | ErrorCode::InvalidRequest => {
                Self::Provider(value.message)
            }
            _ => Self::Agent(value.message),
        }
    }
}
