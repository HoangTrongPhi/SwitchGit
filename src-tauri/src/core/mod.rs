//! Engines: plain functions over files and git, no `tauri` imports, so the commands, the command
//! line (`cli.rs`), the guard hook and the tests all call the same code.

pub mod clients;
pub mod gitconfig;
pub mod guard;
pub mod model;
pub mod remote;
pub mod repos;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

/// Progress and cancel for one long operation. Engines report `(done, total, message)` and check
/// `cancelled()` between units of work (each repository…).
pub struct RunCtx<'a> {
    pub progress: &'a (dyn Fn(u64, u64, &str) + Sync),
    pub cancel: &'a AtomicBool,
}

impl RunCtx<'_> {
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// A context that reports nothing and is never cancelled (tests, quick calls).
pub fn quiet_ctx<R>(f: impl FnOnce(&RunCtx) -> R) -> R {
    let cancel = AtomicBool::new(false);
    f(&RunCtx { progress: &|_, _, _| {}, cancel: &cancel })
}

/// Errors that stop a whole operation. A single unreadable repository is an `Issue` instead.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{0}")]
    Input(String),
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
}

impl CoreError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        CoreError::Io { path: path.into(), source }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// Something a report wants the user to see; the job carries on past it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub severity: Severity,
    /// Stable kebab-case id, e.g. `no-scope`.
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Issue {
    pub fn new(severity: Severity, code: &str, message: impl Into<String>) -> Self {
        Issue { severity, code: code.into(), message: message.into(), path: None }
    }

    pub fn at(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_string_lossy().into_owned());
        self
    }
}

/// Issues at or above `level`.
pub fn count_at_least(issues: &[Issue], level: Severity) -> usize {
    issues.iter().filter(|i| i.severity >= level).count()
}

/// `git <args>` in `cwd`; trimmed stdout when git exits 0. No console window flashes up when the
/// app (GUI subsystem) runs it.
pub fn git(cwd: Option<&Path>, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `C:\a\b` → `C:/a/b`: the form git config and sh scripts take.
pub fn slashes(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}
