// Every invoke() and event name of the app lives here (and in dev/mockBackend.ts, which answers them in ?mock).
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Account, AppInfo, AppSettings, ApplyReport, ClientStatus, GuardEvent, GuardSettings, JobProgressEvent, LogEntry, ScanReport, Snapshot } from "../types/switchgit";

/** False in a plain browser (`npm run dev` without `?mock`): backend calls are skipped. */
export const inTauri = isTauri();

export const EVENTS = {
  log: "log://entry",
  repos: "repos://progress",
} as const;

export const ipc = {
  appInfo: () => invoke<AppInfo>("app_info"),
  logHistory: () => invoke<LogEntry[]>("log_history"),
  uiLog: (level: string, message: string) => invoke<void>("ui_log", { level, message }),
  cancelJob: (jobId: number) => invoke<boolean>("cancel_job", { jobId }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  revealInExplorer: (path: string) => invoke<void>("reveal_in_explorer", { path }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) => invoke<void>("set_settings", { settings }),

  getState: () => invoke<Snapshot>("get_state"),
  saveAccount: (account: Account) => invoke<Snapshot>("save_account", { account }),
  deleteAccount: (id: string) => invoke<Snapshot>("delete_account", { id }),
  setAccountEnabled: (id: string, enabled: boolean) => invoke<Snapshot>("set_account_enabled", { id, enabled }),
  setBinding: (client: string, accountId: string | null) => invoke<Snapshot>("set_binding", { client, accountId }),
  setGuard: (guard: GuardSettings) => invoke<Snapshot>("set_guard", { guard }),
  applyConfig: (dryRun: boolean) => invoke<ApplyReport>("apply_config", { dryRun }),

  listClients: () => invoke<ClientStatus[]>("list_clients"),
  launchClient: (id: string) => invoke<void>("launch_client", { id }),

  scanRepos: (roots: string[]) => invoke<ScanReport>("scan_repos", { roots }),

  guardLog: (limit: number) => invoke<GuardEvent[]>("guard_log", { limit }),
  clearGuardLog: () => invoke<void>("clear_guard_log"),
};

export const on = {
  log: (cb: (e: LogEntry) => void): Promise<UnlistenFn> => listen<LogEntry>(EVENTS.log, (e) => cb(e.payload)),
  scanProgress: (cb: (e: JobProgressEvent) => void): Promise<UnlistenFn> => listen<JobProgressEvent>(EVENTS.repos, (e) => cb(e.payload)),
};
