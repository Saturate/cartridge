use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::{broadcast, mpsc, RwLock};

use super::{
    AgentId, AgentState, AgentStatus, BroadcastMessage, Provider, PtyCommand,
    events::EventBuffer,
    ring_buffer::RingBuffer,
};
use crate::config::Config;

const ENV_DENYLIST: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "NODE_OPTIONS",
    "BASH_ENV",
    "ENV",
    "PYTHONSTARTUP",
    "PYTHONPATH",
    "PERL5OPT",
    "RUBYOPT",
    "CARTRIDGE_AGENT_ID",
    "CARTRIDGE_API_URL",
];

pub fn validate_env(env: &HashMap<String, String>) -> Result<(), String> {
    for key in env.keys() {
        if ENV_DENYLIST.iter().any(|d| key == *d) {
            return Err(format!("env var '{key}' is not allowed"));
        }
        if key.starts_with("BASH_FUNC_") {
            return Err(format!("env var '{key}' is not allowed"));
        }
    }
    Ok(())
}

pub struct SpawnRequest {
    pub provider: Provider,
    pub prompt: String,
    pub command: Vec<String>,
    pub options: super::provider::ProviderOptions,
    pub env: HashMap<String, String>,
    pub cwd: PathBuf,
    pub timeout_secs: Option<u64>,
    pub idle_timeout_secs: Option<u64>,
    pub hooks: bool,
}

pub type PtyChild = Arc<std::sync::Mutex<Box<dyn portable_pty::Child + Send + Sync>>>;

pub fn spawn_agent(
    req: SpawnRequest,
    config: &Config,
) -> Result<(AgentState, mpsc::Receiver<Vec<u8>>, PtyChild), String> {
    validate_env(&req.env)?;

    let id = AgentId::generate();

    let cmd_args = req.provider.build_command(
        &req.options,
        config.safe_mode,
        if req.provider == Provider::Custom {
            Some(&req.command)
        } else {
            None
        },
    );

    if cmd_args.is_empty() {
        return Err("empty command".into());
    }

    let pty_system = native_pty_system();
    let size = PtySize {
        rows: 40,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pair = pty_system
        .openpty(size)
        .map_err(|e| format!("PTY allocation failed: {e}"))?;

    let mut cmd = CommandBuilder::new(&cmd_args[0]);
    if cmd_args.len() > 1 {
        cmd.args(&cmd_args[1..]);
    }
    cmd.cwd(&req.cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("CARTRIDGE_AGENT_ID", &id.0);
    cmd.env("CARTRIDGE_API_URL", format!("http://localhost:{}", config.port));

    for (k, v) in &req.env {
        cmd.env(k, v);
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn failed: {e}"))?;

    let pid = child.process_id().unwrap_or(0);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("clone reader: {e}"))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("take writer: {e}"))?;

    let (broadcast_tx, _) = broadcast::channel(256);
    let (pty_cmd_tx, pty_cmd_rx) = mpsc::channel(64);
    let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(256);
    let prompt_cmd_tx = pty_cmd_tx.clone();

    let state = AgentState {
        id: id.clone(),
        provider: req.provider,
        prompt: req.prompt.clone(),
        command: cmd_args,
        pid,
        status: AgentStatus::Running,
        exit_code: None,
        created_at: Instant::now(),
        ring_buffer: RingBuffer::new(config.buffer_size),
        events: EventBuffer::new(config.max_events),
        messages: super::messages::MessageStore::new(),
        broadcast_tx: broadcast_tx.clone(),
        pty_cmd_tx: Some(pty_cmd_tx),
        hooks_enabled: req.hooks,
        timeout_secs: req.timeout_secs,
        idle_timeout_secs: req.idle_timeout_secs,
        last_activity: Instant::now(),
        cwd: req.cwd,
        env: req.env,
        headless: false,
    };

    // PTY reader thread: reads bytes from the PTY master and sends to channel
    let reader_id = id.clone();
    std::thread::Builder::new()
        .name(format!("pty-reader-{}", reader_id))
        .spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if output_tx.blocking_send(data).is_err() {
                            break;
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        })
        .map_err(|e| format!("reader thread: {e}"))?;

    // PTY command handler: processes input, resize, kill
    let writer = Arc::new(std::sync::Mutex::new(writer));
    let cmd_pid = pid;
    let cmd_master = Arc::new(std::sync::Mutex::new(pair.master));
    tokio::spawn(handle_pty_commands(
        pty_cmd_rx,
        writer,
        cmd_master,
        cmd_pid,
    ));

    let child = Arc::new(std::sync::Mutex::new(child));

    // Send the initial prompt after the CLI has time to initialize.
    // The prompt is sent as PTY keystrokes, not as a CLI argument.
    if !req.prompt.is_empty() && req.provider != Provider::Custom {
        let delay = req.provider.startup_delay_ms();
        let prompt = req.prompt;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            let mut input = prompt.into_bytes();
            if !input.ends_with(b"\n") {
                input.push(b'\n');
            }
            prompt_cmd_tx.send(PtyCommand::Input(input)).await.ok();
        });
    }

    Ok((state, output_rx, child))
}

