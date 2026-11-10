#!/usr/bin/env node
// Fails when a command invoked in src/lib/ipc.ts has no `case "<name>"` in src/dev/mockBackend.ts,
// so ?mock never silently answers null. Part of `npm run check`.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const ipc = readFileSync(join(root, "src", "lib", "ipc.ts"), "utf8");
const mock = readFileSync(join(root, "src", "dev", "mockBackend.ts"), "utf8");

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const commands = [...new Set([...ipc.matchAll(/invoke\s*(?:<[^()]*?>)?\s*\(\s*["'`]([\w:|-]+)["'`]/g)].map((m) => m[1]))];
const missing = commands.filter((c) => !new RegExp(`case\\s+["'\`]${escape(c)}["'\`]`).test(mock));

if (!commands.length) {
  console.error('check-mock: found no invoke("…") calls in src/lib/ipc.ts');
  process.exit(1);
}
if (missing.length) {
  console.error(`check-mock: ${missing.length} command(s) in src/lib/ipc.ts have no case in src/dev/mockBackend.ts:\n  ${missing.join("\n  ")}`);
  process.exit(1);
}
console.log(`check-mock: all ${commands.length} commands are mocked`);
