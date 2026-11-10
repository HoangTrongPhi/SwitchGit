import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect } from "react";
import { routeDrop } from "../lib/dropRouter";
import { inTauri } from "../lib/ipc";
import { uiLog, useAppStore } from "../stores/appStore";

function handleDrop(paths: string[]) {
  const s = useAppStore.getState();
  s.setDragActive(false);
  if (!paths.length) return;
  s.setDroppedPaths(paths);
  uiLog("info", `Dropped ${paths.length === 1 ? paths[0] : `${paths.length} items`}`);
  routeDrop(s.activeTab, paths);
}

/** Window-wide drag & drop onto the active module. Tauri reports real paths; a plain browser only names. */
export function useWindowDragDrop() {
  useEffect(() => {
    const setActive = useAppStore.getState().setDragActive;

    if (inTauri) {
      const unlisten = getCurrentWebview().onDragDropEvent(({ payload }) => {
        if (payload.type === "enter" || payload.type === "over") setActive(true);
        else if (payload.type === "leave") setActive(false);
        else if (payload.type === "drop") handleDrop(payload.paths);
      });
      return () => void unlisten.then((fn) => fn());
    }

    let depth = 0;
    const hasFiles = (e: DragEvent) => !!e.dataTransfer?.types.includes("Files");
    const onEnter = (e: DragEvent) => hasFiles(e) && ++depth === 1 && setActive(true);
    const onLeave = (e: DragEvent) => hasFiles(e) && --depth === 0 && setActive(false);
    const onOver = (e: DragEvent) => hasFiles(e) && e.preventDefault();
    const onDrop = (e: DragEvent) => {
      if (!hasFiles(e)) return;
      e.preventDefault();
      depth = 0;
      handleDrop(Array.from(e.dataTransfer!.files, (f) => f.name));
    };
    window.addEventListener("dragenter", onEnter);
    window.addEventListener("dragleave", onLeave);
    window.addEventListener("dragover", onOver);
    window.addEventListener("drop", onDrop);
    return () => {
      window.removeEventListener("dragenter", onEnter);
      window.removeEventListener("dragleave", onLeave);
      window.removeEventListener("dragover", onOver);
      window.removeEventListener("drop", onDrop);
    };
  }, []);
}
