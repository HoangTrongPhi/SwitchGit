import { cn, EmptyState, PathText, PillBadge, spotlight, type PillTone } from "@abyss/ui";
import { AlertTriangle, Briefcase, Folder, Globe, Info, Pencil, Plus, Trash2, UserRound, UsersRound, XCircle } from "lucide-react";
import { useState } from "react";
import { useAppStore } from "../../stores/appStore";
import { useGitStore } from "../../stores/gitStore";
import type { Account, Issue } from "../../types/switchgit";
import { AccountDialog, emptyAccount } from "../AccountDialog";
import { Switch } from "../Switch";

export const KIND_TONE: Record<Account["kind"], PillTone> = { work: "accent", personal: "emerald", other: "neutral" };
const KIND_ICON = { work: Briefcase, personal: UserRound, other: Globe };

export function IssueList({ issues }: { issues: Issue[] }) {
  if (!issues.length) return null;
  return (
    <ul className="flex flex-col gap-1">
      {issues.map((i, n) => (
        <li key={n} className="flex items-start gap-2 text-[12px] text-ink-muted">
          {i.severity === "error" ? <XCircle className="mt-0.5 size-3.5 shrink-0 text-rose" /> : i.severity === "warn" ? <AlertTriangle className="mt-0.5 size-3.5 shrink-0 text-amber" /> : <Info className="mt-0.5 size-3.5 shrink-0 text-ink-faint" />}
          <span className="min-w-0">{i.message}</span>
        </li>
      ))}
    </ul>
  );
}

/** Shown on every tab while the files on disk differ from the accounts, or a change failed. */
export function StateBanner() {
  const pending = useGitStore((s) => s.snapshot?.pending ?? 0);
  const error = useGitStore((s) => s.error);
  const busy = useGitStore((s) => s.busy);
  const { apply, dismissError } = useGitStore.getState();
  if (!pending && !error) return null;
  return (
    <div className="flex flex-col gap-2">
      {error && (
        <div className="flex items-start gap-2 rounded-lg border border-rose/30 bg-rose/10 px-3 py-2 text-[12px] text-rose">
          <XCircle className="mt-0.5 size-3.5 shrink-0" />
          <span className="min-w-0 flex-1 select-text">{error}</span>
          <button type="button" className="shrink-0 underline-offset-2 hover:underline" onClick={dismissError}>
            Dismiss
          </button>
        </div>
      )}
      {pending > 0 && (
        <div className="flex items-center gap-3 rounded-lg border border-amber/30 bg-amber/10 px-3 py-2 text-[12px] text-amber">
          <AlertTriangle className="size-3.5 shrink-0" />
          <span className="min-w-0 flex-1">
            {pending} file{pending === 1 ? "" : "s"} of the git configuration {pending === 1 ? "is" : "are"} out of date, so the accounts below are not fully in effect.
          </span>
          <button type="button" className="btn" disabled={busy === "apply"} onClick={() => void apply()}>
            Apply
          </button>
        </div>
      )}
    </div>
  );
}

