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
  console.error("[k-agent] could not open desktop window.");
  console.error(err instanceof Error ? err.message : String(err));
  console.error(`[k-agent] server still serving at ${url}`);
  await new Promise(() => {});
}

await server.finished;