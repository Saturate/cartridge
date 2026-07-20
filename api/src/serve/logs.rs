use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::{extract::Query, Json};
use serde::{Deserialize, Serialize};
use tracing_subscriber::Layer;

const MAX_LOG_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub ts: String,
    pub level: String,
    pub msg: String,
    pub fields: serde_json::Value,
}

#[derive(Clone)]
pub struct LogBuffer {
    inner: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(
                MAX_LOG_ENTRIES.min(1024),
            ))),
        }
    }

    pub fn push(&self, entry: LogEntry) {
        let mut buf = self.inner.lock().unwrap();
        if buf.len() >= MAX_LOG_ENTRIES {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    pub fn read(&self, limit: usize, min_level: &str) -> (Vec<LogEntry>, usize) {
        let buf = self.inner.lock().unwrap();
        let total = buf.len();
        let min_severity = level_severity(min_level);

        let entries: Vec<LogEntry> = buf
            .iter()
            .filter(|e| level_severity(&e.level) >= min_severity)
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        (entries, total)
    }
}

fn level_severity(level: &str) -> u8 {
    match level.to_lowercase().as_str() {
        "error" => 4,
        "warn" | "warning" => 3,
        "info" => 2,
        "debug" => 1,
        "trace" => 0,
        _ => 2,
    }
}

// Tracing layer that captures log entries into the buffer
pub struct BufferLayer {
    buffer: LogBuffer,
}

impl BufferLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

impl<S> Layer<S> for BufferLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);

        let level = match *event.metadata().level() {
            tracing::Level::ERROR => "error",
            tracing::Level::WARN => "warn",
            tracing::Level::INFO => "info",
            tracing::Level::DEBUG => "debug",
            tracing::Level::TRACE => "trace",
        };

        let dur = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = dur.as_secs();
        let millis = dur.subsec_millis();

        let entry = LogEntry {
            ts: format!("{secs}.{millis:03}"),
            level: level.to_string(),
            msg: visitor.message.unwrap_or_default(),
            fields: serde_json::Value::Object(visitor.fields),
        };

        self.buffer.push(entry);
    }
}

#[derive(Default)]
struct JsonVisitor {
    message: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
}

impl tracing::field::Visit for JsonVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let s = format!("{value:?}");
        if field.name() == "message" {
            self.message = Some(s);
        } else {
            self.fields
                .insert(field.name().to_string(), serde_json::Value::String(s));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }
}

// Handler

#[derive(Deserialize)]
pub struct LogsQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_level")]
    pub level: String,
}

fn default_limit() -> usize {
    100
}
fn default_level() -> String {
    "info".into()
}

#[derive(Serialize)]
pub struct LogsResponse {
    pub entries: Vec<LogEntry>,
    pub total: usize,
    pub truncated: bool,
}

pub async fn handle_logs(
    axum::extract::State(buffer): axum::extract::State<LogBuffer>,
    Query(query): Query<LogsQuery>,
) -> Json<LogsResponse> {
    let limit = query.limit.min(1000);
    let (entries, total) = buffer.read(limit, &query.level);
    let truncated = entries.len() < total;

    Json(LogsResponse {
        entries,
        total,
        truncated,
    })
}