pub fn start_child_waiter(
    agent: Arc<RwLock<AgentState>>,
    child: PtyChild,
) {
    tokio::spawn(async move {
        let child_clone = child.clone();
        let exit = tokio::task::spawn_blocking(move || {
            let mut c = child_clone.lock().unwrap();
            c.wait()
        })
        .await;

        let (status, code) = match exit {
            Ok(Ok(exit_status)) => {
                let code = exit_status.exit_code() as i32;
                if code == 0 {
                    (AgentStatus::Completed, Some(code))
                } else {
                    (AgentStatus::Failed, Some(code))
                }
            }
            _ => (AgentStatus::Failed, None),
        };

        let mut state = agent.write().await;
        let duration_ms = state.duration_ms();

        // Don't overwrite Stopped or Timeout (set before the kill signal)
        if !matches!(state.status, AgentStatus::Stopped | AgentStatus::Timeout) {
            state.status = status;
        }
        state.exit_code = code;

        tracing::info!(
            agent_id = %state.id,
            exit_code = ?code,
            ?status,
            duration_ms,
            "agent exited"
        );

        let _ = state.broadcast_tx.send(BroadcastMessage::Status {
            status,
            exit_code: code,
            duration_ms,
        });
    });
}

async fn handle_pty_commands(
    mut rx: mpsc::Receiver<PtyCommand>,
    writer: Arc<std::sync::Mutex<Box<dyn std::io::Write + Send>>>,
    master: Arc<std::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    pid: u32,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            PtyCommand::Input(data) => {
                let writer = writer.clone();
                tokio::task::spawn_blocking(move || {
                    if let Ok(mut w) = writer.lock() {
                        use std::io::Write;
                        w.write_all(&data).ok();
                        w.flush().ok();
                    }
                });
            }
            PtyCommand::Resize { cols, rows } => {
                let master = master.clone();
                tokio::task::spawn_blocking(move || {
                    if let Ok(m) = master.lock() {
                        m.resize(PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        })
                        .ok();
                    }
                });
            }
            PtyCommand::Kill => {
                let raw_pid = nix::unistd::Pid::from_raw(pid as i32);
                nix::sys::signal::kill(raw_pid, nix::sys::signal::Signal::SIGTERM).ok();

                let grace = Duration::from_secs(5);
                tokio::time::sleep(grace).await;

                nix::sys::signal::kill(raw_pid, nix::sys::signal::Signal::SIGKILL).ok();
                break;
            }
        }
    }
}

pub fn start_output_pump(
    agent: Arc<RwLock<AgentState>>,
    mut output_rx: mpsc::Receiver<Vec<u8>>,
) {
    tokio::spawn(async move {
        while let Some(data) = output_rx.recv().await {
            let mut state = agent.write().await;
            state.ring_buffer.append(&data);
            state.touch_activity();
            let _ = state.broadcast_tx.send(BroadcastMessage::Terminal(data));
        }
    });
}

