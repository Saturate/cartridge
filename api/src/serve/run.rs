use std::collections::HashMap;
use std::process::Stdio;
use std::time::Instant;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::agent::spawn::validate_env;
use crate::config::Config;

#[derive(Deserialize)]
pub struct RunRequest {
    pub command: Vec<String>,
    #[serde(default = "default_cwd")]
    pub cwd: String,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

fn default_cwd() -> String {
    "/workspace".into()
}

#[derive(Serialize)]
pub struct RunResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
}

pub async fn handle_run(
    State(config): State<std::sync::Arc<Config>>,
    Json(req): Json<RunRequest>,
) -> Result<Json<RunResponse>, (StatusCode, Json<serde_json::Value>)> {
    if req.command.is_empty() {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_command",
            "Empty command array",
        ));
    }

    if let Err(msg) = validate_env(&req.env) {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_request", &msg));
    }

    let timeout = req
        .timeout
        .unwrap_or(config.exec_timeout)
        .min(config.exec_max_timeout);

    let start = Instant::now();

    let mut cmd = Command::new(&req.command[0]);
    if req.command.len() > 1 {
        cmd.args(&req.command[1..]);
    }
    cmd.current_dir(&req.cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    for (k, v) in &req.env {
        cmd.env(k, v);
    }

    let child = cmd.spawn().map_err(|e| {
        error(
            StatusCode::BAD_GATEWAY,
            "spawn_failed",
            &format!("Failed to spawn: {e}"),
        )
    })?;

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout),
        child.wait_with_output(),
    )
    .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(output)) => Ok(Json(RunResponse {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            duration_ms,
            timed_out: false,
        })),
        Ok(Err(e)) => Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            &format!("Wait failed: {e}"),
        )),
        Err(_) => Ok(Json(RunResponse {
            exit_code: -1,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms,
            timed_out: true,
        })),
    }
}

fn error(status: StatusCode, code: &str, message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "error": {
                "code": code,
                "message": message,
                "status": status.as_u16()
            }
        })),
    )
}
