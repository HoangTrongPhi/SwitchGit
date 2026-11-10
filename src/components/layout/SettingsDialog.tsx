import { AppearanceSettings, Modal } from "@abyss/ui";
import { useAppStore } from "../../stores/appStore";
import { useAppearance } from "../../stores/appearance";

/** Appearance (shared) plus the app's own settings, which go below it. */
export function SettingsDialog() {
  const open = useAppStore((s) => s.settingsOpen);
  const setOpen = useAppStore((s) => s.setSettingsOpen);
  return (
    <Modal open={open} title="Settings" onClose={() => setOpen(false)}>
      <AppearanceSettings useAppearance={useAppearance} />
    </Modal>
  );
}
