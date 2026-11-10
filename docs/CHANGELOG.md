# SwitchGit — Changelog

All user-visible changes. Newest first; one section per released version.

## Unreleased

## 0.1.0 — 2026-11-10
- Accounts with their own on/off switch, remotes, folders, HTTPS sign-in name and SSH key.
- Every repository commits as the account that owns its remote or folder.
- Guard: refuses commits and pushes that cross accounts, in every client and terminal; your
  repositories' own hooks (Git LFS, Husky) still run.
- Clients: tie Fork, Sourcetree, GitHub Desktop and others to one account.
- Repositories: scan folders to see whose each repository is and who commits there.
- Command line: `status`, `on`, `off`, `apply`, `scan`.
- Created from the Abyss app template.
