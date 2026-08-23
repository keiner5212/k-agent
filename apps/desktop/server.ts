import { Hono } from "hono";
import { upgradeWebSocket } from "hono/deno";
import {
  API,
  DEFAULT_DESKTOP_PORT,
  type AgentRunRequest,
  type AgentRunResponse,
  type HealthResponse,
  type WsClientMessage,
  type WsServerMessage,
} from "@k-agent/shared";

const startedAt = Date.now();
const VERSION = "0.0.0";

const MIME: Record<string, string> = {
  html: "text/html; charset=utf-8",
  js: "application/javascript",
  mjs: "application/javascript",
  css: "text/css; charset=utf-8",
  json: "application/json",
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  svg: "image/svg+xml",
  ico: "image/x-icon",
  woff: "font/woff",
  woff2: "font/woff2",
  ttf: "font/ttf",
  txt: "text/plain; charset=utf-8",
};

export function createApp() {
  const app = new Hono();

  app.get(API.health, (c) => {
    const body: HealthResponse = {
      status: "ok",
      uptimeSeconds: Math.floor((Date.now() - startedAt) / 1000),
      version: VERSION,
    };
    return c.json(body);
  });

  app.get(API.agents, (c) => c.json([]));

  app.post(API.agentRun(":id"), async (c) => {
    const id = c.req.param("id");
    await c.req.json<AgentRunRequest>().catch(() => null);
    const body: AgentRunResponse = {
      agentId: id,
      acceptedAt: new Date().toISOString(),
    };
    return c.json(body);
  });

  app.get(
    API.ws,
    upgradeWebSocket(() => ({
      onOpen(_event, ws) {
        sendWs(ws, { type: "subscribed", channel: "default" });
      },
      onMessage(event, ws) {
        handleClientMessage(String(event.data), ws);
      },
      onClose() {},
      onError() {},
    })),
  );

  app.get("*", async (c) => {
    if (c.req.path.startsWith("/api/") || c.req.path === "/ws") {
      return c.notFound();
    }
    let fsPath = c.req.path;
    if (fsPath === "/" || fsPath.endsWith("/")) fsPath += "index.html";
    try {
      const file = await Deno.readFile(`./public${fsPath}`);
      const ext = fsPath.split(".").pop()?.toLowerCase() ?? "";
      const ct = MIME[ext] ?? "application/octet-stream";
      return new Response(file, { headers: { "content-type": ct } });
    } catch {
      try {
        const html = await Deno.readFile("./public/index.html");
        return c.html(html);
      } catch {
        return c.notFound();
      }
    }
  });

  return app;
}

interface WsLike {
  send: (data: string) => void;
}

function sendWs(ws: WsLike, msg: WsServerMessage) {
  ws.send(JSON.stringify(msg));
}

function handleClientMessage(raw: string, ws: WsLike) {
  let msg: WsClientMessage;
  try {
    msg = JSON.parse(raw) as WsClientMessage;
  } catch {
    sendWs(ws, { type: "error", code: "BAD_FRAME", message: "invalid JSON" });
    return;
  }
  switch (msg.type) {
    case "ping":
      sendWs(ws, { type: "pong", at: new Date().toISOString() });
      break;
    case "subscribe":
      sendWs(ws, { type: "subscribed", channel: msg.channel });
      break;
    case "agent.run":
      sendWs(ws, {
        type: "agent.event",
        agentId: msg.agentId,
        event: { kind: "queued", input: msg.input },
      });
      break;
  }
}

export async function startServer(port = DEFAULT_DESKTOP_PORT) {
  const app = createApp();
  const server = Deno.serve({ port, hostname: "127.0.0.1" }, app.fetch);
  const addr = server.addr;
  console.log(`[k-agent] server listening on http://${addr.hostname}:${addr.port}`);
  return server;
}