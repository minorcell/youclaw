use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::backend::agent;
use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::{
    BindSessionProviderRequest, BootstrapRequest, ChatCancelRequest, ChatSendRequest,
    ConnectionReadyPayload, CreateProviderModelRequest, CreateProviderRequest,
    CreateSessionRequest, DeleteProviderModelRequest, DeleteSessionRequest, HeartbeatPayload,
    TestProviderModelRequest, ToolApprovalResolveRequest, UpdateProviderModelRequest,
    UpdateProviderRequest, WsEnvelope, WsKind,
};
use crate::backend::{now_timestamp, BackendState};

pub async fn start_ws_server(state: BackendState) -> AppResult<String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| AppError::Ws(err.to_string()))?;
    let address = listener
        .local_addr()
        .map_err(|err| AppError::Ws(err.to_string()))?;
    let endpoint = format!("ws://127.0.0.1:{}/ws", address.port());
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(Arc::new(state));
    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            eprintln!("ws server stopped: {err}");
        }
    });
    Ok(endpoint)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<BackendState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<BackendState>) {
    let (mut sender, mut receiver) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let mut subscription = state.ws_hub.subscribe();

    let writer = tokio::spawn(async move {
        while let Some(message) = out_rx.recv().await {
            if sender.send(Message::Text(message.into())).await.is_err() {
                break;
            }
        }
    });

    if let Ok(ready) = WsEnvelope::event(
        "connection.ready",
        ConnectionReadyPayload {
            server_time: now_timestamp(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
    ) {
        let _ = out_tx.send(serde_json::to_string(&ready).unwrap_or_default());
    }

    let forwarder_tx = out_tx.clone();
    let forwarder = tokio::spawn(async move {
        while let Ok(event) = subscription.recv().await {
            if let Ok(text) = serde_json::to_string(&event) {
                if forwarder_tx.send(text).is_err() {
                    break;
                }
            }
        }
    });

    while let Some(Ok(message)) = receiver.next().await {
        let Message::Text(text) = message else {
            continue;
        };
        let (response, req_id, req_name) = match serde_json::from_str::<WsEnvelope>(&text) {
            Ok(envelope) => {
                let id = envelope.id.clone();
                let name = envelope.name.clone();
                let result = dispatch_request(state.clone(), envelope).await;
                (result, id, name)
            }
            Err(err) => (
                Err(AppError::Validation(format!("invalid ws payload: {err}"))),
                "unknown".to_string(),
                "unknown".to_string(),
            ),
        };
        let serialized = match response {
            Ok(envelope) => serde_json::to_string(&envelope).unwrap_or_default(),
            Err(err) => {
                serde_json::to_string(&WsEnvelope::response_error(&req_id, &req_name, &err))
                    .unwrap_or_default()
            }
        };
        if out_tx.send(serialized).is_err() {
            break;
        }
    }

    forwarder.abort();
    writer.abort();
}

async fn dispatch_request(state: Arc<BackendState>, envelope: WsEnvelope) -> AppResult<WsEnvelope> {
    if !matches!(envelope.kind, WsKind::Request) {
        return Err(AppError::Validation(
            "only request envelopes are accepted from clients".to_string(),
        ));
    }

    let response = match envelope.name.as_str() {
        "bootstrap.get" => {
            let req = serde_json::from_value::<BootstrapRequest>(envelope.payload.clone())
                .unwrap_or_default();
            if req.heartbeat {
                WsEnvelope::response_ok(
                    envelope.id,
                    envelope.name,
                    HeartbeatPayload {
                        server_time: now_timestamp(),
                    },
                )?
            } else {
                WsEnvelope::response_ok(envelope.id, envelope.name, state.bootstrap()?)?
            }
        }
        "providers.list" => {
            let payload = state.list_provider_snapshot()?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "providers.create" => {
            let req = serde_json::from_value::<CreateProviderRequest>(envelope.payload)?;
            let created = state.create_provider(req).await?;
            WsEnvelope::response_ok(envelope.id, envelope.name, created)?
        }
        "providers.update" => {
            let req = serde_json::from_value::<UpdateProviderRequest>(envelope.payload)?;
            let updated = state.update_provider(req).await?;
            WsEnvelope::response_ok(envelope.id, envelope.name, updated)?
        }
        "providers.models.create" => {
            let req = serde_json::from_value::<CreateProviderModelRequest>(envelope.payload)?;
            let created = state.create_provider_model(req).await?;
            WsEnvelope::response_ok(envelope.id, envelope.name, created)?
        }
        "providers.models.update" => {
            let req = serde_json::from_value::<UpdateProviderModelRequest>(envelope.payload)?;
            let updated = state.update_provider_model(req).await?;
            WsEnvelope::response_ok(envelope.id, envelope.name, updated)?
        }
        "providers.models.delete" => {
            let req = serde_json::from_value::<DeleteProviderModelRequest>(envelope.payload)?;
            state.delete_provider_model(&req.id)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "deleted": true }),
            )?
        }
        "providers.models.test" => {
            let req = serde_json::from_value::<TestProviderModelRequest>(envelope.payload)?;
            state.test_provider_model(req).await?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "ok": true }),
            )?
        }
        "sessions.list" => WsEnvelope::response_ok(
            envelope.id,
            envelope.name,
            state.storage.sessions_payload()?,
        )?,
        "sessions.create" => {
            let req = serde_json::from_value::<CreateSessionRequest>(envelope.payload)?;
            let session = state.create_session(req.provider_profile_id)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, session)?
        }
        "sessions.delete" => {
            let req = serde_json::from_value::<DeleteSessionRequest>(envelope.payload)?;
            state.delete_session(&req.session_id)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "deleted": true }),
            )?
        }
        "sessions.bind_provider" => {
            let req = serde_json::from_value::<BindSessionProviderRequest>(envelope.payload)?;
            state.bind_session_provider(&req.session_id, &req.provider_profile_id)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "bound": true }),
            )?
        }
        "chat.send" => {
            let req = serde_json::from_value::<ChatSendRequest>(envelope.payload)?;
            if req.text.trim().is_empty() {
                return Err(AppError::Validation(
                    "message text cannot be empty".to_string(),
                ));
            }
            let run_id = agent::start_run((*state).clone(), req.session_id, req.text)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "run_id": run_id }),
            )?
        }
        "chat.cancel" => {
            let req = serde_json::from_value::<ChatCancelRequest>(envelope.payload)?;
            let cancelled = state.cancel_run(&req.run_id)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "cancelled": cancelled }),
            )?
        }
        "tool_approvals.resolve" => {
            let req = serde_json::from_value::<ToolApprovalResolveRequest>(envelope.payload)?;
            let approval = state.approvals.resolve(&req.approval_id, req.approved)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, approval)?
        }
        other => return Err(AppError::NotFound(format!("unknown request `{other}`"))),
    };
    Ok(response)
}
