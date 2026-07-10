pub mod agents;
pub mod auth;
pub mod logs;
pub mod messages;
pub mod run;
pub mod status;
pub mod terminal;
pub mod ws;

use std::sync::Arc;

use axum::{
    Router,
    middleware,
    routing::{get, post},
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;

use crate::agent::AgentRegistry;
use crate::config::Config;
use crate::hook;

pub async fn run(config: Config, log_buffer: logs::LogBuffer) {
    let port = config.port;
    let socket_path = config.socket_path.clone();
    let cors_origin = config.cors_origin.clone();
    let max_body = config.max_body_size;
    let token = config.token.clone();

    let registry = AgentRegistry::new(config.clone());

    // Periodic idle check + eviction
    let idle_registry = registry.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            idle_registry.kill_idle_agents().await;
            idle_registry.evict_expired().await;
        }
    });

    // Start Unix socket server for hook events
    let hook_registry = registry.clone();
    tokio::spawn(async move {
        if let Err(e) = hook::server::listen(&socket_path, hook_registry).await {
            tracing::error!(error = %e, "hook socket server failed");
        }
    });

    let config_arc = Arc::new(config);

    let app = Router::new()
        .route("/api/agents", post(agents::create_agent).get(agents::list_agents))
        .route("/api/agents/{id}", get(agents::get_agent))
        .route("/api/agents/{id}/output", get(agents::get_output))
        .route("/api/agents/{id}/input", post(agents::send_input))
        .route("/api/agents/{id}/resize", post(agents::resize_agent))
        .route("/api/agents/{id}/stop", post(agents::stop_agent))
        .route("/api/agents/{id}/ws", get(ws::ws_upgrade))
        .route("/api/agents/{id}/terminal", get(terminal::terminal_page))
        .route("/api/agents/{id}/messages", post(messages::send_message).get(messages::list_messages))
        .with_state(registry.clone())
        .route("/api/run", post(run::handle_run))
        .with_state(config_arc)
        .route("/api/health", get(status::handle_health))
        .route("/api/status", get(status::handle_status))
        .with_state(registry)
        .route("/api/logs", get(logs::handle_logs))
        .with_state(log_buffer)
        .route("/api/openapi.json", get(openapi_json))
        .route("/api/docs", get(openapi_docs))
        .layer(RequestBodyLimitLayer::new(max_body))
        .layer(build_cors(cors_origin.as_deref()));

    // Inject token for auth middleware
    let app = if let Some(ref t) = token {
        let t = t.clone();
        app.layer(middleware::from_fn(move |mut req: axum::extract::Request, next: middleware::Next| {
            let token = Some(t.clone());
            req.extensions_mut().insert(token);
            auth::auth_middleware(req, next)
        }))
    } else {
        app
    };

    // Bind dual-stack (IPv6 + IPv4) so Firefox doesn't stall on ::1 fallback
    let listener = match tokio::net::TcpListener::bind(format!("[::]:{port}")).await {
        Ok(l) => l,
        Err(_) => tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
            .await
            .expect("failed to bind"),
    };

    tracing::info!(port, "cartridge-api listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

fn build_cors(origin: Option<&str>) -> CorsLayer {
    match origin {
        None => CorsLayer::new(),
        Some("*") => {
            tracing::warn!("CORS origin set to '*' - ensure CARTRIDGE_API_TOKEN is set");
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        }
        Some(origin) => {
            let origin: axum::http::HeaderValue = origin.parse().expect("invalid CORS origin");
            CorsLayer::new()
                .allow_origin(origin)
                .allow_methods(Any)
                .allow_headers(Any)
                .allow_credentials(true)
        }
    }
}

async fn openapi_json() -> axum::Json<serde_json::Value> {
    axum::Json(openapi_spec())
}

async fn openapi_docs() -> axum::response::Html<String> {
    let spec_url = "/api/openapi.json";
    axum::response::Html(format!(
        r#"<!doctype html><html><head><title>Cartridge API</title><meta charset="utf-8"/>
<style>body {{ margin: 0; }}</style></head><body>
<script id="api-reference" data-url="{spec_url}"></script>
<script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
</body></html>"#
    ))
}

fn openapi_spec() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Cartridge API",
            "description": "HTTP/WebSocket API for the Cartridge AI dev harness container. Spawns CLI agents in real PTYs, streams output over WebSocket, and provides structured events via hook plugins.",
            "version": "0.1.0"
        },
        "servers": [{ "url": "/" }],
        "tags": [
            { "name": "System", "description": "Health, status, logs, and documentation" },
            { "name": "Agents", "description": "Agent lifecycle management and streaming" },
            { "name": "Commands", "description": "One-shot command execution" }
        ],
        "paths": {
            "/api/health": {
                "get": {
                    "summary": "Health check",
                    "description": "Returns OK if the API server is running. No auth required. Use for K8s liveness probes.",
                    "tags": ["System"],
                    "responses": { "200": { "description": "OK", "content": { "application/json": {
                        "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } },
                        "example": { "ok": true }
                    } } } }
                }
            },
            "/api/status": {
                "get": {
                    "summary": "Container status",
                    "description": "Full container status: s6 services, Chrome CDP, provider auth, runtime versions, and agent counts.",
                    "tags": ["System"],
                    "responses": { "200": { "description": "Container status", "content": { "application/json": {
                        "example": {
                            "services": { "xvfb": "running", "ttyd": "running", "chrome": "running", "api": "running" },
                            "cdp": { "available": true, "browser": "Chrome/149.0.7827.0" },
                            "providers": { "claude": { "available": true, "auth": "subscription" } },
                            "runtime": { "node": "v24.18.0", "bun": "1.3.14" },
                            "agents": { "running": 1, "completed": 3, "failed": 0 }
                        }
                    } } } }
                }
            },
            "/api/logs": {
                "get": {
                    "summary": "API server logs",
                    "description": "Returns recent structured log entries from the API server's in-memory buffer.",
                    "tags": ["System"],
                    "parameters": [
                        { "name": "limit", "in": "query", "schema": { "type": "integer", "default": 100, "maximum": 1000 }, "description": "Max entries to return" },
                        { "name": "level", "in": "query", "schema": { "type": "string", "enum": ["error", "warn", "info", "debug"], "default": "info" }, "description": "Minimum log level" }
                    ],
                    "responses": { "200": { "description": "Log entries", "content": { "application/json": {
                        "example": {
                            "entries": [
                                { "ts": "1783600170.404", "level": "info", "msg": "cartridge-api listening", "fields": { "port": 4500 } },
                                { "ts": "1783600186.794", "level": "info", "msg": "agent exited", "fields": { "agent_id": "ag_4a8119bac41f", "exit_code": "Some(0)" } }
                            ],
                            "total": 2,
                            "truncated": false
                        }
                    } } } }
                }
            },
            "/api/agents": {
                "get": {
                    "summary": "List agents",
                    "description": "List all agents (running and recently completed). Filter by status or provider.",
                    "tags": ["Agents"],
                    "parameters": [
                        { "name": "status", "in": "query", "schema": { "type": "string", "enum": ["running", "completed", "failed", "timeout", "stopped"] }, "description": "Filter by agent status" },
                        { "name": "provider", "in": "query", "schema": { "type": "string", "enum": ["claude", "pi", "opencode", "codex", "gemini", "custom"] }, "description": "Filter by provider" }
                    ],
                    "responses": { "200": { "description": "Agent list", "content": { "application/json": {
                        "example": {
                            "agents": [{
                                "id": "ag_31595180e58c",
                                "provider": "claude",
                                "status": "running",
                                "prompt": "fix the failing test...",
                                "pid": 356,
                                "duration_ms": 42000
                            }]
                        }
                    } } } }
                },
                "post": {
                    "summary": "Start an agent",
                    "description": "Spawn a CLI agent in a real PTY. The agent runs with the full interactive TUI. Connect via WebSocket to stream output and events.",
                    "tags": ["Agents"],
                    "requestBody": { "required": true, "content": { "application/json": {
                        "schema": {
                            "type": "object",
                            "required": ["provider"],
                            "properties": {
                                "provider": { "type": "string", "enum": ["claude", "pi", "opencode", "codex", "gemini", "custom"], "description": "CLI agent provider" },
                                "prompt": { "type": "string", "description": "Task prompt. Required for all providers except custom." },
                                "options": { "type": "object", "description": "Provider-specific CLI options (model, maxTurns, permissionMode, etc.)" },
                                "env": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Extra environment variables for the agent process" },
                                "cwd": { "type": "string", "default": "/workspace", "description": "Working directory" },
                                "timeout": { "type": "integer", "default": 3600, "description": "Max runtime in seconds. 0 = no timeout." },
                                "hooks": { "type": "boolean", "default": true, "description": "Enable CLI hook plugins for this agent" },
                                "command": { "type": "array", "items": { "type": "string" }, "description": "Raw command (custom provider only)" }
                            }
                        },
                        "examples": {
                            "claude": {
                                "summary": "Claude Code agent",
                                "value": {
                                    "provider": "claude",
                                    "prompt": "fix the failing test in src/auth.ts",
                                    "options": { "model": "sonnet", "maxTurns": 20 },
                                    "timeout": 600
                                }
                            },
                            "pi": {
                                "summary": "Pi agent",
                                "value": {
                                    "provider": "pi",
                                    "prompt": "refactor the database module to use connection pooling",
                                    "timeout": 1800
                                }
                            },
                            "custom": {
                                "summary": "Custom command",
                                "value": {
                                    "provider": "custom",
                                    "prompt": "",
                                    "command": ["sh", "-c", "echo hello; sleep 2; echo done"],
                                    "timeout": 30
                                }
                            },
                            "claude-with-env": {
                                "summary": "Claude with API key",
                                "value": {
                                    "provider": "claude",
                                    "prompt": "add input validation to the signup form",
                                    "env": { "ANTHROPIC_API_KEY": "sk-ant-..." },
                                    "options": { "model": "opus", "maxTurns": 50, "allowedTools": ["Read", "Edit", "Bash"] }
                                }
                            },
                            "opencode": {
                                "summary": "OpenCode agent",
                                "value": {
                                    "provider": "opencode",
                                    "prompt": "write unit tests for the utils module"
                                }
                            }
                        }
                    } } },
                    "responses": {
                        "201": { "description": "Agent started", "content": { "application/json": {
                            "example": {
                                "id": "ag_31595180e58c",
                                "provider": "claude",
                                "status": "running",
                                "pid": 356,
                                "created_at": "2026-07-09T12:09:43Z",
                                "ws": "/api/agents/ag_31595180e58c/ws"
                            }
                        } } },
                        "429": { "description": "Concurrent agent limit reached" },
                        "502": { "description": "CLI binary not found or crashed immediately" }
                    }
                }
            },
            "/api/agents/{id}": {
                "get": {
                    "summary": "Get agent details",
                    "description": "Full agent details including command, output buffer size, and event count.",
                    "tags": ["Agents"],
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" }, "example": "ag_31595180e58c" }],
                    "responses": {
                        "200": { "description": "Agent details", "content": { "application/json": {
                            "example": {
                                "id": "ag_31595180e58c",
                                "provider": "claude",
                                "status": "completed",
                                "prompt": "fix the failing test in src/auth.ts",
                                "command": ["claude", "-p", "fix the failing test in src/auth.ts"],
                                "pid": 356,
                                "duration_ms": 42000,
                                "exit_code": 0,
                                "cwd": "/workspace",
                                "timeout": 600,
                                "hooks_enabled": true,
                                "buffer_size": 8192,
                                "event_count": 12
                            }
                        } } },
                        "404": { "description": "Agent not found" }
                    }
                }
            },
            "/api/agents/{id}/output": {
                "get": {
                    "summary": "Get agent output",
                    "description": "Read the agent's PTY output ring buffer. Use `since` for incremental polling; the response includes `next_offset` for the next call.",
                    "tags": ["Agents"],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "string" }, "example": "ag_31595180e58c" },
                        { "name": "since", "in": "query", "schema": { "type": "integer", "default": 0 }, "description": "Byte offset to read from. Use next_offset from the previous response." },
                        { "name": "format", "in": "query", "schema": { "type": "string", "enum": ["raw", "base64"], "default": "raw" }, "description": "Output encoding. Use base64 for binary-safe transport." }
                    ],
                    "responses": { "200": { "description": "Ring buffer contents", "content": { "application/json": {
                        "example": {
                            "data": "agent started\r\ntick 1\r\ntick 2\r\nagent done\r\n",
                            "offset": 0,
                            "next_offset": 51,
                            "total_bytes": 51,
                            "wrapped": false,
                            "lost_bytes": 0
                        }
                    } } } }
                }
            },
            "/api/agents/{id}/input": {
                "post": {
                    "summary": "Send keystrokes to agent PTY",
                    "description": "Write data to the agent's PTY stdin. Use for approving prompts, answering questions, or sending Ctrl+C.",
                    "tags": ["Agents"],
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" }, "example": "ag_31595180e58c" }],
                    "requestBody": { "required": true, "content": { "application/json": {
                        "schema": {
                            "type": "object",
                            "required": ["data"],
                            "properties": {
                                "data": { "type": "string", "description": "Keystroke data to send" },
                                "base64": { "type": "boolean", "default": false, "description": "Whether data is base64-encoded" }
                            }
                        },
                        "examples": {
                            "approve": { "summary": "Approve a prompt", "value": { "data": "y\n" } },
                            "ctrl-c": { "summary": "Send Ctrl+C", "value": { "data": "" } },
                            "text": { "summary": "Type text", "value": { "data": "hello world\n" } }
                        }
                    } } },
                    "responses": { "204": { "description": "Input sent" }, "409": { "description": "Agent not running" } }
                }
            },
            "/api/agents/{id}/resize": {
                "post": {
                    "summary": "Resize agent PTY",
                    "description": "Change the terminal dimensions. Sends SIGWINCH to the agent process.",
                    "tags": ["Agents"],
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "requestBody": { "required": true, "content": { "application/json": {
                        "schema": {
                            "type": "object",
                            "required": ["cols", "rows"],
                            "properties": {
                                "cols": { "type": "integer", "description": "Terminal width in columns" },
                                "rows": { "type": "integer", "description": "Terminal height in rows" }
                            }
                        },
                        "example": { "cols": 160, "rows": 50 }
                    } } },
                    "responses": { "204": { "description": "Resized" } }
                }
            },
            "/api/agents/{id}/stop": {
                "post": {
                    "summary": "Stop an agent",
                    "description": "Send SIGTERM to the agent process, wait for grace period, then SIGKILL if still running.",
                    "tags": ["Agents"],
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "requestBody": { "content": { "application/json": {
                        "schema": {
                            "type": "object",
                            "properties": { "grace_period": { "type": "integer", "default": 5, "description": "Seconds to wait after SIGTERM before SIGKILL" } }
                        },
                        "example": { "grace_period": 10 }
                    } } },
                    "responses": {
                        "200": { "description": "Agent stopped", "content": { "application/json": {
                            "example": { "id": "ag_31595180e58c", "status": "stopped", "exit_code": 137 }
                        } } },
                        "409": { "description": "Agent already exited" }
                    }
                }
            },
            "/api/agents/{id}/messages": {
                "post": {
                    "summary": "Send message to agent",
                    "description": "Send a structured message to an agent. The message is stored in the agent's inbox and injected into the PTY as formatted input. Use for inter-agent communication: one agent can message another through the API.",
                    "tags": ["Agents"],
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" }, "example": "ag_31595180e58c" }],
                    "requestBody": { "required": true, "content": { "application/json": {
                        "schema": {
                            "type": "object",
                            "required": ["from", "content"],
                            "properties": {
                                "from": { "type": "string", "description": "Sender agent ID or identifier" },
                                "content": { "type": "string", "description": "Message content" },
                                "msg_type": { "type": "string", "enum": ["request", "response", "info"], "default": "request", "description": "Message type" }
                            }
                        },
                        "examples": {
                            "agent-to-agent": {
                                "summary": "Agent A asks Agent B",
                                "value": { "from": "ag_abc123", "content": "can you also check the auth module?", "msg_type": "request" }
                            },
                            "response": {
                                "summary": "Agent B replies to Agent A",
                                "value": { "from": "ag_def456", "content": "auth module looks fine, no issues found", "msg_type": "response" }
                            },
                            "orchestrator": {
                                "summary": "Orchestrator steers agent",
                                "value": { "from": "barracks", "content": "priority changed: focus on the payment module instead", "msg_type": "info" }
                            }
                        }
                    } } },
                    "responses": {
                        "201": { "description": "Message sent", "content": { "application/json": {
                            "example": { "id": "msg_a1b2c3d4e5f6", "delivered": true }
                        } } },
                        "404": { "description": "Agent not found" }
                    }
                },
                "get": {
                    "summary": "List agent messages",
                    "description": "Get all messages sent to this agent. Includes delivery status.",
                    "tags": ["Agents"],
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": { "200": { "description": "Message list", "content": { "application/json": {
                        "example": {
                            "messages": [
                                { "id": "msg_a1b2c3d4e5f6", "from": "ag_abc123", "to": "ag_def456", "content": "check the auth module", "msg_type": "request", "delivered": true, "timestamp": "1783602170.404" }
                            ],
                            "total": 1
                        }
                    } } } }
                }
            },
            "/api/agents/{id}/ws": {
                "get": {
                    "summary": "WebSocket stream",
                    "description": "Multiplexed WebSocket connection for an agent. Streams three frame types from server: `terminal` (base64 PTY bytes), `event` (structured hook events), `status` (lifecycle changes). Accepts two frame types from client: `input` (keystrokes), `resize` (terminal dimensions). On connect, replays the current ring buffer contents.",
                    "tags": ["Agents"],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "string" }, "example": "ag_31595180e58c" },
                        { "name": "token", "in": "query", "schema": { "type": "string" }, "description": "Bearer token for browser clients that can't set headers" }
                    ],
                    "responses": { "101": { "description": "WebSocket upgrade" }, "401": { "description": "Invalid token" }, "404": { "description": "Agent not found" } }
                }
            },
            "/api/run": {
                "post": {
                    "summary": "Run a one-shot command",
                    "description": "Execute a command and return stdout, stderr, exit code, and timing. No PTY, no hooks. Runs as the dev user.",
                    "tags": ["Commands"],
                    "requestBody": { "required": true, "content": { "application/json": {
                        "schema": {
                            "type": "object",
                            "required": ["command"],
                            "properties": {
                                "command": { "type": "array", "items": { "type": "string" }, "description": "Command as array (no shell interpolation)" },
                                "cwd": { "type": "string", "default": "/workspace", "description": "Working directory" },
                                "timeout": { "type": "integer", "description": "Timeout in seconds (default: 30, max: 300)" },
                                "env": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Extra environment variables" }
                            }
                        },
                        "examples": {
                            "echo": { "summary": "Simple echo", "value": { "command": ["echo", "hello world"] } },
                            "npm-test": { "summary": "Run tests", "value": { "command": ["npm", "test"], "timeout": 120 } },
                            "git-status": { "summary": "Git status", "value": { "command": ["git", "status", "--short"], "cwd": "/workspace" } },
                            "ls": { "summary": "List files", "value": { "command": ["ls", "-la", "/workspace"] } }
                        }
                    } } },
                    "responses": { "200": { "description": "Command result", "content": { "application/json": {
                        "example": {
                            "exit_code": 0,
                            "stdout": "hello world\n",
                            "stderr": "",
                            "duration_ms": 2,
                            "timed_out": false
                        }
                    } } } }
                }
            }
        },
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "Set CARTRIDGE_API_TOKEN env var to enable. Health, docs, and logs endpoints are exempt."
                }
            }
        },
        "security": [{ "bearerAuth": [] }]
    })
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
