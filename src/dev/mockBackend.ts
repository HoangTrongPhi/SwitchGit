/**
 * Dev-only fake backend so the UI can be exercised in a plain browser: `npm run dev`, then open
 * `http://localhost:1522/?mock`. Never active in production builds or inside Tauri.
 * It answers every command in `src/lib/ipc.ts` (checked by `npm run check:mock`) and the plugin
 * commands the UI uses; the scan emits real progress events so Cancel works.
 * `?mock=empty` starts without accounts (the first-run screens).
 */
import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { Account, ClientStatus, Config, GuardEvent, Issue, RepoInfo, ScanReport, Snapshot } from "../types/switchgit";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const DATA = "C:\\Users\\Phi\\AppData\\Local\\com.phineas.switchgit";
const GITCONFIG = "C:\\Users\\Phi\\.gitconfig";
const ROOT = "D:\\src";

let nextJob = 1;
const cancelled = new Set<number>();

const params = new URLSearchParams(location.search);
const empty = params.get("mock") === "empty";

const config: Config = {
  format: "switchgit.config",
  version: 1,
  accounts: empty
    ? []
    : [
        { id: "work", label: "Work", kind: "work", name: "Phi Hoang", email: "phi@magetechcorp.com", enabled: true, remotes: ["github.com/MageTech", "gitlab.magetech.internal"], folders: ["D:\\Work"], username: "phi-magetech", sshKey: null },
        { id: "personal", label: "Personal", kind: "personal", name: "Hoang Trong Phi", email: "phi.personal@example.com", enabled: true, remotes: ["github.com/HoangTrongPhi"], folders: [], username: "HoangTrongPhi", sshKey: "C:\\Users\\Phi\\.ssh\\id_personal" },
        { id: "client-x", label: "Client X", kind: "other", name: "Phi Hoang", email: "phi@client-x.example", enabled: false, remotes: [], folders: [], username: null, sshKey: null },
      ],
  bindings: empty ? {} : { fork: "personal", sourcetree: "work" },
  guard: { enabled: true, checkCommit: true, checkPush: true, checkAuthors: true, blockUnknown: false },
  scanRoots: empty ? [] : [ROOT],
};
/** One file is out of date at the start, so the Apply banner shows. */
let pending = empty ? 0 : 1;

let guardLog: GuardEvent[] = empty
  ? []
  : [
      { tsMs: Date.now() - 4 * 60_000, hook: "push", repo: "D:\\Work\\game-client", remote: "github.com/magetech/game-client", client: "fork", email: "phi@magetechcorp.com", allow: false, code: "client-bound", message: "Fork is tied to Personal, but this repository belongs to Work", owner: "work" },
      { tsMs: Date.now() - 22 * 60_000, hook: "commit", repo: "D:\\src\\Abyss", remote: "github.com/hoangtrongphi/abyss", client: "terminal", email: "phi@magetechcorp.com", allow: false, code: "wrong-identity", message: "This commit would be authored as phi@magetechcorp.com (Work), but the repository belongs to Personal (phi.personal@example.com)", owner: "personal" },
      { tsMs: Date.now() - 58 * 60_000, hook: "push", repo: "D:\\src\\Abyss", remote: "github.com/hoangtrongphi/abyss", client: "fork", email: "phi.personal@example.com", allow: true, code: "ok", message: "Personal repository, Personal account", owner: "personal" },
      { tsMs: Date.now() - 3 * 3600_000, hook: "push", repo: "D:\\src\\dotfiles", remote: "github.com/hoangtrongphi/dotfiles", client: "github-desktop", email: "phi.personal@example.com", allow: false, code: "foreign-commits", message: "This push to a Personal repository carries commits authored as another account: 2 as Work (phi@magetechcorp.com)", owner: "personal" },
      { tsMs: Date.now() - 5 * 3600_000, hook: "commit", repo: "D:\\Work\\tools", remote: "gitlab.magetech.internal/tools/build", client: "sourcetree", email: "phi@magetechcorp.com", allow: true, code: "bypass", message: "SWITCHGIT_BYPASS=1: let through unchecked", owner: "work" },
      { tsMs: Date.now() - 26 * 3600_000, hook: "commit", repo: "D:\\Work\\game-client", remote: "github.com/magetech/game-client", client: "sourcetree", email: "phi@magetechcorp.com", allow: true, code: "ok", message: "Work repository, Work account", owner: "work" },
    ];

