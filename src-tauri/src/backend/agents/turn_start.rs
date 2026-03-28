use crate::backend::errors::AppResult;
use crate::backend::models::domain::{
    new_chat_turn, new_user_chat_message, title_from_first_prompt,
};
use crate::backend::models::events::TurnStartedPayload;
use crate::backend::{AppError, BackendState};

pub(super) fn start_turn(
    state: BackendState,
    session_id: String,
    text: String,
) -> AppResult<String> {
    let provider_service = state.provider_service();
    let session_service = state.session_service();
    let session = state.storage.get_session(&session_id)?;
    let workspace_path = session
        .workspace_path
        .as_deref()
        .ok_or_else(|| AppError::Validation("session has no bound workspace".to_string()))?;
    if !std::path::Path::new(workspace_path).is_dir() {
        return Err(AppError::Validation(format!(
            "workspace path is not available: {workspace_path}"
        )));
    }
    let title = if session.title == "New chat" {
        Some(title_from_first_prompt(&text))
    } else {
        None
    };
    state
        .storage
        .touch_session_for_turn(&session_id, title.as_deref())?;
    state
        .storage
        .set_last_opened_session_id(Some(&session_id))?;

    let turn = new_chat_turn(session_id.clone(), text.clone());
    let user_message = new_user_chat_message(session_id.clone(), turn.id.clone(), text);
    state.storage.insert_turn(&turn)?;
    let provider = if let Some(provider_id) = session.provider_profile_id.as_deref() {
        provider_service.get_profile(provider_id)?
    } else {
        None
    };
    state
        .storage
        .insert_turn_usage_metric_start(&turn, provider.as_ref())?;
    state.storage.insert_message(&user_message)?;
    session_service.publish_changed()?;
    state.ws_hub.emit_turn_event(
        &turn.id,
        "chat.turn.started",
        TurnStartedPayload {
            session_id,
            turn: turn.clone(),
            user_message,
        },
    )?;
    state.register_turn(turn.id.clone());
    super::spawn_turn(state, turn.clone());
    Ok(turn.id)
}
