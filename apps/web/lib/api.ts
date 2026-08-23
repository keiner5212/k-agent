import {
  API,
  type Agent,
  type AgentRunRequest,
  type AgentRunResponse,
  type HealthResponse,
  type WsClientMessage,
  type WsServerMessage,
} from "@k-agent/shared";

function baseUrl(): string {
  if (typeof window === "undefined") return "";
  return window.location.origin;
}

export async function getHealth(): Promise<HealthResponse> {
  const res = await fetch(`${baseUrl()}${API.health}`);
  if (!res.ok) throw new Error(`health failed: ${res.status}`);
  return (await res.json()) as HealthResponse;
}

export async function listAgents(): Promise<Agent[]> {
  const res = await fetch(`${baseUrl()}${API.agents}`);
  if (!res.ok) throw new Error(`list agents failed: ${res.status}`);
  return (await res.json()) as Agent[];
}

export async function runAgent(id: string, body: AgentRunRequest): Promise<AgentRunResponse> {
  const res = await fetch(`${baseUrl()}${API.agentRun(id)}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`run agent failed: ${res.status}`);
  return (await res.json()) as AgentRunResponse;
}

export function openSocket(handlers: {
  onOpen?: () => void;
  onMessage?: (msg: WsServerMessage) => void;
  onClose?: () => void;
  onError?: () => void;
} = {}): WebSocket {
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${proto}//${window.location.host}${API.ws}`);
  ws.onopen = () => handlers.onOpen?.();
  ws.onmessage = (e) => {
    try {
      handlers.onMessage?.(JSON.parse(e.data) as WsServerMessage);
    } catch {
      // ignore malformed frames
    }
  };
  ws.onclose = () => handlers.onClose?.();
  ws.onerror = () => handlers.onError?.();
  return ws;
}

export function send(ws: WebSocket, msg: WsClientMessage): void {
  ws.send(JSON.stringify(msg));
}