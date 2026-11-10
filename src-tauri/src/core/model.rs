//! The accounts file (`switchgit.json`) and where SwitchGit keeps its files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::CoreError;
use crate::utils::paths::write_atomic;

pub const FORMAT: &str = "switchgit.config";
pub const VERSION: u32 = 1;
/// Tauri's `app_local_data_dir()` for this app; the command line and the hook use the same folder.
pub const IDENTIFIER: &str = "com.phineas.switchgit";

/// Every location SwitchGit reads or writes. Tests point all of them into a temp folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Paths {
    /// `%LOCALAPPDATA%\com.phineas.switchgit` (or `SWITCHGIT_HOME`).
    pub data_dir: PathBuf,
    /// The user's global git config (`GIT_CONFIG_GLOBAL`, else `~/.gitconfig`).
    pub gitconfig: PathBuf,
    /// The exe the hooks call.
    pub exe: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        let home = var("USERPROFILE").or_else(|| var("HOME")).unwrap_or_else(std::env::temp_dir);
        let data_dir = var("SWITCHGIT_HOME").unwrap_or_else(|| var("LOCALAPPDATA").unwrap_or_else(|| home.join(".local/share")).join(IDENTIFIER));
        let gitconfig = var("SWITCHGIT_GITCONFIG").or_else(|| var("GIT_CONFIG_GLOBAL")).unwrap_or_else(|| home.join(".gitconfig"));
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("switchgit.exe"));
        Paths { data_dir, gitconfig, exe }
    }

    pub fn config_file(&self) -> PathBuf {
        self.data_dir.join("switchgit.json")
    }
    pub fn accounts_dir(&self) -> PathBuf {
        self.data_dir.join("accounts")
    }
    pub fn account_file(&self, id: &str) -> PathBuf {
        self.accounts_dir().join(format!("{id}.gitconfig"))
    }
    pub fn hooks_dir(&self) -> PathBuf {
        self.data_dir.join("hooks")
    }
    pub fn guard_log(&self) -> PathBuf {
        self.data_dir.join("guard.log")
    }
}

/// One git identity. Accounts are independent: each has its own switch, scope and credentials.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Account {
    /// Stable slug (`work`, `personal`); file names and bindings use it.
    pub id: String,
    pub label: String,
    /// `work`, `personal` or `other`: only a badge in the UI.
    pub kind: String,
    /// `user.name` / `user.email` of commits made as this account.
    pub name: String,
    pub email: String,
    /// The switch. Off: its repositories get no identity and the guard refuses commits and pushes.
    pub enabled: bool,
    /// Remotes this account owns: `github.com/Owner`, `github.com/Owner/repo`, a whole host.
    pub remotes: Vec<String>,
    /// Folders whose repositories belong to this account even without a matching remote.
    pub folders: Vec<String>,
    /// Login for HTTPS pushes (`credential.<host>.username`), so the credential manager picks this
    /// account's stored credential.
    pub username: Option<String>,
    /// Private key for SSH remotes (`core.sshCommand … -i <key> -o IdentitiesOnly=yes`).
    pub ssh_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GuardSettings {
    /// Install the global hooks at all.
    pub enabled: bool,
    pub check_commit: bool,
    pub check_push: bool,
    /// Refuse a push carrying commits authored as another account.
    pub check_authors: bool,
    /// Refuse repositories no account owns (off: they pass untouched).
    pub block_unknown: bool,
}

impl Default for GuardSettings {
    fn default() -> Self {
        GuardSettings { enabled: true, check_commit: true, check_push: true, check_authors: true, block_unknown: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub format: String,
    pub version: u32,
    pub accounts: Vec<Account>,
    /// Git client id (`fork`, `sourcetree`, …) → the only account it may commit and push as.
    pub bindings: BTreeMap<String, String>,
    pub guard: GuardSettings,
    /// Folders the Repositories tab scans.
    pub scan_roots: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config { format: FORMAT.into(), version: VERSION, accounts: Vec::new(), bindings: BTreeMap::new(), guard: GuardSettings::default(), scan_roots: Vec::new() }
    }
}

/// `"Work (MageTech)"` → `work-magetech`.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

fn clean_list(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in list.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if !out.iter().any(|o| o.eq_ignore_ascii_case(item)) {
            out.push(item.to_string());
        }
    }
    out
}

