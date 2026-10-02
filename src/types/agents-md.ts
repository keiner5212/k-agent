export type AgentsMdKind = "global" | "local";

export type AgentsMdFile = {
  kind: AgentsMdKind;
  path: string;
  exists: boolean;
  managed: boolean;
  content: string;
  estimatedTokens: number;
};
