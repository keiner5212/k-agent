import { startServer } from "./server.ts";
import { DEFAULT_DESKTOP_PORT } from "@k-agent/shared";

const port = Number(Deno.env.get("K_AGENT_PORT")) || DEFAULT_DESKTOP_PORT;
const url = `http://127.0.0.1:${port}`;

const server = await startServer(port);

try {
  const mod = await import("./webview.ts");
  console.log(`[k-agent] opening desktop window at ${url}`);
  await mod.openWindow({ url, title: "k-agent" });
} catch (err) {
  console.warn("[k-agent] desktop window unavailable. Server only.");
  console.warn("  See apps/desktop/webview.ts for binding setup.");
  console.warn(`  Error: ${err instanceof Error ? err.message : err}`);
  console.warn(`  Server still serving at ${url}`);
  await new Promise(() => {});
}

await server.finished;