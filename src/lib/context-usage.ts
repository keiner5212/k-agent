import { estimateTokensFromText } from "@/lib/jobs-handlers";
import { AGENT_TOOL_IDS, CHAT_TOOL_DESCRIPTIONS, type AgentToolId } from "@/types/agents";
import {
  skillNameFromCall,
  type ChatMessage,
  type ChatToolCall,
  type SelectedModel,
} from "@/types/chat";
import type { McpServer } from "@/types/mcp-servers";
import { formatTokenCount, type ModelCost, type ModelInfo, type Provider } from "@/types/providers";

const SKILL_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    name: {
      type: "string",
      description: "Skill name",
    },
  },
  required: ["name"],
} as const;

const READ_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    filePath: {
      type: "string",
      description: "Absolute or workspace-relative path",
    },
    offset: {
      type: "integer",
      minimum: 1,
      description: "1-based start line",
    },
    limit: {
      type: "integer",
      minimum: 1,
      description: "Max lines (default 2000)",
    },
  },
  required: ["filePath"],
} as const;

const WRITE_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    content: {
      type: "string",
      description: "File contents",
    },
    filePath: {
      type: "string",
      description: "Absolute or workspace-relative path",
    },
  },
  required: ["content", "filePath"],
} as const;

const EDIT_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    filePath: {
      type: "string",
      description: "Absolute or workspace-relative path",
    },
    oldString: {
      type: "string",
      description: "Text to find",
    },
    newString: {
      type: "string",
      description: "Replacement text",
    },
    replaceAll: {
      type: "boolean",
      description: "Replace every match (default false)",
    },
  },
  required: ["filePath", "oldString", "newString"],
} as const;

const LIST_DIRECTORY_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    dirPath: {
      type: "string",
      description: "Absolute or workspace-relative dir (default: workspace)",
    },
    recursive: {
      type: "boolean",
      description: "Walk subdirectories (default false)",
    },
    maxDepth: {
      type: "integer",
      minimum: 1,
      description: "Max depth when recursive (default 3, max 10)",
    },
  },
} as const;

