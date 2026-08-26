import type { TFunction } from "i18next";
import { estimateTokensFromText } from "@/lib/jobs-handlers";
import { AGENT_TOOL_IDS, parseAgentKey, type AgentContext, type AgentMeta } from "@/types/agents";

export const BUILTIN_AGENT_IDS = ["build", "plan"] as const;

export type BuiltinAgentId = (typeof BUILTIN_AGENT_IDS)[number];

export const BUILTIN_AGENTS_ROOT = "k-agent/builtin/agents";

const BUILTIN_KEY_PREFIX = "builtin:";

export const isBuiltinAgentId = (id: string): id is BuiltinAgentId =>
  (BUILTIN_AGENT_IDS as readonly string[]).includes(id);

export const builtinAgentKey = (id: BuiltinAgentId): string => `${BUILTIN_KEY_PREFIX}${id}`;

export const parseBuiltinAgentKey = (value: string): BuiltinAgentId | null => {
  if (!value.startsWith(BUILTIN_KEY_PREFIX)) return null;
  const id = value.slice(BUILTIN_KEY_PREFIX.length);
  return isBuiltinAgentId(id) ? id : null;
};

export const builtinAgentTools = (): string[] => [...AGENT_TOOL_IDS];

export const builtinAgentMeta = (id: BuiltinAgentId, t: TFunction): AgentMeta => {
  const description = t(`agents.builtin.${id}.description`);
  const personality = t(`agents.builtin.${id}.personality`);
  const tools = builtinAgentTools();
  return {
    id,
    path: `${BUILTIN_AGENTS_ROOT}/${id}`,
    name: id,
    description,
    personality,
    estimatedTokens: estimateTokensFromText(personality),
    skills: [],
    tools,
  };
};

export const listAllBuiltinAgents = (t: TFunction): AgentMeta[] =>
  BUILTIN_AGENT_IDS.map((id) => builtinAgentMeta(id, t));

export const builtinAgentContext = (t: TFunction): AgentContext => ({
  kind: "builtin",
  path: BUILTIN_AGENTS_ROOT,
  agents: listAllBuiltinAgents(t),
});

export const listEnabledBuiltinAgents = (
  t: TFunction,
  enabled: { build: boolean; plan: boolean },
): AgentMeta[] => {
  const out: AgentMeta[] = [];
  if (enabled.build) out.push(builtinAgentMeta("build", t));
  if (enabled.plan) out.push(builtinAgentMeta("plan", t));
  return out;
};

export const resolveAgentMeta = (
  key: string,
  contexts: AgentContext[],
  t: TFunction,
): AgentMeta | null => {
  if (key.length === 0) return null;
  const builtin = parseBuiltinAgentKey(key);
  if (builtin) return builtinAgentMeta(builtin, t);
  const parsed = parseAgentKey(key);
  if (!parsed) return null;
  for (const context of contexts) {
    if (context.kind !== parsed.kind) continue;
    const agent = context.agents.find((item) => item.id === parsed.id);
    if (agent) return agent;
  }
  return null;
};
