import { cn, EmptyState, GlassCard, PathText, PillBadge } from "@abyss/ui";
import { RefreshCw, ShieldCheck, ShieldOff, Trash2 } from "lucide-react";
import { useEffect } from "react";
import { inTauri, ipc } from "../../lib/ipc";
import { accountById, useGitStore } from "../../stores/gitStore";
import type { GuardEvent, GuardSettings } from "../../types/switchgit";
import { Switch } from "../Switch";
import { IssueList, StateBanner } from "./AccountsTab";

const RULES: { key: keyof Omit<GuardSettings, "enabled">; label: string; hint: string }[] = [
  { key: "checkCommit", label: "Check commits", hint: "Refuse a commit authored as anyone but the repository's account." },
  { key: "checkPush", label: "Check pushes", hint: "Refuse a push to a repository whose account is off or that the client is not tied to." },
  { key: "checkAuthors", label: "Check authors of pushed commits", hint: "Refuse a push that carries commits authored as another of your accounts." },
  { key: "blockUnknown", label: "Refuse unowned repositories", hint: "Also refuse repositories no account owns. Off: they pass untouched." },
];

const fmtTime = (ms: number) => new Date(ms).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", second: "2-digit" });

function EventRow({ event }: { event: GuardEvent }) {
  const snapshot = useGitStore((s) => s.snapshot);
  const clients = useGitStore((s) => s.clients);
  const owner = accountById(snapshot, event.owner);
  const client = clients.find((c) => c.id === event.client)?.name ?? event.client;
  return (
    <li className="flex items-start gap-3 px-4 py-2.5">
      <PillBadge tone={event.allow ? (event.code === "bypass" ? "amber" : "emerald") : "rose"} className="mt-0.5 w-[70px] justify-center">
        {event.allow ? (event.code === "bypass" ? "Bypassed" : "Allowed") : "Refused"}
      </PillBadge>
      <div className="min-w-0 flex-1">
        <div className={cn("text-[12.5px] select-text", event.allow ? "text-ink-muted" : "text-ink")}>
          <span className="font-medium capitalize">{event.hook}</span>: {event.message}
        </div>
        <div className="mt-0.5 flex min-w-0 items-center gap-2 text-[11px] text-ink-faint">
          <button type="button" className="min-w-0 hover:text-accent" disabled={!inTauri} onClick={() => void ipc.revealInExplorer(event.repo)} title="Show in Explorer">
            <PathText path={event.repo} className="block" />
          </button>
          {event.remote && <span className="truncate font-mono">{event.remote}</span>}
        </div>
      </div>
      <div className="flex shrink-0 flex-col items-end gap-0.5 text-[11px] text-ink-faint">
        <span className="tabular-nums">{fmtTime(event.tsMs)}</span>
        <span>
          {client ?? "unknown client"}
          {owner ? ` · ${owner.label}` : ""}
        </span>
      </div>
    </li>
  );
}

