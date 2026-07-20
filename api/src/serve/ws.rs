use axum::body::Bytes;
use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{Duration, Instant};

use crate::agent::{AgentId, AgentRegistry, AgentStatus, BroadcastMessage, PtyCommand};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(60);

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
    agent: Arc<tokio::sync::RwLock<crate::agent::AgentState>>,
    _registry: AgentRegistry,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Snapshot replay data and subscribe under the read lock, then release
    // it before doing any awaited WebSocket sends. This avoids holding the
    // lock across slow client I/O while still preventing event loss between
    // the replay snapshot and the live stream.
    let (terminal_data, event_frames, status_frame, mut broadcast_rx) = {
        let state = agent.read().await;

        let terminal_data = {
            let replay = state.ring_buffer.read_all();
            if replay.data.is_empty() {
                None
            } else {
                Some(serde_json::json!({
                    "type": "terminal",
                    "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &replay.data),
                    "replay": true,
                }))
            }
        };

        let event_frames: Vec<serde_json::Value> = state
            .events
            .iter()
            .map(|event| {
                serde_json::json!({
                    "type": "event",
                    "payload": {
                        "event": event.event,
                        "agent_id": event.agent_id,
                        "payload": event.payload,
                    },
                    "replay": true,
                })
            })
            .collect();

        let status_frame = serde_json::json!({
            "type": "status",
            "status": state.status,
            "exit_code": state.exit_code,
            "duration_ms": state.duration_ms(),
            "replay": true,
        });

        let rx = state.broadcast_tx.subscribe();

        (terminal_data, event_frames, status_frame, rx)
    };

    // Send replay frames without holding the agent lock
    if let Some(frame) = terminal_data {
        if ws_tx
            .send(Message::Text(frame.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }

    for frame in &event_frames {
        if ws_tx
            .send(Message::Text(frame.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }

    if ws_tx
        .send(Message::Text(status_frame.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    let done_frame = serde_json::json!({ "type": "replay_done" });
    if ws_tx
        .send(Message::Text(done_frame.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    // If the agent already exited, send a close frame and return immediately
    // rather than entering the live loop where no further status broadcast
    // will ever arrive.
    {
        let state = agent.read().await;
        if matches!(
            state.status,
            AgentStatus::Completed
                | AgentStatus::Failed
                | AgentStatus::Stopped
                | AgentStatus::Timeout
        ) {
            let reason = match state.status {
                AgentStatus::Completed => "agent completed",
                AgentStatus::Failed => "agent failed",
                AgentStatus::Stopped => "agent stopped",
                AgentStatus::Timeout => "agent timed out",
                _ => "agent exited",
            };
            ws_tx
                .send(Message::Close(Some(CloseFrame {
                    code: 1000,
                    reason: reason.into(),
                })))
                .await
                .ok();
            return;
        }
    }

    let last_pong = Arc::new(Mutex::new(Instant::now()));

    let agent_for_read = agent.clone();
    let pong_for_read = last_pong.clone();
    let mut read_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => {
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
                                let cols = frame.get("cols").and_then(|c| c.as_u64()).unwrap_or(120)
                                    as u16;
                                let rows =
                                    frame.get("rows").and_then(|r| r.as_u64()).unwrap_or(40) as u16;
                                let state = agent_for_read.read().await;
                                if let Some(tx) = &state.pty_cmd_tx {
                                    tx.send(PtyCommand::Resize { cols, rows }).await.ok();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Message::Pong(_) => {
                    *pong_for_read.lock().await = Instant::now();
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    let pong_for_write = last_pong;
    let mut write_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        ping_interval.tick().await;

        loop {
            tokio::select! {
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(msg) => {
                            let (frame, is_terminal) = match msg {
                                BroadcastMessage::Terminal(data) => {
                                    (serde_json::json!({
                                        "type": "terminal",
                                        "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data),
                                    }), false)
                                }
                                BroadcastMessage::Event(payload) => {
                                    (serde_json::json!({
                                        "type": "event",
                                        "payload": payload,
                                    }), false)
                                }
                                BroadcastMessage::Status {
                                    status,
                                    exit_code,
                                    duration_ms,
                                } => {
                                    let terminal = matches!(
                                        status,
                                        AgentStatus::Completed
                                            | AgentStatus::Failed
                                            | AgentStatus::Stopped
                                            | AgentStatus::Timeout
                                    );
                                    (serde_json::json!({
                                        "type": "status",
                                        "status": status,
                                        "exit_code": exit_code,
                                        "duration_ms": duration_ms,
                                    }), terminal)
                                }
                            };

                            if ws_tx
                                .send(Message::Text(frame.to_string().into()))
                                .await
                                .is_err()
                            {
                                break;
                            }

                            if is_terminal {
                                let reason = format!("agent {}", frame["status"].as_str().unwrap_or("exited"));
                                ws_tx
                                    .send(Message::Close(Some(CloseFrame {
                                        code: 1000,
                                        reason: reason.into(),
                                    })))
                                    .await
                                    .ok();
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(lagged = n, "WebSocket client lagged behind");
                            let frame = serde_json::json!({
                                "type": "lag",
                                "dropped": n,
                            });
                            if ws_tx
                                .send(Message::Text(frame.to_string().into()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            ws_tx
                                .send(Message::Close(Some(CloseFrame {
                                    code: 1000,
                                    reason: "channel closed".into(),
                                })))
                                .await
                                .ok();
                            break;
                        }
                    }
                }
                _ = ping_interval.tick() => {
                    let elapsed = pong_for_write.lock().await.elapsed();
                    if elapsed > HEARTBEAT_TIMEOUT {
                        tracing::warn!("WebSocket client missed heartbeat, closing");
                        ws_tx
                            .send(Message::Close(Some(CloseFrame {
                                code: 1001,
                                reason: "heartbeat timeout".into(),
                            })))
                            .await
                            .ok();
                        break;
                    }
                    if ws_tx.send(Message::Ping(Bytes::new())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = &mut read_task => { write_task.abort(); },
        _ = &mut write_task => { read_task.abort(); },
    }
}
