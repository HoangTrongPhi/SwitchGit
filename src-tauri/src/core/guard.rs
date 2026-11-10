//! The guard: decides, inside git's `pre-commit` and `pre-push` hooks, whether this commit or push
//! crosses accounts.
//!
//! A repository belongs to the account that claims its remote (or, failing that, its folder). The
//! guard refuses when that account is switched off, when the commit would be authored as someone
//! else, when the push carries commits authored as another account, and when the git client that
//! started it is tied to a different account.

use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::clients;
use super::model::{path_within, Account, Config, Paths};
use super::{git, remote};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Hook {
    PreCommit,
    PrePush,
}

impl Hook {
    pub fn parse(name: &str) -> Option<Hook> {
        match name {
            "pre-commit" => Some(Hook::PreCommit),
            "pre-push" => Some(Hook::PrePush),
            _ => None,
        }
    }
    fn verb(self) -> &'static str {
        match self {
            Hook::PreCommit => "commit",
            Hook::PrePush => "push",
        }
    }
}

/// Everything the decision needs, gathered from git and the process tree (or made up by a test).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GuardInput {
    /// Top folder of the work tree.
    pub repo: String,
    /// `pre-push`: the one URL being pushed to. `pre-commit`: every remote of the repository.
    pub urls: Vec<String>,
    /// The e-mail a commit made now would carry.
    pub email: Option<String>,
    /// The git client that started this (`fork`, …, `terminal`); `None` when unknown.
    pub client: Option<String>,
    /// `pre-push`: author e-mails of the commits being pushed, with how many each.
    pub authors: Vec<(String, u32)>,
    /// `SWITCHGIT_BYPASS=1` was set for this one command.
    pub bypass: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub allow: bool,
    /// Stable id: `ok`, `unowned`, `account-off`, `wrong-identity`, `foreign-commits`, `client-bound`, …
    pub code: String,
    pub message: String,
    /// Id of the account the repository belongs to.
    pub owner: Option<String>,
    /// What to do about a refusal.
    pub fix: Option<String>,
}

fn pass(code: &str, message: impl Into<String>, owner: Option<&Account>) -> Verdict {
    Verdict { allow: true, code: code.into(), message: message.into(), owner: owner.map(|a| a.id.clone()), fix: None }
}

fn refuse(code: &str, message: String, owner: Option<&Account>, fix: impl Into<String>) -> Verdict {
    Verdict { allow: false, code: code.into(), message, owner: owner.map(|a| a.id.clone()), fix: Some(fix.into()) }
}

/// Who a repository belongs to.
#[derive(Debug, PartialEq)]
pub enum Owner<'a> {
    Account(&'a Account),
    /// Its remotes belong to different accounts and no folder settles it (a fork of a work
    /// repository with a personal remote): each push is judged by the remote it goes to.
    Mixed,
    Nobody,
}

pub fn owner_of<'a>(cfg: &'a Config, urls: &[String], repo: &Path) -> Owner<'a> {
    let mut by_remote: Vec<&Account> = Vec::new();
    for url in urls {
        let best = cfg.accounts.iter().filter_map(|a| a.remotes.iter().filter_map(|r| remote::match_len(r, url)).max().map(|len| (len, a))).max_by_key(|(len, _)| *len);
        if let Some((_, a)) = best {
            if !by_remote.iter().any(|b| b.id == a.id) {
                by_remote.push(a);
            }
        }
    }
    if by_remote.len() == 1 {
        return Owner::Account(by_remote[0]);
    }
    let by_folder = cfg.accounts.iter().filter_map(|a| a.folders.iter().filter(|f| path_within(f, repo)).map(|f| f.len()).max().map(|len| (len, a))).max_by_key(|(len, _)| *len);
    match by_folder {
        Some((_, a)) => Owner::Account(a),
        None if by_remote.is_empty() => Owner::Nobody,
        None => Owner::Mixed,
    }
}

