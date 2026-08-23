import { spawnSync } from "node:child_process";

const result = spawnSync("deno", ["--version"], { stdio: "pipe" });
if (result.status === 0) {
  const version = result.stdout.toString().trim().split("\n")[0];
  console.log(`[k-agent] ${version}`);
  process.exit(0);
}

console.error("[k-agent] Deno is required but was not found in PATH.");
console.error("");
console.error("Install Deno:");
console.error("  macOS/Linux: curl -fsSL https://deno.land/install.sh | sh");
console.error("  Windows:     irm https://deno.land/install.ps1 | iex");
console.error("  Docs:        https://docs.deno.com/runtime/getting_started/installation/");
console.error("");
console.error("After installing, restart your shell and run 'pnpm dev:desktop' again.");
process.exit(1);