/** The guard's rules, the block it keeps in the global git config, and what it has done. */
export function GuardTab() {
  const snapshot = useGitStore((s) => s.snapshot);
  const busy = useGitStore((s) => s.busy);
  const log = useGitStore((s) => s.log);
  const lastApply = useGitStore((s) => s.lastApply);
  const { setGuard, apply, loadLog, clearLog, loadClients } = useGitStore.getState();
  useEffect(() => {
    void loadLog();
    void loadClients();
  }, [loadLog, loadClients]);

  if (!snapshot) return <div className="p-6 text-[12.5px] text-ink-faint">Loading…</div>;
  const guard = snapshot.config.guard;
  const refused = log.filter((e) => !e.allow).length;
  const written = lastApply?.changes.filter((c) => c.action !== "unchanged").length ?? 0;

  return (
    <div className="flex flex-col gap-4 p-6">
      <StateBanner />
      <div className="grid gap-4 xl:grid-cols-2">
        <GlassCard
          title="Rules"
          actions={<Switch checked={guard.enabled} disabled={busy === "guard"} label={`Guard: switch ${guard.enabled ? "off" : "on"}`} onChange={(enabled) => void setGuard({ ...guard, enabled })} />}
        >
          <div className="flex items-start gap-3 border-b border-edge px-4 py-3">
            {guard.enabled ? <ShieldCheck className="mt-0.5 size-4 shrink-0 text-emerald" /> : <ShieldOff className="mt-0.5 size-4 shrink-0 text-amber" />}
            <p className="text-[12.5px] text-ink-muted">
              {guard.enabled
                ? "On. Git asks SwitchGit before every commit and push, in every client and terminal on this PC. Your repositories' own hooks (Git LFS, Husky) still run afterwards."
                : "Off. Repositories still pick their account's identity, but nothing refuses a commit or push, and switched-off accounts and client ties are not enforced."}
            </p>
          </div>
          <ul className={cn("divide-y divide-edge", !guard.enabled && "opacity-50")}>
            {RULES.map((r) => (
              <li key={r.key} className="flex items-center gap-3 px-4 py-2.5">
                <div className="min-w-0 flex-1">
                  <div className="text-[12.5px] text-ink">{r.label}</div>
                  <div className="text-[11.5px] text-ink-faint">{r.hint}</div>
                </div>
                <Switch size="sm" checked={guard[r.key]} disabled={!guard.enabled || busy === "guard"} label={r.label} onChange={(v) => void setGuard({ ...guard, [r.key]: v })} />
              </li>
            ))}
          </ul>
        </GlassCard>

        <GlassCard
          title="Global git config"
          actions={
            <button type="button" className="btn" disabled={busy === "apply"} onClick={() => void apply()} title="Write the files again">
              <RefreshCw className="size-3.5" /> Apply
            </button>
          }
        >
          <div className="flex flex-col gap-2 px-4 py-3">
            <div className="flex items-center gap-2">
              <PillBadge tone={snapshot.pending ? "amber" : "emerald"}>{snapshot.pending ? `${snapshot.pending} out of date` : "Up to date"}</PillBadge>
              <button type="button" className="min-w-0 hover:text-accent" disabled={!inTauri} onClick={() => void ipc.revealInExplorer(snapshot.gitconfigPath)} title="Show in Explorer">
                <PathText path={snapshot.gitconfigPath} className="block text-ink-muted" />
              </button>
              <div className="flex-1" />
              {snapshot.gitVersion && <span className="shrink-0 text-[11px] text-ink-faint">git {snapshot.gitVersion}</span>}
            </div>
            <p className="text-[11.5px] text-ink-faint">SwitchGit only writes the block below, at the end of the file; the rest stays as you wrote it. The first change keeps the original next to it as .gitconfig.switchgit.bak.</p>
            {snapshot.block ? (
              <pre className="max-h-56 overflow-auto rounded-md border border-edge bg-canvas/70 p-2 font-mono text-[11px] leading-relaxed text-ink-muted select-text">{snapshot.block}</pre>
            ) : (
              <div className="rounded-md border border-dashed border-edge px-3 py-4 text-center text-[12px] text-ink-faint">Nothing to manage yet: add an account.</div>
            )}
            {lastApply && (
              <div className="text-[11.5px] text-ink-muted">
                Last apply: {written} file{written === 1 ? "" : "s"} written, {lastApply.changes.length - written} already up to date.
                {lastApply.backup && <> Original kept as <span className="font-mono select-text">{lastApply.backup}</span>.</>}
              </div>
            )}
            <IssueList issues={snapshot.issues} />
          </div>
        </GlassCard>
      </div>

      <GlassCard
        title={`Activity${log.length ? ` (${refused} refused of ${log.length})` : ""}`}
        actions={
          <>
            <button type="button" className="icon-btn" aria-label="Refresh activity" title="Refresh" onClick={() => void loadLog()}>
              <RefreshCw className="size-3.5" />
            </button>
            <button type="button" className="icon-btn" aria-label="Clear activity" title="Clear" disabled={!log.length} onClick={() => void clearLog()}>
              <Trash2 className="size-3.5" />
            </button>
          </>
        }
      >
        {log.length ? (
          <ul className="divide-y divide-edge">
            {log.map((e, i) => (
              <EventRow key={`${e.tsMs}-${i}`} event={e} />
            ))}
          </ul>
        ) : (
          <EmptyState icon={<ShieldCheck className="size-5" />} title="Nothing yet">
            Commits and pushes in repositories an account owns show up here, with the ones the guard refused.
          </EmptyState>
        )}
      </GlassCard>
      <p className="text-[11.5px] text-ink-faint">
        The guard runs on this PC and can be skipped with <code className="font-mono">--no-verify</code> or <code className="font-mono">SWITCHGIT_BYPASS=1</code>. It prevents mistakes; for a hard limit, also protect branches on the server.
      </p>
    </div>
  );
}
