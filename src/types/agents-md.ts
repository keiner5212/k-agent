export type AgentsMdKind = "global" | "local";

export type AgentsMdFile = {
  kind: AgentsMdKind;
  path: string;
  exists: boolean;
  content: string;
  estimatedTokens: number;
};
