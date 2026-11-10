//! Turn the accounts into git configuration.
//!
//! - One `accounts/<id>.gitconfig` per account that is switched on (identity, HTTPS login, SSH key).
//! - A managed block at the end of the global git config with an `includeIf` per folder and remote
//!   of those accounts, so every repository picks its own identity, plus `core.hooksPath` for the guard.
//! - `hooks/<name>`: the guard for `pre-commit` / `pre-push`, and for every hook a hand-over to the
//!   repository's own `.git/hooks` (which `core.hooksPath` would otherwise switch off — Git LFS lives there).
//!
//! Nothing outside the block is touched; the first write keeps the original as `.gitconfig.switchgit.bak`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use super::model::{Account, Config, Paths};
use super::{remote, slashes, CoreError, Issue, Severity};
use crate::utils::paths::write_atomic;

pub const BLOCK_START: &str = "# >>> switchgit >>>";
pub const BLOCK_END: &str = "# <<< switchgit <<<";
pub const HOOK_MARKER: &str = "switchgit-managed";

/// Client-side hooks git may look for. With `core.hooksPath` set, a hook missing here would never
/// reach the repository's own copy.
pub const HOOKS: &[&str] = &[
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-rebase",
    "post-checkout",
    "post-merge",
    "pre-push",
    "post-rewrite",
    "pre-auto-gc",
    "push-to-checkout",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    /// `create`, `update`, `remove` or `unchanged`.
    pub action: String,
    /// `gitconfig`, `account` or `hook`.
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub dry_run: bool,
    pub changes: Vec<FileChange>,
    /// The managed block as it is (or would be) written; empty when there is nothing to manage.
    pub block: String,
    /// Where the untouched original went, the first time the global config was changed.
    pub backup: Option<String>,
    pub issues: Vec<Issue>,
    pub elapsed_ms: u64,
    pub ok: bool,
    pub cancelled: bool,
}

impl ApplyReport {
    /// Files that are (or would be) written or removed.
    pub fn changed(&self) -> usize {
        self.changes.iter().filter(|c| c.action != "unchanged").count()
    }
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\"))
}

/// `accounts/<id>.gitconfig`.
pub fn account_file_text(a: &Account) -> String {
    let mut s = format!("# SwitchGit account \"{}\" — written by SwitchGit, changes here are lost.\n[user]\n\tname = {}\n\temail = {}\n", a.label, quoted(&a.name), quoted(&a.email));
    if let Some(user) = &a.username {
        let mut hosts: Vec<String> = a.remotes.iter().filter_map(|r| remote::host_of(r)).collect();
        hosts.sort();
        hosts.dedup();
        for host in hosts {
            s.push_str(&format!("[credential \"https://{host}\"]\n\tusername = {}\n", quoted(user)));
        }
    }
    if let Some(key) = &a.ssh_key {
        let key = key.replace('\\', "/");
        s.push_str(&format!("[core]\n\tsshCommand = \"ssh -i \\\"{key}\\\" -o IdentitiesOnly=yes\"\n"));
    }
    s
}

/// The managed block (without line-ending conversion); `None` when nothing needs managing.
pub fn block_text(cfg: &Config, paths: &Paths) -> Option<String> {
    let on: Vec<&Account> = cfg.accounts.iter().filter(|a| a.enabled).collect();
    let hooks = cfg.guard.enabled && !cfg.accounts.is_empty();
    if on.is_empty() && !hooks {
        return None;
    }
    let mut s = format!("{BLOCK_START} managed by SwitchGit: edit accounts in the app, not here\n");
    // Folders first, remotes after: git lets the last match win, and a remote says more than a folder.
    for a in &on {
        let file = quoted(&slashes(&paths.account_file(&a.id)));
        for folder in &a.folders {
            let folder = folder.replace('\\', "/");
            s.push_str(&format!("[includeIf \"gitdir/i:{}/\"]\n\tpath = {file}\n", folder.trim_end_matches('/')));
        }
    }
    for a in &on {
        let file = quoted(&slashes(&paths.account_file(&a.id)));
        for glob in a.remotes.iter().flat_map(|r| remote::include_globs(r)) {
            s.push_str(&format!("[includeIf \"hasconfig:remote.*.url:{glob}\"]\n\tpath = {file}\n"));
        }
    }
    if hooks {
        s.push_str(&format!("[core]\n\thooksPath = {}\n", quoted(&slashes(&paths.hooks_dir()))));
    }
    s.push_str(BLOCK_END);
    s.push('\n');
    Some(s)
}

