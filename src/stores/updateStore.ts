import { shouldSelfUpdate } from "@abyss/ui";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { create } from "zustand";
import { inTauri } from "../lib/ipc";
import type { AppInfo } from "../types/switchgit";

/**
 * Check for updates against the `latest.json` of the newest GitHub release (`plugins.updater` in
 * tauri.conf.json); the installer is verified with the signing public key before it runs.
 * Started from Abyss, the app does not check on its own: Abyss shows the update and installs it
 * when the app closes, so the user is never asked twice.
 */
export type UpdatePhase = "idle" | "checking" | "upToDate" | "available" | "downloading" | "installing" | "error";

interface UpdateState {
  phase: UpdatePhase;
  available: { version: string; notes?: string } | null;
  error: string | null;
  check: () => Promise<void>;
  installAndRestart: () => Promise<void>;
}

let pending: Update | null = null;

export const useUpdateStore = create<UpdateState>()((set) => ({
  phase: "idle",
  available: null,
  error: null,

  async check() {
    if (!inTauri) return;
    set({ phase: "checking", error: null });
    try {
      pending = await check();
      set(pending ? { phase: "available", available: { version: pending.version, notes: pending.body } } : { phase: "upToDate", available: null });
    } catch (e) {
      set({ phase: "error", error: String(e) });
    }
  },

  async installAndRestart() {
    if (!pending) return;
    set({ phase: "downloading" });
    try {
      await pending.downloadAndInstall((e) => e.event === "Finished" && set({ phase: "installing" }));
      await relaunch();
    } catch (e) {
      set({ phase: "error", error: String(e) });
    }
  },
}));

/** Once per launch, release builds only, and only when Abyss is not managing updates. */
export async function startupUpdateCheck(info: AppInfo) {
  if (!inTauri || import.meta.env.DEV || info.debug) return;
  if (shouldSelfUpdate(info.abyss)) setTimeout(() => void useUpdateStore.getState().check(), 4000);
}
