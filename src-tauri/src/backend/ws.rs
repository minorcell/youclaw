use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use chrono::{Local, Timelike};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::backend::agent;
use crate::backend::errors::{AppError, AppResult};
use crate::backend::models::{
    new_chat_session, AgentActiveHoursConfig, AgentConfigUpdateRequest,
    AgentHeartbeatExecutedPayload, BindSessionProviderRequest, BootstrapRequest, ChatCancelRequest,
    ChatSendRequest, ConnectionReadyPayload, CreateProviderModelRequest, CreateProviderRequest,
    CreateSessionRequest, DeleteProviderModelRequest, DeleteSessionRequest, HeartbeatPayload,
    MemoryGetRequest, MemorySearchRequest, TestProviderModelRequest, ToolApprovalResolveRequest,
    UpdateProviderModelRequest, UpdateProviderRequest, UsageLogDetailRequest, UsageLogsListRequest,
    UsageSettingsUpdateRequest, UsageStatsListRequest, UsageSummaryRequest,
    WorkspaceFileReadRequest, WorkspaceFileWriteRequest, WsEnvelope, WsKind,
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
        .with_state(Arc::new(state.clone()));
    spawn_heartbeat_loop(state.clone());
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
        "usage.settings.get" => {
            let payload = state.storage.usage_settings_payload()?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        "usage.settings.update" => {
            let req = serde_json::from_value::<UsageSettingsUpdateRequest>(envelope.payload)?;
            let payload = state
                .storage
                .set_usage_detail_logging_enabled(req.detail_logging_enabled)?;
            WsEnvelope::response_ok(envelope.id, envelope.name, payload)?
        }
        other => return Err(AppError::NotFound(format!("unknown request `{other}`"))),
    };
    Ok(response)
}

fn spawn_heartbeat_loop(state: BackendState) {
    tokio::spawn(async move {
        loop {
            let config = match state.get_agent_config() {
                Ok(config) => config,
                Err(_) => {
                    sleep(Duration::from_secs(30)).await;
                    continue;
                }
            };

            if !config.heartbeat.enabled {
                sleep(Duration::from_secs(30)).await;
                continue;
            }

            let interval = parse_heartbeat_every(&config.heartbeat.every);
            sleep(interval).await;

            let config = match state.get_agent_config() {
                Ok(config) => config,
                Err(err) => {
                    let _ = state.ws_hub.emit(
                        "agent.heartbeat.executed",
                        AgentHeartbeatExecutedPayload {
                            session_id: "main".to_string(),
                            status: "failed".to_string(),
                            run_id: None,
                            reason: Some(err.message()),
                        },
                    );
                    continue;
                }
            };
            if !config.heartbeat.enabled {
                continue;
            }
            if !within_active_hours(config.heartbeat.active_hours.as_ref()) {
                let _ = state.ws_hub.emit(
                    "agent.heartbeat.executed",
                    AgentHeartbeatExecutedPayload {
                        session_id: config.heartbeat.target.clone(),
                        status: "skipped".to_string(),
                        run_id: None,
                        reason: Some("outside_active_hours".to_string()),
                    },
                );
                continue;
            }

            let query = match state.workspace.read_heartbeat_query() {
                Ok(query) => query,
                Err(err) => {
                    let _ = state.ws_hub.emit(
                        "agent.heartbeat.executed",
                        AgentHeartbeatExecutedPayload {
                            session_id: config.heartbeat.target.clone(),
                            status: "failed".to_string(),
                            run_id: None,
                            reason: Some(err.message()),
                        },
                    );
                    continue;
                }
            };
            if query.trim().is_empty() {
                let _ = state.ws_hub.emit(
                    "agent.heartbeat.executed",
                    AgentHeartbeatExecutedPayload {
                        session_id: config.heartbeat.target.clone(),
                        status: "skipped".to_string(),
                        run_id: None,
                        reason: Some("empty_heartbeat_prompt".to_string()),
                    },
                );
                continue;
            }

            let target_session = match ensure_heartbeat_session(&state, &config.heartbeat.target) {
                Ok(session_id) => session_id,
                Err(err) => {
                    let _ = state.ws_hub.emit(
                        "agent.heartbeat.executed",
                        AgentHeartbeatExecutedPayload {
                            session_id: config.heartbeat.target.clone(),
                            status: "failed".to_string(),
                            run_id: None,
                            reason: Some(err.message()),
                        },
                    );
                    continue;
                }
            };

            match agent::start_run(state.clone(), target_session.clone(), query) {
                Ok(run_id) => {
                    let _ = state.ws_hub.emit(
                        "agent.heartbeat.executed",
                        AgentHeartbeatExecutedPayload {
                            session_id: target_session,
                            status: "executed".to_string(),
                            run_id: Some(run_id),
                            reason: None,
                        },
                    );
                }
                Err(err) => {
                    let _ = state.ws_hub.emit(
                        "agent.heartbeat.executed",
                        AgentHeartbeatExecutedPayload {
                            session_id: target_session,
                            status: "failed".to_string(),
                            run_id: None,
                            reason: Some(err.message()),
                        },
                    );
                }
            }
        }
    });
}

fn parse_heartbeat_every(raw: &str) -> Duration {
    let value = raw.trim().to_ascii_lowercase();
    if let Some(hours) = value.strip_suffix('h') {
        if let Ok(hours) = hours.parse::<u64>() {
            if hours > 0 {
                return Duration::from_secs(hours.saturating_mul(3600));
            }
        }
    }
    if let Some(minutes) = value.strip_suffix('m') {
        if let Ok(minutes) = minutes.parse::<u64>() {
            if minutes > 0 {
                return Duration::from_secs(minutes.saturating_mul(60));
            }
        }
    }
    Duration::from_secs(30 * 60)
}

fn parse_hhmm(raw: &str) -> Option<u32> {
    let mut parts = raw.trim().split(':');
    let hour = parts.next()?.parse::<u32>().ok()?;
    let minute = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 60 + minute)
}

fn within_active_hours(active: Option<&AgentActiveHoursConfig>) -> bool {
    let Some(active) = active else {
        return true;
    };
    let Some(start) = parse_hhmm(&active.start) else {
        return true;
    };
    let Some(end) = parse_hhmm(&active.end) else {
        return true;
    };
    let now = Local::now();
    let current = now.hour() * 60 + now.minute();
    if start <= end {
        current >= start && current <= end
    } else {
        current >= start || current <= end
    }
}

fn ensure_heartbeat_session(state: &BackendState, target: &str) -> AppResult<String> {
    if let Ok(session) = state.storage.get_session(target) {
        return Ok(session.id);
    }
    if target == "main" {
        if let Some(session) = state
            .storage
            .list_sessions()?
            .into_iter()
            .find(|item| item.title.to_ascii_lowercase() == "main")
        {
            return Ok(session.id);
        }
    }

    let mut session = new_chat_session(None);
    session.id = target.to_string();
    session.title = target.to_string();
    if let Some(profile) = state.storage.list_provider_profiles()?.into_iter().next() {
        session.provider_profile_id = Some(profile.id);
    }
    state.storage.insert_session(&session)?;
    state.publish_sessions_changed()?;
    Ok(session.id)
}
