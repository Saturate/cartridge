use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

const MAX_MESSAGES: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub from: String,
    pub to: String,
    pub content: String,
    #[serde(default = "default_type")]
    pub msg_type: MessageType,
    pub timestamp: String,
    #[serde(default)]
    pub delivered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    #[default]
    Request,
    Response,
    Info,
}

fn default_type() -> MessageType {
    MessageType::Request
}

pub struct MessageStore {
    messages: VecDeque<AgentMessage>,
}

impl MessageStore {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
        }
    }

    pub fn push(&mut self, mut msg: AgentMessage) {
        if self.messages.len() >= MAX_MESSAGES {
            self.messages.pop_front();
        }
        msg.delivered = false;
        self.messages.push_back(msg);
    }

    pub fn mark_delivered(&mut self, id: &str) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) {
            msg.delivered = true;
        }
    }

    pub fn list(&self) -> Vec<&AgentMessage> {
        self.messages.iter().collect()
    }

    pub fn count(&self) -> usize {
        self.messages.len()
    }
}

pub fn format_for_pty(msg: &AgentMessage) -> Vec<u8> {
    let type_label = match msg.msg_type {
        MessageType::Request => "",
        MessageType::Response => " (response)",
        MessageType::Info => " (info)",
    };
    let formatted = format!(
        "[message from {}{}: {}]\n",
        msg.from, type_label, msg.content
    );
    formatted.into_bytes()
}

pub fn generate_message_id() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let id: u64 = rng.random();
    format!("msg_{:012x}", id & 0xFFFF_FFFF_FFFF)
}
