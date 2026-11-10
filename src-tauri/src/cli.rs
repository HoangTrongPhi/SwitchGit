//! Command line, same exe as the app: `switchgit status`, `switchgit on work`, `switchgit apply`,
//! `switchgit scan D:\src`, and `switchgit guard <hook>` (what the git hooks call).
//! Runs without a window. Exit codes: 0 done and nothing at the `--fail-on` level, 1 issues at that
//! level (or the guard refused), 2 bad arguments or input.

use std::io::Read;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;

use crate::commands::git_cmd::{mutate, snapshot};
use crate::core::gitconfig;
use crate::core::guard::{self, Hook};
use crate::core::model::{Config, Paths};
use crate::core::repos;
use crate::core::{count_at_least, quiet_ctx, CoreError, Issue, Severity};

const APP: &str = "SwitchGit";
const EXAMPLES: &str = "\
Examples:
  switchgit status
  switchgit off personal
  switchgit on personal
  switchgit apply --dry-run
  switchgit scan D:\\src --report repos.json --fail-on warn

Exit codes: 0 done, 1 issues at the --fail-on level, 2 bad arguments or input.
In cmd.exe run `start /wait switchgit.exe ...` to wait for the exit code.";

/// Subcommands the exe answers on the command line; any other first argument opens the window.
const SUBCOMMANDS: &[&str] = &["status", "on", "off", "apply", "scan", "guard", "help"];

#[derive(Parser, Debug)]
#[command(name = "switchgit", version, about = "SwitchGit — Git identities that never cross", after_help = EXAMPLES)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Accounts with their switch, the guard, and whether the git configuration is up to date.
    Status,
    /// Switch an account on (by id or name).
    On { account: String },
    /// Switch an account off: its repositories refuse commits and pushes until it is on again.
    Off { account: String },
    /// Write the git configuration the accounts ask for.
    Apply {
        /// Only say which files would change.
        #[arg(long)]
        dry_run: bool,
    },
    /// List the repositories under the folders with their account and the identity in use.
    Scan(ScanArgs),
    /// Called by the git hooks SwitchGit installs.
    #[command(hide = true)]
    Guard {
        hook: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Folders to scan (default: the ones scanned last in the app).
    pub folders: Vec<PathBuf>,
    /// Write the whole result (the same structure the app shows) as JSON.
    #[arg(long, value_name = "FILE")]
    pub report: Option<PathBuf>,
    /// Issue level that makes the exit code 1.
    #[arg(long, value_enum, default_value_t = FailOn::Error)]
    pub fail_on: FailOn,
    /// Only the summary line and errors.
    #[arg(long)]
    pub quiet: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailOn {
    Warn,
    Error,
}

impl FailOn {
    pub fn severity(self) -> Severity {
        match self {
            FailOn::Warn => Severity::Warn,
            FailOn::Error => Severity::Error,
        }
    }
}

/// True when the process was started for the command line rather than the app window.
pub fn wants_cli(args: &[String]) -> bool {
    args.get(1).is_some_and(|a| SUBCOMMANDS.contains(&a.as_str()) || matches!(a.as_str(), "--help" | "-h" | "--version" | "-V"))
}

/// Release builds use the Windows GUI subsystem: borrow the calling terminal's console for output.
fn attach_console() {
    #[cfg(windows)]
    {
        extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

// ---------------------------------------------------------------- output

fn title(command: &str, target: &Path) {
    println!("{APP} {} — {command} on {}", env!("CARGO_PKG_VERSION"), target.display());
}

/// One `OK  `/`WARN`/`FAIL` line with at most 20 detail lines, problems (`!`) first.
fn step(mark: &str, name: &str, summary: &str, details: &[String], quiet: bool) {
    println!("{mark} {name}: {summary}");
    let mut lines: Vec<&String> = details.iter().filter(|d| d.trim_start().starts_with('!')).collect();
    if !quiet {
        lines.extend(details.iter().filter(|d| !d.trim_start().starts_with('!')));
    }
    for d in lines.iter().take(20) {
        println!("       {d}");
    }
    if !quiet && details.len() > 20 {
        println!("       … {} more line(s) in the report", details.len() - 20);
    }
}

fn issue_lines(issues: &[Issue]) -> Vec<String> {
    issues
        .iter()
        .map(|i| {
            let at = i.path.as_deref().map(|p| format!(" ({p})")).unwrap_or_default();
            let bang = if i.severity == Severity::Error { "! " } else { "" };
            format!("{bang}{}{at}", i.message)
        })
        .collect()
}

fn mark_for(issues: &[Issue]) -> &'static str {
    if count_at_least(issues, Severity::Error) > 0 {
        "FAIL"
    } else if count_at_least(issues, Severity::Warn) > 0 {
        "WARN"
    } else {
        "OK  "
    }
}

pub fn write_report<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::utils::paths::write_atomic(path, json.as_bytes()).map_err(|e| format!("Cannot write report {}: {e}", path.display()))
}

fn input_error(e: CoreError) -> i32 {
    eprintln!("{e}");
    2
}

// ---------------------------------------------------------------- commands

pub fn run(args: &[String]) -> i32 {
    attach_console();
    run_with(args, &Paths::from_env())
}

/// [`run`] against explicit locations (tests).
pub fn run_with(args: &[String], paths: &Paths) -> i32 {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            let code = if e.use_stderr() { 2 } else { 0 };
            let _ = e.print();
            return code;
        }
    };
    match cli.command {
        Command::Status => status(paths),
        Command::On { account } => switch(paths, &account, true),
        Command::Off { account } => switch(paths, &account, false),
        Command::Apply { dry_run } => apply(paths, dry_run),
        Command::Scan(a) => scan(paths, a),
        Command::Guard { hook, args } => guard_hook(paths, &hook, &args),
    }
}

