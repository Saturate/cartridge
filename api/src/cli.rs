use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::Config;

fn base_url(config: &Config) -> String {
    format!("http://127.0.0.1:{}", config.port)
}

fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .build(),
    )
}

fn auth_header(config: &Config) -> Option<String> {
    config.token.as_ref().map(|t| format!("Bearer {t}"))
}

fn get<T: for<'de> Deserialize<'de>>(config: &Config, path: &str) -> Result<T, String> {
    let url = format!("{}{}", base_url(config), path);
    let mut req = agent().get(&url);
    if let Some(auth) = auth_header(config) {
        req = req.header("Authorization", &auth);
    }
    let resp = req.call().map_err(|e| format!("request failed: {e}"))?;
    resp.into_body()
        .read_json::<T>()
        .map_err(|e| format!("invalid response: {e}"))
}

fn post<T: for<'de> Deserialize<'de>>(
    config: &Config,
    path: &str,
    body: &impl Serialize,
) -> Result<T, String> {
    let url = format!("{}{}", base_url(config), path);
    let mut req = agent().post(&url);
    if let Some(auth) = auth_header(config) {
        req = req.header("Authorization", &auth);
    }
    let resp = req
        .send_json(body)
        .map_err(|e| format!("request failed: {e}"))?;
    resp.into_body()
        .read_json::<T>()
        .map_err(|e| format!("invalid response: {e}"))
}

fn exit_err(e: String) -> ! {
    eprintln!("error: {e}");
    std::process::exit(1);
}

// --- List agents ---

#[derive(Deserialize)]
struct AgentListResponse {
    agents: Vec<AgentSummary>,
}

#[derive(Deserialize)]
struct AgentSummary {
    id: String,
    provider: String,
    status: String,
    prompt: String,
    #[allow(dead_code)]
    pid: u32,
    duration_ms: u64,
}

pub fn list_agents(config: &Config, status: Option<&str>, provider: Option<&str>) {
    let mut path = "/api/agents".to_string();
    let mut params = Vec::new();
    if let Some(s) = status {
        params.push(format!("status={s}"));
    }
    if let Some(p) = provider {
        params.push(format!("provider={p}"));
    }
    if !params.is_empty() {
        path.push('?');
        path.push_str(&params.join("&"));
    }

    let resp: AgentListResponse = get(config, &path).unwrap_or_else(|e| exit_err(e));

    if resp.agents.is_empty() {
        println!("No agents found.");
        return;
    }

    println!(
        "{:<18} {:<10} {:<10} {:>8}  PROMPT",
        "ID", "PROVIDER", "STATUS", "DURATION"
    );
    for a in &resp.agents {
        let duration = format_duration(a.duration_ms);
        let prompt: String = a.prompt.chars().take(50).collect();
        let prompt = prompt.replace('\n', " ");
        println!(
            "{:<18} {:<10} {:<10} {:>8}  {}",
            a.id, a.provider, a.status, duration, prompt
        );
    }
}

// --- Show agent ---

#[derive(Deserialize)]
struct AgentDetail {
    id: String,
    provider: String,
    status: String,
    prompt: String,
    command: Vec<String>,
    pid: u32,
    created_at: String,
    duration_ms: u64,
    exit_code: Option<i32>,
    cwd: String,
    timeout: Option<u64>,
    hooks_enabled: bool,
    buffer_size: u64,
    event_count: u64,
}

pub fn show_agent(config: &Config, id: &str) {
    let a: AgentDetail = get(config, &format!("/api/agents/{id}")).unwrap_or_else(|e| exit_err(e));

    println!("ID:           {}", a.id);
    println!("Provider:     {}", a.provider);
    println!("Status:       {}", a.status);
    println!("PID:          {}", a.pid);
    println!("Created:      {}", a.created_at);
    println!("Duration:     {}", format_duration(a.duration_ms));
    println!("CWD:          {}", a.cwd);
    if let Some(t) = a.timeout {
        println!("Timeout:      {}s", t);
    }
    println!("Hooks:        {}", a.hooks_enabled);
    println!("Buffer:       {} bytes", a.buffer_size);
    println!("Events:       {}", a.event_count);
    if let Some(code) = a.exit_code {
        println!("Exit code:    {}", code);
    }
    if !a.command.is_empty() {
        println!("Command:      {}", a.command.join(" "));
    }
    if !a.prompt.is_empty() {
        println!("Prompt:       {}", a.prompt);
    }
}

// --- Spawn agent ---

#[derive(Serialize)]
struct CreateAgentRequest {
    provider: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<SpawnOptions>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    env: HashMap<String, String>,
    cwd: String,
    timeout: u64,
    hooks: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SpawnOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_turns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_mode: Option<String>,
}

#[derive(Deserialize)]
struct CreateAgentResponse {
    id: String,
    provider: String,
    status: String,
    pid: u32,
    created_at: String,
    ws: String,
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_agent(
    config: &Config,
    provider: &str,
    prompt: &str,
    model: Option<&str>,
    max_turns: Option<u32>,
    cwd: &str,
    timeout: u64,
    no_hooks: bool,
) {
    let options = if model.is_some() || max_turns.is_some() {
        Some(SpawnOptions {
            model: model.map(String::from),
            max_turns,
            permission_mode: None,
        })
    } else {
        None
    };

    let req = CreateAgentRequest {
        provider: provider.to_string(),
        prompt: prompt.to_string(),
        options,
        env: HashMap::new(),
        cwd: cwd.to_string(),
        timeout,
        hooks: !no_hooks,
    };

    let resp: CreateAgentResponse =
        post(config, "/api/agents", &req).unwrap_or_else(|e| exit_err(e));

    println!("Agent spawned:");
    println!("  ID:       {}", resp.id);
    println!("  Provider: {}", resp.provider);
    println!("  Status:   {}", resp.status);
    println!("  PID:      {}", resp.pid);
    println!("  Created:  {}", resp.created_at);
    println!("  WS:       ws://127.0.0.1:{}{}", config.port, resp.ws);
}

// --- Stop agent ---

#[derive(Serialize)]
struct StopRequest {
    grace_period: u64,
}

#[derive(Deserialize)]
struct StopResponse {
    id: String,
    status: String,
    exit_code: Option<i32>,
}

pub fn stop_agent(config: &Config, id: &str, grace: u64) {
    let req = StopRequest {
        grace_period: grace,
    };

    let resp: StopResponse =
        post(config, &format!("/api/agents/{id}/stop"), &req).unwrap_or_else(|e| exit_err(e));

    println!("Agent {} stopped (status: {})", resp.id, resp.status);
    if let Some(code) = resp.exit_code {
        println!("Exit code: {}", code);
    }
}

// --- Logs ---

#[derive(Deserialize)]
struct OutputResponse {
    data: String,
}

pub fn agent_logs(config: &Config, id: &str) {
    let resp: OutputResponse =
        get(config, &format!("/api/agents/{id}/output?format=raw")).unwrap_or_else(|e| exit_err(e));

    if resp.data.is_empty() {
        println!("(no output yet)");
    } else {
        print!("{}", resp.data);
    }
}

fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{mins}m{secs}s")
    }
}
