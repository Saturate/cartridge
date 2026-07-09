pub mod events;
pub mod provider;
pub mod ring_buffer;
pub mod spawn;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::config::Config;
use events::EventBuffer;
use ring_buffer::RingBuffer;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn generate() -> Self {
        use rand::Rng;
        let mut rng = rand::rng();
        let id: u64 = rng.random();
        Self(format!("ag_{:012x}", id & 0xFFFF_FFFF_FFFF))
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Starting,
    Running,
    Completed,
    Failed,
    Timeout,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Claude,
    Pi,
    Opencode,
    Codex,
    Gemini,
    Custom,
}

impl Provider {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "claude" => Some(Self::Claude),
            "pi" => Some(Self::Pi),
            "opencode" => Some(Self::Opencode),
            "codex" => Some(Self::Codex),
            "gemini" => Some(Self::Gemini),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

pub enum PtyCommand {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Kill,
}

pub struct AgentState {
    pub id: AgentId,
    pub provider: Provider,
    pub prompt: String,
    pub command: Vec<String>,
    pub pid: u32,
    pub status: AgentStatus,
    pub exit_code: Option<i32>,
    pub created_at: Instant,
    pub ring_buffer: RingBuffer,
    pub events: EventBuffer,
    pub broadcast_tx: broadcast::Sender<BroadcastMessage>,
    pub pty_cmd_tx: Option<mpsc::Sender<PtyCommand>>,
    pub hooks_enabled: bool,
    pub timeout_secs: Option<u64>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
}

impl AgentState {
    pub fn duration_ms(&self) -> u64 {
        self.created_at.elapsed().as_millis() as u64
    }
}

#[derive(Debug, Clone)]
pub enum BroadcastMessage {
    Terminal(Vec<u8>),
    Event(serde_json::Value),
    Status {
        status: AgentStatus,
        exit_code: Option<i32>,
        duration_ms: u64,
    },
}

#[derive(Clone)]
pub struct AgentRegistry {
    inner: Arc<RwLock<HashMap<AgentId, Arc<RwLock<AgentState>>>>>,
    config: Arc<Config>,
}

impl AgentRegistry {
    pub fn new(config: Config) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(config),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub async fn insert(&self, state: AgentState) -> Arc<RwLock<AgentState>> {
        let id = state.id.clone();
        let arc = Arc::new(RwLock::new(state));
        self.inner.write().await.insert(id, arc.clone());
        arc
    }

    pub async fn get(&self, id: &AgentId) -> Option<Arc<RwLock<AgentState>>> {
        self.inner.read().await.get(id).cloned()
    }

    pub async fn list(&self) -> Vec<Arc<RwLock<AgentState>>> {
        self.inner.read().await.values().cloned().collect()
    }

    pub async fn running_count(&self) -> usize {
        let map = self.inner.read().await;
        let mut count = 0;
        for agent in map.values() {
            let a = agent.read().await;
            if matches!(a.status, AgentStatus::Starting | AgentStatus::Running) {
                count += 1;
            }
        }
        count
    }

    pub async fn stop_all(&self) {
        let map = self.inner.read().await;
        for agent in map.values() {
            let a = agent.read().await;
            if matches!(a.status, AgentStatus::Starting | AgentStatus::Running) {
                if let Some(tx) = &a.pty_cmd_tx {
                    tx.send(PtyCommand::Kill).await.ok();
                }
            }
        }
    }

    pub async fn evict_expired(&self) {
        let mut map = self.inner.write().await;
        let now = Instant::now();
        let retain_dur = std::time::Duration::from_secs(self.config.retain_seconds);

        map.retain(|_, agent| {
            if let Ok(a) = agent.try_read() {
                if matches!(a.status, AgentStatus::Starting | AgentStatus::Running) {
                    return true;
                }
                now.duration_since(a.created_at) < retain_dur
            } else {
                true
            }
        });

        while map.len() > self.config.max_agents {
            let oldest = map
                .iter()
                .filter_map(|(id, a)| {
                    a.try_read().ok().and_then(|a| {
                        if !matches!(a.status, AgentStatus::Starting | AgentStatus::Running) {
                            Some((id.clone(), a.created_at))
                        } else {
                            None
                        }
                    })
                })
                .min_by_key(|(_, t)| *t)
                .map(|(id, _)| id);

            if let Some(id) = oldest {
                map.remove(&id);
            } else {
                break;
            }
        }
    }
}
