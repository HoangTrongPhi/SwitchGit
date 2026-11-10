// Mirrors of the Rust structs (serde camelCase). Keep every field in sync with src-tauri/src.
import type { AbyssContext } from "@abyss/ui";

export type { LogEntry, LogLevel } from "@abyss/ui";

export type TabId = "accounts" | "clients" | "repos" | "guard";

export interface AppInfo {
  version: string;
  debug: boolean;
  /** Started from the Abyss launcher (which then installs this app's updates). */
  abyss: AbyssContext;
}

export type Severity = "info" | "warn" | "error";

/** `core::Issue`: something to look at; the job carried on past it. */
export interface Issue {
  severity: Severity;
  code: string;
  message: string;
  path?: string;
}

/** `{ jobId, done, total, message }` on every `<module>://progress` event. */
export interface JobProgressEvent {
  jobId: number;
  done: number;
  total: number;
  message: string;
}

// ---------------------------------------------------------------- accounts (core::model)

export type AccountKind = "work" | "personal" | "other";

/** One git identity with its own switch, scope and credentials. */
export interface Account {
  /** Empty for a new account: the backend makes the id from the label. */
  id: string;
  label: string;
  kind: AccountKind;
  /** `user.name` / `user.email` of its commits. */
  name: string;
  email: string;
  enabled: boolean;
  /** `github.com/Owner`, `github.com/Owner/repo`, or a whole host. */
  remotes: string[];
  folders: string[];
  /** Login for HTTPS pushes. */
  username: string | null;
  /** Private key file for SSH remotes. */
  sshKey: string | null;
}

export interface GuardSettings {
  enabled: boolean;
  checkCommit: boolean;
  checkPush: boolean;
  checkAuthors: boolean;
  blockUnknown: boolean;
}

/** `switchgit.json`. */
export interface Config {
  format: string;
  version: number;
  accounts: Account[];
  /** Client id → account id. */
  bindings: Record<string, string>;
  guard: GuardSettings;
  scanRoots: string[];
}

/** `commands::git_cmd::Snapshot`: what every changing command answers with. */
export interface Snapshot {
  config: Config;
  globalName: string | null;
  globalEmail: string | null;
  gitVersion: string | null;
  gitconfigPath: string;
  dataDir: string;
  /** Files that differ from what the accounts ask for. */
  pending: number;
  /** The managed block of the global git config. */
  block: string;
  issues: Issue[];
}

// ---------------------------------------------------------------- apply (core::gitconfig)

export interface FileChange {
  path: string;
  action: "create" | "update" | "remove" | "unchanged";
  kind: "gitconfig" | "account" | "hook";
}

export interface ApplyReport {
  dryRun: boolean;
  changes: FileChange[];
  block: string;
  backup: string | null;
  issues: Issue[];
  elapsedMs: number;
  ok: boolean;
  cancelled: boolean;
}

// ---------------------------------------------------------------- clients (core::clients)

export interface ClientStatus {
  /** `fork`, `sourcetree`, `github-desktop`, …, `terminal`. */
  id: string;
  name: string;
  installed: boolean;
  exePath: string | null;
  running: boolean;
  /** The account this client is tied to. */
  accountId: string | null;
  note: string;
}

// ---------------------------------------------------------------- repositories (core::repos)

export type RepoStatus = "ok" | "wrong-identity" | "account-off" | "mixed" | "unowned";

export interface RepoInfo {
  path: string;
  remotes: { name: string; url: string }[];
  owner: string | null;
  email: string | null;
  identity: string | null;
  status: RepoStatus;
}

export interface ScanReport {
  roots: string[];
  repos: RepoInfo[];
  issues: Issue[];
  elapsedMs: number;
  ok: boolean;
  cancelled: boolean;
}

// ---------------------------------------------------------------- guard (core::guard)

/** One line of the activity list. */
export interface GuardEvent {
  tsMs: number;
  /** `commit` or `push`. */
  hook: string;
  repo: string;
  remote: string | null;
  client: string | null;
  email: string | null;
  allow: boolean;
  code: string;
  message: string;
  owner: string | null;
}

/** `app_data_dir()/settings.json`; each app defines its fields. */
export type AppSettings = Record<string, unknown>;
