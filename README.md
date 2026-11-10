# SwitchGit

Git identities that never cross — part of the Phineas suite, installed and updated by
[Abyss](https://github.com/HoangTrongPhi/Abyss). Tauri 2 + Rust + React, Studio Dark UI from `@abyss/ui`.
What it does and how it is built: [docs/SPEC.md](docs/SPEC.md); where it stands:
[docs/PROGRESS.md](docs/PROGRESS.md).

You have a work account and a personal one on the same PC. SwitchGit makes every repository commit as
the account that owns it, and refuses the commits and pushes that would cross over.

- **Accounts** — each with its own name, e-mail, remotes (`github.com/YourCompany`), folders, HTTPS
  sign-in name, SSH key, and an **on/off switch**. Accounts are independent: switching one off never
  touches another. Off means its repositories refuse commits and pushes until it is on again.
- **Clients** — tie Fork, Sourcetree, GitHub Desktop, GitKraken, TortoiseGit, Git Extensions,
  SmartGit, VS Code, Visual Studio, JetBrains IDEs or the terminal to one account. Fork tied to
  Personal cannot commit or push in a Work repository, whoever is signed in to it.
- **Repositories** — scan folders: whose is each repository, and who would commit there right now.
- **Guard** — the rules, the block SwitchGit keeps in the global git config, and a list of what was
  allowed and refused.

## How it works

1. Each account that is on gets `accounts/<id>.gitconfig` (identity, `credential.<host>.username`,
   `core.sshCommand`) in `%LOCALAPPDATA%\com.phineas.switchgit`.
2. A managed block at the end of `~/.gitconfig` includes those files per repository:
   `includeIf "hasconfig:remote.*.url:…"` for remotes and `includeIf "gitdir/i:…"` for folders
   (git 2.36+). Nothing outside the block is touched; the first change keeps the original as
   `.gitconfig.switchgit.bak`.
3. The block also sets `core.hooksPath` to `hooks/`. Its `pre-commit` and `pre-push` call
   `switchgit.exe guard`, which refuses when the repository's account is off, when the commit would
   be authored as someone else, when the push carries commits authored as another of your accounts,
   or when the client that started git (found by walking up the parent processes) is tied to another
   account. Every hook then hands over to the repository's own `.git/hooks` (Git LFS, Husky), so those
   keep working.

The guard prevents mistakes; it is not a lock. `git … --no-verify` skips it, and so does
`SWITCHGIT_BYPASS=1` for one command. For a hard limit, protect branches on the server as well.
SwitchGit cannot see or change which account a client is signed in to.

## Commands

```bash
npm install
npm run check          # typecheck + mock coverage + cargo test: green before every hand-off
npm run dev            # UI only; open http://localhost:1522/?mock for the fake backend
npm run tauri dev      # desktop app with hot reload
npm run tauri build    # release build (unsigned updater artifacts fail; use npm run release)
npm run release -- -Version 0.1.0 -Notes "First release" [-Publish]
```

`@abyss/ui` is linked from `file:../Abyss/packages/abyss-ui`; keep the Abyss folder next to this one
(`npm install` in Abyss first).

To try the app without touching your real git setup, point it somewhere else first:

```powershell
$env:SWITCHGIT_HOME = "D:\tmp\switchgit"; $env:SWITCHGIT_GITCONFIG = "D:\tmp\gitconfig"; npm run tauri dev
```

## Command line

The same exe runs headless when its first argument is a subcommand (anything else opens the window):

```powershell
switchgit.exe status                       # accounts, switches, guard; exit 1 when files are out of date
switchgit.exe off personal                 # the switch, by id or name
switchgit.exe on personal
switchgit.exe apply --dry-run              # which files would change
switchgit.exe apply                        # write them
switchgit.exe scan D:\src --report repos.json --fail-on warn
switchgit.exe --help
```

| Exit code | Meaning |
|---|---|
| 0 | Done; no issue at the `--fail-on` level (default `error`) |
| 1 | Ran, but found issues at that level (`scan`), or files are out of date (`status`) |
| 2 | Bad arguments, an unknown account, a broken accounts file or global config |

In `cmd.exe` use `start /wait switchgit.exe …` to wait for the exit code; PowerShell and CI call it
directly. A pre-build check that no repository would commit under the wrong identity:

```yaml
- name: SwitchGit check
  run: '& "$env:LOCALAPPDATA\SwitchGit\switchgit.exe" scan . --report repos.json --fail-on warn'
  shell: pwsh
```

`switchgit.exe guard <hook> …` is what the installed hooks call; it is not meant to be typed.

## Mock mode

`npm run dev`, then `http://localhost:1522/?mock`: `src/dev/mockBackend.ts` answers every command in
`src/lib/ipc.ts` with sample data (three accounts, two tied clients, repositories in every state, a
guard history), emits scan progress so Cancel works and fakes the dialog / updater plugins.
`?mock=empty` shows the first-run screens. Only in dev builds with `?mock` in the URL, never inside
Tauri. `npm run check:mock` fails when a command has no mock case.

## Project layout

```
src/                     React UI
  main.tsx               imports dev/mockBackend first
  App.tsx                rail + header + active tab + log drawer + dialogs
  components/tabs/       AccountsTab, ClientsTab, ReposTab, GuardTab (rail order: lib/modules.ts)
  components/            AccountDialog, Switch
  lib/ipc.ts             every invoke() and event name
  stores/                zustand: appStore (tabs, log, drag), gitStore (accounts, clients, scan, guard log)
  types/switchgit.ts     TypeScript mirrors of the Rust structs
src-tauri/src/
  main.rs / cli.rs       window or command line, same exe
  lib.rs                 configure(): state + commands, shared with ipc_tests.rs
  commands/              thin IPC wrappers (+ contract tests in mod.rs)
  core/                  no tauri imports
    model.rs             accounts file, paths
    remote.rs            remote URLs and scope patterns
    gitconfig.rs         account files, the managed block, the hooks (dry run, backup)
    guard.rs             the decision, what the hook gathers, the activity log
    clients.rs           known git clients, process ancestry
    repos.rs             repository scan
  utils/                 logbus, jobs (one per kind, cancel), paths (atomic write)
docs/                    SPEC, PROGRESS, CHANGELOG, FORMATS
```

## Releasing

`npm run release` follows the suite's release standard (Abyss `docs/RELEASE_STANDARD.md`): signed NSIS
installer, `latest.json`, `SHA256SUMS.txt`, GitHub release `v<version>` on `HoangTrongPhi/SwitchGit`.
The signing key is `%USERPROFILE%\.tauri\switchgit.key` — back it up. In GitHub Actions,
`.github/workflows/release.yml` does the same with the key in the `TAURI_SIGNING_PRIVATE_KEY` secret.

After the first release, set `"status": "available"` for `switchgit` in Abyss `catalog/catalog.json` and
bump its `revision`. After an update moves the exe, press **Apply** once (or run `switchgit apply`) so
the hooks point at the new path.
