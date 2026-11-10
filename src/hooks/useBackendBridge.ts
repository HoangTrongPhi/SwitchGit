import { useEffect } from "react";
import { inTauri, ipc, on } from "../lib/ipc";
import { errorText, uiLog, useAppStore } from "../stores/appStore";
import { useGitStore } from "../stores/gitStore";
import { startupUpdateCheck } from "../stores/updateStore";

if (!inTauri) {
  uiLog("warn", "Running outside Tauri — backend commands are unavailable (open with ?mock for sample data).");
}

/** Subscribes to backend log events, loads app info once at startup, then checks for updates. */
export function useBackendBridge() {
  useEffect(() => {
    if (!inTauri) return;
    const { addLogs, setAppInfo } = useAppStore.getState();
    let disposed = false;
    // Listen first, then backfill, so no entry falls between the two.
    const unlisten = on.log((entry) => addLogs([entry]));
    unlisten
      .then(() => ipc.logHistory())
      .then((history) => !disposed && addLogs(history))
      .catch((err) => uiLog("error", `Log bridge failed: ${errorText(err)}`));
    ipc
      .appInfo()
      .then((info) => {
        if (disposed) return;
        setAppInfo(info);
        void startupUpdateCheck(info);
      })
      .catch((err) => uiLog("error", `app_info failed: ${errorText(err)}`));
    // Accounts and clients, for every tab; again whenever the window gets the focus back, since
    // the command line, a hook or a client starting may have changed them meanwhile.
    const git = useGitStore.getState();
    const reload = () => {
      if (useGitStore.getState().busy) return;
      void git.refresh();
      void git.loadClients();
    };
    reload();
    window.addEventListener("focus", reload);
    return () => {
      window.removeEventListener("focus", reload);
      disposed = true;
      void unlisten.then((fn) => fn());
    };
  }, []);
}
