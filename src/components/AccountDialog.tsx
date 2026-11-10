import { FieldLabel, Modal, PathText, Segmented, TextField } from "@abyss/ui";
import { FolderPlus, KeyRound, X } from "lucide-react";
import { useEffect, useState } from "react";
import { pickOne } from "../lib/dialogs";
import { useGitStore } from "../stores/gitStore";
import type { Account, AccountKind } from "../types/switchgit";

export const emptyAccount = (name: string | null, email: string | null, first: boolean): Account => ({
  id: "",
  label: "",
  kind: first ? "work" : "personal",
  // The first account starts from the identity git already uses.
  name: name ?? "",
  email: first ? (email ?? "") : "",
  enabled: true,
  remotes: [],
  folders: [],
  username: null,
  sshKey: null,
});

const lines = (text: string) => text.split(/[\n,]/).map((s) => s.trim()).filter(Boolean);

/** Add or edit one account. `account` null: closed. */
export function AccountDialog({ account, onClose }: { account: Account | null; onClose: () => void }) {
  const save = useGitStore((s) => s.saveAccount);
  const error = useGitStore((s) => s.error);
  const busy = useGitStore((s) => s.busy !== null);
  const [draft, setDraft] = useState<Account | null>(account);
  const [remotes, setRemotes] = useState("");
  useEffect(() => {
    setDraft(account);
    setRemotes(account?.remotes.join("\n") ?? "");
  }, [account]);
  if (!draft) return null;

  const patch = (p: Partial<Account>) => setDraft({ ...draft, ...p });
  const isNew = !draft.id;
  const scoped = lines(remotes).length > 0 || draft.folders.length > 0;
  const ready = draft.label.trim() && draft.name.trim() && draft.email.includes("@");

  const submit = async () => {
    if (await save({ ...draft, remotes: lines(remotes) })) onClose();
  };

  return (
    <Modal
      open
      title={isNew ? "Add account" : `Edit ${account?.label ?? "account"}`}
      onClose={onClose}
      className="max-w-xl"
      footer={
        <>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn btn-primary" disabled={!ready || busy} onClick={() => void submit()}>
            {isNew ? "Add account" : "Save"}
          </button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <div className="flex items-end gap-3">
          <div className="min-w-0 flex-1">
            <FieldLabel>Account name</FieldLabel>
            <TextField autoFocus className="w-full" placeholder="Work" value={draft.label} onChange={(e) => patch({ label: e.target.value })} />
          </div>
          <Segmented<AccountKind>
            value={draft.kind}
            onChange={(kind) => patch({ kind })}
            options={[
              { value: "work", label: "Work" },
              { value: "personal", label: "Personal" },
              { value: "other", label: "Other" },
            ]}
          />
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <FieldLabel>Commit author</FieldLabel>
            <TextField className="w-full" placeholder="Phi Hoang" value={draft.name} onChange={(e) => patch({ name: e.target.value })} />
          </div>
          <div>
            <FieldLabel>Commit e-mail</FieldLabel>
            <TextField className="w-full" placeholder="you@company.com" value={draft.email} onChange={(e) => patch({ email: e.target.value })} />
          </div>
        </div>

        <div>
          <FieldLabel>Remotes this account owns (one per line)</FieldLabel>
          <textarea
            spellCheck={false}
            rows={3}
            placeholder={"github.com/YourCompany\ngithub.com/you/one-repo"}
            value={remotes}
            onChange={(e) => setRemotes(e.target.value)}
            className="w-full resize-y rounded-md border border-edge bg-canvas/70 px-2 py-1.5 font-mono text-[11.5px] text-ink outline-none select-text placeholder:text-ink-faint focus:border-accent/60"
          />
          <p className="mt-1 text-[11px] text-ink-faint">An owner, a single repository, or a whole host. Every repository whose remote is under one of these commits as this account.</p>
        </div>

        <div>
          <FieldLabel>Folders (optional)</FieldLabel>
          <div className="flex flex-col gap-1">
            {draft.folders.map((f) => (
              <div key={f} className="flex items-center gap-2 rounded-md border border-edge bg-canvas/50 py-0.5 pr-1 pl-2">
                <PathText path={f} className="min-w-0 flex-1 text-ink-muted" />
                <button type="button" className="icon-btn" aria-label={`Remove ${f}`} onClick={() => patch({ folders: draft.folders.filter((x) => x !== f) })}>
                  <X className="size-3.5" />
                </button>
              </div>
            ))}
            <button
              type="button"
              className="btn self-start"
              onClick={async () => {
                const folder = await pickOne("folder");
                if (folder && !draft.folders.includes(folder)) patch({ folders: [...draft.folders, folder] });
              }}
            >
              <FolderPlus className="size-3.5" /> Add folder…
            </button>
          </div>
          <p className="mt-1 text-[11px] text-ink-faint">Repositories inside also belong to this account, even before they have a remote.</p>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <FieldLabel>HTTPS sign-in name (optional)</FieldLabel>
            <TextField className="w-full" placeholder="github-login" value={draft.username ?? ""} onChange={(e) => patch({ username: e.target.value || null })} />
          </div>
          <div>
            <FieldLabel>SSH key (optional)</FieldLabel>
            <div className="flex gap-1">
              <TextField mono className="min-w-0 flex-1" placeholder="C:/Users/you/.ssh/id_work" value={draft.sshKey ?? ""} onChange={(e) => patch({ sshKey: e.target.value || null })} />
              <button
                type="button"
                className="icon-btn"
                aria-label="Choose key file"
                title="Choose key file"
                onClick={async () => {
                  const file = await pickOne("file");
                  if (file) patch({ sshKey: file });
                }}
              >
                <KeyRound className="size-3.5" />
              </button>
            </div>
          </div>
        </div>
        <p className="-mt-2 text-[11px] text-ink-faint">The sign-in name makes the credential manager offer this account's saved login; the key is the only one SSH tries for its repositories.</p>

        {!scoped && <div className="rounded-lg border border-amber/30 bg-amber/10 px-3 py-2 text-[12px] text-amber">Without a remote or a folder no repository will use this account.</div>}
        {error && <div className="rounded-lg border border-rose/30 bg-rose/10 px-3 py-2 text-[12px] text-rose">{error}</div>}
      </div>
    </Modal>
  );
}
