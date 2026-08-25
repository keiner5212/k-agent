export type SelectedModel = {
  providerId: string;
  modelId: string;
};

export const AGENT_MODES = ["plan", "build"] as const;
export type AgentMode = (typeof AGENT_MODES)[number];
export const DEFAULT_AGENT_MODE: AgentMode = "plan";

export const isAgentMode = (value: string): value is AgentMode =>
  (AGENT_MODES as readonly string[]).includes(value);
