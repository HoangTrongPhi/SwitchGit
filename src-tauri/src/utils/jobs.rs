//! Long-running jobs: one per kind at a time, cancellable by id, with throttled progress events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

#[derive(Default)]
struct Inner {
    next: u64,
    jobs: HashMap<u64, (String, Arc<AtomicBool>)>,
}

/// Tauri state: the running jobs.
#[derive(Default, Clone)]
pub struct JobRegistry {
    inner: Arc<Mutex<Inner>>,
}

/// A registered job; dropping it (when the command returns) frees its kind.
pub struct JobHandle {
    pub id: u64,
    pub cancel: Arc<AtomicBool>,
    registry: Arc<Mutex<Inner>>,
}

impl Drop for JobHandle {
    fn drop(&mut self) {
        self.registry.lock().unwrap_or_else(|e| e.into_inner()).jobs.remove(&self.id);
    }
}

fn capitalized(kind: &str) -> String {
    let mut c = kind.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

impl JobRegistry {
    /// Err(`"<Kind> is already running"`) while another job of the same kind runs.
    pub fn start(&self, kind: &str) -> Result<JobHandle, String> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.jobs.values().any(|(k, _)| k == kind) {
            return Err(format!("{} is already running", capitalized(kind)));
        }
        g.next += 1;
        let id = g.next;
        let cancel = Arc::new(AtomicBool::new(false));
        g.jobs.insert(id, (kind.to_string(), cancel.clone()));
        Ok(JobHandle { id, cancel, registry: self.inner.clone() })
    }

    /// True when a job with this id was running (it stops at its next check).
    pub fn cancel(&self, id: u64) -> bool {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.jobs.get(&id).map(|(_, c)| c.store(true, Ordering::Relaxed)).is_some()
    }

    pub fn running(&self) -> Vec<(u64, String)> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<(u64, String)> = g.jobs.iter().map(|(id, (k, _))| (*id, k.clone())).collect();
        v.sort();
        v
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub job_id: u64,
    pub done: u64,
    pub total: u64,
    pub message: String,
}

const MIN_INTERVAL: Duration = Duration::from_millis(50);

/// `(done, total, message)` → `event` with `{ jobId, done, total, message }`, at most ~20 per second
/// (the first and the last always go out). Emits a `0 / 0 "Starting…"` at once, so the UI learns the
/// job id (for Cancel) before the first real step.
pub fn progress_emitter<R: Runtime>(app: &AppHandle<R>, event: &'static str, job: u64) -> impl Fn(u64, u64, &str) + Sync {
    let app = app.clone();
    let send = move |done: u64, total: u64, message: &str| {
        let _ = app.emit(event, ProgressPayload { job_id: job, done, total, message: message.to_string() });
    };
    send(0, 0, "Starting…");
    let last = Mutex::new(Instant::now() - MIN_INTERVAL);
    move |done, total, message| {
        let mut l = last.lock().unwrap_or_else(|e| e.into_inner());
        if done >= total || l.elapsed() >= MIN_INTERVAL {
            *l = Instant::now();
            send(done, total, message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_job_per_kind_and_cancel_by_id() {
        let r = JobRegistry::default();
        let a = r.start("stats").unwrap();
        assert_eq!(r.start("stats").err().unwrap(), "Stats is already running");
        let b = r.start("export").unwrap();
        assert_ne!(a.id, b.id);
        assert!(r.cancel(a.id) && a.cancel.load(Ordering::Relaxed));
        assert!(!b.cancel.load(Ordering::Relaxed));
        drop(a);
        assert!(!r.cancel(1), "finished job");
        assert!(r.start("stats").is_ok(), "kind is free again");
    }
}
