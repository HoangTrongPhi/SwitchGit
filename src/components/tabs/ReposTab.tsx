import { cn, EmptyState, GlassCard, JobProgress, PathText, PillBadge, Segmented, type PillTone } from "@abyss/ui";
import { FolderGit2, FolderPlus, FolderSearch, Play, X } from "lucide-react";
import { useMemo, useState } from "react";
import { pickPaths } from "../../lib/dialogs";
import { inTauri, ipc } from "../../lib/ipc";
import { useAppStore } from "../../stores/appStore";
import { accountById, useGitStore } from "../../stores/gitStore";
import type { RepoInfo, RepoStatus } from "../../types/switchgit";
import { IssueList, StateBanner } from "./AccountsTab";

const STATUS: Record<RepoStatus, { label: string; tone: PillTone; hint: string }> = {
  ok: { label: "OK", tone: "emerald", hint: "Commits as the account that owns it" },
  "wrong-identity": { label: "Wrong identity", tone: "rose", hint: "Would commit as someone other than its owner; the guard refuses it" },
  "account-off": { label: "Account off", tone: "amber", hint: "Its account is switched off; commits and pushes are refused" },
  mixed: { label: "Mixed remotes", tone: "amber", hint: "Its remotes belong to different accounts; each push is checked on its own" },
  unowned: { label: "No account", tone: "neutral", hint: "No account owns it; the guard leaves it alone" },
};

/** `git@github.com:Owner/repo.git` → `github.com/Owner` (what an account scope looks like). */
export function scopeOf(url: string): string | null {
  const m = url.trim().match(/^(?:[a-z+]+:\/\/)?(?:[^@/]+@)?([^/:]+)[:/](?:\d+\/)?([^/]+)\//i);
  return m && m[1].includes(".") ? `${m[1].toLowerCase()}/${m[2]}` : null;
}

function RepoRow({ repo }: { repo: RepoInfo }) {
  const snapshot = useGitStore((s) => s.snapshot);
  const owner = accountById(snapshot, repo.owner);
  const identity = accountById(snapshot, repo.identity);
  const status = STATUS[repo.status] ?? STATUS.unowned;
  const scope = repo.remotes.length ? scopeOf(repo.remotes[0].url) : null;
  const accounts = snapshot?.config.accounts ?? [];

  const assign = async (id: string) => {
    const account = accounts.find((a) => a.id === id);
    if (!account || !scope) return;
    const s = useGitStore.getState();
    if (await s.saveAccount({ ...account, remotes: [...account.remotes, scope] })) await s.scan();
  };

  return (
    <tr className="border-b border-edge align-top last:border-0">
      <td className="py-2 pr-2 pl-4">
        <PillBadge tone={status.tone} className="cursor-help">
          <span title={status.hint}>{status.label}</span>
        </PillBadge>
      </td>
      <td className="max-w-0 px-2 py-2">
        <button type="button" className="block w-full text-left hover:text-accent" disabled={!inTauri} onClick={() => void ipc.revealInExplorer(repo.path)} title="Show in Explorer">
          <PathText path={repo.path} className="block text-ink" />
        </button>
        <div className="truncate font-mono text-[11px] text-ink-faint" title={repo.remotes.map((r) => `${r.name}  ${r.url}`).join("\n")}>
          {repo.remotes.length ? repo.remotes.map((r) => r.url).join("  ·  ") : "no remote"}
        </div>
      </td>
      <td className="px-2 py-2 text-[12px] whitespace-nowrap">
        {owner ? (
          <span className={cn("text-ink", !owner.enabled && "text-ink-muted")}>{owner.label}</span>
        ) : scope && accounts.length ? (
          <select
            aria-label={`Give ${scope} to an account`}
            value=""
            onChange={(e) => void assign(e.target.value)}
            className="h-6 w-full rounded-md border border-edge bg-canvas/70 px-1.5 text-[11.5px] text-ink-muted outline-none focus:border-accent/60"
          >
            <option value="">Give {scope} to…</option>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.label}
              </option>
            ))}
          </select>
        ) : (
          <span className="text-ink-faint">—</span>
        )}
      </td>
      <td className="py-2 pr-4 pl-2 text-[12px]">
        <div className={cn("truncate select-text", repo.status === "wrong-identity" ? "text-rose" : "text-ink-muted")} title={repo.email ?? undefined}>
          {repo.email ?? "no user.email"}
        </div>
        {identity && identity.id !== repo.owner && <div className="text-[11px] text-ink-faint">{identity.label}</div>}
      </td>
    </tr>
  );
}

