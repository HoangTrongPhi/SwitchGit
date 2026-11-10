//! End-to-end IPC tests: the real command set (`configure`) on Tauri's mock runtime, invoked
//! with the same JSON the frontend stores send (`src/stores/*Store.ts` via `src/lib/ipc.ts`).
//! Every file the commands touch lives in the harness's temp folder.

use std::path::Path;
use std::sync::mpsc;

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, Manager, WebviewWindow, WebviewWindowBuilder};

use crate::commands::git_cmd::SCAN_PROGRESS_EVENT;
use crate::core::model::{testing, Paths};
use crate::utils::jobs::JobRegistry;

pub(crate) struct Harness {
    pub app: App<MockRuntime>,
    webview: WebviewWindow<MockRuntime>,
    pub paths: Paths,
    _dir: tempfile::TempDir,
}

#[allow(dead_code)]
impl Harness {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let paths = testing::paths(dir.path());
        let app = super::configure(mock_builder(), paths.clone()).build(mock_context(noop_assets())).expect("build mock app");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default()).build().expect("main window");
        Harness { app, webview, paths, _dir: dir }
    }

    /// `invoke(cmd, args)` exactly as `@tauri-apps/api/core` would send it.
    pub fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        self.invoke_body(cmd, args).map(|body| body.deserialize::<Value>().expect("JSON response"))
    }

    fn invoke_body(&self, cmd: &str, args: Value) -> Result<InvokeResponseBody, Value> {
        let url = if cfg!(windows) { "http://tauri.localhost" } else { "tauri://localhost" };
        get_ipc_response(
            &self.webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: url.parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
    }

    pub fn ok(&self, cmd: &str, args: Value) -> Value {
        self.invoke(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
    }

    /// Collect payloads of a backend event.
    pub fn listen(&self, event: &str) -> mpsc::Receiver<Value> {
        let (tx, rx) = mpsc::channel();
        self.app.listen_any(event, move |e| {
            let _ = tx.send(serde_json::from_str(e.payload()).unwrap_or(Value::Null));
        });
        rx
    }

    fn add(&self, label: &str, email: &str, remote: &str) -> Value {
        self.ok("save_account", json!({ "account": { "id": "", "label": label, "kind": label.to_lowercase(), "name": "Phi Hoang", "email": email, "enabled": true, "remotes": [remote], "folders": [], "username": null, "sshKey": null } }))
    }
}

pub(crate) fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn app_info_log_and_settings() {
    let h = Harness::new();
    let info = h.ok("app_info", json!({}));
    assert!(info["version"].as_str().is_some_and(|v| !v.is_empty()));
    assert_eq!(info["debug"], cfg!(debug_assertions));
    assert!(info["abyss"]["managed"].is_boolean());
    // A command logs, then the UI backfills it.
    h.add("Work", "phi@magetechcorp.com", "github.com/MageTech");
    let logs = h.ok("log_history", json!({}));
    assert!(logs.as_array().unwrap().iter().any(|l| l["source"] == "accounts" && l["level"] == "info"));
    assert_eq!(h.ok("ui_log", json!({ "level": "warn", "message": "from the UI" })), Value::Null);
    assert!(h.invoke("open_url", json!({ "url": "file:///C:/x" })).is_err());
    assert!(h.invoke("set_settings", json!({ "settings": [1] })).is_err(), "object only");
    assert!(h.ok("get_settings", json!({})).is_object());
}

#[test]
fn accounts_switch_independently_and_apply() {
    let h = Harness::new();
    let empty = h.ok("get_state", json!({}));
    assert_eq!((empty["config"]["accounts"].as_array().map(Vec::len), empty["pending"].as_u64(), empty["block"].as_str()), (Some(0), Some(0), Some("")));

    h.add("Work", "phi@magetechcorp.com", "github.com/MageTech");
    let state = h.add("Personal", "phi.personal@example.com", "github.com/HoangTrongPhi");
    assert_eq!(state["config"]["accounts"][1]["id"], "personal");
    assert_eq!(state["pending"], 0, "saving applies");
    assert!(state["block"].as_str().unwrap().contains("HoangTrongPhi"));
    let gitconfig = || std::fs::read_to_string(&h.paths.gitconfig).unwrap();
    assert!(gitconfig().contains("MageTech") && gitconfig().contains("HoangTrongPhi") && gitconfig().contains("hooksPath"));
    assert!(h.paths.hooks_dir().join("pre-push").is_file());

    // One switch: only that account leaves the git configuration.
    let off = h.ok("set_account_enabled", json!({ "id": "personal", "enabled": false }));
    assert_eq!((off["config"]["accounts"][0]["enabled"].as_bool(), off["config"]["accounts"][1]["enabled"].as_bool()), (Some(true), Some(false)));
    assert!(gitconfig().contains("MageTech") && !gitconfig().contains("HoangTrongPhi"));
    assert!(h.paths.account_file("work").is_file() && !h.paths.account_file("personal").exists());
    h.ok("set_account_enabled", json!({ "id": "personal", "enabled": true }));
    assert!(gitconfig().contains("HoangTrongPhi"));

    // Editing by id; errors come back as text and change nothing.
    let mut work = state["config"]["accounts"][0].clone();
    work["folders"] = json!(["D:/Work"]);
    let edited = h.ok("save_account", json!({ "account": work }));
    assert_eq!((edited["config"]["accounts"].as_array().map(Vec::len), edited["config"]["accounts"][0]["folders"][0].as_str()), (Some(2), Some("D:/Work")));
    let err = h.invoke("save_account", json!({ "account": { "label": "Dup", "name": "x", "email": "phi@magetechcorp.com" } })).unwrap_err();
    assert!(err.as_str().unwrap().contains("already uses"), "{err}");
    assert!(h.invoke("set_account_enabled", json!({ "id": "nobody", "enabled": true })).is_err());

    // Hand edits to the files show up as pending; Apply (after a dry run) repairs them.
    std::fs::remove_file(h.paths.account_file("work")).unwrap();
    assert_eq!(h.ok("get_state", json!({}))["pending"], 1);
    let dry = h.ok("apply_config", json!({ "dryRun": true }));
    assert_eq!((dry["dryRun"].as_bool(), dry["changes"].as_array().unwrap().iter().filter(|c| c["action"] == "create").count()), (Some(true), 1));
    assert!(!h.paths.account_file("work").exists());
    let applied = h.ok("apply_config", json!({}));
    assert_eq!((applied["dryRun"].as_bool(), applied["ok"].as_bool()), (Some(false), Some(true)));
    assert_eq!(h.ok("get_state", json!({}))["pending"], 0);

    let gone = h.ok("delete_account", json!({ "id": "work" }));
    assert_eq!(gone["config"]["accounts"].as_array().map(Vec::len), Some(1));
    assert!(!gitconfig().contains("MageTech") && h.invoke("delete_account", json!({ "id": "work" })).is_err());
}

#[test]
fn bindings_guard_settings_and_log() {
    let h = Harness::new();
    h.add("Work", "phi@magetechcorp.com", "github.com/MageTech");
    h.add("Personal", "phi.personal@example.com", "github.com/HoangTrongPhi");

    let bound = h.ok("set_binding", json!({ "client": "fork", "accountId": "personal" }));
    assert_eq!(bound["config"]["bindings"]["fork"], "personal");
    h.ok("set_binding", json!({ "client": "sourcetree", "accountId": "work" }));
    h.ok("set_binding", json!({ "client": "terminal", "accountId": "work" }));
    let clients = h.ok("list_clients", json!({}));
    let clients = clients.as_array().unwrap();
    let get = |id: &str| clients.iter().find(|c| c["id"] == id).unwrap_or_else(|| panic!("{id}"));
    assert_eq!((get("fork")["accountId"].as_str(), get("sourcetree")["accountId"].as_str(), get("github-desktop")["accountId"].as_str()), (Some("personal"), Some("work"), None));
    assert!(get("fork")["installed"].is_boolean() && get("fork")["note"].as_str().is_some_and(|n| !n.is_empty()));
    assert_eq!(clients.last().unwrap()["id"], "terminal");

    let freed = h.ok("set_binding", json!({ "client": "fork", "accountId": null }));
    assert!(freed["config"]["bindings"].get("fork").is_none());
    assert!(h.invoke("set_binding", json!({ "client": "notepad", "accountId": "work" })).is_err());
    assert!(h.invoke("set_binding", json!({ "client": "fork", "accountId": "nobody" })).is_err());
    assert!(h.invoke("launch_client", json!({ "id": "notepad" })).is_err());
    // Removing an account frees the clients tied to it.
    let after = h.ok("delete_account", json!({ "id": "work" }));
    assert_eq!(after["config"]["bindings"], json!({}));

    // Guard off: the hooks go, the identities stay.
    let off = h.ok("set_guard", json!({ "guard": { "enabled": false, "checkCommit": true, "checkPush": true, "checkAuthors": true, "blockUnknown": false } }));
    assert_eq!(off["config"]["guard"]["enabled"], false);
    assert!(!h.paths.hooks_dir().join("pre-push").exists());
    let text = std::fs::read_to_string(&h.paths.gitconfig).unwrap();
    assert!(!text.contains("hooksPath") && text.contains("HoangTrongPhi"));

    assert_eq!(h.ok("guard_log", json!({})), json!([]));
    crate::core::guard::log_event(&h.paths, &crate::core::guard::GuardEvent { ts_ms: 7, hook: "push".into(), allow: false, code: "client-bound".into(), ..Default::default() }).unwrap();
    let log = h.ok("guard_log", json!({ "limit": 5 }));
    assert_eq!((log[0]["tsMs"].as_u64(), log[0]["allow"].as_bool(), log[0]["code"].as_str()), (Some(7), Some(false), Some("client-bound")));
    assert_eq!(h.ok("clear_guard_log", json!({})), Value::Null);
    assert_eq!(h.ok("guard_log", json!({})), json!([]));
}

#[test]
fn scan_reports_progress_and_one_job_per_kind() {
    let h = Harness::new();
    h.add("Work", "phi@magetechcorp.com", "github.com/MageTech");
    let progress = h.listen(SCAN_PROGRESS_EVENT);
    let root = h.paths.data_dir.parent().unwrap().join("src");
    std::fs::create_dir_all(root.join("plain/folder")).unwrap();

    let r = h.ok("scan_repos", json!({ "roots": [s(&root)] }));
    assert_eq!((r["repos"].as_array().map(Vec::len), r["ok"].as_bool(), r["cancelled"].as_bool()), (Some(0), Some(true), Some(false)));
    assert!(r["elapsedMs"].is_u64() && r["roots"][0].as_str().is_some());
    let events: Vec<Value> = progress.try_iter().collect();
    assert!(events.len() >= 2 && events[0]["jobId"].as_u64().unwrap() > 0, "{events:?}");
    assert_eq!(h.ok("get_state", json!({}))["config"]["scanRoots"][0], s(&root), "roots are remembered");

    assert!(h.invoke("scan_repos", json!({ "roots": [] })).unwrap_err().as_str().unwrap().starts_with("Choose a folder"));
    let missing = h.ok("scan_repos", json!({ "roots": [s(&root.join("missing"))] }));
    assert_eq!((missing["ok"].as_bool(), missing["issues"][0]["code"].as_str()), (Some(false), Some("folder-not-found")));

    let jobs = h.app.state::<JobRegistry>();
    let running = jobs.start("scan").unwrap();
    assert_eq!(h.invoke("scan_repos", json!({ "roots": [s(&root)] })).unwrap_err(), "Scan is already running");
    assert_eq!(h.ok("cancel_job", json!({ "jobId": running.id })), true);
    drop(running);
    assert_eq!(h.ok("cancel_job", json!({ "jobId": 999 })), false);
    assert!(h.invoke("scan_repos", json!({ "roots": [s(&root)] })).is_ok());
}