impl Config {
    pub fn load(paths: &Paths) -> Result<Config, CoreError> {
        let file = paths.config_file();
        match std::fs::read_to_string(&file) {
            Ok(text) => {
                let mut cfg: Config = serde_json::from_str(&text).map_err(|e| CoreError::Input(format!("{} is not a valid accounts file: {e}", file.display())))?;
                if cfg.format != FORMAT {
                    return Err(CoreError::Input(format!("{} is not a SwitchGit accounts file (format \"{}\")", file.display(), cfg.format)));
                }
                cfg.version = VERSION;
                Ok(cfg)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(CoreError::io(file, e)),
        }
    }

    pub fn save(&self, paths: &Paths) -> Result<(), CoreError> {
        let file = paths.config_file();
        let json = serde_json::to_string_pretty(self).map_err(|e| CoreError::Input(e.to_string()))?;
        write_atomic(&file, format!("{json}\n").as_bytes()).map_err(|e| CoreError::io(file, e))
    }

    pub fn account(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// The account commits with this e-mail are made as.
    pub fn account_by_email(&self, email: &str) -> Option<&Account> {
        let email = email.trim();
        self.accounts.iter().find(|a| !a.email.is_empty() && a.email.eq_ignore_ascii_case(email))
    }

    /// Add or replace an account (matched by id; an empty id makes a new one from the label).
    /// Returns the id it was saved under.
    pub fn upsert(&mut self, mut account: Account) -> Result<String, CoreError> {
        account.label = account.label.trim().to_string();
        account.name = account.name.trim().to_string();
        account.email = account.email.trim().to_string();
        account.remotes = clean_list(&account.remotes);
        account.folders = clean_list(&account.folders);
        account.username = account.username.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        account.ssh_key = account.ssh_key.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        if !matches!(account.kind.as_str(), "work" | "personal" | "other") {
            account.kind = "other".into();
        }
        if account.label.is_empty() {
            return Err(CoreError::Input("Give the account a name".into()));
        }
        if account.name.is_empty() {
            return Err(CoreError::Input("The commit author name is empty".into()));
        }
        let email_ok = account.email.split_once('@').is_some_and(|(a, b)| !a.is_empty() && b.contains('.')) && !account.email.contains(char::is_whitespace);
        if !email_ok {
            return Err(CoreError::Input(format!("\"{}\" is not an e-mail address", account.email)));
        }
        for text in [&account.name, &account.email].into_iter().chain(&account.remotes).chain(&account.folders).chain(&account.username).chain(&account.ssh_key) {
            if text.contains(['\n', '\r', '"']) {
                return Err(CoreError::Input(format!("Line breaks and double quotes are not allowed: {text}")));
            }
        }
        for r in &account.remotes {
            if super::remote::normalize_pattern(r).is_empty() {
                return Err(CoreError::Input(format!("\"{r}\" is not a remote such as github.com/Owner")));
            }
        }
        if let Some(other) = self.accounts.iter().find(|a| a.id != account.id && a.email.eq_ignore_ascii_case(&account.email)) {
            return Err(CoreError::Input(format!("{} already uses {}: two accounts with one e-mail cannot be told apart", other.label, account.email)));
        }
        if let Some(i) = self.accounts.iter().position(|a| !account.id.is_empty() && a.id == account.id) {
            self.accounts[i] = account.clone();
            return Ok(account.id);
        }
        let base = Some(slug(&account.label)).filter(|s| !s.is_empty()).unwrap_or_else(|| "account".into());
        let mut id = base.clone();
        let mut n = 2;
        while self.accounts.iter().any(|a| a.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        account.id = id.clone();
        self.accounts.push(account);
        Ok(id)
    }

    /// Remove an account and the client bindings that pointed at it.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.accounts.len();
        self.accounts.retain(|a| a.id != id);
        self.bindings.retain(|_, v| v != id);
        self.accounts.len() != before
    }
}

/// True when `path` is `folder` or inside it (case-insensitive, either slash).
pub fn path_within(folder: &str, path: &Path) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let (f, p) = (norm(folder), norm(&path.to_string_lossy()));
    !f.is_empty() && (p == f || p.starts_with(&format!("{f}/")))
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    pub fn paths(dir: &Path) -> Paths {
        Paths { data_dir: dir.join("data"), gitconfig: dir.join("home/.gitconfig"), exe: dir.join("bin/switchgit.exe") }
    }

    pub fn account(label: &str, email: &str, remote: &str) -> Account {
        Account { label: label.into(), kind: slug(label), name: "Phi Hoang".into(), email: email.into(), enabled: true, remotes: vec![remote.into()], ..Default::default() }
    }

    /// Work owns github.com/MageTech, Personal owns github.com/HoangTrongPhi.
    pub fn two_accounts() -> Config {
        let mut cfg = Config::default();
        cfg.upsert(account("Work", "phi@magetechcorp.com", "github.com/MageTech")).unwrap();
        cfg.upsert(account("Personal", "phi.personal@example.com", "github.com/HoangTrongPhi")).unwrap();
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn upsert_validates_and_makes_ids() {
        let mut cfg = two_accounts();
        assert_eq!(cfg.accounts.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["work", "personal"]);
        assert_eq!(cfg.upsert(account("Work", "second@example.com", "gitlab.com/team")).unwrap(), "work-2");
        assert!(cfg.upsert(account("", "a@b.co", "github.com/x")).is_err(), "no label");
        assert!(cfg.upsert(account("X", "not-an-email", "github.com/x")).is_err());
        assert!(cfg.upsert(account("X", "PHI@magetechcorp.com", "github.com/x")).is_err(), "e-mail taken");
        assert!(cfg.upsert(account("X", "x@y.co", "just words")).is_err(), "not a remote");
        assert!(cfg.upsert(Account { name: "a\"b".into(), ..account("X", "x@y.co", "github.com/x") }).is_err(), "quote");

        // Editing keeps the id; lists are trimmed and de-duplicated.
        let mut work = cfg.account("work").unwrap().clone();
        work.remotes = vec![" github.com/MageTech ".into(), "github.com/magetech".into(), "".into()];
        work.username = Some("  ".into());
        assert_eq!(cfg.upsert(work).unwrap(), "work");
        let work = cfg.account("work").unwrap();
        assert_eq!((work.remotes.as_slice(), &work.username), (["github.com/MageTech".to_string()].as_slice(), &None));
        assert_eq!(cfg.account_by_email("Phi@MageTechCorp.com").map(|a| a.id.as_str()), Some("work"));
    }

    #[test]
    fn remove_drops_bindings() {
        let mut cfg = two_accounts();
        cfg.bindings.insert("fork".into(), "personal".into());
        cfg.bindings.insert("sourcetree".into(), "work".into());
        assert!(cfg.remove("personal") && !cfg.remove("personal"));
        assert_eq!(cfg.bindings.keys().collect::<Vec<_>>(), ["sourcetree"]);
    }

    #[test]
    fn save_load_round_trip_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        assert_eq!(Config::load(&p).unwrap(), Config::default(), "missing file");
        let mut cfg = two_accounts();
        cfg.guard.block_unknown = true;
        cfg.save(&p).unwrap();
        assert_eq!(Config::load(&p).unwrap(), cfg);
        // Missing fields take their defaults; a foreign file is refused, never overwritten silently.
        std::fs::write(p.config_file(), r#"{ "format": "switchgit.config", "accounts": [{ "id": "a", "email": "a@b.co" }] }"#).unwrap();
        let loaded = Config::load(&p).unwrap();
        assert!(loaded.guard.enabled && loaded.guard.check_authors && !loaded.accounts[0].enabled);
        std::fs::write(p.config_file(), r#"{ "format": "other.thing" }"#).unwrap();
        assert!(Config::load(&p).is_err());
    }

    #[test]
    fn slugs_and_folders() {
        assert_eq!(slug("  Work (MageTech) "), "work-magetech");
        assert!(path_within("D:\\Work\\", Path::new("d:/work/game")) && path_within("D:/Work", Path::new("D:/Work")));
        assert!(!path_within("D:/Work", Path::new("D:/Workshop")) && !path_within("", Path::new("D:/x")));
    }
}
