use std::collections::HashMap;
use std::path::PathBuf;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::agent::{
    AgentId, AgentRegistry, AgentStatus, Provider, PtyCommand,
    provider::ProviderOptions,
    spawn::{SpawnRequest, spawn_agent, validate_env},
};

#[derive(Deserialize)]
pub struct CreateAgentRequest {
    pub provider: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub options: ProviderOptions,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_cwd")]
    pub cwd: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
    #[serde(default = "default_hooks")]
    pub hooks: bool,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub headless: bool,
}

fn default_cwd() -> String { "/workspace".into() }
fn default_timeout() -> u64 { 3600 }
fn default_idle_timeout() -> u64 { 300 }
fn default_hooks() -> bool { true }

#[derive(Serialize)]
pub struct CreateAgentResponse {
    pub id: String,
    pub provider: Provider,
    pub status: AgentStatus,
    pub pid: u32,
    pub created_at: String,
    pub ws: String,
    pub headless: bool,
}

#[derive(Serialize)]
pub struct AgentSummary {
    pub id: String,
    pub provider: Provider,
    pub status: AgentStatus,
    pub prompt: String,
    pub pid: u32,
    pub created_at: String,
    pub duration_ms: u64,
    pub headless: bool,
}

#[derive(Serialize)]
pub struct AgentDetail {
    pub id: String,
    pub provider: Provider,
    pub status: AgentStatus,
    pub prompt: String,
    pub command: Vec<String>,
    pub pid: u32,
    pub created_at: String,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub cwd: String,
    pub timeout: Option<u64>,
    pub hooks_enabled: bool,
    pub buffer_size: u64,
    pub event_count: u64,
    pub headless: bool,
}

#[derive(Serialize)]
pub struct AgentListResponse {
    pub agents: Vec<AgentSummary>,
}

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub status: Option<String>,
    pub provider: Option<String>,
}

#[derive(Deserialize)]
pub struct OutputQuery {
    #[serde(default)]
    pub since: u64,
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String { "raw".into() }

#[derive(Serialize)]
pub struct OutputResponse {
    pub data: String,
    pub offset: u64,
    pub next_offset: u64,
    pub total_bytes: u64,
    pub wrapped: bool,
    pub lost_bytes: u64,
}

#[derive(Deserialize)]
pub struct InputRequest {
    pub data: String,
    #[serde(default)]
    pub base64: bool,
}

#[derive(Deserialize)]
pub struct ResizeRequest {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Deserialize, Default)]
pub struct StopRequest {
    #[serde(default = "default_grace")]
    pub grace_period: u64,
}

fn default_grace() -> u64 { 5 }

#[derive(Serialize)]
pub struct StopResponse {
    pub id: String,
    pub status: AgentStatus,
    pub exit_code: Option<i32>,
}

type ApiError = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, code: &str, msg: &str) -> ApiError {
    (status, Json(serde_json::json!({ "error": { "code": code, "message": msg, "status": status.as_u16() } })))
}

fn now_iso() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let hours = day_secs / 3600;
    let mins = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    // Days since 1970-01-01 to Y-M-D (simplified, no leap second handling)
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let year_days = if is_leap(y) { 366 } else { 365 };
        if remaining < year_days { break; }
        remaining -= year_days;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0usize;
    while m < 12 && remaining >= month_days[m] {
        remaining -= month_days[m];
        m += 1;
    }
    format!("{y:04}-{:02}-{:02}T{hours:02}:{mins:02}:{s:02}Z", m + 1, remaining + 1)
}

fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

