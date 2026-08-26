import { estimateTokensFromText } from "@/lib/jobs-handlers";
import type { ChatMessage, SelectedModel } from "@/types/chat";
import type { ModelCost, ModelInfo, Provider } from "@/types/providers";

export const CONTEXT_CATEGORY_IDS = [
  "systemPrompt",
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

export const conversationTokens = (messages: ChatMessage[]): number =>
  messages.reduce((sum, message) => sum + estimateTokensFromText(message.content), 0);

export const estimateMessageCostUsd = (
  messages: ChatMessage[],
  cost: ModelCost | undefined,
): number => {
  if (!cost) return 0;
  let inputTokens = 0;
  let outputTokens = 0;
  for (const message of messages) {
    const tokens = estimateTokensFromText(message.content);
    if (message.role === "user") inputTokens += tokens;
    else outputTokens += tokens;
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
  return {
    windowTokens: window,
    usedTokens,
    freeTokens,
    percent,
    buckets,
    costUsd: estimateMessageCostUsd(messages, cost),
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

export const formatUsageTokens = (tokens: number): string => {
  if (tokens < 1_000) return String(tokens);
  const k = tokens / 1_000;
  const digits = Number.isInteger(k) && k >= 100 ? 0 : 1;
  return `${k.toFixed(digits)}K`;
};
