//! Find the repositories under some folders and say, for each, whose it is and who would commit
//! there right now. Read-only.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use walkdir::WalkDir;

use super::guard::{owner_of, Owner};
use super::model::Config;
use super::{git, remote, CoreError, Issue, RunCtx, Severity};

/// Folders that never hold a repository worth listing (and are slow to walk).
const SKIP: &[&str] = &["node_modules", "target", "Library", "Intermediate", "Saved", "Binaries", "DerivedDataCache", ".venv", "venv", "$RECYCLE.BIN", "System Volume Information", "AppData", "Windows", "Program Files", "Program Files (x86)"];
const MAX_DEPTH: usize = 6;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRemote {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub path: String,
    pub remotes: Vec<RepoRemote>,
    /// Account that owns it (by remote, else by folder).
    pub owner: Option<String>,
    /// `user.email` a commit made there now would carry, and the account that e-mail is.
    pub email: Option<String>,
    pub identity: Option<String>,
    /// `ok`, `wrong-identity`, `account-off`, `mixed`, `unowned`.
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub roots: Vec<String>,
    pub repos: Vec<RepoInfo>,
    pub issues: Vec<Issue>,
    pub elapsed_ms: u64,
    pub ok: bool,
    pub cancelled: bool,
}

pub fn summary(r: &ScanReport) -> String {
    let count = |s: &str| r.repos.iter().filter(|x| x.status == s).count();
    format!("{} repositories: {} ok, {} wrong identity, {} switched off, {} unowned", r.repos.len(), count("ok"), count("wrong-identity"), count("account-off"), count("unowned") + count("mixed"))
}

/// Work trees under `root` (a folder with `.git` in it); nested repositories are not looked for.
fn find(root: &Path, ctx: &RunCtx, issues: &mut Vec<Issue>) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut walker = WalkDir::new(root).max_depth(MAX_DEPTH).follow_links(false).into_iter();
    while let Some(entry) = walker.next() {
        if ctx.cancelled() {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                // Folders Windows will not let us read are everyday; note the first few only.
                if issues.len() < 20 {
                    issues.push(Issue::new(Severity::Info, "unreadable-folder", e.to_string()));
                }
                continue;
            }
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if entry.depth() > 0 && (SKIP.contains(&name.as_ref()) || name.starts_with('.')) {
            walker.skip_current_dir();
            continue;
        }
        if entry.path().join(".git").exists() {
            found.push(entry.path().to_path_buf());
            walker.skip_current_dir();
        }
    }
    found
}

/// One repository as the guard would see it at commit time.
pub fn inspect(cfg: &Config, path: &Path) -> RepoInfo {
    let remotes: Vec<RepoRemote> = git(Some(path), &["config", "--get-regexp", r"^remote\..*\.url$"])
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(' '))
        .map(|(key, url)| RepoRemote { name: key.trim_start_matches("remote.").trim_end_matches(".url").to_string(), url: url.trim().to_string() })
        .collect();
    let email = git(Some(path), &["config", "user.email"]).filter(|e| !e.is_empty());
    let identity = email.as_deref().and_then(|e| cfg.account_by_email(e)).map(|a| a.id.clone());
    let urls: Vec<String> = remotes.iter().map(|r| r.url.clone()).collect();
    let (owner, status) = match owner_of(cfg, &urls, path) {
        Owner::Nobody => (None, "unowned"),
        Owner::Mixed => (None, "mixed"),
        Owner::Account(a) if !a.enabled => (Some(a.id.clone()), "account-off"),
        Owner::Account(a) => (Some(a.id.clone()), if email.as_deref().is_some_and(|e| e.eq_ignore_ascii_case(&a.email)) { "ok" } else { "wrong-identity" }),
    };
    RepoInfo { path: path.to_string_lossy().into_owned(), remotes, owner, email, identity, status: status.into() }
}