pub async fn create_agent(
    State(registry): State<AgentRegistry>,
    Json(req): Json<CreateAgentRequest>,
) -> Result<(StatusCode, Json<CreateAgentResponse>), ApiError> {
    let provider = Provider::from_str(&req.provider)
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "invalid_request", &format!("Unknown provider: {}", req.provider)))?;

    if provider != Provider::Custom && req.prompt.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "invalid_request", "prompt is required"));
    }

    if provider == Provider::Custom && req.command.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "invalid_command", "command is required for custom provider"));
    }

    if let Err(msg) = validate_env(&req.env) {
        return Err(err(StatusCode::BAD_REQUEST, "invalid_request", &msg));
    }

    let config = registry.config();
    let running = registry.running_count().await;
    if running >= config.max_concurrent {
        return Err(err(StatusCode::TOO_MANY_REQUESTS, "too_many_agents", "Concurrent agent limit reached"));
    }

    let timeout_secs = if req.timeout == 0 {
        None
    } else {
        Some(req.timeout.min(config.max_timeout))
    };

    let idle_timeout_secs = if req.idle_timeout == 0 {
        None
    } else {
        Some(req.idle_timeout)
    };

    let headless = req.headless;
    let spawn_req = SpawnRequest {
        provider,
        prompt: req.prompt,
        command: req.command,
        options: req.options,
        env: req.env,
        cwd: PathBuf::from(&req.cwd),
        timeout_secs,
        idle_timeout_secs,
        hooks: req.hooks && config.hooks_enabled,
    };

    let (id, pid, provider) = if headless {
        let (state, output_rx, child) =
            crate::agent::spawn::spawn_headless_agent(spawn_req, config)
                .await
                .map_err(|e| {
                    if e.starts_with("unsupported:") {
                        err(StatusCode::BAD_REQUEST, "headless_unsupported", &e)
                    } else {
                        err(StatusCode::BAD_GATEWAY, "spawn_failed", &e)
                    }
                })?;

        let id = state.id.clone();
        let pid = state.pid;
        let provider = state.provider;
        let agent_arc = registry.insert(state).await;
        crate::agent::spawn::start_output_pump(agent_arc.clone(), output_rx);
        crate::agent::spawn::start_headless_child_waiter(agent_arc, child);
        (id, pid, provider)
    } else {
        let (state, output_rx, child) = spawn_agent(spawn_req, config)
            .map_err(|e| err(StatusCode::BAD_GATEWAY, "spawn_failed", &e))?;

        let id = state.id.clone();
        let pid = state.pid;
        let provider = state.provider;
        let agent_arc = registry.insert(state).await;
        crate::agent::spawn::start_output_pump(agent_arc.clone(), output_rx);
        crate::agent::spawn::start_child_waiter(agent_arc, child);
        (id, pid, provider)
    };

    // Schedule eviction check
    let reg = registry.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        reg.evict_expired().await;
    });

    Ok((
        StatusCode::CREATED,
        Json(CreateAgentResponse {
            ws: format!("/api/agents/{}/ws", id),
            id: id.0,
            provider,
            status: AgentStatus::Running,
            pid,
            created_at: now_iso(),
            headless,
        }),
    ))
}

pub async fn list_agents(
    State(registry): State<AgentRegistry>,
    Query(query): Query<ListQuery>,
) -> Json<AgentListResponse> {
    let agents = registry.list().await;
    let mut summaries = Vec::new();

    for agent_lock in &agents {
        let a = agent_lock.read().await;

        if let Some(ref s) = query.status {
            let status_str = serde_json::to_value(a.status)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();
            if &status_str != s {
                continue;
            }
        }

        if let Some(ref p) = query.provider {
            if Provider::from_str(p) != Some(a.provider) {
                continue;
            }
        }

        summaries.push(AgentSummary {
            id: a.id.0.clone(),
            provider: a.provider,
            status: a.status,
            prompt: a.prompt.chars().take(200).collect(),
            pid: a.pid,
            created_at: now_iso(),
            duration_ms: a.duration_ms(),
            headless: a.headless,
        });
    }

    Json(AgentListResponse { agents: summaries })
}

pub async fn get_agent(
    State(registry): State<AgentRegistry>,
    Path(id): Path<String>,
) -> Result<Json<AgentDetail>, ApiError> {
    let agent_id = AgentId(id.clone());
    let agent_lock = registry
        .get(&agent_id)
        .await
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "agent_not_found", &format!("Agent {id} does not exist")))?;

    let a = agent_lock.read().await;
    Ok(Json(AgentDetail {
        id: a.id.0.clone(),
        provider: a.provider,
        status: a.status,
        prompt: a.prompt.clone(),
        command: a.command.clone(),
        pid: a.pid,
        created_at: now_iso(),
        duration_ms: a.duration_ms(),
        exit_code: a.exit_code,
        cwd: a.cwd.to_string_lossy().into_owned(),
        timeout: a.timeout_secs,
        hooks_enabled: a.hooks_enabled,
        buffer_size: a.ring_buffer.total_written(),
        event_count: a.events.total_count(),
        headless: a.headless,
    }))
}

