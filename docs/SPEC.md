# SwitchGit — Spec

> Follows `Abyss/docs/app-specs/00_SHARED_FRAMEWORK.md`. Written with the app (2026-11-10); the
> shared framework wins where this file is silent.

## Problem

One PC, several git accounts (work, personal). Git has one global identity, clients keep their own
sign-ins, and nothing stops a commit authored as the company from landing in a personal repository,
or the other way round. The user also wants Fork to be the personal client and Sourcetree the work
client, without mixing them up.

## Goals

1. Every repository commits as the account that owns it, without switching anything by hand.
2. Cross-account commits and pushes are refused, with a message that says why and what to do.
3. Accounts are independent, each with an on/off switch. Off: its repositories refuse work.
4. A git client can be tied to one account.
5. Nothing of the user's is lost: only a marked block of the global git config is written, with a
   backup of the original and a dry run; repositories are never modified.

## Non-goals

- Signing in to or out of clients, or reading their stored accounts and tokens.
- Storing passwords or tokens. HTTPS credentials stay in Git Credential Manager; SwitchGit only
  names which login to use (`credential.<host>.username`).
- A hard security boundary. Client-side hooks can be skipped; server-side branch protection is the
  hard limit.

## Modules

| Tab | What |
|---|---|
| Accounts | Cards with the switch; add / edit / remove; scope = remotes + folders |
| Clients | Detected clients (installed, running), tie each to an account, open it |
| Repositories | Scan folders; status per repository; give an unowned remote to an account |
| Guard | Rules, managed block preview, Apply, activity (allowed / refused / bypassed) |

## Rules of the guard (`core/guard.rs`, `decide`)

A repository belongs to the account with the most specific matching remote; failing that, the
longest matching folder. Remotes of different accounts and no folder: *mixed*, commits pass and each
push is judged by the remote it goes to.

| Situation | Commit | Push |
|---|---|---|
| Owner is switched off | refused | refused |
| Client is tied to another account | refused | refused |
| `user.email` is not the owner's | refused | — |
| Pushed commits authored as another known account | — | refused |
| No account owns it | passes (refused with "Refuse unowned repositories") | same |
| `SWITCHGIT_BYPASS=1` | passes, logged as bypassed | same |

Anything that fails inside SwitchGit itself (unreadable accounts file, missing exe) lets git carry on.

## Phases

- P0 Scaffold from the Abyss template — done.
- P1 Accounts, managed git config, guard hooks, client ties, repository scan, CLI — done (0.1.0).
- P2 Tray icon with the switches; start with Windows; notification when the guard refuses.
- P3 Fix-ups: rewrite the author of unpushed commits; clone with an account; SSH key generation.
- P4 Schedules (work account off outside office hours); token / key expiry reminders; profile export.

## Open questions

- Product name: the suite uses sea names (Coral, Kraken…). Kept `SwitchGit` (the folder's name).
- Should a tied client also be refused in *unowned* repositories? Currently it passes.