pub fn scan(cfg: &Config, roots: &[String], ctx: &RunCtx) -> Result<ScanReport, CoreError> {
    let started = Instant::now();
    if roots.is_empty() {
        return Err(CoreError::Input("Choose a folder to scan".into()));
    }
    let mut issues = Vec::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    for root in roots {
        let root_path = Path::new(root);
        if !root_path.is_dir() {
            issues.push(Issue::new(Severity::Error, "folder-not-found", "Folder not found").at(root_path));
            continue;
        }
        (ctx.progress)(0, 0, &format!("Looking for repositories in {root}"));
        paths.extend(find(root_path, ctx, &mut issues));
    }
    paths.sort();
    paths.dedup();

    let total = paths.len() as u64;
    let mut repos = Vec::with_capacity(paths.len());
    for (i, path) in paths.iter().enumerate() {
        if ctx.cancelled() {
            break;
        }
        (ctx.progress)(i as u64, total, &path.to_string_lossy());
        repos.push(inspect(cfg, path));
    }
    let cancelled = ctx.cancelled();
    if !cancelled {
        (ctx.progress)(total, total, "Done");
    }
    for r in repos.iter().filter(|r| r.status == "wrong-identity") {
        let who = r.email.as_deref().unwrap_or("nobody");
        let owner = r.owner.as_deref().and_then(|id| cfg.account(id)).map(|a| a.label.as_str()).unwrap_or("?");
        issues.push(Issue::new(Severity::Warn, "wrong-identity", format!("Commits as {who}, but belongs to {owner}")).at(&r.path));
    }
    let ok = !issues.iter().any(|i| i.severity == Severity::Error);
    Ok(ScanReport { roots: roots.to_vec(), repos, issues, elapsed_ms: started.elapsed().as_millis() as u64, ok, cancelled })
}

/// Remote of a repository in the form account scopes are written in (`github.com/Owner`), to offer
/// as a scope for an unowned repository.
pub fn suggested_scope(url: &str) -> Option<String> {
    let n = remote::normalize(url);
    let mut parts = n.split('/');
    match (parts.next(), parts.next()) {
        (Some(host), Some(owner)) if !host.is_empty() => Some(format!("{host}/{owner}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::testing::*;
    use super::super::quiet_ctx;
    use super::*;

    /// A repository with a remote and a local identity; `None` when git is not on PATH.
    fn repo(dir: &Path, url: Option<&str>, email: &str) -> Option<()> {
        std::fs::create_dir_all(dir).unwrap();
        git(Some(dir), &["init", "-q"])?;
        git(Some(dir), &["config", "user.email", email])?;
        if let Some(url) = url {
            git(Some(dir), &["remote", "add", "origin", url])?;
        }
        Some(())
    }

    #[test]
    fn scan_classifies_repositories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let made = (|| {
            repo(&root.join("work/game"), Some("https://github.com/MageTech/game.git"), "phi@magetechcorp.com")?;
            repo(&root.join("home/abyss"), Some("git@github.com:HoangTrongPhi/Abyss.git"), "phi@magetechcorp.com")?;
            repo(&root.join("home/other"), Some("https://gitlab.com/x/y.git"), "phi@magetechcorp.com")?;
            repo(&root.join("node_modules/dep"), None, "x@y.z")
        })();
        if made.is_none() {
            eprintln!("git not available: skipped");
            return;
        }
        std::fs::create_dir_all(root.join("home/abyss/sub/.git")).unwrap(); // nested: not listed

        let cfg = two_accounts();
        let r = quiet_ctx(|ctx| scan(&cfg, &[root.to_string_lossy().into_owned(), root.join("missing").to_string_lossy().into_owned()], ctx)).unwrap();
        let by = |suffix: &str| r.repos.iter().find(|x| x.path.replace('\\', "/").ends_with(suffix)).unwrap_or_else(|| panic!("{suffix} in {:?}", r.repos));
        assert_eq!(r.repos.len(), 3);
        assert_eq!((by("work/game").status.as_str(), by("work/game").owner.as_deref()), ("ok", Some("work")));
        let abyss = by("home/abyss");
        assert_eq!((abyss.status.as_str(), abyss.owner.as_deref(), abyss.identity.as_deref()), ("wrong-identity", Some("personal"), Some("work")));
        assert_eq!(abyss.remotes, [RepoRemote { name: "origin".into(), url: "git@github.com:HoangTrongPhi/Abyss.git".into() }]);
        assert_eq!(by("home/other").status, "unowned");
        let codes: Vec<&str> = r.issues.iter().map(|i| i.code.as_str()).collect();
        assert_eq!(codes, ["folder-not-found", "wrong-identity"]);
        assert!(!r.ok && !r.cancelled);
        assert!(summary(&r).starts_with("3 repositories: 1 ok, 1 wrong identity"));

        let mut off = cfg.clone();
        off.accounts[0].enabled = false;
        assert_eq!(inspect(&off, &root.join("work/game")).status, "account-off");
        assert!(quiet_ctx(|ctx| scan(&cfg, &[], ctx)).is_err());
    }

    #[test]
    fn scope_suggestion() {
        assert_eq!(suggested_scope("git@github.com:HoangTrongPhi/Abyss.git").as_deref(), Some("github.com/hoangtrongphi"));
        assert_eq!(suggested_scope("D:/local/repo"), None);
    }
}
