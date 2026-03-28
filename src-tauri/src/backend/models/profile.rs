use serde::{Deserialize, Serialize};

use crate::backend::errors::AppError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProfileTarget {
    User,
    Soul,
}

impl ProfileTarget {
    pub const ALL: [Self; 2] = [Self::User, Self::Soul];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Soul => "soul",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::User => "User Profile",
            Self::Soul => "Agent Soul",
        }
    }
}

impl std::str::FromStr for ProfileTarget {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "user" => Ok(Self::User),
            "soul" => Ok(Self::Soul),
            _ => Err(AppError::Validation(format!(
                "invalid profile target `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub target: ProfileTarget,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}
