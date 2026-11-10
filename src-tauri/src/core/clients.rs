//! Git clients (Fork, SourceTree, GitHub Desktop, …): which are installed and running, and which
//! one a git command came from.
//!
//! Clients keep their own sign-in, which SwitchGit cannot see or change. What it can do is tie a
//! client to one account: the guard hook walks up its parent processes, finds the client that
//! started git, and refuses work on repositories of any other account.

use std::path::PathBuf;

use serde::Serialize;

use super::model::Config;

/// Stands for git run from a shell, an IDE terminal or a script: no known client above it.
pub const TERMINAL: &str = "terminal";

pub struct ClientDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Process names, lower case.
    pub exes: &'static [&'static str],
    /// Where the exe usually is; `%VAR%` is expanded.
    pub candidates: &'static [&'static str],
    /// How the client runs git, in one line for the UI.
    pub note: &'static str,
}

pub const CLIENTS: &[ClientDef] = &[
    ClientDef {
        id: "fork",
        name: "Fork",
        exes: &["fork.exe"],
        candidates: &["%LOCALAPPDATA%\\Fork\\current\\Fork.exe", "%LOCALAPPDATA%\\Fork\\Fork.exe"],
        note: "Uses its bundled git or the system git; both read the global config, so identities and the guard apply.",
    },
    ClientDef {
        id: "sourcetree",
        name: "Sourcetree",
        exes: &["sourcetree.exe"],
        candidates: &["%LOCALAPPDATA%\\SourceTree\\SourceTree.exe", "%ProgramFiles(x86)%\\Atlassian\\Sourcetree\\SourceTree.exe"],
        note: "Embedded or system git (Tools → Options → Git); both read the global config. Keep Sourcetree from rewriting the global config (Tools → Options → General).",
    },
    ClientDef {
        id: "github-desktop",
        name: "GitHub Desktop",
        exes: &["githubdesktop.exe"],
        candidates: &["%LOCALAPPDATA%\\GitHubDesktop\\GitHubDesktop.exe"],
        note: "Bundled git; pushes with the account signed in to the app, whatever the repository. Tying it here is what keeps that account off other repositories.",
    },
    ClientDef {
        id: "gitkraken",
        name: "GitKraken",
        exes: &["gitkraken.exe"],
        candidates: &["%LOCALAPPDATA%\\gitkraken\\gitkraken.exe"],
        note: "Does part of its work through its own engine, so hooks may not run for every action: check the Activity list after a first push.",
    },
    ClientDef {
        id: "tortoisegit",
        name: "TortoiseGit",
        exes: &["tortoisegitproc.exe"],
        candidates: &["%ProgramFiles%\\TortoiseGit\\bin\\TortoiseGitProc.exe"],
        note: "Drives Git for Windows, so identities and the guard apply.",
    },
    ClientDef {
        id: "git-extensions",
        name: "Git Extensions",
        exes: &["gitextensions.exe"],
        candidates: &["%ProgramFiles%\\GitExtensions\\GitExtensions.exe", "%ProgramFiles(x86)%\\GitExtensions\\GitExtensions.exe"],
        note: "Drives Git for Windows, so identities and the guard apply.",
    },
    ClientDef {
        id: "smartgit",
        name: "SmartGit",
        exes: &["smartgit.exe"],
        candidates: &["%ProgramFiles%\\SmartGit\\bin\\smartgit.exe"],
        note: "Drives the git executable set in its preferences, so identities and the guard apply.",
    },
    ClientDef {
        id: "vscode",
        name: "Visual Studio Code",
        exes: &["code.exe"],
        candidates: &["%LOCALAPPDATA%\\Programs\\Microsoft VS Code\\Code.exe", "%ProgramFiles%\\Microsoft VS Code\\Code.exe"],
        note: "Source Control and the integrated terminal both run the system git.",
    },
    ClientDef {
        id: "visual-studio",
        name: "Visual Studio",
        exes: &["devenv.exe"],
        candidates: &[
            "%ProgramFiles%\\Microsoft Visual Studio\\2022\\Community\\Common7\\IDE\\devenv.exe",
            "%ProgramFiles%\\Microsoft Visual Studio\\2022\\Professional\\Common7\\IDE\\devenv.exe",
            "%ProgramFiles%\\Microsoft Visual Studio\\2022\\Enterprise\\Common7\\IDE\\devenv.exe",
        ],
        note: "Bundled git that reads the global config.",
    },
    ClientDef {
        id: "jetbrains",
        name: "JetBrains IDEs",
        exes: &["rider64.exe", "idea64.exe", "pycharm64.exe", "webstorm64.exe", "clion64.exe", "phpstorm64.exe", "goland64.exe", "rustrover64.exe"],
        candidates: &[],
        note: "Rider, IntelliJ IDEA, PyCharm and the others run the system git. Found only while one is running.",
    },
];

pub fn client_def(id: &str) -> Option<&'static ClientDef> {
    CLIENTS.iter().find(|c| c.id == id)
}

/// Name for messages: `Fork`, `the terminal`, or the id itself for something unknown.
pub fn client_name(id: &str) -> String {
    if id == TERMINAL {
        "the terminal".into()
    } else {
        client_def(id).map(|c| c.name.to_string()).unwrap_or_else(|| id.to_string())
    }
}

/// One running process: id, parent id, exe name in lower case.
#[derive(Debug, Clone, PartialEq)]
pub struct Proc {
    pub pid: u32,
    pub parent: u32,
    pub name: String,
}

