use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::backend::errors::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsKind {
    Request,
    Response,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEnvelope {
    pub id: String,
    pub kind: WsKind,
    pub name: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WsErrorPayload>,
}

impl WsEnvelope {
    pub fn event(name: impl Into<String>, payload: impl Serialize) -> AppResult<Self> {
        let payload = serde_json::to_value(payload)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            kind: WsKind::Event,
            name: name.into(),
            payload,
            turn_id: None,
            ok: None,
            error: None,
        })
    }

    pub fn event_for_turn(
        turn_id: impl Into<String>,
        name: impl Into<String>,
        payload: impl Serialize,
    ) -> AppResult<Self> {
        let payload = serde_json::to_value(payload)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            kind: WsKind::Event,
            name: name.into(),
            payload,
            turn_id: Some(turn_id.into()),
            ok: None,
            error: None,
        })
    }

    pub fn response_ok(
        id: impl Into<String>,
        name: impl Into<String>,
        payload: impl Serialize,
    ) -> AppResult<Self> {
        let payload = serde_json::to_value(payload)?;
        Ok(Self {
            id: id.into(),
            kind: WsKind::Response,
            name: name.into(),
            payload,
            turn_id: None,
            ok: Some(true),
            error: None,
        })
    }

    pub fn response_error(id: impl Into<String>, name: impl Into<String>, err: &AppError) -> Self {
        Self {
            id: id.into(),
            kind: WsKind::Response,
            name: name.into(),
            payload: Value::Null,
            turn_id: None,
            ok: Some(false),
            error: Some(WsErrorPayload {
                code: err.code().to_string(),
                message: err.message(),
            }),
        }
    }
}
