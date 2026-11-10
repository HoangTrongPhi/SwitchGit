import { RailButton } from "@abyss/ui";
import { Command, Settings2, Terminal, UserRound } from "lucide-react";
import { MODULES } from "../../lib/modules";
import { useAppStore } from "../../stores/appStore";

export function NavigationRail() {
  const activeTab = useAppStore((s) => s.activeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const logOpen = useAppStore((s) => s.logDrawerOpen);
  const errors = useAppStore((s) => s.logs.filter((l) => l.level === "error").length);
  const toggleLog = useAppStore((s) => s.toggleLogDrawer);
  const setPaletteOpen = useAppStore((s) => s.setPaletteOpen);
  const setAboutOpen = useAppStore((s) => s.setAboutOpen);
  const setSettingsOpen = useAppStore((s) => s.setSettingsOpen);

  return (
    <nav className="flex w-14 shrink-0 flex-col items-center gap-1 border-r border-edge bg-panel py-3">
      <img src="/app-icon.svg" alt="SwitchGit" className="mb-3 size-8" draggable={false} />
      {MODULES.map((m) => (
        <RailButton key={m.id} label={m.label} shortcut={`Ctrl+${m.hotkey}`} active={activeTab === m.id} onClick={() => setActiveTab(m.id)}>
          <m.icon className="size-[18px]" strokeWidth={1.75} />
        </RailButton>
      ))}
      <div className="flex-1" />
      <RailButton label="Settings" shortcut="Ctrl+," onClick={() => setSettingsOpen(true)}>
        <Settings2 className="size-[18px]" strokeWidth={1.75} />
      </RailButton>
      <RailButton label="About & contact" onClick={() => setAboutOpen(true)}>
        <UserRound className="size-[18px]" strokeWidth={1.75} />
      </RailButton>
      <RailButton label="Command palette" shortcut="Ctrl+K" onClick={() => setPaletteOpen(true)}>
        <Command className="size-[18px]" strokeWidth={1.75} />
      </RailButton>
      <RailButton label={logOpen ? "Hide log" : "Show log"} shortcut="Ctrl+`" active={logOpen} badge={logOpen ? 0 : errors} onClick={() => toggleLog()}>
        <Terminal className="size-[18px]" strokeWidth={1.75} />
      </RailButton>
    </nav>
  );
}
