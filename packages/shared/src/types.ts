// Shared contract between web and desktop runtimes.
// Keep this file dependency-free so both Deno (npm:) and Next.js can import it.

export interface HealthResponse {
  status: "ok" | "degraded" | "down";
  uptimeSeconds: number;
  version: string;
}

export type AgentStatus = "idle" | "running" | "error" | "stopped";

export interface Agent {
  id: string;
  name: string;
  status: AgentStatus;
  createdAt: string;
}

export interface AgentRunRequest {
  input: string;
}

export interface AgentRunResponse {
  agentId: string;
  acceptedAt: string;
}

export interface ApiError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

export type WsClientMessage =
  | { type: "ping" }
  | { type: "subscribe"; channel: string }
  | { type: "agent.run"; agentId: string; input: string };

export type WsServerMessage =
  | { type: "pong"; at: string }
  | { type: "subscribed"; channel: string }
  | { type: "agent.event"; agentId: string; event: unknown }
  | { type: "error"; code: string; message: string };

export const API = {
  health: "/api/health",
  agents: "/api/agents",
  agentById: (id: string) => `/api/agents/${id}`,
  agentRun: (id: string) => `/api/agents/${id}/run`,
  ws: "/ws",
} as const;

export const DEFAULT_DESKTOP_PORT = 7421;