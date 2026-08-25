export const MCP_TRANSPORTS = ["stdio", "sse", "http"] as const;

export type McpTransport = (typeof MCP_TRANSPORTS)[number];

export type McpToolSummary = {
  name: string;
  description?: string;
};

export type McpServer = {
  id: string;
  name: string;
  enabled: boolean;
  transport: McpTransport;
  command?: string;
  args?: string[];
  cwd?: string;
  url?: string;
  hasEnvSecrets?: boolean;
  hasHeaderSecrets?: boolean;
  tools?: McpToolSummary[];
  toolsSyncedAt?: number;
  toolsProbeError?: string;
};

export type McpServerDraft = {
  id?: string;
  name: string;
  enabled: boolean;
  transport: McpTransport;
  command?: string;
  args?: string[];
  cwd?: string;
  url?: string;
  env?: Record<string, string>;
  headers?: Record<string, string>;
  clearSecrets?: boolean;
};