fn status(paths: &Paths) -> i32 {
    let snap = match snapshot(paths) {
        Ok(s) => s,
        Err(e) => return input_error(e),
    };
    title("status", &paths.gitconfig);
    if snap.config.accounts.is_empty() {
        println!("No accounts yet: add them in the app.");
    }
    for a in &snap.config.accounts {
        let scope: Vec<&str> = a.remotes.iter().chain(&a.folders).map(String::as_str).collect();
        println!("{} {:<12} {:<32} {}", if a.enabled { "ON  " } else { "OFF " }, a.id, a.email, scope.join(", "));
    }
    for (client, id) in &snap.config.bindings {
        println!("     {client} is tied to {id}");
    }
    let g = &snap.config.guard;
    println!("Guard: {}", if g.enabled { format!("on (commit {}, push {}, authors {})", g.check_commit, g.check_push, g.check_authors) } else { "off".into() });
    if snap.pending > 0 {
        println!("WARN {} file(s) differ from the accounts: run `switchgit apply`", snap.pending);
        return 1;
    }
    println!("OK   git configuration is up to date");
    0
}

fn switch(paths: &Paths, account: &str, enabled: bool) -> i32 {
    let result = mutate(paths, |cfg| {
        let found = cfg.accounts.iter_mut().find(|a| a.id.eq_ignore_ascii_case(account) || a.label.eq_ignore_ascii_case(account));
        let a = found.ok_or_else(|| CoreError::Input(format!("No account \"{account}\" (see `switchgit status`)")))?;
        a.enabled = enabled;
        Ok(format!("{} switched {}", a.label, if enabled { "on" } else { "off" }))
    });
    match result {
        Ok((_, line)) => {
            println!("OK   {line}");
            0
        }
        Err(e) => input_error(e),
    }
}

fn apply(paths: &Paths, dry_run: bool) -> i32 {
    let report = match Config::load(paths).and_then(|cfg| gitconfig::apply(&cfg, paths, dry_run)) {
        Ok(r) => r,
        Err(e) => return input_error(e),
    };
    title(if dry_run { "apply (dry run)" } else { "apply" }, &paths.gitconfig);
    let mut details: Vec<String> = report.changes.iter().filter(|c| c.action != "unchanged").map(|c| format!("{:<7} {}", c.action, c.path)).collect();
    details.extend(issue_lines(&report.issues));
    let verb = if dry_run { "would change" } else { "changed" };
    step(mark_for(&report.issues), "Git configuration", &format!("{} file(s) {verb}, {} up to date", report.changed(), report.changes.len() - report.changed()), &details, false);
    if let Some(bak) = &report.backup {
        println!("Original global config kept as {bak}");
    }
    println!("Finished in {} ms", report.elapsed_ms);
    if report.ok {
        0
    } else {
        1
    }
}

fn scan(paths: &Paths, a: ScanArgs) -> i32 {
    let cfg = match Config::load(paths) {
        Ok(c) => c,
        Err(e) => return input_error(e),
    };
    let roots: Vec<String> = if a.folders.is_empty() { cfg.scan_roots.clone() } else { a.folders.iter().map(|f| f.to_string_lossy().into_owned()).collect() };
    let report = match quiet_ctx(|ctx| repos::scan(&cfg, &roots, ctx)) {
        Ok(r) => r,
        Err(e) => return input_error(e),
    };
    if !a.quiet {
        title("scan", Path::new(&roots.join(", ")));
    }
    let mut details: Vec<String> = report
        .repos
        .iter()
        .map(|r| {
            let bang = if r.status == "wrong-identity" { "! " } else { "" };
            format!("{bang}{:<14} {:<10} {}", r.status, r.owner.as_deref().unwrap_or("-"), r.path)
        })
        .collect();
    details.extend(issue_lines(&report.issues).into_iter().filter(|l| l.starts_with('!')));
    step(mark_for(&report.issues), "Repositories", &repos::summary(&report), &details, a.quiet);
    if let Some(path) = &a.report {
        if let Err(e) = write_report(path, &report) {
            eprintln!("{e}");
            return 1;
        }
        if !a.quiet {
            println!("Report written to {}", path.display());
        }
    }
    let failed = count_at_least(&report.issues, a.fail_on.severity()) > 0;
    println!("Finished in {} ms", report.elapsed_ms);
    if failed {
        1
    } else {
        0
    }
}

