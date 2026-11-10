//! SwitchGit — Tauri 2 backend: IPC command registration and app bootstrap.

pub mod cli;
mod commands;
pub mod core;
pub mod utils;

#[cfg(test)]
mod ipc_tests;

use tauri::{Builder, Runtime};
use utils::jobs::JobRegistry;
use utils::logbus::{self, Level, LogBus};

/// State, startup hook and IPC commands, shared by the real app and the IPC tests (which pass
/// paths inside a temp folder).
fn configure<R: Runtime>(builder: Builder<R>, paths: core::model::Paths) -> Builder<R> {
    builder
        .manage(LogBus::default())
        .manage(JobRegistry::default())
        .manage(commands::git_cmd::Workspace::new(paths))
        .setup(|app| {
            logbus::emit(
                app.handle(),
                Level::Info,
                "app",
                format!("SwitchGit v{} started ({} {})", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_cmd::app_info,
            commands::app_cmd::log_history,
            commands::app_cmd::ui_log,
            commands::app_cmd::cancel_job,
            commands::app_cmd::open_url,
            commands::app_cmd::reveal_in_explorer,
            commands::app_cmd::get_settings,
            commands::app_cmd::set_settings,
            commands::git_cmd::get_state,
            commands::git_cmd::save_account,
            commands::git_cmd::delete_account,
            commands::git_cmd::set_account_enabled,
            commands::git_cmd::set_binding,
            commands::git_cmd::set_guard,
            commands::git_cmd::apply_config,
            commands::git_cmd::list_clients,
            commands::git_cmd::launch_client,
            commands::git_cmd::scan_repos,
            commands::git_cmd::guard_log,
            commands::git_cmd::clear_guard_log,
        ])
}

pub fn run() {
    configure(
        Builder::default()
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_fs::init())
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init()),
        core::model::Paths::from_env(),
    )
    .run(tauri::generate_context!())
    .expect("error while running SwitchGit");
}
