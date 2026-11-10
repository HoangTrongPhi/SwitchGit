# SwitchGit — File formats

Every file SwitchGit reads or writes. JSON conventions (Abyss `docs/app-specs/00_SHARED_FRAMEWORK.md`
§6): a `"format"` and a `"version"` field, 2-space indent, stable key order, unknown fields ignored,
missing fields take their defaults.

All of SwitchGit's own files live in `%LOCALAPPDATA%\com.phineas.switchgit\` (`SWITCHGIT_HOME`
overrides it).

## Accounts — `switchgit.json`

```json
{
  "format": "switchgit.config",
  "version": 1,
  "accounts": [
    {
      "id": "work",
      "label": "Work",
      "kind": "work",
      "name": "Phi Hoang",
      "email": "phi@magetechcorp.com",
      "enabled": true,
      "remotes": ["github.com/MageTech"],
      "folders": ["D:/Work"],
      "username": "phi-magetech",
      "sshKey": null
    }
  ],
  "bindings": { "fork": "personal", "sourcetree": "work" },
  "guard": { "enabled": true, "checkCommit": true, "checkPush": true, "checkAuthors": true, "blockUnknown": false },
  "scanRoots": ["D:/src"]
}
```

- `kind`: `work`, `personal` or `other` (a badge only).
- `remotes`: `host/owner`, `host/owner/repo`, a whole `host`, or any git URL; compared without scheme,
  user, port, `.git` and case.
- `bindings`: client id (`fork`, `sourcetree`, `github-desktop`, `gitkraken`, `tortoisegit`,
  `git-extensions`, `smartgit`, `vscode`, `visual-studio`, `jetbrains`, `terminal`) → account id.
- No secrets are stored.

## Generated git files

| File | Content |
|---|---|
| `accounts/<id>.gitconfig` | `[user]`, `[credential "https://<host>"] username`, `[core] sshCommand`; only for accounts that are on |
| `hooks/<name>` | One sh script per client-side hook: guard for `pre-commit` / `pre-push`, then the repository's own hook |
| `~/.gitconfig` | Only the block between `# >>> switchgit >>>` and `# <<< switchgit <<<`, at the end of the file |
| `~/.gitconfig.switchgit.bak` | The global config as it was before the first change |

## Activity — `guard.log`

JSON lines, newest last; trimmed to its newer half past 512 KB.

```json
{"tsMs":1794297186852,"hook":"commit","repo":"D:/src/Abyss","remote":"github.com/hoangtrongphi/abyss","client":"terminal","email":"phi@magetechcorp.com","allow":false,"code":"wrong-identity","message":"This commit would be authored as phi@magetechcorp.com (Work), but the repository belongs to Personal (phi.personal@example.com)","owner":"personal"}
```

`code`: `ok`, `bypass`, `mixed-remotes`, `account-off`, `client-bound`, `wrong-identity`,
`foreign-commits`, `unknown-repo`. Repositories no account owns are not logged.

## Reports (`--report`)

The command line writes the same structure the app shows. Every report has `elapsedMs`, `ok`,
`cancelled` and `issues[]` of `{ severity: "info" | "warn" | "error", code, message, path? }`.

### Repository scan (`switchgit scan <folder> --report repos.json`)

```json
{
  "roots": ["D:/src"],
  "repos": [
    {
      "path": "D:/src/Abyss",
      "remotes": [{ "name": "origin", "url": "https://github.com/HoangTrongPhi/Abyss.git" }],
      "owner": "personal",
      "email": "phi@magetechcorp.com",
      "identity": "work",
      "status": "wrong-identity"
    }
  ],
  "issues": [{ "severity": "warn", "code": "wrong-identity", "message": "Commits as phi@magetechcorp.com, but belongs to Personal", "path": "D:/src/Abyss" }],
  "elapsedMs": 412,
  "ok": true,
  "cancelled": false
}
```

`status`: `ok`, `wrong-identity`, `account-off`, `mixed`, `unowned`.
