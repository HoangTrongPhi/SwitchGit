import { useAppStore } from "../stores/appStore";
import { useGitStore } from "../stores/gitStore";
import type { TabId } from "../types/switchgit";

/** F5 / "Run again": what each module re-runs. */
const RERUN: Record<TabId, () => unknown> = {
  accounts: () => useGitStore.getState().refresh(),
  clients: () => useGitStore.getState().loadClients(),
  repos: () => useGitStore.getState().scan(),
  guard: () => Promise.all([useGitStore.getState().refresh(), useGitStore.getState().loadLog()]),
};

export function runActiveModule() {
  void RERUN[useAppStore.getState().activeTab]?.();
}