function block(): string {
  const on = config.accounts.filter((a) => a.enabled);
  const hooks = config.guard.enabled && config.accounts.length > 0;
  if (!on.length && !hooks) return "";
  const file = (id: string) => `"${DATA.replace(/\\/g, "/")}/accounts/${id}.gitconfig"`;
  const lines = ["# >>> switchgit >>> managed by SwitchGit: edit accounts in the app, not here"];
  for (const a of on) for (const f of a.folders) lines.push(`[includeIf "gitdir/i:${f.replace(/\\/g, "/")}/"]`, `\tpath = ${file(a.id)}`);
  for (const a of on)
    for (const r of a.remotes) {
      const [host, ...rest] = r.split("/");
      const path = rest.length ? `${rest.join("/")}/` : "";
      for (const glob of [`https://${host}/${path}**`, `https://*@${host}/${path}**`, `git@${host}:${path}**`, `ssh://git@${host}/${path}**`]) lines.push(`[includeIf "hasconfig:remote.*.url:${glob}"]`, `\tpath = ${file(a.id)}`);
    }
  if (hooks) lines.push("[core]", `\thooksPath = "${DATA.replace(/\\/g, "/")}/hooks"`);
  lines.push("# <<< switchgit <<<", "");
  return lines.join("\n");
}

function issues(): Issue[] {
  const out: Issue[] = config.accounts.filter((a) => !a.remotes.length && !a.folders.length).map((a) => ({ severity: "warn", code: "no-scope", message: `${a.label} owns no remote and no folder, so no repository will use it` }));
  if (config.accounts.some((a) => !a.enabled) && !config.guard.enabled) out.push({ severity: "warn", code: "off-without-guard", message: "The guard is off: a switched-off account only loses its identity, nothing refuses its pushes" });
  out.push({ severity: "info", code: "global-default", message: "Repositories no account owns commit as the global default phi@magetechcorp.com (Work)" });
  return out;
}

const snapshot = (): Snapshot => ({
  config: structuredClone(config),
  globalName: "Phi Hoang",
  globalEmail: "phi@magetechcorp.com",
  gitVersion: "2.45.1.windows.1",
  gitconfigPath: GITCONFIG,
  dataDir: DATA,
  pending,
  block: block(),
  issues: issues(),
});

/** A change applies the configuration, like the real commands do. */
function changed(line: string): Snapshot {
  pending = 0;
  void log("info", "accounts", line);
  return snapshot();
}

const CLIENTS: Omit<ClientStatus, "accountId">[] = [
  { id: "fork", name: "Fork", installed: true, exePath: "C:\\Users\\Phi\\AppData\\Local\\Fork\\current\\Fork.exe", running: true, note: "Uses its bundled git or the system git; both read the global config, so identities and the guard apply." },
  { id: "sourcetree", name: "Sourcetree", installed: true, exePath: "C:\\Users\\Phi\\AppData\\Local\\SourceTree\\SourceTree.exe", running: false, note: "Embedded or system git (Tools → Options → Git); both read the global config. Keep Sourcetree from rewriting the global config (Tools → Options → General)." },
  { id: "github-desktop", name: "GitHub Desktop", installed: true, exePath: "C:\\Users\\Phi\\AppData\\Local\\GitHubDesktop\\GitHubDesktop.exe", running: false, note: "Bundled git; pushes with the account signed in to the app, whatever the repository. Tying it here is what keeps that account off other repositories." },
  { id: "vscode", name: "Visual Studio Code", installed: true, exePath: "C:\\Users\\Phi\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe", running: true, note: "Source Control and the integrated terminal both run the system git." },
  { id: "gitkraken", name: "GitKraken", installed: false, exePath: null, running: false, note: "Does part of its work through its own engine, so hooks may not run for every action: check the Activity list after a first push." },
  { id: "tortoisegit", name: "TortoiseGit", installed: false, exePath: null, running: false, note: "Drives Git for Windows, so identities and the guard apply." },
  { id: "terminal", name: "Terminal & scripts", installed: true, exePath: null, running: false, note: "Git typed in a shell, an IDE terminal or run by a script: anything not started by a client above." },
];

