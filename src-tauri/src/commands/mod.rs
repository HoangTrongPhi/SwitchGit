//! Tauri IPC handlers. Each submodule wraps `core` engines; handlers stay thin and run blocking
//! work on the async runtime's blocking pool.

pub mod app_cmd;
pub mod git_cmd;

/// The JSON the frontend sends (see `src/types/switchgit.ts`) must decode into the command inputs.
#[cfg(test)]
mod contract_tests {
    use crate::core::model::{Account, Config, GuardSettings};
    use serde_json::json;

    #[test]
    fn account_from_ts() {
        let a: Account = serde_json::from_value(json!({
            "id": "", "label": "Work", "kind": "work", "name": "Phi Hoang", "email": "phi@magetechcorp.com", "enabled": true,
            "remotes": ["github.com/MageTech"], "folders": ["D:/Work"], "username": "phi-magetech", "sshKey": null
        }))
        .unwrap();
        assert_eq!((a.label.as_str(), a.enabled, a.username.as_deref(), a.ssh_key), ("Work", true, Some("phi-magetech"), None));
        assert_eq!((a.remotes.len(), a.folders.len()), (1, 1));
        // Fields the form leaves out fall back to safe values: a new account is off until asked.
        let bare: Account = serde_json::from_value(json!({ "label": "X", "email": "x@y.co" })).unwrap();
        assert!(!bare.enabled && bare.remotes.is_empty() && bare.ssh_key.is_none());
    }

    #[test]
    fn guard_settings_from_ts() {
        let g: GuardSettings = serde_json::from_value(json!({ "enabled": true, "checkCommit": true, "checkPush": false, "checkAuthors": true, "blockUnknown": true })).unwrap();
        assert!(g.enabled && !g.check_push && g.block_unknown);
        assert_eq!(serde_json::from_value::<GuardSettings>(json!({})).unwrap(), GuardSettings::default());
    }

    #[test]
    fn config_round_trips_in_camel_case() {
        let mut cfg = Config::default();
        cfg.bindings.insert("fork".into(), "personal".into());
        cfg.scan_roots.push("D:/src".into());
        let v = serde_json::to_value(&cfg).unwrap();
        assert_eq!((v["format"].as_str(), v["scanRoots"][0].as_str(), v["bindings"]["fork"].as_str()), (Some("switchgit.config"), Some("D:/src"), Some("personal")));
        assert!(v["guard"]["checkAuthors"].as_bool().unwrap());
        assert_eq!(serde_json::from_value::<Config>(v).unwrap(), cfg);
    }
}
