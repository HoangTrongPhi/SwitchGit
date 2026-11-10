import type { TabId } from "../types/switchgit";

type DropHandler = (paths: string[]) => void;

const handlers: Partial<Record<TabId, DropHandler>> = {};

/** Module stores register here so dropped or picked paths reach the tab that should load them. */
export function registerDropHandler(tab: TabId, handler: DropHandler) {
  handlers[tab] = handler;
}

export function routeDrop(tab: TabId, paths: string[]) {
  handlers[tab]?.(paths);
}