/// Every running process (empty where the platform has no cheap way to list them).
pub fn process_table() -> Vec<Proc> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
        let mut out = Vec::new();
        // SAFETY: plain Win32 calls on a snapshot handle we own; `entry` is a zeroed POD struct
        // with its size set, as the API requires.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snap, &mut entry) != 0 {
                loop {
                    let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    out.push(Proc { pid: entry.th32ProcessID, parent: entry.th32ParentProcessID, name: String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase() });
                    if Process32NextW(snap, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn client_of_exe(name: &str) -> Option<&'static str> {
    CLIENTS.iter().find(|c| c.exes.contains(&name)).map(|c| c.id)
}

/// The client that (indirectly) started process `pid`: the nearest ancestor that is a known
/// client, else [`TERMINAL`]. `None` when `pid` is not in the table (nothing can be said).
pub fn ancestor_client(table: &[Proc], pid: u32) -> Option<&'static str> {
    let mut current = table.iter().find(|p| p.pid == pid)?;
    let mut seen = vec![pid];
    // Process ids are reused, so a parent id may point at an unrelated newer process or loop.
    for _ in 0..64 {
        let Some(parent) = table.iter().find(|p| p.pid == current.parent && !seen.contains(&p.pid)) else { break };
        if let Some(id) = client_of_exe(&parent.name) {
            return Some(id);
        }
        seen.push(parent.pid);
        current = parent;
    }
    Some(TERMINAL)
}

/// The client above this very process (what the guard hook asks).
pub fn calling_client() -> Option<&'static str> {
    ancestor_client(&process_table(), std::process::id())
}

fn expand(template: &str) -> Option<PathBuf> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('%') {
        let end = rest[start + 1..].find('%')? + start + 1;
        out.push_str(&rest[..start]);
        out.push_str(&std::env::var(&rest[start + 1..end]).ok()?);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(PathBuf::from(out))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStatus {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub exe_path: Option<String>,
    pub running: bool,
    /// The account this client is tied to (`bindings` in the accounts file).
    pub account_id: Option<String>,
    pub note: String,
}

/// Every known client plus the terminal, installed ones first.
pub fn list(cfg: &Config, table: &[Proc]) -> Vec<ClientStatus> {
    let mut out: Vec<ClientStatus> = CLIENTS
        .iter()
        .map(|c| {
            let exe = c.candidates.iter().filter_map(|t| expand(t)).find(|p| p.is_file());
            let running = table.iter().any(|p| c.exes.contains(&p.name.as_str()));
            ClientStatus {
                id: c.id.into(),
                name: c.name.into(),
                installed: exe.is_some() || running,
                exe_path: exe.map(|p| p.to_string_lossy().into_owned()),
                running,
                account_id: cfg.bindings.get(c.id).cloned(),
                note: c.note.into(),
            }
        })
        .collect();
    out.sort_by_key(|c| !c.installed);
    out.push(ClientStatus {
        id: TERMINAL.into(),
        name: "Terminal & scripts".into(),
        installed: true,
        exe_path: None,
        running: false,
        account_id: cfg.bindings.get(TERMINAL).cloned(),
        note: "Git typed in a shell, an IDE terminal or run by a script: anything not started by a client above.".into(),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, parent: u32, name: &str) -> Proc {
        Proc { pid, parent, name: name.into() }
    }

    #[test]
    fn finds_the_client_above_the_hook() {
        let table = [p(4, 0, "system"), p(100, 4, "explorer.exe"), p(200, 100, "fork.exe"), p(300, 200, "git.exe"), p(400, 300, "sh.exe"), p(500, 400, "switchgit.exe"), p(600, 100, "windowsterminal.exe"), p(700, 600, "pwsh.exe"), p(800, 700, "git.exe"), p(900, 800, "switchgit.exe")];
        assert_eq!(ancestor_client(&table, 500), Some("fork"));
        assert_eq!(ancestor_client(&table, 900), Some(TERMINAL));
        assert_eq!(ancestor_client(&table, 12345), None, "unknown pid");
        // A parent id cycle (reused ids) ends instead of spinning.
        let cycle = [p(1, 2, "a.exe"), p(2, 1, "b.exe")];
        assert_eq!(ancestor_client(&cycle, 1), Some(TERMINAL));
        // The nearest client wins: git from VS Code's terminal started inside… Fork? Still VS Code.
        let nested = [p(1, 0, "fork.exe"), p(2, 1, "code.exe"), p(3, 2, "git.exe")];
        assert_eq!(ancestor_client(&nested, 3), Some("vscode"));
    }

    #[test]
    fn lists_clients_with_bindings() {
        let mut cfg = Config::default();
        cfg.bindings.insert("fork".into(), "personal".into());
        cfg.bindings.insert(TERMINAL.into(), "work".into());
        let table = [p(1, 0, "githubdesktop.exe")];
        let list = list(&cfg, &table);
        assert_eq!(list.len(), CLIENTS.len() + 1);
        let get = |id: &str| list.iter().find(|c| c.id == id).unwrap();
        assert!(get("github-desktop").running && get("github-desktop").installed);
        assert_eq!(get("fork").account_id.as_deref(), Some("personal"));
        assert_eq!(list.last().map(|c| (c.id.as_str(), c.account_id.as_deref())), Some((TERMINAL, Some("work"))));
        assert_eq!((client_name("sourcetree"), client_name(TERMINAL), client_name("x")), ("Sourcetree".into(), "the terminal".into(), "x".into()));
    }

    #[cfg(windows)]
    #[test]
    fn process_table_has_this_process() {
        let table = process_table();
        assert!(table.iter().any(|p| p.pid == std::process::id()));
        assert!(calling_client().is_some());
        assert_eq!(expand("%LOCALAPPDATA%\\x").map(|p| p.ends_with("x")), Some(true));
        assert_eq!(expand("%NO_SUCH_VAR_HERE%\\x"), None);
    }
}