/// `pre-push` gets the refs on stdin (the hook script passes them on); other hooks get nothing.
fn guard_hook(paths: &Paths, hook: &str, args: &[String]) -> i32 {
    let Some(hook) = Hook::parse(hook) else {
        return 0; // a hook the guard has no opinion on
    };
    let mut stdin = String::new();
    if hook == Hook::PrePush {
        let _ = std::io::stdin().read_to_string(&mut stdin);
    }
    guard::run_hook(paths, hook, args, &stdin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::testing::{paths, two_accounts};

    fn args(list: &[&str]) -> Vec<String> {
        std::iter::once("switchgit").chain(list.iter().copied()).map(String::from).collect()
    }

    #[test]
    fn parses_flags() {
        assert!(!wants_cli(&args(&[])));
        assert!(!wants_cli(&args(&["D:/some/file.json"])), "Open with → window");
        for sub in ["status", "on", "off", "apply", "scan", "guard", "--help", "--version"] {
            assert!(wants_cli(&args(&[sub])), "{sub}");
        }

        let c = Cli::try_parse_from(args(&["scan", "D:/a", "D:/b", "--report", "r.json", "--quiet", "--fail-on", "warn"])).unwrap();
        let Command::Scan(s) = c.command else { panic!("scan") };
        assert_eq!(s.folders, [PathBuf::from("D:/a"), PathBuf::from("D:/b")]);
        assert_eq!((s.report, s.quiet, s.fail_on), (Some(PathBuf::from("r.json")), true, FailOn::Warn));
        assert!(matches!(Cli::try_parse_from(args(&["apply", "--dry-run"])).unwrap().command, Command::Apply { dry_run: true }));
        assert!(matches!(Cli::try_parse_from(args(&["off", "personal"])).unwrap().command, Command::Off { account } if account == "personal"));
        // The hook's own arguments pass through untouched.
        let g = Cli::try_parse_from(args(&["guard", "pre-push", "origin", "https://github.com/x/y.git"])).unwrap();
        assert!(matches!(g.command, Command::Guard { hook, args } if hook == "pre-push" && args == ["origin", "https://github.com/x/y.git"]));

        assert!(Cli::try_parse_from(args(&["on"])).is_err(), "account required");
        assert!(Cli::try_parse_from(args(&["scan", "--report"])).is_err(), "value missing");
        assert!(Cli::try_parse_from(args(&["apply", "--bogus"])).is_err(), "unknown flag");
        assert!(Cli::try_parse_from(args(&["scan", "D:/a", "--fail-on", "info"])).is_err(), "bad level");
    }

    #[test]
    fn runs_status_on_off_apply() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        assert_eq!(run_with(&args(&["status"]), &p), 0, "nothing to manage is up to date");
        two_accounts().save(&p).unwrap();
        assert_eq!(run_with(&args(&["status"]), &p), 1, "accounts saved but not applied");
        assert_eq!(run_with(&args(&["apply", "--dry-run"]), &p), 0);
        assert!(!p.gitconfig.exists());
        assert_eq!(run_with(&args(&["apply"]), &p), 0);
        assert_eq!(run_with(&args(&["status"]), &p), 0);

        assert_eq!(run_with(&args(&["off", "Personal"]), &p), 0, "by name, any case");
        assert!(!Config::load(&p).unwrap().account("personal").unwrap().enabled);
        assert!(!p.account_file("personal").exists() && !std::fs::read_to_string(&p.gitconfig).unwrap().contains("HoangTrongPhi"));
        assert_eq!(run_with(&args(&["on", "personal"]), &p), 0);
        assert!(p.account_file("personal").exists());
        assert_eq!(run_with(&args(&["on", "nobody"]), &p), 2);
        assert_eq!(run_with(&args(&["apply", "--nope"]), &p), 2);
        assert_eq!(run_with(&args(&["guard", "post-checkout"]), &p), 0);
    }

    #[test]
    fn runs_scan() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        two_accounts().save(&p).unwrap();
        let root = dir.path().join("src");
        std::fs::create_dir_all(&root).unwrap();
        let report = dir.path().join("out/repos.json");
        assert_eq!(run_with(&args(&["scan", &root.to_string_lossy(), "--report", &report.to_string_lossy(), "--quiet"]), &p), 0);
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
        assert_eq!((json["repos"].as_array().map(Vec::len), json["ok"].as_bool(), json["cancelled"].as_bool()), (Some(0), Some(true), Some(false)));
        assert_eq!(run_with(&args(&["scan", &dir.path().join("missing").to_string_lossy()]), &p), 1, "folder-not-found is an error issue");
        assert_eq!(run_with(&args(&["scan"]), &p), 2, "no folder given and none remembered");
    }
}
