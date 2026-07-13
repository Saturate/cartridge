mod agent;
mod cli;
mod config;
mod hook;
mod serve;

use clap::{Parser, Subcommand};
use config::Config;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser)]
#[command(name = "cartridge", about = "Cartridge container API")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the HTTP/WebSocket API server
    Serve,
    /// Forward a hook event to the running server via Unix socket
    Hook {
        /// Event type (e.g., pre-tool-use, post-tool-use, notification, stop)
        event: String,
    },
    /// Print container status as JSON
    Status,
    /// Check if the API server is running (exit 0 = running, 1 = not)
    Health,
    /// Spawn a new agent
    Spawn {
        /// Provider (claude, pi, opencode, codex, gemini, custom)
        #[arg(short, long)]
        provider: String,
        /// Task prompt
        prompt: String,
        /// Model override
        #[arg(short, long)]
        model: Option<String>,
        /// Max turns
        #[arg(long)]
        max_turns: Option<u32>,
        /// Working directory
        #[arg(long, default_value = "/workspace")]
        cwd: String,
        /// Timeout in seconds
        #[arg(short, long, default_value = "3600")]
        timeout: u64,
        /// Disable hooks
        #[arg(long)]
        no_hooks: bool,
    },
    /// List agents
    #[command(alias = "ls")]
    List {
        /// Filter by status (running, completed, failed, timeout, stopped)
        #[arg(short, long)]
        status: Option<String>,
        /// Filter by provider
        #[arg(short, long)]
        provider: Option<String>,
    },
    /// Show agent details
    Show {
        /// Agent ID
        id: String,
    },
    /// Stop a running agent
    Stop {
        /// Agent ID
        id: String,
        /// Grace period in seconds before SIGKILL
        #[arg(short, long, default_value = "5")]
        grace: u64,
    },
    /// Show agent PTY output
    Logs {
        /// Agent ID
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve => {
            let config = Config::from_env();
            let log_buffer = serve::logs::LogBuffer::new();
            init_tracing(&config.log_level, log_buffer.clone());
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime")
                .block_on(serve::run(config, log_buffer));
        }
        Command::Hook { event } => {
            hook::run(&event);
        }
        Command::Status => {
            let config = Config::from_env();
            serve::status::print_status(&config);
        }
        Command::Health => {
            let config = Config::from_env();
            let ok = hook::probe_socket(&config.socket_path);
            std::process::exit(if ok { 0 } else { 1 });
        }
        Command::Spawn {
            provider,
            prompt,
            model,
            max_turns,
            cwd,
            timeout,
            no_hooks,
        } => {
            let config = Config::from_env();
            cli::spawn_agent(
                &config,
                &provider,
                &prompt,
                model.as_deref(),
                max_turns,
                &cwd,
                timeout,
                no_hooks,
            );
        }
        Command::List { status, provider } => {
            let config = Config::from_env();
            cli::list_agents(&config, status.as_deref(), provider.as_deref());
        }
        Command::Show { id } => {
            let config = Config::from_env();
            cli::show_agent(&config, &id);
        }
        Command::Stop { id, grace } => {
            let config = Config::from_env();
            cli::stop_agent(&config, &id, grace);
        }
        Command::Logs { id } => {
            let config = Config::from_env();
            cli::agent_logs(&config, &id);
        }
    }
}

fn init_tracing(level: &str, log_buffer: serve::logs::LogBuffer) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let buffer_layer = serve::logs::BufferLayer::new(log_buffer);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json().with_target(false))
        .with(buffer_layer)
        .init();
}
