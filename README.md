# k-agent

Desktop AI agent orchestrator. Empty template.

Stack: **Deno + Hono + WebView (Deno bindings) + Next.js (static export)** in a pnpm monorepo.

## Why this stack

- **Deno** runs the backend and serves the frontend. One runtime, no Node in production.
- **Hono** is the HTTP framework (Express-like DX, native Deno support).
- **WebView** is the native desktop window. Lightweight, no Electron bloat.
- **Next.js** builds to plain static files (`out/`). Deno serves them directly.

## Prerequisites

| Tool | Version |
| --- | --- |
| Node | >= 20 |
| pnpm | >= 9 |
| Deno | >= 1.45 |

Install Deno: `curl -fsSL https://deno.land/install.sh | sh`

## Repo layout

```
k-agent/
|-- apps/
|   |-- desktop/           Deno entrypoint: Hono server + WebView shell
|   |   |-- main.ts        Boot order: start server, open window
|   |   |-- server.ts      Hono app: REST, WebSocket, static frontend
|   |   |-- webview.ts     Native window wrapper (binding placeholder)
|   |   |-- deno.json      Deno tasks + import map
|   |   `-- public/        Static frontend (populated by sync)
|   `-- web/               Next.js frontend (App Router)
|       |-- app/           Pages and layouts
|       |-- lib/api.ts     REST + WebSocket client stubs
|       `-- next.config.mjs  Configured for `output: export`
|-- packages/
|   `-- shared/            TypeScript contract shared by both runtimes
|       `-- src/types.ts   API paths, WS envelope, domain types
|-- scripts/
|   `-- sync-web.mjs       Copies web/out -> desktop/public
|-- package.json           Root scripts (dev/build/clean)
|-- pnpm-workspace.yaml    Workspace definition
`-- README.md
```

## Run modes

### 1. Web dev only (browser)

```
pnpm install
pnpm dev:web
```

Opens Next.js dev server at <http://localhost:3000>. Use this for fast UI iteration. The browser talks to whatever backend you point it at (default: same origin).

### 2. Desktop dev (Hono server, no native window yet)

In one terminal:

```
pnpm install
pnpm build:web       # produces apps/web/out
pnpm sync:web        # copies out -> apps/desktop/public
pnpm dev:desktop     # deno task dev, runs main.ts with --watch
```

Server runs at <http://127.0.0.1:7421>. If the WebView binding is not yet configured, the server still runs and logs a warning. You can hit it from any browser at that URL.

### 3. Full desktop (native window + bundled binary)

Same setup as mode 2, then install a WebView binding (see Caveats below) and run `pnpm dev:desktop`. The window will open pointing at the Hono server.

To produce a standalone binary:

```
pnpm build           # builds web, syncs, then deno compile
./apps/desktop/bin/k-agent
```

## Communications contract

Both runtimes import from `@k-agent/shared`. Edit `packages/shared/src/types.ts` and both sides stay in sync.

### REST endpoints (Hono)

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/health` | Liveness check |
| GET | `/api/agents` | List agents (returns empty array) |
| POST | `/api/agents/:id/run` | Trigger an agent run |

The catch-all route `app.get('*')` serves `public/` for static assets and falls back to `public/index.html` for client-side routing.

### WebSocket (`/ws`)

Envelope is JSON. The `WsClientMessage` and `WsServerMessage` discriminated unions live in `packages/shared/src/types.ts`.

Client -> server:

```
{ "type": "ping" }
{ "type": "subscribe", "channel": "agents" }
{ "type": "agent.run", "agentId": "abc", "input": "..." }
```

Server -> client:

```
{ "type": "pong", "at": "<iso8601>" }
{ "type": "subscribed", "channel": "..." }
{ "type": "agent.event", "agentId": "abc", "event": { ... } }
{ "type": "error", "code": "BAD_FRAME", "message": "..." }
```

The current handler is a stub. Replace `handleClientMessage` in `apps/desktop/server.ts` with your orchestrator logic.

### Frontend client

`apps/web/lib/api.ts` exposes:

- `getHealth()`, `listAgents()`, `runAgent(id, body)`
- `openSocket(handlers)` and `send(ws, msg)`

These wrap the shared types so paths and envelopes stay consistent.

## Environment

| Var | Default | Purpose |
| --- | --- | --- |
| `K_AGENT_PORT` | `7421` (from `@k-agent/shared`) | Server port. Overrides `DEFAULT_DESKTOP_PORT`. |

## Caveats

### WebView binding

`apps/desktop/webview.ts` imports a placeholder URL. Pick a binding that works with your Deno version:

- `npm:webview` via Deno's npm: specifier
- `https://deno.land/x/webview` (verify the latest version on deno.land/x)
- A JSR package if one is published

The `Webview` constructor shape differs between bindings. Update `openWindow` to match your chosen API.

### Static export limitations

`output: 'export'` in `next.config.mjs` means:

- No server components that fetch per request
- No middleware
- No ISR

If you need SSR, you will need to switch to `next start` and have Deno proxy to a Node Next.js server, or run Next.js on Deno via node compat.

### Port

Default is `7421` to avoid common dev ports. Change it in `packages/shared/src/types.ts` (`DEFAULT_DESKTOP_PORT`) or override with `K_AGENT_PORT`.

## Next steps

This template is intentionally empty. Suggested layering order:

1. Define your agent domain types in `packages/shared/src/types.ts`.
2. Replace the stub handlers in `apps/desktop/server.ts`.
3. Build the UI in `apps/web/app/` using the client stubs from `apps/web/lib/api.ts`.
4. Add persistence, queues, or external model APIs as needed.
5. Configure the WebView binding once you settle on the production binary shape.