/// Spawn a headless (non-PTY) agent with piped stdio.
///
/// The prompt is baked into CLI arguments rather than typed via PTY input.
/// Only stdout feeds into the output channel; stderr is logged separately
/// so structured JSON output is not corrupted.
///
/// **Custom provider note:** the prompt is *not* delivered to custom commands.
/// Custom headless commands must embed their own prompt handling. The prompt
/// is stored in `AgentState.prompt` for reference but not piped to stdin.
pub async fn spawn_headless_agent(
    req: SpawnRequest,
    config: &Config,
) -> Result<(AgentState, mpsc::Receiver<Vec<u8>>, tokio::process::Child), String> {
    validate_env(&req.env)?;

    let id = AgentId::generate();

    let cmd_args = req
        .provider
        .build_headless_command(
            &req.prompt,
            &req.options,
            config.safe_mode,
            if req.provider == Provider::Custom {
                Some(&req.command)
            } else {
                None
            },
        )
        .ok_or_else(|| format!("unsupported: provider {:?} does not support headless mode", req.provider))?;

    if cmd_args.is_empty() {
        return Err("empty command".into());
    }

    let mut cmd = tokio::process::Command::new(&cmd_args[0]);
    if cmd_args.len() > 1 {
        cmd.args(&cmd_args[1..]);
    }
    cmd.current_dir(&req.cwd);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdin(std::process::Stdio::piped());
    cmd.env("TERM", "dumb");
    cmd.env("CARTRIDGE_AGENT_ID", &id.0);
    cmd.env("CARTRIDGE_API_URL", format!("http://localhost:{}", config.port));

    for (k, v) in &req.env {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let pid = child.id().ok_or("failed to get child pid")?;

    let stdout = child.stdout.take().ok_or("failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("failed to capture stderr")?;
    let stdin = child.stdin.take().ok_or("failed to capture stdin")?;

    let (broadcast_tx, _) = broadcast::channel(256);
    let (pty_cmd_tx, pty_cmd_rx) = mpsc::channel(64);
    let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(256);

    // Only stdout goes into the output channel. Headless mode produces
    // structured JSON on stdout, so mixing stderr in would corrupt it.
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut reader = stdout;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if output_tx.send(buf[..n].to_vec()).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // stderr is logged separately so it doesn't interleave with stdout.
    let stderr_id = id.clone();
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut reader = stderr;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]);
                    tracing::info!(agent_id = %stderr_id, stderr = %text, "headless agent stderr");
                }
                Err(_) => break,
            }
        }
    });

    tokio::spawn(handle_headless_commands(pty_cmd_rx, stdin, pid));

    let state = AgentState {
        id: id.clone(),
        provider: req.provider,
        prompt: req.prompt,
        command: cmd_args,
        pid,
        status: AgentStatus::Running,
        exit_code: None,
        created_at: Instant::now(),
        ring_buffer: RingBuffer::new(config.buffer_size),
        events: EventBuffer::new(config.max_events),
        messages: super::messages::MessageStore::new(),
        broadcast_tx,
        pty_cmd_tx: Some(pty_cmd_tx),
        hooks_enabled: req.hooks,
        timeout_secs: req.timeout_secs,
        idle_timeout_secs: req.idle_timeout_secs,
        last_activity: Instant::now(),
        cwd: req.cwd,
        env: req.env,
        headless: true,
    };

    Ok((state, output_rx, child))
}

async fn handle_headless_commands(
    mut rx: mpsc::Receiver<PtyCommand>,
    mut stdin: tokio::process::ChildStdin,
    pid: u32,
) {
    use tokio::io::AsyncWriteExt;

    while let Some(cmd) = rx.recv().await {
        match cmd {
            PtyCommand::Input(data) => {
                if stdin.write_all(&data).await.is_err() {
                    break;
                }
                stdin.flush().await.ok();
            }
            // No-op: headless agents have no PTY to resize.
            // The API layer rejects resize requests with 422 before this point.
            PtyCommand::Resize { .. } => {}
            PtyCommand::Kill => {
                let raw_pid = nix::unistd::Pid::from_raw(pid as i32);
                nix::sys::signal::kill(raw_pid, nix::sys::signal::Signal::SIGTERM).ok();
                tokio::time::sleep(Duration::from_secs(5)).await;
                nix::sys::signal::kill(raw_pid, nix::sys::signal::Signal::SIGKILL).ok();
                break;
            }
        }
    }
}

pub fn start_headless_child_waiter(
    agent: Arc<RwLock<AgentState>>,
    mut child: tokio::process::Child,
) {
    tokio::spawn(async move {
        let exit = child.wait().await;

        let (status, code) = match exit {
            Ok(exit_status) => {
                let code = exit_status.code().unwrap_or(-1);
                if code == 0 {
                    (AgentStatus::Completed, Some(code))
                } else {
                    (AgentStatus::Failed, Some(code))
                }
            }
            Err(_) => (AgentStatus::Failed, None),
        };

        let mut state = agent.write().await;
        let duration_ms = state.duration_ms();

        if !matches!(state.status, AgentStatus::Stopped | AgentStatus::Timeout) {
            state.status = status;
        }
        state.exit_code = code;

        tracing::info!(
            agent_id = %state.id,
            exit_code = ?code,
            ?status,
            duration_ms,
            "agent exited"
        );

        let _ = state.broadcast_tx.send(BroadcastMessage::Status {
            status,
            exit_code: code,
            duration_ms,
        });
    });
}
