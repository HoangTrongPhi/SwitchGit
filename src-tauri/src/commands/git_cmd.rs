//! Accounts, client ties, the guard and the repository scan, as IPC commands. Every command that
//! changes something saves the accounts file, brings the git configuration in line with it, and
//! answers with the new [`Snapshot`], so the UI never shows a state that is not on disk.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use crate::core::clients::{self, ClientStatus};
use crate::core::gitconfig::{self, ApplyReport};
use crate::core::guard::{self, GuardEvent};
use crate::core::model::{Account, Config, GuardSettings, Paths};
use crate::core::repos::{self, ScanReport};
use crate::core::{git, CoreError, Issue, RunCtx};
use crate::utils::jobs::{progress_emitter, JobRegistry};
use crate::utils::logbus::{self, Level};

/// Event name; keep in sync with `EVENTS.repos` in `src/lib/ipc.ts`.
pub const SCAN_PROGRESS_EVENT: &str = "repos://progress";

/// Tauri state: where the files are, and a lock so two changes never interleave.
pub struct Workspace {
    pub paths: Paths,
    lock: Mutex<()>,
}

impl Workspace {
    pub fn new(paths: Paths) -> Self {
        Workspace { paths, lock: Mutex::new(()) }
    }
}

/// Everything the UI shows about the current setup.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub config: Config,
    /// The global default identity, to pre-fill the first account.
    pub global_name: Option<String>,
    pub global_email: Option<String>,
    pub git_version: Option<String>,
    pub gitconfig_path: String,
    pub data_dir: String,
    /// Files that differ from what the accounts ask for (0: everything is applied).
    pub pending: usize,
    /// The managed block as it should be in the global git config.
    pub block: String,
    pub issues: Vec<Issue>,
}

pub fn snapshot(paths: &Paths) -> Result<Snapshot, CoreError> {
    let config = Config::load(paths)?;
    let plan = gitconfig::apply(&config, paths, true)?;
    let (global_name, global_email) = gitconfig::global_identity(paths);
    Ok(Snapshot {
        config,
        global_name,
        global_email,
        git_version: git(None, &["--version"]).map(|v| v.trim_start_matches("git version ").to_string()),
        gitconfig_path: paths.gitconfig.to_string_lossy().into_owned(),
        data_dir: paths.data_dir.to_string_lossy().into_owned(),
        pending: plan.changed(),
        block: plan.block,
        issues: plan.issues,
    })
}

/// Load, change, check it can be applied, save, apply. `change` returns the log line.
pub fn mutate(paths: &Paths, change: impl FnOnce(&mut Config) -> Result<String, CoreError>) -> Result<(Snapshot, String), CoreError> {
    let mut config = Config::load(paths)?;
    let line = change(&mut config)?;
    gitconfig::apply(&config, paths, true)?; // a damaged global config stops here, before anything is saved
    config.save(paths)?;
    gitconfig::apply(&config, paths, false)?;
    Ok((snapshot(paths)?, line))
}

