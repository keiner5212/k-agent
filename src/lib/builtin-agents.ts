import type { TFunction } from "i18next";
import { AGENT_TOOL_IDS, type AgentMeta } from "@/types/agents";

export const BUILTIN_AGENT_IDS = ["build", "plan"] as const;

export type BuiltinAgentId = (typeof BUILTIN_AGENT_IDS)[number];

const BUILTIN_KEY_PREFIX = "builtin:";

const PLAN_TOOLS = ["read_file", "search_code", "web_fetch"] as const;

export const isBuiltinAgentId = (id: string): id is BuiltinAgentId =>
  (BUILTIN_AGENT_IDS as readonly string[]).includes(id);

export const builtinAgentKey = (id: BuiltinAgentId): string => `${BUILTIN_KEY_PREFIX}${id}`;

export const parseBuiltinAgentKey = (value: string): BuiltinAgentId | null => {
  if (!value.startsWith(BUILTIN_KEY_PREFIX)) return null;
  const id = value.slice(BUILTIN_KEY_PREFIX.length);
  return isBuiltinAgentId(id) ? id : null;
};

export const builtinAgentTools = (id: BuiltinAgentId): string[] => {
  if (id === "plan") return [...PLAN_TOOLS];
  return [...AGENT_TOOL_IDS];
};

export const builtinAgentMeta = (id: BuiltinAgentId, t: TFunction): AgentMeta => ({
  id,
  path: "",
  name: t(`builtinAgents.${id}.name`),
  description: t(`builtinAgents.${id}.description`),
  personality: t(`builtinAgents.${id}.personality`),
  estimatedTokens: 0,
  skills: [],
  tools: builtinAgentTools(id),
});

export const listAllBuiltinAgents = (t: TFunction): AgentMeta[] =>
  BUILTIN_AGENT_IDS.map((id) => builtinAgentMeta(id, t));

export const listEnabledBuiltinAgents = (
  t: TFunction,
  enabled: { build: boolean; plan: boolean },
): AgentMeta[] => {
  const out: AgentMeta[] = [];
  if (enabled.build) out.push(builtinAgentMeta("build", t));
  if (enabled.plan) out.push(builtinAgentMeta("plan", t));
  return out;
};