/** Scan folders for repositories: whose each one is, and who would commit there now. Read-only. */
export function ReposTab() {
  const { roots, setRoots, job, report, scanError, scan, cancelScan } = useGitStore();
  const dragActive = useAppStore((s) => s.dragActive);
  const [show, setShow] = useState<"all" | "problems">("all");

  const problems = useMemo(() => report?.repos.filter((r) => r.status === "wrong-identity" || r.status === "account-off") ?? [], [report]);
  const rows = show === "problems" ? problems : (report?.repos ?? []);
  const count = (s: RepoStatus) => report?.repos.filter((r) => r.status === s).length ?? 0;

  return (
    <div className="flex flex-col gap-4 p-6">
      <StateBanner />
      <div className="flex flex-wrap items-center gap-2">
        <button type="button" className="btn" onClick={() => void pickPaths("folder")}>
          <FolderPlus className="size-3.5" /> Add folder…
        </button>
        {roots.map((r) => (
          <span key={r} className="flex h-7 max-w-72 items-center gap-1 rounded-md border border-edge bg-canvas/50 pr-0.5 pl-2">
            <PathText path={r} className="min-w-0 text-ink-muted" />
            <button type="button" className="icon-btn size-5" aria-label={`Remove ${r}`} onClick={() => setRoots(roots.filter((x) => x !== r))}>
              <X className="size-3" />
            </button>
          </span>
        ))}
        <div className="flex-1" />
        <button type="button" className="btn btn-primary" disabled={!roots.length || !!job} onClick={() => void scan()} title="F5">
          <Play className="size-3.5" /> Scan
        </button>
      </div>

      {job && <JobProgress done={job.done} total={job.total} message={job.message} startedAt={job.startedAt} onCancel={() => void cancelScan()} cancelling={job.cancelling} />}
      {scanError && <div className="rounded-lg border border-rose/30 bg-rose/10 px-3 py-2 text-[12px] text-rose">{scanError}</div>}

      {!report && !job && (
        <div className={dragActive ? "rounded-xl border border-dashed border-accent bg-accent/5" : "rounded-xl border border-dashed border-edge"}>
          <EmptyState icon={roots.length ? <FolderGit2 className="size-5" /> : <FolderSearch className="size-5" />} title={roots.length ? "Ready to scan" : "Drop the folder that holds your repositories"}>
            {roots.length ? "Press Scan (F5) to list every repository with its account and the identity it commits as." : "Or add one with the button above. Repositories are only read, never changed."}
          </EmptyState>
        </div>
      )}

      {report && (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <PillBadge tone={problems.length ? "rose" : report.cancelled ? "amber" : "emerald"}>{report.cancelled ? "Cancelled — partial" : problems.length ? `${problems.length} to fix` : "All clear"}</PillBadge>
            <span className="text-[12.5px] text-ink">
              {report.repos.length} repositor{report.repos.length === 1 ? "y" : "ies"}: {count("ok")} OK, {count("wrong-identity")} wrong identity, {count("account-off")} off, {count("unowned") + count("mixed")} without one account
            </span>
            <span className="text-[12px] text-ink-faint">in {report.elapsedMs} ms</span>
            <div className="flex-1" />
            <Segmented
              size="sm"
              value={show}
              onChange={setShow}
              options={[
                { value: "all", label: `All (${report.repos.length})` },
                { value: "problems", label: `To fix (${problems.length})` },
              ]}
            />
          </div>
          <GlassCard>
            {rows.length ? (
              <table className="w-full table-fixed text-left">
                <thead>
                  <tr className="border-b border-edge text-[11px] font-medium tracking-wide text-ink-faint uppercase">
                    <th className="w-36 py-2 pr-2 pl-4 font-medium">Status</th>
                    <th className="px-2 py-2 font-medium">Repository</th>
                    <th className="w-56 px-2 py-2 font-medium">Account</th>
                    <th className="w-56 py-2 pr-4 pl-2 font-medium">Commits as</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((r) => (
                    <RepoRow key={r.path} repo={r} />
                  ))}
                </tbody>
              </table>
            ) : (
              <EmptyState icon={<FolderGit2 className="size-5" />} title={show === "problems" ? "Nothing to fix" : "No repositories in these folders"} />
            )}
          </GlassCard>
          {report.issues.some((i) => i.severity === "error") && (
            <div className="rounded-xl border border-edge bg-panel/50 px-4 py-3">
              <IssueList issues={report.issues.filter((i) => i.severity === "error").map((i) => ({ ...i, message: i.path ? `${i.message}: ${i.path}` : i.message }))} />
            </div>
          )}
        </>
      )}
    </div>
  );
}
