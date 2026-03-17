mod bootstrap;
mod chat;
mod providers;
mod runtime;
mod sessions;
mod usage;

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::domain::now_timestamp;
use crate::backend::models::events::ConnectionReadyPayload;
use crate::backend::models::{WsEnvelope, WsKind};
use crate::backend::BackendState;

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
        .with_state(Arc::new(state.clone()));
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

    if let Some(response) = bootstrap::try_handle(state.as_ref(), &envelope)? {
        return Ok(response);
    }
    if let Some(response) = providers::try_handle(state.as_ref(), &envelope).await? {
        return Ok(response);
    }
    if let Some(response) = sessions::try_handle(state.as_ref(), &envelope)? {
        return Ok(response);
    }
    if let Some(response) = chat::try_handle(state.as_ref(), &envelope).await? {
        return Ok(response);
    }
    if let Some(response) = runtime::try_handle(state.as_ref(), &envelope)? {
        return Ok(response);
    }
    if let Some(response) = usage::try_handle(state.as_ref(), &envelope)? {
        return Ok(response);
    }
    Err(AppError::NotFound(format!(
        "unknown request `{}`",
        envelope.name
    )))
}
