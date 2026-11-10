//! Structured log stream from Rust to the UI log drawer.
//!
//! Every entry is written to stderr, kept in a bounded ring buffer (so the UI can backfill
//! what it missed before subscribing) and emitted as the `log://entry` event.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// Event name; keep in sync with `EVENTS.log` in `src/lib/ipc.ts`.
pub const LOG_EVENT: &str = "log://entry";
const HISTORY_LIMIT: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub ts_ms: u64,
    pub level: Level,
    pub source: String,
    pub message: String,
}

#[derive(Default)]
pub struct LogBus {
    history: Mutex<VecDeque<LogEntry>>,
}

impl LogBus {
    pub fn push(&self, entry: LogEntry) {
        let mut h = self.history.lock().unwrap_or_else(|e| e.into_inner());
        if h.len() == HISTORY_LIMIT {
            h.pop_front();
        }
        h.push_back(entry);
    }

    pub fn history(&self) -> Vec<LogEntry> {
        let h = self.history.lock().unwrap_or_else(|e| e.into_inner());
        h.iter().cloned().collect()
    }
}

pub fn emit<R: Runtime>(app: &AppHandle<R>, level: Level, source: &str, message: impl Into<String>) {
    let entry = LogEntry {
        ts_ms: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
        level,
        source: source.to_string(),
        message: message.into(),
    };
    eprintln!("[{:?}] {}: {}", entry.level, entry.source, entry.message);
    if let Some(bus) = app.try_state::<LogBus>() {
        bus.push(entry.clone());
    }
    // Logging must never fail the caller.
    let _ = app.emit(LOG_EVENT, entry);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(i: usize) -> LogEntry {
        LogEntry { ts_ms: i as u64, level: Level::Info, source: "test".into(), message: i.to_string() }
    }

    #[test]
    fn history_is_bounded_and_keeps_newest() {
        let bus = LogBus::default();
        for i in 0..HISTORY_LIMIT + 5 {
            bus.push(entry(i));
        }
        let h = bus.history();
        assert_eq!(h.len(), HISTORY_LIMIT);
        assert_eq!(h[0].ts_ms, 5);
        assert_eq!(h.last().unwrap().ts_ms, (HISTORY_LIMIT + 4) as u64);
    }

    #[test]
    fn entry_serializes_to_frontend_contract() {
        let json = serde_json::to_value(entry(1)).unwrap();
        assert_eq!(json["tsMs"], 1);
        assert_eq!(json["level"], "info");
        assert_eq!(json["source"], "test");
    }
}
