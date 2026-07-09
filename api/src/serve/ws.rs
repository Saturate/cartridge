use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::agent::{AgentId, AgentRegistry, BroadcastMessage, PtyCommand};

#[derive(Deserialize, Default)]
pub struct WsQuery {
    pub token: Option<String>,
}

pub async fn ws_upgrade(
    State(registry): State<AgentRegistry>,
    Path(id): Path<String>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, axum::http::StatusCode> {
    if let Some(expected) = registry.config().token.as_ref() {
        let provided = query.token.as_deref();
        if provided != Some(expected.as_str()) {
            return Err(axum::http::StatusCode::UNAUTHORIZED);
        }
    }

    let agent_id = AgentId(id);
    let agent_lock = registry
        .get(&agent_id)
        .await
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    Ok(ws.on_upgrade(move |socket| handle_ws(socket, agent_lock, registry)))
}

async fn handle_ws(
    socket: WebSocket,
    agent: std::sync::Arc<tokio::sync::RwLock<crate::agent::AgentState>>,
    _registry: AgentRegistry,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Replay ring buffer
    {
        let state = agent.read().await;
        let replay = state.ring_buffer.read_all();
        if !replay.data.is_empty() {
            let frame = serde_json::json!({
                "type": "terminal",
                "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &replay.data),
                "replay": true,
            });
            if ws_tx
                .send(Message::Text(frame.to_string().into()))
                .await
                .is_err()
            {
                return;
            }
        }

        // Send current status
        let status_frame = serde_json::json!({
            "type": "status",
            "status": state.status,
            "exit_code": state.exit_code,
            "duration_ms": state.duration_ms(),
        });
        ws_tx
            .send(Message::Text(status_frame.to_string().into()))
            .await
            .ok();
    }

    // Subscribe to broadcast
    let mut broadcast_rx = {
        let state = agent.read().await;
        state.broadcast_tx.subscribe()
    };

    // Read from client (input, resize)
    let agent_for_read = agent.clone();
    let read_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            if let Message::Text(text) = msg {
                if let Ok(frame) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = frame.get("type").and_then(|t| t.as_str());
                    match msg_type {
                        Some("input") => {
                            if let Some(data) = frame.get("data").and_then(|d| d.as_str()) {
                                let state = agent_for_read.read().await;
                                if let Some(tx) = &state.pty_cmd_tx {
                                    tx.send(PtyCommand::Input(data.as_bytes().to_vec()))
                                        .await
                                        .ok();
                                }
                            }
                        }
                        Some("resize") => {
                            let cols = frame.get("cols").and_then(|c| c.as_u64()).unwrap_or(120) as u16;
                            let rows = frame.get("rows").and_then(|r| r.as_u64()).unwrap_or(40) as u16;
                            let state = agent_for_read.read().await;
                            if let Some(tx) = &state.pty_cmd_tx {
                                tx.send(PtyCommand::Resize { cols, rows }).await.ok();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    // Write to client (broadcast messages)
    let write_task = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(msg) => {
                    let frame = match msg {
                        BroadcastMessage::Terminal(data) => {
                            serde_json::json!({
                                "type": "terminal",
                                "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data),
                            })
                        }
                        BroadcastMessage::Event(payload) => {
                            serde_json::json!({
                                "type": "event",
                                "payload": payload,
                            })
                        }
                        BroadcastMessage::Status {
                            status,
                            exit_code,
                            duration_ms,
                        } => {
                            serde_json::json!({
                                "type": "status",
                                "status": status,
                                "exit_code": exit_code,
                                "duration_ms": duration_ms,
                            })
                        }
                    };

                    if ws_tx
                        .send(Message::Text(frame.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(lagged = n, "WebSocket client lagged behind");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    tokio::select! {
        _ = read_task => {},
        _ = write_task => {},
    }
}
