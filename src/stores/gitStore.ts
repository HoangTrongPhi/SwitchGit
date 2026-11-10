import { create } from "zustand";
import { registerDropHandler } from "../lib/dropRouter";
import { inTauri, ipc, on } from "../lib/ipc";
import type { Account, ApplyReport, ClientStatus, GuardEvent, GuardSettings, JobProgressEvent, ScanReport, Snapshot } from "../types/switchgit";
import { errorText, uiLog } from "./appStore";

/** The running scan as the UI shows it (JobProgress). */
export interface RunningJob extends JobProgressEvent {
  startedAt: number;
  cancelling: boolean;
}

interface GitState {
  /** Accounts, ties, guard settings and whether git is in line with them; null until loaded. */
  snapshot: Snapshot | null;
  /** The last failed change, shown above the tabs until the next one succeeds. */
  error: string | null;
  /** Id of what is being saved right now (an account id, a client id, "guard", "apply"). */
  busy: string | null;
  refresh: () => Promise<void>;
  saveAccount: (account: Account) => Promise<boolean>;
  deleteAccount: (id: string) => Promise<void>;
  setEnabled: (id: string, enabled: boolean) => Promise<void>;
  setBinding: (client: string, accountId: string | null) => Promise<void>;
  setGuard: (guard: GuardSettings) => Promise<void>;
  lastApply: ApplyReport | null;
  apply: () => Promise<void>;
  dismissError: () => void;

  clients: ClientStatus[];
  loadClients: () => Promise<void>;
  launchClient: (id: string) => Promise<void>;

  roots: string[];
  setRoots: (roots: string[]) => void;
  job: RunningJob | null;
  report: ScanReport | null;
  scanError: string | null;
  scan: () => Promise<void>;
  cancelScan: () => Promise<void>;

  log: GuardEvent[];
  loadLog: () => Promise<void>;
  clearLog: () => Promise<void>;
}

export const useGitStore = create<GitState>()((set, get) => {
  /** Run one changing command: its answer is the new snapshot; a failure leaves the old one. */
  async function change(busy: string, call: () => Promise<Snapshot>): Promise<boolean> {
    if (!inTauri) return false;
    set({ busy });
    try {
      const snapshot = await call();
      set({ snapshot, error: null, roots: get().roots.length ? get().roots : snapshot.config.scanRoots });
      return true;
    } catch (e) {
      set({ error: errorText(e) });
      return false;
    } finally {
      set({ busy: null });
    }
  }

  return {
    snapshot: null,
    error: null,
    busy: null,
    refresh: async () => void (await change("load", ipc.getState)),
    saveAccount: (account) => change(account.id || "new", () => ipc.saveAccount(account)),
    deleteAccount: async (id) => void (await change(id, () => ipc.deleteAccount(id))),
    setEnabled: async (id, enabled) => void (await change(id, () => ipc.setAccountEnabled(id, enabled))),
    async setBinding(client, accountId) {
      if (await change(client, () => ipc.setBinding(client, accountId))) await get().loadClients();
    },
    setGuard: async (guard) => void (await change("guard", () => ipc.setGuard(guard))),
    lastApply: null,
    async apply() {
      if (!inTauri) return;
      set({ busy: "apply" });
      try {
        set({ lastApply: await ipc.applyConfig(false), error: null });
      } catch (e) {
        set({ error: errorText(e) });
      } finally {
        set({ busy: null });
      }
      await get().refresh();
    },
    dismissError: () => set({ error: null }),

    clients: [],
    async loadClients() {
      if (!inTauri) return;
      try {
        set({ clients: await ipc.listClients() });
      } catch (e) {
        uiLog("error", `Clients: ${errorText(e)}`, "clients");
      }
    },
    async launchClient(id) {
      try {
        await ipc.launchClient(id);
        setTimeout(() => void get().loadClients(), 1500);
      } catch (e) {
        uiLog("error", errorText(e), "clients");
        set({ error: errorText(e) });
      }
    },

    roots: [],
    setRoots: (roots) => set({ roots: [...new Set(roots)] }),
    job: null,
    report: null,
    scanError: null,
    async scan() {
      const { roots, job } = get();
      if (!roots.length || job || !inTauri) return;
      set({ job: { jobId: 0, done: 0, total: 0, message: "Starting…", startedAt: Date.now(), cancelling: false }, scanError: null });
      const unlisten = await on.scanProgress((p) => set((s) => (s.job ? { job: { ...s.job, ...p } } : {})));
      try {
        const report = await ipc.scanRepos(roots);
        set({ report });
        if (report.cancelled) uiLog("warn", "The scan was cancelled; the list is partial");
      } catch (e) {
        set({ scanError: errorText(e) });
      } finally {
        unlisten();
        set({ job: null });
      }
    },
    async cancelScan() {
      const job = get().job;
      if (!job || !job.jobId) return;
      set({ job: { ...job, cancelling: true } });
      await ipc.cancelJob(job.jobId);
    },

    log: [],
    async loadLog() {
      if (!inTauri) return;
      try {
        set({ log: await ipc.guardLog(200) });
      } catch (e) {
        uiLog("error", `Activity: ${errorText(e)}`, "guard");
      }
    },
    async clearLog() {
      try {
        await ipc.clearGuardLog();
        set({ log: [] });
      } catch (e) {
        uiLog("error", errorText(e), "guard");
      }
    },
  };
});

export const accountById = (snapshot: Snapshot | null, id: string | null | undefined) => (id ? snapshot?.config.accounts.find((a) => a.id === id) : undefined);

// A dropped or picked folder joins the folders to scan.
registerDropHandler("repos", (paths) => {
  const s = useGitStore.getState();
  s.setRoots([...s.roots, ...paths]);
});