pub async fn get_output(
    State(registry): State<AgentRegistry>,
    Path(id): Path<String>,
    Query(query): Query<OutputQuery>,
) -> Result<Json<OutputResponse>, ApiError> {
    let agent_id = AgentId(id.clone());
    let agent_lock = registry
        .get(&agent_id)
        .await
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "agent_not_found", &format!("Agent {id} does not exist")))?;

    let a = agent_lock.read().await;
    let result = a.ring_buffer.read_since(query.since);

    let data = if query.format == "base64" {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&result.data)
    } else {
        String::from_utf8_lossy(&result.data).into_owned()
    };

    Ok(Json(OutputResponse {
        data,
        offset: query.since,
        next_offset: result.next_offset,
        total_bytes: a.ring_buffer.total_written(),
        wrapped: result.wrapped,
        lost_bytes: result.lost_bytes,
    }))
}

pub async fn send_input(
    State(registry): State<AgentRegistry>,
    Path(id): Path<String>,
    Json(req): Json<InputRequest>,
) -> Result<StatusCode, ApiError> {
    let agent_id = AgentId(id.clone());
    let agent_lock = registry
        .get(&agent_id)
        .await
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "agent_not_found", &format!("Agent {id} does not exist")))?;

    let a = agent_lock.read().await;
    if !matches!(a.status, AgentStatus::Starting | AgentStatus::Running) {
        return Err(err(StatusCode::CONFLICT, "agent_not_running", "Agent has already exited"));
    }

    let data = if req.base64 {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&req.data)
            .map_err(|e| err(StatusCode::BAD_REQUEST, "invalid_request", &format!("Invalid base64: {e}")))?
    } else {
        req.data.into_bytes()
    };

    if let Some(tx) = &a.pty_cmd_tx {
        tx.send(PtyCommand::Input(data))
            .await
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "PTY channel closed"))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn resize_agent(
    State(registry): State<AgentRegistry>,
    Path(id): Path<String>,
    Json(req): Json<ResizeRequest>,
) -> Result<StatusCode, ApiError> {
    let agent_id = AgentId(id.clone());
    let agent_lock = registry
        .get(&agent_id)
        .await
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "agent_not_found", &format!("Agent {id} does not exist")))?;

    let a = agent_lock.read().await;
    if !matches!(a.status, AgentStatus::Starting | AgentStatus::Running) {
        return Err(err(StatusCode::CONFLICT, "agent_not_running", "Agent has already exited"));
    }

    if a.headless {
        return Err(err(StatusCode::UNPROCESSABLE_ENTITY, "headless_no_resize", "Headless agents do not have a PTY to resize"));
    }

    if let Some(tx) = &a.pty_cmd_tx {
        tx.send(PtyCommand::Resize {
            cols: req.cols,
            rows: req.rows,
        })
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "PTY channel closed"))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn stop_agent(
    State(registry): State<AgentRegistry>,
    Path(id): Path<String>,
    body: Option<Json<StopRequest>>,
) -> Result<Json<StopResponse>, ApiError> {
    let agent_id = AgentId(id.clone());
    let agent_lock = registry
        .get(&agent_id)
        .await
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "agent_not_found", &format!("Agent {id} does not exist")))?;

    {
        let mut a = agent_lock.write().await;
        if !matches!(a.status, AgentStatus::Starting | AgentStatus::Running) {
            return Err(err(StatusCode::CONFLICT, "agent_not_running", "Agent has already exited"));
        }
        // Mark as Stopped before sending Kill so the child waiter doesn't overwrite
        a.status = AgentStatus::Stopped;
        if let Some(tx) = &a.pty_cmd_tx {
            tx.send(PtyCommand::Kill).await.ok();
        }
    }

    let _grace = body.map(|b| b.0.grace_period).unwrap_or(5);
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let a = agent_lock.read().await;

    Ok(Json(StopResponse {
        id: a.id.0.clone(),
        status: a.status,
        exit_code: a.exit_code,
    }))
}