/// The decision itself: no I/O.
pub fn decide(cfg: &Config, hook: Hook, input: &GuardInput) -> Verdict {
    let checked = match hook {
        Hook::PreCommit => cfg.guard.check_commit,
        Hook::PrePush => cfg.guard.check_push,
    };
    if !cfg.guard.enabled || !checked {
        return pass("guard-off", "The guard is off for this step", None);
    }
    let owner = match owner_of(cfg, &input.urls, Path::new(&input.repo)) {
        Owner::Account(a) => a,
        Owner::Mixed => return pass("mixed-remotes", "The remotes belong to different accounts; each push is checked against its own remote", None),
        Owner::Nobody if cfg.guard.block_unknown && !input.bypass => {
            return refuse("unknown-repo", format!("No account owns this repository, and unowned repositories are set to be refused ({})", hook.verb()), None, "Add its remote or folder to an account in SwitchGit, or turn off \"Refuse unowned repositories\".");
        }
        Owner::Nobody => return pass("unowned", "No account owns this repository", None),
    };
    if input.bypass {
        return pass("bypass", "SWITCHGIT_BYPASS=1: let through unchecked", Some(owner));
    }
    if !owner.enabled {
        return refuse("account-off", format!("This repository belongs to {}, which is switched off", owner.label), Some(owner), format!("Switch {} on in SwitchGit, then {} again.", owner.label, hook.verb()));
    }
    if let Some(client) = &input.client {
        if let Some(bound) = cfg.bindings.get(client).filter(|id| **id != owner.id) {
            let bound_label = cfg.account(bound).map(|a| a.label.as_str()).unwrap_or(bound);
            let other = cfg.bindings.iter().find(|(_, id)| **id == owner.id).map(|(c, _)| format!("Use {} for it", clients::client_name(c))).unwrap_or_else(|| "Use another client for it".into());
            return refuse(
                "client-bound",
                format!("{} is tied to {bound_label}, but this repository belongs to {}", capitalized(&clients::client_name(client)), owner.label),
                Some(owner),
                format!("{other}, or change the tie in SwitchGit → Clients."),
            );
        }
    }
    match hook {
        Hook::PreCommit => {
            let email = input.email.as_deref().unwrap_or("").trim();
            if !email.eq_ignore_ascii_case(&owner.email) {
                let who = match cfg.account_by_email(email) {
                    Some(a) => format!("{email} ({})", a.label),
                    None if email.is_empty() => "nobody (no user.email)".into(),
                    None => email.to_string(),
                };
                return refuse(
                    "wrong-identity",
                    format!("This commit would be authored as {who}, but the repository belongs to {} ({})", owner.label, owner.email),
                    Some(owner),
                    "Press Apply in SwitchGit; if it stays, the repository's own .git/config sets user.email: remove that line.",
                );
            }
        }
        Hook::PrePush if cfg.guard.check_authors => {
            let foreign: Vec<String> = input.authors.iter().filter_map(|(email, n)| cfg.account_by_email(email).filter(|a| a.id != owner.id).map(|a| format!("{n} as {} ({email})", a.label))).collect();
            if !foreign.is_empty() {
                return refuse(
                    "foreign-commits",
                    format!("This push to a {} repository carries commits authored as another account: {}", owner.label, foreign.join(", ")),
                    Some(owner),
                    format!("Rewrite their author (git rebase -r <base> --exec \"git commit --amend --no-edit --reset-author\") while committing as {}, then push again.", owner.label),
                );
            }
        }
        Hook::PrePush => {}
    }
    pass("ok", format!("{} repository, {} account", owner.label, owner.label), Some(owner))
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

// ---------------------------------------------------------------- gathering

/// `pre-push` stdin (`<local ref> <local sha> <remote ref> <remote sha>` per line) → the revision
/// ranges being pushed, as `git log` arguments.
pub fn pushed_ranges(stdin: &str) -> Vec<Vec<String>> {
    let zero = |sha: &str| !sha.is_empty() && sha.chars().all(|c| c == '0');
    stdin
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            let (local, remote_sha) = (*f.get(1)?, *f.get(3)?);
            if zero(local) {
                None // deleting a branch pushes no commits
            } else if zero(remote_sha) {
                // A new branch: everything no remote has yet.
                Some(vec![local.to_string(), "--not".into(), "--remotes".into()])
            } else {
                Some(vec![format!("{remote_sha}..{local}")])
            }
        })
        .collect()
}