/// One script for every hook name: guard first (`pre-commit`, `pre-push`), then whatever hook the
/// repository (or a global hooks folder that was there before) has under the same name.
pub fn hook_text(paths: &Paths, foreign_hooks: Option<&str>) -> String {
    let exe = slashes(&paths.exe).replace('\'', "'\\''");
    let foreign = foreign_hooks.map(|f| f.replace('\\', "/").replace('\'', "'\\''")).unwrap_or_default();
    format!(
        r#"#!/bin/sh
# {HOOK_MARKER}: written by SwitchGit, changes here are lost.
hook=$(basename "$0")
exe='{exe}'
input=''
if [ "$hook" = pre-push ]; then input=$(cat); fi
feed() {{ if [ -n "$input" ]; then printf '%s\n' "$input"; fi; }}

case "$hook" in
  pre-commit|pre-push)
    # A missing exe (SwitchGit uninstalled) never blocks git.
    if [ -x "$exe" ]; then
      feed | "$exe" guard "$hook" "$@" || exit $?
    fi
    ;;
esac

for dir in '{foreign}' "$(git rev-parse --git-common-dir 2>/dev/null)/hooks"; do
  if [ -n "$dir" ] && [ "$dir" != /hooks ] && [ -x "$dir/$hook" ]; then
    feed | "$dir/$hook" "$@" || exit $?
  fi
done
exit 0
"#
    )
}

struct Scan {
    /// Text before and after the block, and the block itself.
    outside: String,
    has_block: bool,
    foreign_hooks: Option<String>,
    default_email: Option<String>,
}

/// Split the global config around the managed block and read the two settings outside it that
/// matter here.
fn scan(text: &str, file: &Path) -> Result<Scan, CoreError> {
    let mut outside = String::new();
    let (mut inside, mut has_block) = (false, false);
    let (mut section, mut foreign_hooks, mut default_email) = (String::new(), None, None);
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        if t.starts_with(BLOCK_START) {
            if inside || has_block {
                return Err(CoreError::Input(format!("{} has more than one SwitchGit block; remove the extra one by hand", file.display())));
            }
            (inside, has_block) = (true, true);
            continue;
        }
        if inside {
            inside = !t.starts_with(BLOCK_END);
            continue;
        }
        outside.push_str(line);
        if t.starts_with('[') {
            section = t.trim_matches(['[', ']']).trim().to_lowercase();
        } else if let Some((key, value)) = t.split_once('=') {
            let value = value.trim().trim_matches('"').to_string();
            match (section.as_str(), key.trim().to_lowercase().as_str()) {
                ("core", "hookspath") => foreign_hooks = Some(value),
                ("user", "email") => default_email = Some(value),
                _ => {}
            }
        }
    }
    if inside {
        return Err(CoreError::Input(format!("{}: the SwitchGit block has no end line ({BLOCK_END}); fix or remove it by hand", file.display())));
    }
    Ok(Scan { outside, has_block, foreign_hooks, default_email })
}

fn with_block(outside: &str, block: Option<&str>, crlf: bool) -> String {
    let mut out = outside.to_string();
    if let Some(block) = block {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(if crlf { "\r\n" } else { "\n" });
        }
        out.push_str(&if crlf { block.replace('\n', "\r\n") } else { block.to_string() });
    }
    out
}

struct Planned {
    path: PathBuf,
    kind: &'static str,
    /// `None`: the file must not exist.
    content: Option<String>,
}

