use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

const MAX_ENTRIES: usize = 500;

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub index: u64,
    pub level: String,
    pub message: String,
}

struct Inner {
    entries: VecDeque<LogEntry>,
    next_index: u64,
}

static BUFFER: OnceLock<Mutex<Inner>> = OnceLock::new();

fn buffer() -> &'static Mutex<Inner> {
    BUFFER.get_or_init(|| {
        Mutex::new(Inner {
            entries: VecDeque::new(),
            next_index: 0,
        })
    })
}

pub fn push(level: &str, message: String) {
    if let Ok(mut inner) = buffer().lock() {
        let index = inner.next_index;
        inner.next_index += 1;
        inner.entries.push_back(LogEntry {
            index,
            level: level.to_string(),
            message,
        });
        while inner.entries.len() > MAX_ENTRIES {
            inner.entries.pop_front();
        }
    }
}

pub fn read_since(since_index: u64) -> Vec<LogEntry> {
    buffer()
        .lock()
        .map(|inner| {
            inner
                .entries
                .iter()
                .filter(|e| e.index >= since_index)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

// Tracing layer
use tracing::{Event, Subscriber};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;

struct MessageVisitor {
    message: String,
    fields: Vec<String>,
}

impl Visit for MessageVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields.push(format!("{}={}", field.name(), value));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.fields.push(format!("{}={:?}", field.name(), value));
        }
    }
}

pub struct LogBufferLayer;

impl<S: Subscriber> tracing_subscriber::Layer<S> for LogBufferLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        if !meta.target().starts_with("wrtctrl") {
            return;
        }
        let level = meta.level().to_string();
        let mut visitor = MessageVisitor { message: String::new(), fields: Vec::new() };
        event.record(&mut visitor);
        if !visitor.message.is_empty() {
            let full = if visitor.fields.is_empty() {
                visitor.message
            } else {
                format!("{} ({})", visitor.message, visitor.fields.join(", "))
            };
            push(&level, full);
        }
    }
}
