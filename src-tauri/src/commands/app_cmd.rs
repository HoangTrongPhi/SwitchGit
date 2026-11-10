use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::utils::jobs::JobRegistry;
use crate::utils::logbus::{LogBus, LogEntry};
use crate::utils::paths::write_atomic;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbyssContext {
    pub managed: bool,
    pub launcher_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub debug: bool,
    /// Whether the Abyss launcher started us (it sets `ABYSS=1`).
    pub abyss: AbyssContext,
}

#[tauri::command]
pub fn app_info<R: Runtime>(app: AppHandle<R>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        debug: cfg!(debug_assertions),
        abyss: AbyssContext { managed: std::env::var("ABYSS").is_ok_and(|v| v == "1"), launcher_version: std::env::var("ABYSS_VERSION").ok() },
    }
}

/// Entries logged before the frontend subscribed to `log://entry`.
#[tauri::command]
pub fn log_history(bus: State<'_, LogBus>) -> Vec<LogEntry> {
    bus.history()
}

/// Mirror a UI-side log line to stderr so `tauri dev` output shows frontend errors too.
/// (Not re-emitted: the UI already shows its own lines in the drawer.)
#[tauri::command]
pub fn ui_log(level: String, message: String) {
    eprintln!("[UI {level}] {message}");
}

/// Stop a running job at its next check; false when it already finished.
#[tauri::command]
pub fn cancel_job(jobs: State<'_, JobRegistry>, job_id: u64) -> bool {
    jobs.cancel(job_id)
}

/// Open a web or mail link in the default browser / mail app.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let allowed = (url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:"))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"');
    if !allowed {
        return Err(format!("Not an http(s) or mailto link: {url}"));
    }
    #[cfg(windows)]
    let result = std::process::Command::new("explorer").arg(&url).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(&url).spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
}

/// Show a file selected in Explorer / Finder / the file manager.
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Not found: {path}"));
    }
    #[cfg(windows)]
    let result = std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg("-R").arg(&p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(p.parent().unwrap_or(&p)).spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
}

/// `%LOCALAPPDATA%\<identifier>\settings.json`: app-level settings the command line reads too.
pub fn settings_path<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path().app_local_data_dir().unwrap_or_else(|_| std::env::temp_dir().join(env!("CARGO_PKG_NAME"))).join("settings.json")
}

/// Saved settings, `{}` when none were saved yet.
#[tauri::command]
pub fn get_settings<R: Runtime>(app: AppHandle<R>) -> serde_json::Value {
    std::fs::read_to_string(settings_path(&app)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| serde_json::json!({}))
}

#[tauri::command]
pub fn set_settings<R: Runtime>(app: AppHandle<R>, settings: serde_json::Value) -> Result<(), String> {
    if !settings.is_object() {
        return Err("Settings must be a JSON object".into());
    }
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    let path = settings_path(&app);
    write_atomic(&path, json.as_bytes()).map_err(|e| format!("Cannot write {}: {e}", path.display()))
}