/** Repositories in every state the table can show. */
function scanReport(roots: string[], partial: boolean): ScanReport {
  const owner = (remote: string) => config.accounts.find((a) => a.remotes.some((r) => remote.toLowerCase().replace(":", "/").includes(r.toLowerCase())));
  const make = (path: string, url: string | null, email: string): RepoInfo => {
    const o = url ? owner(url) : undefined;
    const identity = config.accounts.find((a) => a.email === email)?.id ?? null;
    const status = !o ? "unowned" : !o.enabled ? "account-off" : o.email === email ? "ok" : "wrong-identity";
    return { path, remotes: url ? [{ name: "origin", url }] : [], owner: o?.id ?? null, email, identity, status };
  };
  const repos = [
    make(`${ROOT}\\Abyss`, "https://github.com/HoangTrongPhi/Abyss.git", "phi.personal@example.com"),
    make(`${ROOT}\\SwitchGit`, "git@github.com:HoangTrongPhi/SwitchGit.git", "phi@magetechcorp.com"),
    make(`${ROOT}\\dotfiles`, "https://github.com/HoangTrongPhi/dotfiles.git", "phi.personal@example.com"),
    make("D:\\Work\\game-client", "https://github.com/MageTech/game-client.git", "phi@magetechcorp.com"),
    make("D:\\Work\\tools\\build", "https://gitlab.magetech.internal/tools/build.git", "phi@magetechcorp.com"),
    make(`${ROOT}\\oss\\tauri`, "https://github.com/tauri-apps/tauri.git", "phi@magetechcorp.com"),
    make(`${ROOT}\\scratch`, null, "phi@magetechcorp.com"),
  ];
  const fork: RepoInfo = { path: `${ROOT}\\engine-fork`, remotes: [{ name: "origin", url: "git@github.com:HoangTrongPhi/engine.git" }, { name: "upstream", url: "git@github.com:MageTech/engine.git" }], owner: null, email: "phi@magetechcorp.com", identity: "work", status: "mixed" };
  const all = partial ? repos.slice(0, 3) : [...repos, fork];
  return {
    roots,
    repos: all,
    issues: [
      { severity: "error", code: "folder-not-found", message: "Folder not found", path: "E:\\old-drive" },
      ...all.filter((r) => r.status === "wrong-identity").map<Issue>((r) => ({ severity: "warn", code: "wrong-identity", message: `Commits as ${r.email}, but belongs to ${owner(r.remotes[0].url)?.label}`, path: r.path })),
    ],
    elapsedMs: partial ? 420 : 1630,
    ok: false,
    cancelled: partial,
  };
}

const log = (level: string, source: string, message: string) => emit("log://entry", { tsMs: Date.now(), level, source, message });
const slug = (s: string) => s.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