fn pushed_authors(stdin: &str) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    for range in pushed_ranges(stdin) {
        let mut args = vec!["log", "--format=%ae", "-n", "5000"];
        args.extend(range.iter().map(String::as_str));
        for email in git(None, &args).unwrap_or_default().lines().map(str::trim).filter(|e| !e.is_empty()) {
            match out.iter_mut().find(|(e, _)| e.eq_ignore_ascii_case(email)) {
                Some((_, n)) => *n += 1,
                None => out.push((email.to_string(), 1)),
            }
        }
    }
    out
}

/// Read what [`decide`] needs from the repository in the current folder. `args` are the hook's
/// own arguments (`pre-push`: remote name, URL).
pub fn gather(cfg: &Config, hook: Hook, args: &[String], stdin: &str) -> GuardInput {
    let repo = git(None, &["rev-parse", "--show-toplevel"]).unwrap_or_default();
    let urls = match hook {
        Hook::PrePush => args.get(1).or(args.first()).cloned().into_iter().collect(),
        Hook::PreCommit => git(None, &["config", "--get-regexp", r"^remote\..*\.url$"]).unwrap_or_default().lines().filter_map(|l| l.split_once(' ').map(|(_, url)| url.trim().to_string())).collect(),
    };
    let email = std::env::var("GIT_AUTHOR_EMAIL").ok().filter(|e| !e.is_empty()).or_else(|| git(None, &["config", "user.email"])).filter(|e| !e.is_empty());
    let authors = if hook == Hook::PrePush && cfg.guard.check_authors { pushed_authors(stdin) } else { Vec::new() };
    GuardInput { repo, urls, email, client: clients::calling_client().map(str::to_string), authors, bypass: std::env::var("SWITCHGIT_BYPASS").is_ok_and(|v| v == "1") }
}

// ---------------------------------------------------------------- activity log

/// One line of `guard.log` (JSON lines).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GuardEvent {
    pub ts_ms: u64,
    pub hook: String,
    pub repo: String,
    /// `host/owner/repo` of the remote (the first one, for a commit).
    pub remote: Option<String>,
    pub client: Option<String>,
    pub email: Option<String>,
    pub allow: bool,
    pub code: String,
    pub message: String,
    pub owner: Option<String>,
}

impl Default for GuardEvent {
    fn default() -> Self {
        GuardEvent { ts_ms: 0, hook: String::new(), repo: String::new(), remote: None, client: None, email: None, allow: true, code: String::new(), message: String::new(), owner: None }
    }
}

const LOG_LIMIT: u64 = 512 * 1024;

pub fn log_event(paths: &Paths, event: &GuardEvent) -> std::io::Result<()> {
    let file = paths.guard_log();
    std::fs::create_dir_all(&paths.data_dir)?;
    // Keep the newer half once the file grows past the limit.
    if std::fs::metadata(&file).map(|m| m.len() > LOG_LIMIT).unwrap_or(false) {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let kept = lines[lines.len() / 2..].join("\n");
        std::fs::write(&file, format!("{kept}\n"))?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&file)?;
    writeln!(f, "{}", serde_json::to_string(event).unwrap_or_default())
}

