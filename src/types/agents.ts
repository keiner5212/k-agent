export type AgentContextKind = "global" | "local";

export const AGENT_TOOL_IDS = [
  "read_file",
  "write_file",
  "edit_file",
  "delete_file",
  "run_command",
  "search_code",
  "web_fetch",
] as const;

export type AgentToolId = (typeof AGENT_TOOL_IDS)[number];

export const MAX_AGENT_SKILLS = 10;
export const MAX_AGENT_PERSONALITY_LINES = 200;

export const personalityLineCount = (text: string): number => {
  if (text.length === 0) return 0;
  return text.split("\n").length;
};

export const clampPersonality = (text: string): string => {
  if (text.length === 0) return "";
  const lines = text.split("\n");
  if (lines.length <= MAX_AGENT_PERSONALITY_LINES) return text;
  return lines.slice(0, MAX_AGENT_PERSONALITY_LINES).join("\n");
};

export type AgentSkillRef = {
  kind: AgentContextKind;
  id: string;
};

export type AgentMeta = {
  id: string;
  path: string;
  name: string;
  description: string;
  personality: string;
  estimatedTokens: number;
  skills: AgentSkillRef[];
  tools: string[];
};

export type AgentContext = {
  kind: AgentContextKind;
  path: string;
  agents: AgentMeta[];
};

export const agentKey = (kind: AgentContextKind, id: string): string => `${kind}:${id}`;

export const parseAgentKey = (value: string): { kind: AgentContextKind; id: string } | null => {
  const sep = value.indexOf(":");
  if (sep <= 0) return null;
  const kind = value.slice(0, sep);
  const id = value.slice(sep + 1);
  if ((kind !== "global" && kind !== "local") || id.length === 0) return null;
  return { kind, id };
};

export const skillRefKey = (skill: AgentSkillRef): string => `${skill.kind}/${skill.id}`;
