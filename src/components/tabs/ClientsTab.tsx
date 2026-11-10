import { cn, EmptyState, GlassCard, PillBadge } from "@abyss/ui";
import { AppWindow, ExternalLink, Link2, RefreshCw, SquareTerminal } from "lucide-react";
import { useEffect } from "react";
import { useAppStore } from "../../stores/appStore";
import { accountById, useGitStore } from "../../stores/gitStore";
import type { Account, ClientStatus } from "../../types/switchgit";
import { StateBanner } from "./AccountsTab";

const FOLLOW = "";

function ClientRow({ client, accounts }: { client: ClientStatus; accounts: Account[] }) {
  const busy = useGitStore((s) => s.busy === client.id);
  const guardOn = useGitStore((s) => s.snapshot?.config.guard.enabled ?? true);
  const { setBinding, launchClient } = useGitStore.getState();
  const tied = accounts.find((a) => a.id === client.accountId);
  const terminal = client.id === "terminal";
  const Icon = terminal ? SquareTerminal : AppWindow;

  return (
    <li className={cn("flex items-start gap-3 px-4 py-3", !client.installed && "opacity-55")}>
      <div className={cn("mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg ring-1 ring-inset", tied ? "bg-accent/12 text-accent ring-accent/25" : "bg-tint/5 text-ink-faint ring-edge")}>
        <Icon className="size-4" strokeWidth={1.75} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-[13px] font-medium text-ink">{client.name}</span>
          {!terminal && (client.running ? <PillBadge tone="emerald">Running</PillBadge> : client.installed ? <PillBadge>Installed</PillBadge> : <PillBadge>Not found</PillBadge>)}
          {tied && !tied.enabled && <PillBadge tone="amber">{tied.label} is off</PillBadge>}
        </div>
        <p className="mt-0.5 text-[12px] text-ink-muted">{client.note}</p>
        {tied && (
          <p className="mt-1 text-[12px] text-ink">
            {terminal ? "Git from the command line" : client.name} can only commit and push in <strong className="font-semibold">{tied.label}</strong> repositories
            {terminal ? "." : `. Sign in to ${client.name} with ${tied.username ?? tied.email} and no other account.`}
            {!guardOn && <span className="text-amber"> The guard is off, so this tie is not enforced.</span>}
          </p>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <label className="sr-only" htmlFor={`tie-${client.id}`}>
          Account for {client.name}
        </label>
        <select
          id={`tie-${client.id}`}
          value={client.accountId ?? FOLLOW}
          disabled={busy || !accounts.length}
          onChange={(e) => void setBinding(client.id, e.target.value || null)}
          className="h-7 max-w-44 rounded-md border border-edge bg-canvas/70 px-2 text-[12px] text-ink outline-none focus:border-accent/60 disabled:opacity-50"
        >
          <option value={FOLLOW}>Any account</option>
          {accounts.map((a) => (
            <option key={a.id} value={a.id}>
              Only {a.label}
            </option>
          ))}
        </select>
        {!terminal && (
          <button type="button" className="icon-btn" disabled={!client.exePath} aria-label={`Open ${client.name}`} title={client.exePath ? `Open ${client.name}` : "Not found on this PC"} onClick={() => void launchClient(client.id)}>
            <ExternalLink className="size-3.5" />
          </button>
        )}
      </div>
    </li>
  );
}

/** Tie each git client to one account, so Fork stays personal and Sourcetree stays work. */
export function ClientsTab() {
  const snapshot = useGitStore((s) => s.snapshot);
  const clients = useGitStore((s) => s.clients);
  const loadClients = useGitStore((s) => s.loadClients);
  const accounts = snapshot?.config.accounts ?? [];
  useEffect(() => void loadClients(), [loadClients]);

  const found = clients.filter((c) => c.installed);
  const missing = clients.filter((c) => !c.installed);
  const ties = Object.entries(snapshot?.config.bindings ?? {});

  return (
    <div className="flex flex-col gap-4 p-6">
      <StateBanner />
      <div className="flex items-center gap-3">
        <p className="min-w-0 flex-1 text-[12.5px] text-ink-muted">
          {ties.length
            ? ties.map(([c, id]) => `${clients.find((x) => x.id === c)?.name ?? c} → ${accountById(snapshot, id)?.label ?? id}`).join("  ·  ")
            : "Tie a client to an account and it can only commit and push in that account's repositories. The guard recognises the client that started git."}
        </p>
        <button type="button" className="btn" onClick={() => void loadClients()}>
          <RefreshCw className="size-3.5" /> Refresh
        </button>
      </div>

      {!accounts.length && (
        <div className="flex items-center gap-3 rounded-lg border border-edge bg-panel/50 px-3 py-2 text-[12px] text-ink-muted">
          <Link2 className="size-3.5 shrink-0" />
          <span className="flex-1">Add an account first; then choose it here for each client.</span>
          <button type="button" className="btn" onClick={() => useAppStore.getState().setActiveTab("accounts")}>
            Go to Accounts
          </button>
        </div>
      )}

      {clients.length === 0 ? (
        <div className="rounded-xl border border-dashed border-edge">
          <EmptyState icon={<AppWindow className="size-5" />} title="Looking for git clients…" />
        </div>
      ) : (
        <>
          <GlassCard title={`On this PC (${found.length})`}>
            <ul className="divide-y divide-edge">
              {found.map((c) => (
                <ClientRow key={c.id} client={c} accounts={accounts} />
              ))}
            </ul>
          </GlassCard>
          {missing.length > 0 && (
            <GlassCard title={`Not found (${missing.length})`}>
              <ul className="divide-y divide-edge">
                {missing.map((c) => (
                  <ClientRow key={c.id} client={c} accounts={accounts} />
                ))}
              </ul>
            </GlassCard>
          )}
        </>
      )}
      <p className="text-[11.5px] text-ink-faint">
        SwitchGit cannot see or change which account a client is signed in to. The tie is enforced when the client commits or pushes: git runs the guard, which refuses repositories of any other account.
      </p>
    </div>
  );
}
