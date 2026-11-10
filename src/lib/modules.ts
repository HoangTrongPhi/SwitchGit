import { AppWindow, FolderGit2, ShieldCheck, UsersRound, type LucideIcon } from "lucide-react";
import type { TabId } from "../types/switchgit";

export interface ModuleDef {
  id: TabId;
  label: string;
  description: string;
  icon: LucideIcon;
  /** Ctrl+<digit> shortcut. */
  hotkey: string;
  /** What a drop on this module loads (dropRouter / useWindowDragDrop). */
  accepts: "folder" | "files" | "any";
}

/** The app's modules, in navigation order (the rail, Ctrl+1…9, the command palette). */
export const MODULES: ModuleDef[] = [
  { id: "accounts", label: "Accounts", description: "Git identities with their on/off switch", icon: UsersRound, hotkey: "1", accepts: "any" },
  { id: "clients", label: "Clients", description: "Tie Fork, Sourcetree, GitHub Desktop and others to an account", icon: AppWindow, hotkey: "2", accepts: "any" },
  { id: "repos", label: "Repositories", description: "Which account owns each repository and who commits there", icon: FolderGit2, hotkey: "3", accepts: "folder" },
  { id: "guard", label: "Guard", description: "Hooks that stop cross-account commits and pushes, and what they did", icon: ShieldCheck, hotkey: "4", accepts: "any" },
];

export const moduleById = (id: TabId) => MODULES.find((m) => m.id === id) ?? MODULES[0];