if (import.meta.env.DEV && params.has("mock")) {
  console.info("mockBackend: active (?mock)");
  // `isTauri()` reads this flag; set it before lib/ipc.ts evaluates `inTauri`.
  (globalThis as { isTauri?: boolean }).isTauri = true;
  mockWindows("main");
  mockIPC(
    async (cmd, args) => {
      const a = (args ?? {}) as Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
      switch (cmd) {
        case "app_info":
          return { version: "0.1.0-mock", debug: true, abyss: { managed: true, launcherVersion: "0.1.0" } };
        case "log_history":
          return [
            { tsMs: Date.now() - 2000, level: "info", source: "app", message: "SwitchGit started (mock backend)" },
            { tsMs: Date.now() - 1000, level: "debug", source: "app", message: "Debug lines are hidden until you click the debug pill" },
          ];
        case "ui_log":
          return null;
        case "cancel_job":
          cancelled.add(a.jobId);
          return true;
        case "open_url":
          window.open(a.url, "_blank", "noopener");
          return null;
        case "reveal_in_explorer":
          console.info("mockBackend: reveal", a.path);
          return null;
        case "get_settings":
          return JSON.parse(localStorage.getItem("switchgit.mock-settings") ?? "{}");
        case "set_settings":
          localStorage.setItem("switchgit.mock-settings", JSON.stringify(a.settings));
          return null;

        case "get_state":
          return snapshot();
        case "save_account": {
          const account = a.account as Account;
          if (!account.label.trim()) throw "Give the account a name";
          if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(account.email)) throw `"${account.email}" is not an e-mail address`;
          const taken = config.accounts.find((x) => x.id !== account.id && x.email.toLowerCase() === account.email.toLowerCase());
          if (taken) throw `${taken.label} already uses ${account.email}: two accounts with one e-mail cannot be told apart`;
          await sleep(150);
          const i = config.accounts.findIndex((x) => account.id && x.id === account.id);
          if (i >= 0) config.accounts[i] = account;
          else {
            let id = slug(account.label) || "account";
            for (let n = 2; config.accounts.some((x) => x.id === id); n++) id = `${slug(account.label)}-${n}`;
            config.accounts.push({ ...account, id });
          }
          return changed(`Saved account ${account.label}`);
        }
        case "delete_account": {
          const account = config.accounts.find((x) => x.id === a.id);
          if (!account) throw `No account "${a.id}"`;
          config.accounts = config.accounts.filter((x) => x.id !== a.id);
          for (const [client, id] of Object.entries(config.bindings)) if (id === a.id) delete config.bindings[client];
          return changed(`Removed account ${account.label}`);
        }
        case "set_account_enabled": {
          const account = config.accounts.find((x) => x.id === a.id);
          if (!account) throw `No account "${a.id}"`;
          await sleep(120);
          account.enabled = a.enabled;
          return changed(`${account.label} switched ${a.enabled ? "on" : "off"}`);
        }
        case "set_binding": {
          const client = CLIENTS.find((c) => c.id === a.client);
          if (!client) throw `Unknown git client "${a.client}"`;
          if (a.accountId) config.bindings[a.client] = a.accountId;
          else delete config.bindings[a.client];
          return changed(a.accountId ? `${client.name} is now tied to ${config.accounts.find((x) => x.id === a.accountId)?.label}` : `${client.name} follows each repository's account`);
        }
        case "set_guard":
          config.guard = a.guard;
          return changed(`Guard ${a.guard.enabled ? "on" : "off"}`);
        case "apply_config": {
          await sleep(300);
          const wasPending = pending;
          if (!a.dryRun) pending = 0;
          return {
            dryRun: !!a.dryRun,
            changes: [
              { path: GITCONFIG, action: wasPending ? "update" : "unchanged", kind: "gitconfig" },
              ...config.accounts.filter((x) => x.enabled).map((x) => ({ path: `${DATA}\\accounts\\${x.id}.gitconfig`, action: "unchanged", kind: "account" })),
              { path: `${DATA}\\hooks\\pre-commit`, action: "unchanged", kind: "hook" },
              { path: `${DATA}\\hooks\\pre-push`, action: "unchanged", kind: "hook" },
            ],
            block: block(),
            backup: null,
            issues: issues(),
            elapsedMs: 12,
            ok: true,
            cancelled: false,
          };
        }

        case "list_clients":
          return CLIENTS.map((c) => ({ ...c, accountId: config.bindings[c.id] ?? null }));
        case "launch_client": {
          const client = CLIENTS.find((c) => c.id === a.id);
          if (!client?.exePath) throw `${client?.name ?? a.id} was not found on this PC`;
          client.running = true;
          return null;
        }

        case "scan_repos": {
          if (!a.roots?.length) throw "Choose a folder to scan";
          config.scanRoots = a.roots;
          const jobId = nextJob++;
          await emit("repos://progress", { jobId, done: 0, total: 0, message: "Starting…" });
          await sleep(400);
          for (let i = 1; i <= 8; i++) {
            await sleep(220);
            if (cancelled.has(jobId)) {
              await log("warn", "scan", "3 repositories (cancelled)");
              return scanReport(a.roots, true);
            }
            await emit("repos://progress", { jobId, done: i, total: 8, message: `${ROOT}\\repo-${i}` });
          }
          await log("info", "scan", "8 repositories: 4 ok, 1 wrong identity, 0 switched off, 3 unowned in 1630 ms");
          return scanReport(a.roots, false);
        }

        case "guard_log":
          return guardLog.slice(0, a.limit ?? 200);
        case "clear_guard_log":
          guardLog = [];
          return null;

        case "plugin:dialog|open":
          return a.options?.directory ? "D:\\Work" : a.options?.multiple ? ["C:\\Users\\Phi\\.ssh\\id_work"] : "C:\\Users\\Phi\\.ssh\\id_work";
        case "plugin:dialog|save":
          return `${ROOT}\\repos.json`;
        case "plugin:updater|check":
          return { rid: 1, currentVersion: "0.1.0-mock", version: "0.2.0", date: "2026-11-01T00:00:00Z", body: "- Tray icon with account switches", rawJson: {} };
        case "plugin:updater|download_and_install":
          await sleep(1500);
          return null;
        case "plugin:window|set_theme":
        case "plugin:webview|set_webview_zoom":
          return null;
        case "plugin:process|restart":
          location.reload();
          return null;
        default:
          if (cmd.startsWith("plugin:event|")) return 0;
          console.warn("mockBackend: unhandled", cmd, args);
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
