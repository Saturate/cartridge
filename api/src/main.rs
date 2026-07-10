mod agent;
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
    }
}

fn init_tracing(level: &str, log_buffer: serve::logs::LogBuffer) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let buffer_layer = serve::logs::BufferLayer::new(log_buffer);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json().with_target(false))
        .with(buffer_layer)
        .init();
}
