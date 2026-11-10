# SwitchGit — Progress

## Status
- Current phase: P1 — Accounts, guard, clients, scan, CLI (done; released as 0.1.0 on 2026-11-10)
- Last session: 2026-11-10 — app created from templates/tauri-app; sample module replaced by the four
  tabs; core, commands, CLI and tests written; guard tried end to end with real git in an isolated
  global config (identity by remote and folder, refused commit / push, bypass, hand-over to a
  repository's own pre-push with its stdin)
- `npm run check`: green (typecheck, 20 commands mocked, 40 Rust tests)

## Phases
- [x] P0 Scaffold
- [x] P1 Accounts · managed git config · guard hooks · client ties · repository scan · CLI
- [ ] P2 Tray icon with the switches, start with Windows, notification on refusal
- [ ] P3 Fix author of unpushed commits, clone with an account, SSH key generation
- [ ] P4 Schedules, expiry reminders, profile export

## Decisions (answers to the spec's open questions)
- 2026-11-10 — Enforcement is by repository owner (remote, then folder), not by branch name (user:
  work vs personal accounts, Fork = personal, Sourcetree = work).
- 2026-11-10 — Client ties are enforced in the hook by process ancestry; client sign-ins are not read.
- 2026-11-10 — The CLI and hooks use `%LOCALAPPDATA%\com.phineas.switchgit` directly (same folder as
  Tauri's `app_local_data_dir`), overridable with `SWITCHGIT_HOME` / `SWITCHGIT_GITCONFIG`.
- 2026-11-10 — Template parts removed: Folder Stats sample, watch (`utils/watch.rs`, `watch_cmd.rs`)
  and their dependencies (notify, ctrlc, rayon, globset). Added `windows-sys` (process list, tiny).
- 2026-11-10 — `Switch` lives in `src/components/Switch.tsx`; candidate to move into `@abyss/ui`.

## Known issues / next steps
- Not yet run as the real desktop window against the real `~/.gitconfig`; UI checked in `?mock`,
  backend through tests and an isolated end-to-end run.
- Not verified inside Fork / Sourcetree / GitHub Desktop themselves (client detection is unit-tested
  on a process table and seen working for the terminal).
- GitKraken does part of its work without the git executable; its hooks may not always run.
- `hasconfig:remote.*.url` globs are case-sensitive: a remote written `github.com/magetech/...` is
  not matched by scope `github.com/MageTech` for the identity (the guard, which ignores case, then
  refuses the commit and says so).
- After an app update moves the exe, hooks point at the old path until Apply (a missing exe never
  blocks git).
- The app icon is still the template placeholder.
- Abyss catalog: SwitchGit is `available` since revision 6 (pushed 2026-11-10).
