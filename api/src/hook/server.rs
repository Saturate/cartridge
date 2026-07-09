use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;

use crate::agent::{AgentId, AgentRegistry, BroadcastMessage};
use crate::agent::events::AgentEvent;

const MAX_MESSAGE_SIZE: usize = 65_536;

pub async fn listen(path: &str, registry: AgentRegistry) -> Result<(), String> {
    // Remove stale socket
    let _ = std::fs::remove_file(path);

    let listener = UnixListener::bind(path)
        .map_err(|e| format!("bind {path}: {e}"))?;

    tracing::info!(path, "hook socket listening");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let reg = registry.clone();
                tokio::spawn(handle_connection(stream, reg));
            }
            Err(e) => {
                tracing::warn!(error = %e, "hook socket accept error");
            }
        }
    }
}

async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    registry: AgentRegistry,
) {
    // Protocol: 4-byte big-endian length prefix, then JSON payload
    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).await.is_err() {
        return;
    }

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_SIZE {
        tracing::warn!(len, "hook message too large, dropping");
        return;
    }

    let mut data = vec![0u8; len];
    if stream.read_exact(&mut data).await.is_err() {
        return;
    }

    let envelope: serde_json::Value = match serde_json::from_slice(&data) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "invalid hook JSON");
            return;
        }
    };

    let agent_id_str = envelope
        .get("agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if agent_id_str.is_empty() {
        tracing::warn!("hook event with no agent_id, dropping");
        return;
    }

    let agent_id = AgentId(agent_id_str.to_string());
    let event_type = envelope
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let payload_str = envelope
        .get("payload")
        .and_then(|v| v.as_str())
        .unwrap_or("{}");

    let payload: serde_json::Value =
        serde_json::from_str(payload_str).unwrap_or(serde_json::Value::String(payload_str.into()));

    let event = AgentEvent {
        event: event_type.clone(),
        provider: None,
        agent_id: agent_id_str.to_string(),
        payload: payload.clone(),
        timestamp: {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            format!("{secs}")
        },
    };

    if let Some(agent_lock) = registry.get(&agent_id).await {
        let mut state = agent_lock.write().await;
        state.events.push(event);
        let _ = state.broadcast_tx.send(BroadcastMessage::Event(
            serde_json::json!({
                "event": event_type,
                "agent_id": agent_id_str,
                "payload": payload,
            }),
        ));
    } else {
        tracing::warn!(agent_id = %agent_id, event = %event_type, "hook event for unknown agent");
    }
}
