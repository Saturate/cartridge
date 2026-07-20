use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub event: String,
    pub provider: Option<String>,
    pub agent_id: String,
    pub payload: serde_json::Value,
    pub timestamp: String,
}

pub struct EventBuffer {
    events: VecDeque<AgentEvent>,
    capacity: usize,
    total_count: u64,
}

impl EventBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity.min(1024)),
            capacity,
            total_count: 0,
        }
    }

    pub fn push(&mut self, event: AgentEvent) {
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
        self.total_count += 1;
    }

    pub fn total_count(&self) -> u64 {
        self.total_count
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &AgentEvent> {
        self.events.iter()
    }
}
