export const MCP_TRANSPORTS = ["stdio", "http"] as const;
export const MCP_SCOPES = ["global", "local"] as const;

export type McpTransport = (typeof MCP_TRANSPORTS)[number];
export type McpScope = (typeof MCP_SCOPES)[number];

export type McpToolSummary = {
  name: string;
  description?: string;
  inputSchema?: Record<string, unknown>;
};

export type McpServer = {
  id: string;
  scope: McpScope;
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
  scope: McpScope;
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
