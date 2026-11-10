import type { PaletteItem } from "@abyss/ui";
import { createElement } from "react";
import { FolderPlus, PanelBottom, Power, RefreshCw, Settings2, ShieldCheck, Trash2, UserRound, type LucideIcon } from "lucide-react";
import { useAppStore } from "../stores/appStore";
import { useGitStore } from "../stores/gitStore";
import { useUpdateStore } from "../stores/updateStore";
import { pickPaths } from "./dialogs";
import { MODULES } from "./modules";
import { runActiveModule } from "./rerun";

const icon = (I: LucideIcon) => createElement(I, { strokeWidth: 1.75 });

/** Everything reachable from the command palette (Ctrl+K). Modules append their actions here. */
export function buildCommands(): PaletteItem[] {
  const s = useAppStore.getState();
  const git = useGitStore.getState();
  const accounts = git.snapshot?.config.accounts ?? [];
  return [
    ...MODULES.map<PaletteItem>((m) => ({
      id: `nav.${m.id}`,
      label: `Go to ${m.label}`,
      group: "Navigate",
      icon: icon(m.icon),
      hint: `Ctrl+${m.hotkey}`,
      keywords: m.description,
      run: () => s.setActiveTab(m.id),
    })),
    // One entry per account: the switch, from anywhere.
    ...accounts.map<PaletteItem>((a) => ({
      id: `account.${a.id}`,
      label: `Switch ${a.label} ${a.enabled ? "off" : "on"}`,
      group: "Accounts",
      icon: icon(Power),
      keywords: `${a.email} ${a.kind} toggle enable disable`,
      run: () => git.setEnabled(a.id, !a.enabled),
    })),
    { id: "act.apply", label: "Apply git configuration", group: "Actions", icon: icon(ShieldCheck), keywords: "write gitconfig hooks repair", run: () => git.apply() },
    {
      id: "act.scanFolder",
      label: "Scan a folder for repositories…",
      group: "Actions",
      icon: icon(FolderPlus),
      keywords: "browse directory repos",
      run: async () => {
        s.setActiveTab("repos");
        if ((await pickPaths("folder")).length) await useGitStore.getState().scan();
      },
    },
    { id: "act.rerun", label: "Refresh the current tab", group: "Actions", icon: icon(RefreshCw), hint: "F5", keywords: "refresh rerun reload", run: runActiveModule },
    { id: "app.checkUpdates", label: "Check for updates…", group: "Actions", icon: icon(RefreshCw), keywords: "update upgrade version", run: () => useUpdateStore.getState().check() },
    { id: "view.toggleLog", label: "Toggle log drawer", group: "View", icon: icon(PanelBottom), hint: "Ctrl+`", keywords: "console terminal output", run: () => s.toggleLogDrawer() },
    { id: "view.settings", label: "Settings…", group: "View", icon: icon(Settings2), hint: "Ctrl+,", keywords: "theme dark light font size appearance preferences", run: () => s.setSettingsOpen(true) },
    { id: "view.about", label: "About & contact", group: "View", icon: icon(UserRound), keywords: "author contact version help", run: () => s.setAboutOpen(true) },
    { id: "view.clearLog", label: "Clear log", group: "View", icon: icon(Trash2), keywords: "console reset", run: () => s.clearLogs() },
  ];
}