fn action(p: &Planned) -> &'static str {
    match (std::fs::read(&p.path).ok(), &p.content) {
        (None, None) => "unchanged",
        (None, Some(_)) => "create",
        (Some(_), None) => "remove",
        (Some(old), Some(new)) if old == new.as_bytes() => "unchanged",
        (Some(_), Some(_)) => "update",
    }
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir).map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_file()).collect()).unwrap_or_default()
}

/// Bring the files in line with `cfg`. With `dry_run` nothing is written and the report says what
/// would be.
pub fn apply(cfg: &Config, paths: &Paths, dry_run: bool) -> Result<ApplyReport, CoreError> {
    let started = Instant::now();
    let mut issues = Vec::new();

    let current = match std::fs::read_to_string(&paths.gitconfig) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(CoreError::io(&paths.gitconfig, e)),
    };
    let scanned = scan(&current, &paths.gitconfig)?;
    let block = block_text(cfg, paths);
    let hooks_on = cfg.guard.enabled && !cfg.accounts.is_empty();

    let mut planned: Vec<Planned> = Vec::new();
    for a in &cfg.accounts {
        planned.push(Planned { path: paths.account_file(&a.id), kind: "account", content: a.enabled.then(|| account_file_text(a)) });
    }
    // Files of accounts that no longer exist.
    for stale in files_in(&paths.accounts_dir()) {
        if stale.extension().is_some_and(|e| e == "gitconfig") && !planned.iter().any(|p| p.path == stale) {
            planned.push(Planned { path: stale, kind: "account", content: None });
        }
    }
    let hook = hook_text(paths, scanned.foreign_hooks.as_deref());
    for name in HOOKS {
        planned.push(Planned { path: paths.hooks_dir().join(name), kind: "hook", content: hooks_on.then(|| hook.clone()) });
    }
    let new_gitconfig = with_block(&scanned.outside, block.as_deref(), current.contains("\r\n"));
    // An empty config that never existed stays absent.
    let gitconfig_content = (!new_gitconfig.is_empty() || paths.gitconfig.exists()).then_some(new_gitconfig);
    planned.push(Planned { path: paths.gitconfig.clone(), kind: "gitconfig", content: gitconfig_content });

    // ---- what the user should know
    for a in cfg.accounts.iter().filter(|a| a.remotes.is_empty() && a.folders.is_empty()) {
        issues.push(Issue::new(Severity::Warn, "no-scope", format!("{} owns no remote and no folder, so no repository will use it", a.label)));
    }
    if cfg.accounts.iter().any(|a| !a.enabled) && !hooks_on {
        issues.push(Issue::new(Severity::Warn, "off-without-guard", "The guard is off: a switched-off account only loses its identity, nothing refuses its pushes"));
    }
    if let Some(dir) = scanned.foreign_hooks.as_deref().filter(|_| hooks_on) {
        issues.push(Issue::new(Severity::Info, "foreign-hooks-path", format!("core.hooksPath was already set; its hooks still run after the guard ({dir})")).at(dir));
    }
    if let Some(email) = &scanned.default_email {
        let who = cfg.account_by_email(email).map(|a| format!(" ({})", a.label)).unwrap_or_default();
        issues.push(Issue::new(Severity::Info, "global-default", format!("Repositories no account owns commit as the global default {email}{who}")));
    }
    for a in cfg.accounts.iter().filter(|a| a.enabled) {
        if let Some(key) = a.ssh_key.as_deref().filter(|k| !Path::new(k).is_file()) {
            issues.push(Issue::new(Severity::Warn, "ssh-key-missing", format!("{}: the SSH key file does not exist", a.label)).at(key));
        }
    }

    // ---- write
    let mut changes = Vec::new();
    let mut backup = None;
    for p in &planned {
        let act = action(p);
        changes.push(FileChange { path: p.path.to_string_lossy().into_owned(), action: act.into(), kind: p.kind.into() });
        if dry_run || act == "unchanged" {
            continue;
        }
        if p.kind == "gitconfig" && !scanned.has_block && !current.is_empty() {
            let bak = PathBuf::from(format!("{}.switchgit.bak", p.path.display()));
            if !bak.exists() {
                std::fs::write(&bak, &current).map_err(|e| CoreError::io(&bak, e))?;
                backup = Some(bak.to_string_lossy().into_owned());
            }
        }
        match &p.content {
            Some(text) => write_atomic(&p.path, text.as_bytes()).map_err(|e| CoreError::io(&p.path, e))?,
            None => std::fs::remove_file(&p.path).map_err(|e| CoreError::io(&p.path, e))?,
        }
    }

    let ok = !issues.iter().any(|i| i.severity == Severity::Error);
    Ok(ApplyReport { dry_run, changes, block: block.unwrap_or_default(), backup, issues, elapsed_ms: started.elapsed().as_millis() as u64, ok, cancelled: false })
}

