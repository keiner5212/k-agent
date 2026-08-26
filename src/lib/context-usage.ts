import { estimateTokensFromText } from "@/lib/jobs-handlers";
import {
  AGENT_TOOL_IDS,
  CHAT_TOOL_DESCRIPTIONS,
  type AgentSkillRef,
  type AgentToolId,
} from "@/types/agents";
import type { ChatMessage, SelectedModel } from "@/types/chat";
import { formatTokenCount, type ModelCost, type ModelInfo, type Provider } from "@/types/providers";
import type { SkillContext } from "@/types/skills";

const SKILL_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    name: {
      type: "string",
      description: "Skill name from the available skills list",
    },
  },
  required: ["name"],
} as const;

const TOOL_PARAMETERS: Record<AgentToolId, object> = {
  skill: SKILL_TOOL_PARAMETERS,
};

export const CONTEXT_CATEGORY_IDS = [
  "systemPrompt",
  "languageDirective",
  "toolDefinitions",
  "rules",
  "skills",
  "mcpTools",
  "subagentDefinitions",
  "conversation",
] as const;

export type ContextCategoryId = (typeof CONTEXT_CATEGORY_IDS)[number];

export type ContextExtras = Partial<Omit<Record<ContextCategoryId, number>, "conversation">>;

export type ContextBucket = {
  id: ContextCategoryId;
  tokens: number;
};

export type ContextUsageSnapshot = {
  windowTokens: number;
  usedTokens: number;
  freeTokens: number;
  percent: number;
  buckets: ContextBucket[];
  costUsd: number;
};

export const resolveSelectedModel = (
  providers: Provider[],
  selection: SelectedModel | null,
): ModelInfo | null => {
  if (!selection) return null;
  const provider = providers.find((item) => item.id === selection.providerId);
  if (!provider) return null;
  return provider.models.find((item) => item.id === selection.modelId) ?? null;
};

const messageBody = (message: ChatMessage): string => message.shellAiSummary ?? message.content;

export const conversationTokens = (messages: ChatMessage[]): number =>
  messages.reduce((sum, message) => {
    let tokens = estimateTokensFromText(messageBody(message));
    tokens += estimateTokensFromText(message.reasoning ?? "");
    for (const item of message.attachments ?? []) {
      tokens += estimateTokensFromText(item.text ?? "");
    }
    return sum + tokens;
  }, 0);

export const estimateToolDefinitionTokens = (toolNames: readonly string[]): number => {
  const parts: string[] = [];
  for (const name of toolNames) {
    if (!(AGENT_TOOL_IDS as readonly string[]).includes(name)) continue;
    const id = name as AgentToolId;
    parts.push(
      JSON.stringify({
        type: "function",
        function: {
          name: id,
          description: CHAT_TOOL_DESCRIPTIONS[id],
          parameters: TOOL_PARAMETERS[id],
        },
      }),
    );
  }
  return estimateTokensFromText(parts.join("\n"));
};

const skillBodyTokens = (name: string, skillContexts: readonly SkillContext[]): number => {
  for (const context of skillContexts) {
    const skill = context.skills.find((item) => item.name === name || item.id === name);
    if (skill) return skill.estimatedTokens;
  }
  return 0;
};

export const estimateLoadedSkillTokens = (
  messages: ChatMessage[],
  skillContexts: readonly SkillContext[],
  bound: readonly AgentSkillRef[] = [],
): number => {
  const names = new Set<string>();
  for (const ref of bound) {
    const context = skillContexts.find((item) => item.kind === ref.kind);
    const skill = context?.skills.find((item) => item.id === ref.id);
    const name = skill?.name.trim() || skill?.id;
    if (name) names.add(name);
  }
  for (const message of messages) {
    for (const call of message.toolCalls ?? []) {
      if (call.name !== "skill") continue;
      const argument = call.argument?.trim();
      if (argument) names.add(argument);
    }
  }
  let tokens = 0;
  for (const name of names) tokens += skillBodyTokens(name, skillContexts);
  return tokens;
};

export const estimateMessageCostUsd = (
  messages: ChatMessage[],
  cost: ModelCost | undefined,
  extraInputTokens = 0,
): number => {
  if (!cost) return 0;
  let inputTokens = extraInputTokens;
  let outputTokens = 0;
  for (const message of messages) {
    const tokens = estimateTokensFromText(messageBody(message));
    const reasoning = estimateTokensFromText(message.reasoning ?? "");
    if (message.role === "user") inputTokens += tokens;
    else outputTokens += tokens + reasoning;
  }
  return (inputTokens / 1_000_000) * cost.input + (outputTokens / 1_000_000) * cost.output;
};

export const buildContextUsage = ({
  windowTokens,
  extras,
  cost,
  messages,
}: {
  windowTokens: number | undefined;
  extras?: ContextExtras;
  cost: ModelCost | undefined;
  messages: ChatMessage[];
}): ContextUsageSnapshot => {
  const window = windowTokens && windowTokens > 0 ? windowTokens : 0;
  const conversation = conversationTokens(messages);
  const buckets: ContextBucket[] = CONTEXT_CATEGORY_IDS.map((id) => ({
    id,
    tokens: id === "conversation" ? conversation : Math.max(0, extras?.[id] ?? 0),
  }));
  const usedTokens = buckets.reduce((sum, item) => sum + item.tokens, 0);
  const freeTokens = Math.max(0, window - usedTokens);
  const percent = window === 0 ? 0 : Math.min(100, Math.round((usedTokens / window) * 100));
  const extraInputTokens = usedTokens - conversation;
  return {
    windowTokens: window,
    usedTokens,
    freeTokens,
    percent,
    buckets,
    costUsd: estimateMessageCostUsd(messages, cost, extraInputTokens),
  };
};

export const formatUsageCost = (usd: number): string => {
  if (!Number.isFinite(usd) || usd <= 0) {
    return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }).format(0);
  }
  if (usd < 0.01) {
    return `$${usd.toFixed(4)}`;
  }
  return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }).format(usd);
};

export const formatUsageTokens = (tokens: number): string => formatTokenCount(tokens);
