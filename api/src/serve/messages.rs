use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::agent::{
    AgentId, AgentRegistry, AgentStatus, PtyCommand,
    messages::{AgentMessage, MessageType, format_for_pty, generate_message_id},
};

#[derive(Deserialize)]
pub struct SendMessageRequest {
    pub from: String,
    pub content: String,
    #[serde(default)]
    pub msg_type: MessageType,
}

#[derive(Serialize)]
pub struct SendMessageResponse {
    pub id: String,
    pub delivered: bool,
}

#[derive(Serialize)]
pub struct MessageListResponse {
    pub messages: Vec<AgentMessage>,
    pub total: usize,
}

type ApiError = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, code: &str, msg: &str) -> ApiError {
    (
        status,
        Json(serde_json::json!({ "error": { "code": code, "message": msg, "status": status.as_u16() } })),
    )
}

fn now_ts() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    format!("{secs}.{millis:03}")
}

pub async fn send_message(
    State(registry): State<AgentRegistry>,
    Path(id): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> Result<(StatusCode, Json<SendMessageResponse>), ApiError> {
    if req.content.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "invalid_request", "content is required"));
    }

    let agent_id = AgentId(id.clone());
    let agent_lock = registry
        .get(&agent_id)
        .await
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "agent_not_found", &format!("Agent {id} does not exist")))?;

    let msg_id = generate_message_id();
    let message = AgentMessage {
        id: msg_id.clone(),
        from: req.from.clone(),
        to: id.clone(),
        content: req.content,
        msg_type: req.msg_type,
        timestamp: now_ts(),
        delivered: false,
    };

    let pty_data = format_for_pty(&message);

    let mut state = agent_lock.write().await;

    let delivered = if matches!(state.status, AgentStatus::Starting | AgentStatus::Running) {
        if let Some(tx) = &state.pty_cmd_tx {
            tx.send(PtyCommand::Input(pty_data)).await.ok();
            true
        } else {
            false
        }
    } else {
        false
    };

    let mut stored = message;
    stored.delivered = delivered;
    state.messages.push(stored);

    if delivered {
        state.messages.mark_delivered(&msg_id);
    }

    Ok((
        StatusCode::CREATED,
        Json(SendMessageResponse {
            id: msg_id,
            delivered,
        }),
    ))
}

pub async fn list_messages(
    State(registry): State<AgentRegistry>,
    Path(id): Path<String>,
) -> Result<Json<MessageListResponse>, ApiError> {
    let agent_id = AgentId(id.clone());
    let agent_lock = registry
        .get(&agent_id)
        .await
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "agent_not_found", &format!("Agent {id} does not exist")))?;

    let state = agent_lock.read().await;
    let messages: Vec<AgentMessage> = state.messages.list().into_iter().cloned().collect();
    let total = messages.len();

    Ok(Json(MessageListResponse { messages, total }))
}
