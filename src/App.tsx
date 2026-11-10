import { CommandPalette, LogDrawer } from "@abyss/ui";
import { Loader2 } from "lucide-react";
import { Suspense, useMemo, type ComponentType } from "react";
import { AppAbout } from "./components/layout/AppAbout";
import { HeaderBar } from "./components/layout/HeaderBar";
import { NavigationRail } from "./components/layout/NavigationRail";
import { SettingsDialog } from "./components/layout/SettingsDialog";
import { AccountsTab } from "./components/tabs/AccountsTab";
import { ClientsTab } from "./components/tabs/ClientsTab";
import { GuardTab } from "./components/tabs/GuardTab";
import { ReposTab } from "./components/tabs/ReposTab";
import { useBackendBridge } from "./hooks/useBackendBridge";
import { useGlobalHotkeys } from "./hooks/useGlobalHotkeys";
import { useWindowDragDrop } from "./hooks/useWindowDragDrop";
import { buildCommands } from "./lib/commands";
import { errorText, uiLog, useAppStore } from "./stores/appStore";
import type { TabId } from "./types/switchgit";

// Heavy modules (three.js, uPlot, big canvases) go in with lazy(() => import(…)) to keep startup light.
const TABS: Record<TabId, ComponentType> = {
  accounts: AccountsTab,
  clients: ClientsTab,
  repos: ReposTab,
  guard: GuardTab,
};

export default function App() {
  useBackendBridge();
  useGlobalHotkeys();
  useWindowDragDrop();

  const activeTab = useAppStore((s) => s.activeTab);
  const logs = useAppStore((s) => s.logs);
  const logOpen = useAppStore((s) => s.logDrawerOpen);
  const paletteOpen = useAppStore((s) => s.paletteOpen);
  const { toggleLogDrawer, clearLogs, setPaletteOpen } = useAppStore.getState();
  const commands = useMemo(() => (paletteOpen ? buildCommands() : []), [paletteOpen]);
  const ActiveTab = TABS[activeTab];

  return (
    <div className="flex h-full">
      <NavigationRail />
      <div className="flex min-w-0 flex-1 flex-col">
        <HeaderBar />
        <main className="min-h-0 flex-1 overflow-y-auto">
          <Suspense
            fallback={
              <div className="grid h-full place-items-center text-ink-faint">
                <Loader2 className="size-5 animate-spin" />
              </div>
            }
          >
            <ActiveTab />
          </Suspense>
        </main>
        <LogDrawer entries={logs} open={logOpen} onToggle={() => toggleLogDrawer()} onClear={clearLogs} />
      </div>
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} items={commands} onError={(item, err) => uiLog("error", `${item.label} failed: ${errorText(err)}`)} />
      <AppAbout />
      <SettingsDialog />
    </div>
  );
}
