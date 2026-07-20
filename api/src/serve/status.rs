use axum::Json;

use crate::agent::AgentRegistry;
use crate::config::Config;

pub fn print_status(config: &Config) {
    let status = collect_status_sync(config);
    println!(
        "{}",
        serde_json::to_string_pretty(&status).unwrap_or_else(|_| "{}".into())
    );
}

fn collect_status_sync(config: &Config) -> serde_json::Value {
    let services = probe_services();
    let cdp = probe_cdp();
    let providers = probe_providers();
    let runtime = probe_runtime();

    serde_json::json!({
        "services": services,
        "cdp": cdp,
        "providers": providers,
        "tunnels": {
            "tailscale": { "enabled": std::env::var("TAILSCALE_AUTHKEY").is_ok() },
            "cloudflared": { "enabled": std::env::var("CF_TUNNEL_TOKEN").is_ok() },
        },
        "runtime": runtime,
        "agents": { "running": 0, "completed": 0, "failed": 0 },
        "api": { "port": config.port, "auth": config.token.is_some() },
    })
}

pub async fn handle_status(
    axum::extract::State(registry): axum::extract::State<AgentRegistry>,
) -> Json<serde_json::Value> {
    let config = registry.config();
    let mut status = collect_status_sync(config);

    let agents = registry.list().await;
    let mut running = 0u32;
    let mut completed = 0u32;
    let mut failed = 0u32;

    for agent_lock in &agents {
        let a = agent_lock.read().await;
        match a.status {
            crate::agent::AgentStatus::Starting | crate::agent::AgentStatus::Running => {
                running += 1
            }
            crate::agent::AgentStatus::Completed => completed += 1,
            _ => failed += 1,
        }
    }

    status["agents"] = serde_json::json!({
        "running": running,
        "completed": completed,
        "failed": failed,
    });

    Json(status)
}

pub async fn handle_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

fn probe_services() -> serde_json::Value {
    let services = [
        "xvfb",
        "ttyd",
        "chrome",
        "state-sync",
        "novnc",
        "sshd",
        "api",
    ];
    let mut result = serde_json::Map::new();

    for svc in &services {
        let status = std::process::Command::new("pgrep")
            .args(["-f", &format!("svc-{svc}")])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        result.insert(
            svc.to_string(),
            serde_json::Value::String(if status {
                "running".into()
            } else {
                "stopped".into()
            }),
        );
    }

    serde_json::Value::Object(result)
}

fn probe_cdp() -> serde_json::Value {
    let output = std::process::Command::new("curl")
        .args(["-sf", "http://localhost:9222/json/version"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let body: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
            serde_json::json!({
                "available": true,
                "browser": body.get("Browser").and_then(|b| b.as_str()).unwrap_or("unknown"),
            })
        }
        _ => serde_json::json!({ "available": false }),
    }
}

fn probe_providers() -> serde_json::Value {
    let mut providers = serde_json::Map::new();

    let has_anthropic = std::env::var("ANTHROPIC_API_KEY").is_ok();
    providers.insert(
        "claude".into(),
        serde_json::json!({
            "available": has_anthropic || which("claude"),
            "auth": if has_anthropic { "api_key" } else { "subscription" },
        }),
    );

    let has_ollama = std::env::var("OLLAMA_HOST").is_ok();
    providers.insert(
        "pi".into(),
        serde_json::json!({
            "available": which("pi"),
            "auth": if has_ollama { "ollama" } else { "none" },
        }),
    );

    providers.insert(
        "github".into(),
        serde_json::json!({
            "available": std::env::var("GH_TOKEN").is_ok() || std::env::var("GITHUB_TOKEN").is_ok(),
            "auth": "token",
        }),
    );

    serde_json::Value::Object(providers)
}

fn probe_runtime() -> serde_json::Value {
    let node = cmd_version("node", &["--version"]);
    let bun = cmd_version("bun", &["--version"]);
    let pnpm = cmd_version("pnpm", &["--version"]);

    serde_json::json!({
        "node": node,
        "bun": bun,
        "pnpm": pnpm,
    })
}

fn cmd_version(cmd: &str, args: &[&str]) -> String {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "not found".into())
}

fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
