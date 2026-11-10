import { open, save, type DialogFilter } from "@tauri-apps/plugin-dialog";
import { uiLog, useAppStore } from "../stores/appStore";
import { routeDrop } from "./dropRouter";
import { inTauri } from "./ipc";

function requireTauri(): boolean {
  if (!inTauri) uiLog("warn", "Native dialogs are only available inside the desktop app.");
  return inTauri;
}

/** Folder / file pickers whose result is handled like a drop on the active tab. */
export async function pickPaths(kind: "folder" | "files", filters?: DialogFilter[]): Promise<string[]> {
  if (!requireTauri()) return [];
  const result = await open(kind === "folder" ? { directory: true } : { multiple: true, filters });
  const paths = result == null ? [] : Array.isArray(result) ? result : [result];
  if (paths.length) {
    const s = useAppStore.getState();
    s.setDroppedPaths(paths);
    uiLog("info", `Selected ${paths.length === 1 ? paths[0] : `${paths.length} items`}`);
    routeDrop(s.activeTab, paths);
  }
  return paths;
}

/** Pick a single file or folder for a form field (not routed anywhere). */
export async function pickOne(kind: "folder" | "file", filters?: DialogFilter[]): Promise<string | null> {
  if (!requireTauri()) return null;
  const result = await open(kind === "folder" ? { directory: true } : { filters });
  return typeof result === "string" ? result : null;
}

export async function pickSavePath(defaultPath?: string, filters?: DialogFilter[]): Promise<string | null> {
  if (!requireTauri()) return null;
  return save({ defaultPath, filters });
}