const TOOL_PARAMETERS: Record<AgentToolId, object> = {
  skill: SKILL_TOOL_PARAMETERS,
  read: READ_TOOL_PARAMETERS,
  write: WRITE_TOOL_PARAMETERS,
  edit: EDIT_TOOL_PARAMETERS,
  list_directory: LIST_DIRECTORY_TOOL_PARAMETERS,
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

const callArgsText = (call: ChatToolCall): string => {
  const args = call.arguments?.trim();
  if (args) return `${call.name}\n${args}`;
  const argument = call.argument?.trim();
  return argument ? `${call.name}\n${argument}` : call.name;
};

const roundsForTokens = (
  message: ChatMessage,
): { reasoning: string; content: string; calls: ChatToolCall[] }[] => {
  if (message.toolRounds && message.toolRounds.length > 0) {
    return message.toolRounds.map((round) => ({
      reasoning: round.reasoning,
      content: round.content ?? "",
      calls: round.calls ?? [],
    }));
  }
  if (message.toolCalls && message.toolCalls.length > 0) {
    return [{ reasoning: "", content: "", calls: message.toolCalls }];
  }
  return [];
};

const messageBody = (message: ChatMessage): string => message.shellAiSummary ?? message.content;

type MessageTokenWalk = {
  conversation: number;
  skillOutputs: number;
  input: number;
  output: number;
  reasoning: number;
};

const addTokens = (
  walk: MessageTokenWalk,
  field: "conversation" | "skillOutputs" | "input" | "output" | "reasoning",
  text: string,
): void => {
  const tokens = estimateTokensFromText(text);
  if (tokens === 0) return;
  walk[field] += tokens;
};

const walkMessageTokens = (messages: ChatMessage[]): MessageTokenWalk => {
  const walk: MessageTokenWalk = {
    conversation: 0,
    skillOutputs: 0,
    input: 0,
    output: 0,
    reasoning: 0,
  };
  for (const message of messages) {
    const body = messageBody(message);
    const bodyTokens = estimateTokensFromText(body);
    walk.conversation += bodyTokens;
    if (message.role === "user") walk.input += bodyTokens;
    else walk.output += bodyTokens;

    for (const item of message.attachments ?? []) {
      const textTokens = estimateTokensFromText(item.text ?? "");
      walk.conversation += textTokens;
      if (message.role === "user") walk.input += textTokens;
      else walk.output += textTokens;
    }

    addTokens(walk, "conversation", message.reasoning ?? "");
    addTokens(walk, "reasoning", message.reasoning ?? "");

    for (const round of roundsForTokens(message)) {
      const contentTokens = estimateTokensFromText(round.content);
      walk.conversation += contentTokens;
      walk.output += contentTokens;
      addTokens(walk, "conversation", round.reasoning);
      addTokens(walk, "reasoning", round.reasoning);
      for (const call of round.calls) {
        const argsTokens = estimateTokensFromText(callArgsText(call));
        walk.conversation += argsTokens;
        walk.output += argsTokens;
        const outputTokens = estimateTokensFromText(call.output ?? "");
        walk.input += outputTokens;
        if (call.name === "skill") walk.skillOutputs += outputTokens;
        else walk.conversation += outputTokens;
      }
    }
  }
  return walk;
};

const costFromWalk = (
  walk: MessageTokenWalk,
  cost: ModelCost,
  extraInputTokens: number,
): number => {
  const reasoningRate = cost.reasoning ?? cost.output;
  return (
    ((walk.input + extraInputTokens) / 1_000_000) * cost.input +
    (walk.output / 1_000_000) * cost.output +
    (walk.reasoning / 1_000_000) * reasoningRate
  );
};

export const conversationTokens = (messages: ChatMessage[]): number =>
  walkMessageTokens(messages).conversation;

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

export const estimateMcpToolTokens = (servers: readonly McpServer[]): number => {
  const parts: string[] = [];
  for (const server of servers) {
    if (!server.enabled) continue;
    for (const tool of server.tools ?? []) {
      parts.push(
        JSON.stringify({
          type: "function",
          function: {
            name: `mcp_${server.name}_${tool.name}`,
            description: tool.description ?? "",
            parameters: tool.inputSchema ?? { type: "object", properties: {} },
          },
        }),
      );
    }
  }
  return estimateTokensFromText(parts.join("\n"));
};

export const loadedSkillNamesFromMessages = (messages: ChatMessage[]): string[] => {
  const names = new Set<string>();
  for (const message of messages) {
    for (const round of roundsForTokens(message)) {
      for (const call of round.calls) {
        if (call.name !== "skill" || !call.output) continue;
        const name = skillNameFromCall(call);
        if (name) names.add(name);
      }
    }
  }
  return [...names];
};

export const estimateLoadedSkillTokens = (messages: ChatMessage[]): number =>
  walkMessageTokens(messages).skillOutputs;

export const estimateMessageCostUsd = (
  messages: ChatMessage[],
  cost: ModelCost | undefined,
  extraInputTokens = 0,
): number => {
  if (!cost) return 0;
  return costFromWalk(walkMessageTokens(messages), cost, extraInputTokens);
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
  const walk = walkMessageTokens(messages);
  const buckets: ContextBucket[] = CONTEXT_CATEGORY_IDS.map((id) => ({
    id,
    tokens:
      id === "conversation"
        ? walk.conversation
        : id === "skills"
          ? walk.skillOutputs
          : Math.max(0, extras?.[id] ?? 0),
  }));
  const usedTokens = buckets.reduce((sum, item) => sum + item.tokens, 0);
  const freeTokens = Math.max(0, window - usedTokens);
  const percent = window === 0 ? 0 : Math.min(100, Math.round((usedTokens / window) * 100));
  const extraInputTokens = Math.max(0, usedTokens - walk.conversation - walk.skillOutputs);
  return {
    windowTokens: window,
    usedTokens,
    freeTokens,
    percent,
    buckets,
    costUsd: cost ? costFromWalk(walk, cost, extraInputTokens) : 0,
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