function AccountCard({ account, onEdit }: { account: Account; onEdit: () => void }) {
  const busy = useGitStore((s) => s.busy === account.id);
  const clients = useGitStore((s) => s.clients);
  const tied = clients.filter((c) => c.accountId === account.id);
  const { setEnabled, deleteAccount } = useGitStore.getState();
  const [confirming, setConfirming] = useState(false);
  const Icon = KIND_ICON[account.kind] ?? Globe;
  const scopeEmpty = !account.remotes.length && !account.folders.length;

  return (
    <div onPointerMove={spotlight} className={cn("fx-card flex flex-col rounded-xl border bg-panel/80 transition-colors", account.enabled ? "border-edge" : "border-edge bg-panel/40")}>
      <div className="flex items-start gap-3 p-4">
        <div className={cn("grid size-9 shrink-0 place-items-center rounded-lg ring-1 ring-inset", account.enabled ? "bg-accent/12 text-accent ring-accent/25" : "bg-tint/5 text-ink-faint ring-edge")}>
          <Icon className="size-[18px]" strokeWidth={1.75} />
        </div>
        <div className={cn("min-w-0 flex-1", !account.enabled && "opacity-60")}>
          <div className="flex items-center gap-2">
            <h3 className="truncate text-[14px] font-semibold text-ink">{account.label}</h3>
            <PillBadge tone={KIND_TONE[account.kind] ?? "neutral"}>{account.kind}</PillBadge>
          </div>
          <div className="mt-0.5 truncate text-[12px] text-ink-muted select-text" title={`${account.name} <${account.email}>`}>
            {account.name} · {account.email}
          </div>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1">
          <Switch checked={account.enabled} disabled={busy} label={`${account.label}: switch ${account.enabled ? "off" : "on"}`} onChange={(v) => void setEnabled(account.id, v)} />
          <span className={cn("text-[11px] font-medium", account.enabled ? "text-emerald" : "text-ink-faint")}>{account.enabled ? "On" : "Off"}</span>
        </div>
      </div>

      <div className={cn("flex flex-col gap-1.5 border-t border-edge px-4 py-3", !account.enabled && "opacity-60")}>
        {account.remotes.map((r) => (
          <div key={r} className="flex items-center gap-2 text-[12px]">
            <Globe className="size-3.5 shrink-0 text-ink-faint" />
            <span className="truncate font-mono text-[11.5px] text-ink">{r}</span>
          </div>
        ))}
        {account.folders.map((f) => (
          <div key={f} className="flex items-center gap-2 text-[12px]">
            <Folder className="size-3.5 shrink-0 text-ink-faint" />
            <PathText path={f} className="min-w-0 text-ink" />
          </div>
        ))}
        {scopeEmpty && <div className="text-[12px] text-amber">Owns no remote or folder yet, so no repository uses it.</div>}
        {!account.enabled && !scopeEmpty && <div className="text-[12px] text-ink-muted">Off: commits and pushes in these repositories are refused.</div>}
      </div>

      <div className="flex items-center gap-2 border-t border-edge px-4 py-2">
        <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1">
          {tied.length ? tied.map((c) => <PillBadge key={c.id}>{c.name}</PillBadge>) : <span className="text-[11.5px] text-ink-faint">No client tied</span>}
        </div>
        {confirming ? (
          <>
            <span className="text-[11.5px] text-ink-muted">Remove {account.label}?</span>
            <button type="button" className="btn btn-danger" onClick={() => void deleteAccount(account.id)}>
              Remove
            </button>
            <button type="button" className="btn" onClick={() => setConfirming(false)}>
              Keep
            </button>
          </>
        ) : (
          <>
            <button type="button" className="icon-btn" aria-label={`Edit ${account.label}`} title="Edit" onClick={onEdit}>
              <Pencil className="size-3.5" />
            </button>
            <button type="button" className="icon-btn" aria-label={`Remove ${account.label}`} title="Remove" onClick={() => setConfirming(true)}>
              <Trash2 className="size-3.5" />
            </button>
          </>
        )}
      </div>
    </div>
  );
}

/** The accounts with their switches. Each one stands alone: switching one never touches another. */
export function AccountsTab() {
  const snapshot = useGitStore((s) => s.snapshot);
  const [editing, setEditing] = useState<Account | null>(null);
  const accounts = snapshot?.config.accounts ?? [];
  const on = accounts.filter((a) => a.enabled).length;
  const add = () => {
    useGitStore.getState().dismissError();
    setEditing(emptyAccount(snapshot?.globalName ?? null, snapshot?.globalEmail ?? null, accounts.length === 0));
  };

  return (
    <div className="flex flex-col gap-4 p-6">
      <StateBanner />
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1 text-[12.5px] text-ink-muted">
          {accounts.length ? `${on} of ${accounts.length} account${accounts.length === 1 ? "" : "s"} on. Each repository commits as the account that owns its remote.` : "No accounts yet."}
        </div>
        <button type="button" className="btn btn-primary" disabled={!snapshot} onClick={add}>
          <Plus className="size-3.5" /> Add account
        </button>
      </div>

      {accounts.length === 0 ? (
        <div className="rounded-xl border border-dashed border-edge">
          <EmptyState icon={<UsersRound className="size-5" />} title="Add your work and personal accounts">
            Give each one the remotes it owns, such as <code className="font-mono">github.com/YourCompany</code>. SwitchGit then picks the right identity in every repository and refuses commits and pushes that would cross over.
            {snapshot?.globalEmail && (
              <>
                {" "}
                Git currently commits everywhere as <span className="text-ink select-text">{snapshot.globalEmail}</span>.
              </>
            )}
          </EmptyState>
        </div>
      ) : (
        <div className="grid gap-4 xl:grid-cols-2">
          {accounts.map((a) => (
            <AccountCard
              key={a.id}
              account={a}
              onEdit={() => {
                useGitStore.getState().dismissError();
                setEditing(a);
              }}
            />
          ))}
        </div>
      )}

      {snapshot && snapshot.issues.length > 0 && accounts.length > 0 && (
        <div className="rounded-xl border border-edge bg-panel/50 px-4 py-3">
          <IssueList issues={snapshot.issues} />
        </div>
      )}
      <button type="button" className="self-start text-[11.5px] text-ink-faint hover:text-ink-muted" onClick={() => useAppStore.getState().setActiveTab("guard")}>
        {snapshot?.config.guard.enabled ? "The guard is on: cross-account commits and pushes are refused." : "The guard is off: nothing refuses a cross-account commit or push."} Open Guard →
      </button>
      <AccountDialog account={editing} onClose={() => setEditing(null)} />
    </div>
  );
}