/// The newest `limit` events, newest first. Lines that do not parse are skipped.
pub fn read_log(paths: &Paths, limit: usize) -> Vec<GuardEvent> {
    let text = std::fs::read_to_string(paths.guard_log()).unwrap_or_default();
    text.lines().rev().filter_map(|l| serde_json::from_str(l).ok()).take(limit).collect()
}

pub fn clear_log(paths: &Paths) -> std::io::Result<()> {
    match std::fs::remove_file(paths.guard_log()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// What the hook prints when it refuses (git shows stderr in the terminal and in clients' error boxes).
pub fn refusal_text(hook: Hook, input: &GuardInput, verdict: &Verdict) -> String {
    let mut s = format!("\nSwitchGit stopped this {}.\n  {}\n", hook.verb(), verdict.message);
    if !input.repo.is_empty() {
        s.push_str(&format!("  repository: {}\n", input.repo));
    }
    if let Some(url) = input.urls.first().filter(|_| hook == Hook::PrePush) {
        s.push_str(&format!("  remote:     {url}\n"));
    }
    if let Some(fix) = &verdict.fix {
        s.push_str(&format!("  {fix}\n"));
    }
    s.push_str("  To let this one command through anyway, run it with SWITCHGIT_BYPASS=1.\n");
    s
}

/// The whole hook: gather, decide, log, explain. Returns the hook's exit code. Anything that goes
/// wrong inside SwitchGit itself lets git carry on: the guard must never be the thing that breaks git.
pub fn run_hook(paths: &Paths, hook: Hook, args: &[String], stdin: &str) -> i32 {
    let cfg = match Config::load(paths) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SwitchGit: guard skipped ({e})");
            return 0;
        }
    };
    if !cfg.guard.enabled {
        return 0;
    }
    let input = gather(&cfg, hook, args, stdin);
    let verdict = decide(&cfg, hook, &input);
    // Unowned repositories are none of the guard's business: they stay out of the activity list.
    if verdict.code != "unowned" && verdict.code != "guard-off" {
        let event = GuardEvent {
            ts_ms: now_ms(),
            hook: hook.verb().into(),
            repo: input.repo.clone(),
            remote: input.urls.first().map(|u| Some(remote::normalize(u)).filter(|n| !n.is_empty()).unwrap_or_else(|| u.clone())),
            client: input.client.clone(),
            email: input.email.clone(),
            allow: verdict.allow,
            code: verdict.code.clone(),
            message: verdict.message.clone(),
            owner: verdict.owner.clone(),
        };
        let _ = log_event(paths, &event);
    }
    if verdict.allow {
        0
    } else {
        eprint!("{}", refusal_text(hook, &input, &verdict));
        1
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::testing::*;
    use super::*;

    const WORK_URL: &str = "https://github.com/MageTech/game-client.git";
    const HOME_URL: &str = "git@github.com:HoangTrongPhi/Abyss.git";

    fn input(url: &str, email: &str) -> GuardInput {
        GuardInput { repo: "D:/src/repo".into(), urls: vec![url.into()], email: Some(email.into()), ..Default::default() }
    }

    #[test]
    fn commits_must_carry_the_owners_identity() {
        let cfg = two_accounts();
        let ok = decide(&cfg, Hook::PreCommit, &input(HOME_URL, "Phi.Personal@example.com"));
        assert_eq!((ok.allow, ok.code.as_str(), ok.owner.as_deref()), (true, "ok", Some("personal")));

        // The case the app exists for: the company identity in a personal repository, and back.
        let v = decide(&cfg, Hook::PreCommit, &input(HOME_URL, "phi@magetechcorp.com"));
        assert_eq!((v.allow, v.code.as_str()), (false, "wrong-identity"));
        assert!(v.message.contains("phi@magetechcorp.com (Work)") && v.message.contains("belongs to Personal"), "{}", v.message);
        assert_eq!(decide(&cfg, Hook::PreCommit, &input(WORK_URL, "phi.personal@example.com")).code, "wrong-identity");

        let none = decide(&cfg, Hook::PreCommit, &GuardInput { email: None, ..input(WORK_URL, "") });
        assert!(!none.allow && none.message.contains("no user.email"));
        let stranger = decide(&cfg, Hook::PreCommit, &input(WORK_URL, "someone@else.org"));
        assert!(!stranger.allow && stranger.message.contains("someone@else.org,"), "{}", stranger.message);
    }

    #[test]
    fn pushes_must_not_carry_another_accounts_commits() {
        let cfg = two_accounts();
        let mut push = input(HOME_URL, "phi.personal@example.com");
        push.authors = vec![("phi.personal@example.com".into(), 3), ("friend@example.org".into(), 2)];
        assert!(decide(&cfg, Hook::PrePush, &push).allow, "own and third-party commits are fine");

        push.authors.push(("PHI@magetechcorp.com".into(), 4));
        let v = decide(&cfg, Hook::PrePush, &push);
        assert_eq!((v.allow, v.code.as_str()), (false, "foreign-commits"));
        assert!(v.message.contains("4 as Work"), "{}", v.message);

        let mut lax = cfg.clone();
        lax.guard.check_authors = false;
        assert!(decide(&lax, Hook::PrePush, &push).allow);
        // The current e-mail does not matter for a push: only what is in the commits does.
        assert!(decide(&cfg, Hook::PrePush, &input(HOME_URL, "phi@magetechcorp.com")).allow);
    }

    #[test]
    fn switched_off_accounts_are_refused() {
        let mut cfg = two_accounts();
        cfg.accounts[1].enabled = false;
        for hook in [Hook::PreCommit, Hook::PrePush] {
            let v = decide(&cfg, hook, &input(HOME_URL, "phi.personal@example.com"));
            assert_eq!((v.allow, v.code.as_str()), (false, "account-off"), "{hook:?}");
        }
        assert!(decide(&cfg, Hook::PrePush, &input(WORK_URL, "phi@magetechcorp.com")).allow, "the other account is untouched");
    }

    #[test]
    fn clients_stay_with_their_account() {
        let mut cfg = two_accounts();
        cfg.bindings.insert("fork".into(), "personal".into());
        cfg.bindings.insert("sourcetree".into(), "work".into());
        let from = |client: &str, url: &str, email: &str| GuardInput { client: Some(client.into()), ..input(url, email) };

        assert!(decide(&cfg, Hook::PrePush, &from("fork", HOME_URL, "phi.personal@example.com")).allow);
        assert!(decide(&cfg, Hook::PreCommit, &from("sourcetree", WORK_URL, "phi@magetechcorp.com")).allow);
        let v = decide(&cfg, Hook::PrePush, &from("fork", WORK_URL, "phi@magetechcorp.com"));
        assert_eq!((v.allow, v.code.as_str()), (false, "client-bound"));
        assert_eq!(v.message, "Fork is tied to Personal, but this repository belongs to Work");
        assert!(v.fix.as_deref().unwrap().starts_with("Use Sourcetree for it"));
        assert_eq!(decide(&cfg, Hook::PreCommit, &from("sourcetree", HOME_URL, "phi.personal@example.com")).code, "client-bound");
        // Unbound clients and the terminal follow the repository.
        assert!(decide(&cfg, Hook::PrePush, &from("github-desktop", WORK_URL, "phi@magetechcorp.com")).allow);
        assert!(decide(&cfg, Hook::PrePush, &from("terminal", HOME_URL, "phi.personal@example.com")).allow);
    }

    #[test]
    fn ownership_unowned_mixed_and_folders() {
        let mut cfg = two_accounts();
        let other = "https://gitlab.com/someone/thing.git";
        assert_eq!(decide(&cfg, Hook::PreCommit, &input(other, "x@y.z")).code, "unowned");
        cfg.guard.block_unknown = true;
        assert_eq!(decide(&cfg, Hook::PrePush, &input(other, "x@y.z")).code, "unknown-repo");
        assert!(decide(&cfg, Hook::PrePush, &GuardInput { bypass: true, ..input(other, "x@y.z") }).allow);
        cfg.guard.block_unknown = false;

        // A fork: origin is personal, upstream is work. Commits pass; each push is judged alone.
        let both = GuardInput { urls: vec![HOME_URL.into(), WORK_URL.into()], ..input("", "phi@magetechcorp.com") };
        assert_eq!(owner_of(&cfg, &both.urls, Path::new(&both.repo)), Owner::Mixed);
        assert_eq!(decide(&cfg, Hook::PreCommit, &both).code, "mixed-remotes");
        // …unless a folder settles it.
        cfg.accounts[0].folders = vec!["D:/src".into()];
        assert_eq!(decide(&cfg, Hook::PreCommit, &both).owner.as_deref(), Some("work"));
        // A folder also claims repositories without any remote; the remote still beats the folder.
        assert_eq!(decide(&cfg, Hook::PreCommit, &GuardInput { urls: vec![], ..input("", "phi@magetechcorp.com") }).code, "ok");
        assert_eq!(decide(&cfg, Hook::PreCommit, &input(HOME_URL, "phi.personal@example.com")).owner.as_deref(), Some("personal"));

        // The most specific remote wins.
        cfg.upsert(account("Client", "phi@client.example", "github.com/MageTech/client-x")).unwrap();
        assert_eq!(decide(&cfg, Hook::PrePush, &input("https://github.com/MageTech/client-x.git", "")).owner.as_deref(), Some("client"));

        cfg.guard.enabled = false;
        assert_eq!(decide(&cfg, Hook::PreCommit, &input(HOME_URL, "phi@magetechcorp.com")).code, "guard-off");
        let bypass = decide(&two_accounts(), Hook::PreCommit, &GuardInput { bypass: true, ..input(HOME_URL, "phi@magetechcorp.com") });
        assert_eq!((bypass.allow, bypass.code.as_str()), (true, "bypass"));
    }

    #[test]
    fn push_ranges_from_stdin() {
        let z = "0".repeat(40);
        let stdin = format!("refs/heads/main aaa1 refs/heads/main bbb2\nrefs/heads/new ccc3 refs/heads/new {z}\n(delete) {z} refs/heads/old ddd4\n\ngarbage\n");
        assert_eq!(pushed_ranges(&stdin), [vec!["bbb2..aaa1".to_string()], vec!["ccc3".to_string(), "--not".into(), "--remotes".into()]]);
        assert!(pushed_ranges("").is_empty());
    }

    #[test]
    fn log_round_trip_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        assert!(read_log(&p, 10).is_empty());
        for (i, allow) in [true, false, true].into_iter().enumerate() {
            log_event(&p, &GuardEvent { ts_ms: i as u64, hook: "push".into(), allow, code: if allow { "ok" } else { "account-off" }.into(), ..Default::default() }).unwrap();
        }
        std::fs::OpenOptions::new().append(true).open(p.guard_log()).unwrap().write_all(b"not json\n").unwrap();
        let log = read_log(&p, 2);
        assert_eq!(log.iter().map(|e| (e.ts_ms, e.allow)).collect::<Vec<_>>(), [(2, true), (1, false)]);
        clear_log(&p).unwrap();
        clear_log(&p).unwrap();
        assert!(read_log(&p, 10).is_empty());

        let v = decide(&two_accounts(), Hook::PrePush, &GuardInput { authors: vec![("phi@magetechcorp.com".into(), 1)], ..input(HOME_URL, "") });
        let text = refusal_text(Hook::PrePush, &input(HOME_URL, ""), &v);
        assert!(text.contains("SwitchGit stopped this push.") && text.contains("remote:     git@github.com") && text.contains("SWITCHGIT_BYPASS=1"));
    }
}