fn mutate_logged<R: Runtime>(app: &AppHandle<R>, ws: &Workspace, change: impl FnOnce(&mut Config) -> Result<String, CoreError>) -> Result<Snapshot, String> {
    let _guard = ws.lock.lock().unwrap_or_else(|e| e.into_inner());
    match mutate(&ws.paths, change) {
        Ok((snap, line)) => {
            logbus::emit(app, Level::Info, "accounts", line);
            Ok(snap)
        }
        Err(e) => {
            logbus::emit(app, Level::Error, "accounts", e.to_string());
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub fn get_state(ws: State<'_, Workspace>) -> Result<Snapshot, String> {
    snapshot(&ws.paths).map_err(|e| e.to_string())
}

/// Add an account (empty `id`) or replace the one with that id.
#[tauri::command]
pub fn save_account<R: Runtime>(app: AppHandle<R>, ws: State<'_, Workspace>, account: Account) -> Result<Snapshot, String> {
    mutate_logged(&app, &ws, |cfg| {
        let label = account.label.trim().to_string();
        let id = cfg.upsert(account)?;
        Ok(format!("Saved account {label} ({id})"))
    })
}

#[tauri::command]
pub fn delete_account<R: Runtime>(app: AppHandle<R>, ws: State<'_, Workspace>, id: String) -> Result<Snapshot, String> {
    mutate_logged(&app, &ws, |cfg| {
        let label = cfg.account(&id).map(|a| a.label.clone()).ok_or_else(|| CoreError::Input(format!("No account \"{id}\"")))?;
        cfg.remove(&id);
        Ok(format!("Removed account {label}"))
    })
}

/// The on/off switch of one account; the others are not touched.
#[tauri::command]
pub fn set_account_enabled<R: Runtime>(app: AppHandle<R>, ws: State<'_, Workspace>, id: String, enabled: bool) -> Result<Snapshot, String> {
    mutate_logged(&app, &ws, |cfg| {
        let account = cfg.accounts.iter_mut().find(|a| a.id == id).ok_or_else(|| CoreError::Input(format!("No account \"{id}\"")))?;
        account.enabled = enabled;
        Ok(format!("{} switched {}", account.label, if enabled { "on" } else { "off" }))
    })
}

/// Tie a git client to one account, or (no `accountId`) let it follow each repository.
#[tauri::command]
pub fn set_binding<R: Runtime>(app: AppHandle<R>, ws: State<'_, Workspace>, client: String, account_id: Option<String>) -> Result<Snapshot, String> {
    mutate_logged(&app, &ws, |cfg| {
        if client != clients::TERMINAL && clients::client_def(&client).is_none() {
            return Err(CoreError::Input(format!("Unknown git client \"{client}\"")));
        }
        let name = clients::client_name(&client);
        match account_id.filter(|id| !id.is_empty()) {
            Some(id) => {
                let label = cfg.account(&id).map(|a| a.label.clone()).ok_or_else(|| CoreError::Input(format!("No account \"{id}\"")))?;
                cfg.bindings.insert(client, id);
                Ok(format!("{name} is now tied to {label}"))
            }
            None => {
                cfg.bindings.remove(&client);
                Ok(format!("{name} follows each repository's account"))
            }
        }
    })
}

#[tauri::command]
pub fn set_guard<R: Runtime>(app: AppHandle<R>, ws: State<'_, Workspace>, guard: GuardSettings) -> Result<Snapshot, String> {
    mutate_logged(&app, &ws, |cfg| {
        let line = format!("Guard {}", if guard.enabled { "on" } else { "off" });
        cfg.guard = guard;
        Ok(line)
    })
}

/// Write the git configuration again (or, with `dryRun`, say what would be written).
#[tauri::command]
pub fn apply_config<R: Runtime>(app: AppHandle<R>, ws: State<'_, Workspace>, dry_run: Option<bool>) -> Result<ApplyReport, String> {
    let _guard = ws.lock.lock().unwrap_or_else(|e| e.into_inner());
    let dry_run = dry_run.unwrap_or(false);
    let config = Config::load(&ws.paths).map_err(|e| e.to_string())?;
    let report = gitconfig::apply(&config, &ws.paths, dry_run).map_err(|e| {
        logbus::emit(&app, Level::Error, "apply", e.to_string());
        e.to_string()
    })?;
    if !dry_run {
        logbus::emit(&app, Level::Info, "apply", format!("{} file(s) written, {} already up to date", report.changed(), report.changes.len() - report.changed()));
    }
    Ok(report)
}

#[tauri::command]
pub fn list_clients(ws: State<'_, Workspace>) -> Result<Vec<ClientStatus>, String> {
    let config = Config::load(&ws.paths).map_err(|e| e.to_string())?;
    Ok(clients::list(&config, &clients::process_table()))
}

/// Start an installed client.
#[tauri::command]
pub fn launch_client(ws: State<'_, Workspace>, id: String) -> Result<(), String> {
    let config = Config::load(&ws.paths).map_err(|e| e.to_string())?;
    let status = clients::list(&config, &[]).into_iter().find(|c| c.id == id).ok_or_else(|| format!("Unknown git client \"{id}\""))?;
    let exe = status.exe_path.ok_or_else(|| format!("{} was not found on this PC", status.name))?;
    std::process::Command::new(&exe).spawn().map(|_| ()).map_err(|e| format!("Cannot start {exe}: {e}"))
}

/// Repository scan as a job (kind `scan`): progress on `repos://progress`, cancel with `cancel_job`.
/// The scanned folders are remembered for the next visit.
#[tauri::command]
pub async fn scan_repos<R: Runtime>(app: AppHandle<R>, ws: State<'_, Workspace>, jobs: State<'_, JobRegistry>, roots: Vec<String>) -> Result<ScanReport, String> {
    let job = jobs.start("scan")?;
    let paths = ws.paths.clone();
    let config = {
        let _guard = ws.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut config = Config::load(&paths).map_err(|e| e.to_string())?;
        if config.scan_roots != roots && !roots.is_empty() {
            config.scan_roots = roots.clone();
            config.save(&paths).map_err(|e| e.to_string())?;
        }
        config
    };
    let handle = app.clone();
    let report = tauri::async_runtime::spawn_blocking(move || {
        let progress = progress_emitter(&handle, SCAN_PROGRESS_EVENT, job.id);
        let r = repos::scan(&config, &roots, &RunCtx { progress: &progress, cancel: &job.cancel });
        drop(job);
        r
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| {
        logbus::emit(&app, Level::Error, "scan", e.to_string());
        e.to_string()
    })?;
    let level = if report.cancelled { Level::Warn } else { Level::Info };
    logbus::emit(&app, level, "scan", format!("{} in {} ms{}", repos::summary(&report), report.elapsed_ms, if report.cancelled { " (cancelled)" } else { "" }));
    Ok(report)
}

/// What the guard let through and refused, newest first.
#[tauri::command]
pub fn guard_log(ws: State<'_, Workspace>, limit: Option<usize>) -> Vec<GuardEvent> {
    guard::read_log(&ws.paths, limit.unwrap_or(200).min(2000))
}

#[tauri::command]
pub fn clear_guard_log(ws: State<'_, Workspace>) -> Result<(), String> {
    guard::clear_log(&ws.paths).map_err(|e| e.to_string())
}
