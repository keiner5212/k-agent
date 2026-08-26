import type { TFunction } from "i18next";
import { estimateTokensFromText } from "@/lib/jobs-handlers";
import { AGENT_TOOL_IDS, parseAgentKey, type AgentContext, type AgentMeta } from "@/types/agents";

export const BUILTIN_AGENT_IDS = ["build", "plan"] as const;

export type BuiltinAgentId = (typeof BUILTIN_AGENT_IDS)[number];

export const BUILTIN_AGENTS_ROOT = "k-agent/builtin/agents";

const BUILTIN_KEY_PREFIX = "builtin:";

const PLAN_TOOLS = ["read_file", "search_code", "web_fetch"] as const;

const yamlQuote = (value: string): string => {
  if (value.length === 0) return '""';
  if (/^[a-zA-Z0-9_./-]+$/.test(value)) return value;
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
};

const buildAgentMarkdown = (
  name: string,
  description: string,
  skills: readonly { kind: string; id: string }[],
  tools: readonly string[],
  body: string,
): string => {
  let out = "---\n";
  out += `name: ${yamlQuote(name)}\n`;
  out += `description: ${yamlQuote(description)}\n`;
  if (skills.length === 0) {
    out += "skills: []\n";
  } else {
    out += "skills:\n";
    for (const skill of skills) {
      out += `  - ${skill.kind}/${skill.id}\n`;
    }
  }
  if (tools.length === 0) {
    out += "tools: []\n";
  } else {
    out += "tools:\n";
    for (const tool of tools) {
      out += `  - ${tool}\n`;
    }
  }
  out += "---\n";
  const trimmedBody = body.replace(/\n+$/, "");
  if (trimmedBody.length === 0) {
    out += "\n";
  } else {
    out += `\n${trimmedBody}\n`;
  }
  return out;
};

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

export const builtinAgentMeta = (id: BuiltinAgentId, t: TFunction): AgentMeta => {
  const description = t(`agents.builtin.${id}.description`);
  const personality = t(`agents.builtin.${id}.personality`);
  const tools = builtinAgentTools(id);
  const markdown = buildAgentMarkdown(id, description, [], tools, personality);
  return {
    id,
    path: `${BUILTIN_AGENTS_ROOT}/${id}`,
    name: id,
    description,
    personality,
    estimatedTokens: estimateTokensFromText(markdown),
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

export const resolveAgentPersonality = (
  key: string,
  contexts: AgentContext[],
  t: TFunction,
): string => {
  if (key.length === 0) return "";
  const builtin = parseBuiltinAgentKey(key);
  if (builtin) return builtinAgentMeta(builtin, t).personality.trim();
  const parsed = parseAgentKey(key);
  if (!parsed) return "";
  for (const context of contexts) {
    if (context.kind !== parsed.kind) continue;
    const agent = context.agents.find((item) => item.id === parsed.id);
    if (agent) return agent.personality.trim();
  }
  return "";
};
