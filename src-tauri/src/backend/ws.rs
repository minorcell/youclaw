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
    now_timestamp, AgentConfigUpdateRequest, BindSessionProviderRequest, ChatTurnCancelRequest,
    ChatTurnStartRequest, ConnectionReadyPayload, CreateProviderModelRequest,
    CreateProviderRequest, CreateSessionRequest, DeleteProviderModelRequest, DeleteSessionRequest,
    MemoryGetRequest, MemorySearchRequest, PurgeSessionRequest, RenameSessionRequest,
    RestoreSessionRequest, TestProviderModelRequest, ToolApprovalResolveRequest,
    TurnStepsListPayload, TurnStepsListRequest, UpdateProviderModelRequest,
    UpdateProviderRequest, UpdateSessionApprovalModeRequest, UsageLogDetailRequest,
    UsageLogsListRequest, UsageStatsListRequest, UsageSummaryRequest, WorkspaceFileReadRequest,
    WorkspaceFileWriteRequest, WsEnvelope, WsKind,
};
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

    let response = match envelope.name.as_str() {
        "bootstrap.get" => WsEnvelope::response_ok(envelope.id, envelope.name, state.bootstrap()?)?,
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
        "sessions.archived.list" => {
            let payload = state.list_archived_sessions()?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
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
                serde_json::json!({ "archived": true }),
            )?
        }
        "sessions.restore" => {
            let req = serde_json::from_value::<RestoreSessionRequest>(envelope.payload)?;
            state.restore_session(&req.session_id)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "restored": true }),
            )?
        }
        "sessions.purge" => {
            let req = serde_json::from_value::<PurgeSessionRequest>(envelope.payload)?;
            state.purge_session(&req.session_id)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "purged": true }),
            )?
        }
        "sessions.rename" => {
            let req = serde_json::from_value::<RenameSessionRequest>(envelope.payload)?;
            state.rename_session(&req.session_id, &req.title)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "renamed": true }),
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
        "sessions.update_approval_mode" => {
            let req = serde_json::from_value::<UpdateSessionApprovalModeRequest>(envelope.payload)?;
            state.update_session_approval_mode(&req.session_id, req.approval_mode)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "updated": true }),
            )?
        }
        "chat.turn.start" => {
            let req = serde_json::from_value::<ChatTurnStartRequest>(envelope.payload)?;
            if req.text.trim().is_empty() {
                return Err(AppError::Validation(
                    "message text cannot be empty".to_string(),
                ));
            }
            let turn_id = agent::start_turn((*state).clone(), req.session_id, req.text)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "turn_id": turn_id }),
            )?
        }
        "chat.turn.cancel" => {
            let req = serde_json::from_value::<ChatTurnCancelRequest>(envelope.payload)?;
            let cancelled = state.cancel_turn(&req.turn_id)?;
            WsEnvelope::response_ok(
                envelope.id,
                envelope.name,
                serde_json::json!({ "cancelled": cancelled }),
            )?
        }
        "chat.turn.steps.list" => {
            let req = serde_json::from_value::<TurnStepsListRequest>(envelope.payload)?;
            let payload = TurnStepsListPayload {
                turn_id: req.turn_id.clone(),
                steps: state.storage.list_turn_steps(&req.turn_id)?,
            };
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "agent.config.get" => {
            let payload = state.get_agent_config()?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "agent.config.update" => {
            let req = serde_json::from_value::<AgentConfigUpdateRequest>(envelope.payload)?;
            let payload = state.update_agent_config(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "agent.workspace.files.list" => {
            let payload = state.list_workspace_files()?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "agent.workspace.files.read" => {
            let req = serde_json::from_value::<WorkspaceFileReadRequest>(envelope.payload)?;
            let payload = state.read_workspace_file(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "agent.workspace.files.write" => {
            let req = serde_json::from_value::<WorkspaceFileWriteRequest>(envelope.payload)?;
            let payload = state.write_workspace_file(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "agent.memory.search" => {
            let req = serde_json::from_value::<MemorySearchRequest>(envelope.payload)?;
            let payload = state.memory_search(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "agent.memory.get" => {
            let req = serde_json::from_value::<MemoryGetRequest>(envelope.payload)?;
            let payload = state.memory_get(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "agent.memory.reindex" => {
            let payload = state.reindex_memory()?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "tool_approvals.resolve" => {
            let req = serde_json::from_value::<ToolApprovalResolveRequest>(envelope.payload)?;
            let approval = state.approvals.resolve(&req.approval_id, req.approved)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, approval)?
        }
        "usage.summary.get" => {
            let req = serde_json::from_value::<UsageSummaryRequest>(envelope.payload)?;
            let payload = state.storage.usage_summary(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "usage.logs.list" => {
            let req = serde_json::from_value::<UsageLogsListRequest>(envelope.payload)?;
            let payload = state.storage.list_usage_logs(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "usage.logs.detail" => {
            let req = serde_json::from_value::<UsageLogDetailRequest>(envelope.payload)?;
            let payload = state.storage.usage_log_detail(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "usage.stats.providers.list" => {
            let req = serde_json::from_value::<UsageStatsListRequest>(envelope.payload)?;
            let payload = state.storage.list_usage_provider_stats(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "usage.stats.models.list" => {
            let req = serde_json::from_value::<UsageStatsListRequest>(envelope.payload)?;
            let payload = state.storage.list_usage_model_stats(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "usage.stats.tools.list" => {
            let req = serde_json::from_value::<UsageStatsListRequest>(envelope.payload)?;
            let payload = state.storage.list_usage_tool_stats(req)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        other => return Err(AppError::NotFound(format!("unknown request `{other}`"))),
    };
    Ok(response)
}
