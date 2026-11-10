import { create } from "zustand";
import { inTauri, ipc } from "../lib/ipc";
import type { AppInfo, LogEntry, LogLevel, TabId } from "../types/switchgit";

const MAX_LOG_LINES = 2000;

interface AppState {
  activeTab: TabId;
  setActiveTab: (tab: TabId) => void;

  paletteOpen: boolean;
  setPaletteOpen: (open: boolean) => void;
  aboutOpen: boolean;
  setAboutOpen: (open: boolean) => void;
  settingsOpen: boolean;
  setSettingsOpen: (open: boolean) => void;

  logDrawerOpen: boolean;
  toggleLogDrawer: (open?: boolean) => void;
  logs: LogEntry[];
  addLogs: (entries: LogEntry[]) => void;
  clearLogs: () => void;

  appInfo: AppInfo | null;
  setAppInfo: (info: AppInfo) => void;

  /** A drag carrying files is hovering the window. */
  dragActive: boolean;
  setDragActive: (active: boolean) => void;
  /** Paths most recently dropped onto the window or picked from a dialog. */
  droppedPaths: string[];
  setDroppedPaths: (paths: string[]) => void;
}

export const useAppStore = create<AppState>()((set) => ({
  activeTab: "accounts",
  setActiveTab: (activeTab) => set({ activeTab }),

  paletteOpen: false,
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  aboutOpen: false,
  setAboutOpen: (aboutOpen) => set({ aboutOpen }),
  settingsOpen: false,
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),

  logDrawerOpen: false,
  toggleLogDrawer: (open) => set((s) => ({ logDrawerOpen: open ?? !s.logDrawerOpen })),
  logs: [],
  addLogs: (entries) =>
    set((s) => {
      const logs = s.logs.concat(entries);
      return { logs: logs.length > MAX_LOG_LINES ? logs.slice(-MAX_LOG_LINES) : logs };
    }),
  clearLogs: () => set({ logs: [] }),

  appInfo: null,
  setAppInfo: (appInfo) => set({ appInfo }),

  dragActive: false,
  setDragActive: (dragActive) => set({ dragActive }),
  droppedPaths: [],
  setDroppedPaths: (droppedPaths) => set({ droppedPaths }),
}));

/** Log from the UI side into the same drawer the backend writes to (and to the app's stderr). */
export function uiLog(level: LogLevel, message: string, source = "ui") {
  useAppStore.getState().addLogs([{ tsMs: Date.now(), level, source, message }]);
  if (inTauri) ipc.uiLog(level, message).catch(() => {});
}

/** Message of an invoke rejection (Rust commands reject with a string). */
export const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e));