/// The global default identity (`user.name`, `user.email` outside the managed block), to pre-fill
/// the first account.
pub fn global_identity(paths: &Paths) -> (Option<String>, Option<String>) {
    let text = std::fs::read_to_string(&paths.gitconfig).unwrap_or_default();
    let (mut section, mut name, mut email, mut inside) = (String::new(), None, None, false);
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with(BLOCK_START) || t.starts_with(BLOCK_END) {
            inside = t.starts_with(BLOCK_START);
        } else if inside {
        } else if t.starts_with('[') {
            section = t.trim_matches(['[', ']']).trim().to_lowercase();
        } else if let Some((k, v)) = t.split_once('=').filter(|_| section == "user") {
            let v = v.trim().trim_matches('"').to_string();
            match k.trim().to_lowercase().as_str() {
                "name" => name = Some(v),
                "email" => email = Some(v),
                _ => {}
            }
        }
    }
    (name, email)
}

#[cfg(test)]
mod tests {
    use super::super::model::testing::*;
    use super::*;

    const ORIGINAL: &str = "[core]\r\n\tautocrlf = true\r\n[user]\r\n\tname = Phi Hoang\r\n\temail = phi@magetechcorp.com\r\n[filter \"lfs\"]\r\n\trequired = true\r\n";

    fn setup(original: Option<&str>) -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        if let Some(text) = original {
            std::fs::create_dir_all(p.gitconfig.parent().unwrap()).unwrap();
            std::fs::write(&p.gitconfig, text).unwrap();
        }
        (dir, p)
    }

    #[test]
    fn dry_run_writes_nothing() {
        let (_d, p) = setup(Some(ORIGINAL));
        let r = apply(&two_accounts(), &p, true).unwrap();
        assert!(r.dry_run && r.ok && r.changed() == 2 + HOOKS.len() + 1, "{:?}", r.changes);
        assert!(r.block.contains("hasconfig:remote.*.url:git@github.com:MageTech/**"));
        assert_eq!(std::fs::read_to_string(&p.gitconfig).unwrap(), ORIGINAL);
        assert!(!p.data_dir.exists() && r.backup.is_none());
    }

    #[test]
    fn apply_is_idempotent_and_keeps_the_rest_of_the_file() {
        let (_d, p) = setup(Some(ORIGINAL));
        let cfg = two_accounts();
        let first = apply(&cfg, &p, false).unwrap();
        let bak = first.backup.clone().expect("backup of the original");
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), ORIGINAL);

        let text = std::fs::read_to_string(&p.gitconfig).unwrap();
        assert!(text.starts_with(ORIGINAL), "the user's lines stay first and untouched");
        assert!(!text.replace("\r\n", "").contains('\n'), "CRLF kept throughout");
        assert_eq!(text.matches(BLOCK_START).count(), 1);
        assert!(text.contains("hooksPath = \"") && text.trim_end().ends_with(BLOCK_END));
        let work = std::fs::read_to_string(p.account_file("work")).unwrap();
        assert!(work.contains("email = \"phi@magetechcorp.com\""));
        assert!(std::fs::read_to_string(p.hooks_dir().join("pre-push")).unwrap().contains("guard \"$hook\""));

        let again = apply(&cfg, &p, false).unwrap();
        assert_eq!((again.changed(), again.backup), (0, None));
        assert_eq!(std::fs::read_to_string(&p.gitconfig).unwrap(), text);
        assert!(again.issues.iter().any(|i| i.code == "global-default" && i.message.contains("(Work)")));
    }

    #[test]
    fn switching_off_removes_only_that_account() {
        let (_d, p) = setup(Some(ORIGINAL));
        let mut cfg = two_accounts();
        apply(&cfg, &p, false).unwrap();
        cfg.accounts[1].enabled = false; // Personal
        let r = apply(&cfg, &p, false).unwrap();
        assert_eq!(r.changes.iter().filter(|c| c.action != "unchanged").map(|c| (c.kind.as_str(), c.action.as_str())).collect::<Vec<_>>(), [("account", "remove"), ("gitconfig", "update")]);
        let text = std::fs::read_to_string(&p.gitconfig).unwrap();
        assert!(text.contains("MageTech") && !text.contains("HoangTrongPhi"));
        assert!(!p.account_file("personal").exists() && p.account_file("work").exists());
        assert!(text.contains("hooksPath"), "the guard stays to refuse the switched-off account");

        // Everything off and removed: the file is the original again, byte for byte.
        cfg.remove("work");
        cfg.remove("personal");
        let r = apply(&cfg, &p, false).unwrap();
        assert_eq!(std::fs::read_to_string(&p.gitconfig).unwrap(), ORIGINAL);
        assert!(r.block.is_empty() && !p.hooks_dir().join("pre-push").exists() && !p.account_file("work").exists());
    }

    #[test]
    fn missing_gitconfig_and_guard_off() {
        let (_d, p) = setup(None);
        let mut cfg = two_accounts();
        cfg.guard.enabled = false;
        cfg.accounts[0].enabled = false;
        let r = apply(&cfg, &p, false).unwrap();
        let text = std::fs::read_to_string(&p.gitconfig).unwrap();
        assert!(text.starts_with(BLOCK_START) && !text.contains("hooksPath") && !text.contains('\r'));
        assert!(!p.hooks_dir().join("pre-commit").exists());
        assert!(r.issues.iter().any(|i| i.code == "off-without-guard"));
        assert!(r.backup.is_none(), "nothing to back up");
    }

    #[test]
    fn account_file_and_scope_details() {
        let (_d, p) = setup(Some("[core]\n\thooksPath = C:/my/hooks\n"));
        let mut cfg = two_accounts();
        cfg.accounts[0].username = Some("phi-magetech".into());
        cfg.accounts[0].ssh_key = Some("C:\\Users\\PC\\.ssh\\id_work".into());
        cfg.accounts[0].folders = vec!["D:\\Work\\".into()];
        cfg.accounts[1].remotes.clear();
        let text = account_file_text(&cfg.accounts[0]);
        assert!(text.contains("[credential \"https://github.com\"]\n\tusername = \"phi-magetech\""));
        assert!(text.contains("sshCommand = \"ssh -i \\\"C:/Users/PC/.ssh/id_work\\\" -o IdentitiesOnly=yes\""));

        let r = apply(&cfg, &p, false).unwrap();
        assert!(r.block.contains("[includeIf \"gitdir/i:D:/Work/\"]"));
        let codes: Vec<&str> = r.issues.iter().map(|i| i.code.as_str()).collect();
        assert_eq!(codes, ["no-scope", "foreign-hooks-path", "ssh-key-missing"]);
        assert!(std::fs::read_to_string(p.hooks_dir().join("post-checkout")).unwrap().contains("for dir in 'C:/my/hooks'"));
    }

    #[test]
    fn damaged_block_is_refused() {
        let (_d, p) = setup(Some(&format!("[user]\n\tname = x\n{BLOCK_START}\n[core]\n")));
        assert!(apply(&two_accounts(), &p, false).unwrap_err().to_string().contains("no end line"));
        assert_eq!(global_identity(&setup(Some(ORIGINAL)).1), (Some("Phi Hoang".into()), Some("phi@magetechcorp.com".into())));
    }
}
