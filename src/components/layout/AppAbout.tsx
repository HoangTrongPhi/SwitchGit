import { AboutDialog } from "@abyss/ui";
import { AUTHOR } from "../../lib/author";
import { inTauri, ipc } from "../../lib/ipc";
import { errorText, uiLog, useAppStore } from "../../stores/appStore";
import { useUpdateStore } from "../../stores/updateStore";

/** The shared About & contact dialog, wired to this app's state and `open_url`. */
export function AppAbout() {
  const open = useAppStore((s) => s.aboutOpen);
  const setOpen = useAppStore((s) => s.setAboutOpen);
  const info = useAppStore((s) => s.appInfo);
  return (
    <AboutDialog
      open={open}
      onClose={() => setOpen(false)}
      author={AUTHOR}
      appName="SwitchGit"
      tagline="Git identities that never cross"
      version={info?.version}
      onCheckUpdates={() => void useUpdateStore.getState().check()}
      onOpenUrl={inTauri ? (url) => void ipc.openUrl(url).catch((e) => uiLog("error", errorText(e))) : undefined}
      onCopied={(what) => uiLog("info", `Copied ${what}`)}
    />
  );
}
