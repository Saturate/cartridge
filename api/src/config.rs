use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub token: Option<String>,
    pub safe_mode: bool,
    pub buffer_size: usize,
    pub max_agents: usize,
    pub max_concurrent: usize,
    pub max_timeout: u64,
    pub retain_seconds: u64,
    pub exec_timeout: u64,
    pub exec_max_timeout: u64,
    pub max_body_size: usize,
    pub cors_origin: Option<String>,
    pub max_events: usize,
    pub log_level: String,
    pub hooks_enabled: bool,
    pub socket_path: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: parse_env("CARTRIDGE_API_PORT", 4500),
            token: env::var("CARTRIDGE_API_TOKEN").ok().filter(|s| !s.is_empty()),
            safe_mode: parse_env_bool("CARTRIDGE_API_SAFE_MODE", false),
            buffer_size: parse_env("CARTRIDGE_API_BUFFER_SIZE", 2_097_152),
            max_agents: parse_env("CARTRIDGE_API_MAX_AGENTS", 100),
            max_concurrent: parse_env("CARTRIDGE_API_MAX_CONCURRENT", 10),
            max_timeout: parse_env("CARTRIDGE_API_MAX_TIMEOUT", 7200),
            retain_seconds: parse_env("CARTRIDGE_API_RETAIN_SECONDS", 3600),
            exec_timeout: parse_env("CARTRIDGE_API_EXEC_TIMEOUT", 30),
            exec_max_timeout: parse_env("CARTRIDGE_API_EXEC_MAX_TIMEOUT", 300),
            max_body_size: parse_env("CARTRIDGE_API_MAX_BODY_SIZE", 1_048_576),
            cors_origin: env::var("CARTRIDGE_API_CORS_ORIGIN").ok().filter(|s| !s.is_empty()),
            max_events: parse_env("CARTRIDGE_API_MAX_EVENTS", 10_000),
            log_level: env::var("CARTRIDGE_API_LOG_LEVEL").unwrap_or_else(|_| "info".into()),
            hooks_enabled: parse_env_bool("CARTRIDGE_HOOKS", true),
            socket_path: env::var("CARTRIDGE_API_SOCKET")
                .unwrap_or_else(|_| "/tmp/cartridge-api.sock".into()),
        }
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(default)
}
