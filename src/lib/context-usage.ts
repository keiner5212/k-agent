import { imageSizeFromBase64, imageTokens, type VisionModel } from "@/lib/image-tokens";
import { estimateTokensFromText } from "@/lib/jobs-handlers";
import { toonFieldValue } from "@/lib/session-files";
import { AGENT_TOOL_IDS, CHAT_TOOL_DESCRIPTIONS, type AgentToolId } from "@/types/agents";
import {
  skillNameFromCall,
  type ChatAttachment,
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
      description: "Max depth when recursive (default 3, or 10 when glob is set, max 10)",
    },
    glob: {
      type: "string",
      description:
        "Argument of list_directory, not a separate tool. Return only matching file paths. *.rs matches any depth. *.{ts,tsx} matches either suffix.",
    },
  },
} as const;

const ASK_USER_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    questions: {
      type: "array",
      description: "1-4 questions. Each has options, optional multiSelect, optional allowFreeText.",
      items: {
        type: "object",
        properties: {
          id: { type: "string", description: "Stable id" },
          header: { type: "string", description: "Short tab label" },
          question: { type: "string", description: "Full question text" },
          options: {
            type: "array",
            items: {
              type: "object",
              properties: {
                label: { type: "string" },
                description: { type: "string" },
                preview: { type: "string" },
              },
              required: ["label"],
            },
          },
          multiSelect: { type: "boolean" },
          allowFreeText: { type: "boolean", description: "Show a free-text input (default true)" },
        },
        required: ["id", "header", "question", "options"],
      },
    },
  },
  required: ["questions"],
} as const;

const CREATE_FOLDER_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    dirPath: {
      type: "string",
      description: "Absolute or workspace-relative path",
    },
  },
  required: ["dirPath"],
} as const;

const DELETE_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    path: {
      type: "string",
      description: "Absolute or workspace-relative path to delete",
    },
  },
  required: ["path"],
} as const;

const FETCH_URL_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    url: {
      type: "string",
      description:
        "Public HTTPS URL. Only port 443. Credentials, IP literals, and private hostnames are refused.",
    },
    lang: {
      type: "string",
      description: "Optional BCP 47 language tag (defaults to en-US).",
    },
  },
  required: ["url"],
} as const;

const INTERNET_SEARCH_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    query: {
      type: "string",
      description: "Search query. Trimmed to 200 characters.",
    },
    lang: {
      type: "string",
      description: "Optional BCP 47 language tag (defaults to en-US).",
    },
    pages: {
      type: "number",
      description: "Optional page count, 1 or 2.",
    },
  },
  required: ["query"],
} as const;

const HTTP_REQUEST_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    url: { type: "string", description: "Absolute http or https URL, including localhost." },
    method: {
      type: "string",
      description: "GET, HEAD, POST, PUT, PATCH, DELETE, or OPTIONS. Default GET.",
    },
    headers: { type: "object", description: "Optional header map. Values are strings." },
    query: { type: "object", description: "Optional query parameter map appended to the URL." },
    body: { type: "string", description: "Optional raw body. Ignored for GET and HEAD." },
    timeoutMs: {
      type: "number",
      description: "Timeout in milliseconds. Default 20000, max 60000.",
    },
  },
  required: ["url"],
} as const;

const GRAPHQL_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    url: { type: "string", description: "GraphQL endpoint. http or https, including localhost." },
    query: { type: "string", description: "GraphQL query or mutation document." },
    variables: { type: "object", description: "Optional variables object." },
    operationName: { type: "string", description: "Optional operation name." },
    headers: { type: "object", description: "Optional header map." },
    method: {
      type: "string",
      description: "POST (default) or GET. Non-GET waits for confirmation.",
    },
    timeoutMs: {
      type: "number",
      description: "Timeout in milliseconds. Default 20000, max 60000.",
    },
  },
  required: ["url", "query"],
} as const;

const PAGE_SHOT_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    url: { type: "string", description: "Page URL. http or https, including localhost." },
    width: {
      type: "number",
      description: "Viewport width in pixels. Default 1280. Range 320-1600.",
    },
    height: {
      type: "number",
      description: "Viewport height in pixels. Default 720. Range 240-1200.",
    },
    selector: {
      type: "string",
      description: "Optional CSS selector. When set, the image is only that element.",
    },
    waitMs: {
      type: "number",
      description: "Extra wait after load before capture. Default 300. Max 5000.",
    },
  },
  required: ["url"],
} as const;

const TODO_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    todos: {
      type: "array",
      minItems: 0,
      maxItems: 64,
      description: "Full ordered list. Empty array clears the list.",
      items: {
        type: "object",
        properties: {
          content: {
            type: "string",
            description: "Short description of the task.",
          },
          status: {
            type: "string",
            enum: ["pending", "in_progress", "completed", "cancelled"],
            description: "Current status of the task.",
          },
          priority: {
            type: "string",
            enum: ["high", "medium", "low"],
            description: "Priority of the task.",
          },
        },
        required: ["content", "status", "priority"],
      },
    },
  },
  required: ["todos"],
} as const;

const BACKGROUND_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    command: {
      type: "string",
      description: "One command to keep until the turn ends. No &, nohup, or disown.",
    },
  },
  required: ["command"],
} as const;

const BASH_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    command: { type: "string", description: "One shell command." },
  },
  required: ["command"],
} as const;

const GREP_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    pattern: { type: "string", description: "Ripgrep regex." },
    path: { type: "string", description: "File or directory. Default workspace root." },
    glob: {
      type: "string",
      description:
        "Argument of grep, not a separate tool. Include only paths matching this glob. A leading ! excludes.",
    },
    caseInsensitive: {
      type: "boolean",
      description: "Match letters regardless of case. Default false.",
    },
    filesOnly: {
      type: "boolean",
      description: "Return matching paths only, no line text. Default false.",
    },
  },
  required: ["pattern"],
} as const;

const APPLY_PATCH_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    patchText: {
      type: "string",
      description: "Full patch, from *** Begin Patch through *** End Patch.",
    },
  },
  required: ["patchText"],
} as const;

const LSP_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    operation: { type: "string", description: "LSP operation." },
    filePath: { type: "string", description: "Absolute or workspace-relative file." },
    line: { type: "integer", description: "1-based line." },
    character: { type: "integer", description: "1-based character." },
    query: { type: "string", description: "workspaceSymbol filter." },
  },
  required: ["operation", "filePath"],
} as const;

const DIAGRAM_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    brief: {
      type: "string",
      description: "Full description of the diagram. Not Mermaid source.",
    },
  },
  required: ["brief"],
} as const;

const TASK_TOOL_PARAMETERS = {
  type: "object",
  properties: {
    description: { type: "string", description: "3 to 5 words." },
    prompt: { type: "string", description: "Full task for the other agent." },
    agent: { type: "string", description: "Name of a saved agent." },
  },
  required: ["description", "prompt", "agent"],
} as const;

const TOOL_PARAMETERS: Record<AgentToolId, object> = {
  skill: SKILL_TOOL_PARAMETERS,
  read: READ_TOOL_PARAMETERS,
  write: WRITE_TOOL_PARAMETERS,
  edit: EDIT_TOOL_PARAMETERS,
  list_directory: LIST_DIRECTORY_TOOL_PARAMETERS,
  ask_user: ASK_USER_TOOL_PARAMETERS,
  create_folder: CREATE_FOLDER_TOOL_PARAMETERS,
  delete: DELETE_TOOL_PARAMETERS,
  fetch_url: FETCH_URL_TOOL_PARAMETERS,
  internet_search: INTERNET_SEARCH_TOOL_PARAMETERS,
  http_request: HTTP_REQUEST_TOOL_PARAMETERS,
  graphql: GRAPHQL_TOOL_PARAMETERS,
  page_shot: PAGE_SHOT_TOOL_PARAMETERS,
  todowrite: TODO_TOOL_PARAMETERS,
  bash: BASH_TOOL_PARAMETERS,
  background: BACKGROUND_TOOL_PARAMETERS,
  grep: GREP_TOOL_PARAMETERS,
  apply_patch: APPLY_PATCH_TOOL_PARAMETERS,
  lsp: LSP_TOOL_PARAMETERS,
  task: TASK_TOOL_PARAMETERS,
  diagram: DIAGRAM_TOOL_PARAMETERS,
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

export const resolveVisionModel = (
  providers: Provider[],
  selection: SelectedModel | null,
): VisionModel | null => {
  if (!selection) return null;
  const provider = providers.find((item) => item.id === selection.providerId);
  if (!provider) return null;
  return { kind: provider.kind, modelId: selection.modelId };
};

const positiveInt = (value: string): number => {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 1) return 0;
  return Math.round(parsed);
};

export const toolImageSize = (call: ChatToolCall): { width: number; height: number } | null => {
  const width = positiveInt(toonFieldValue(call.output ?? "", "width"));
  const height = positiveInt(toonFieldValue(call.output ?? "", "height"));
  if (width > 0 && height > 0) return { width, height };
  const data = call.display?.imageData;
  if (!data) return null;
  return imageSizeFromBase64(data);
};

const attachmentImageTokens = (item: ChatAttachment, vision: VisionModel | null): number => {
  if (item.kind !== "image" || !item.data) return 0;
  const size = imageSizeFromBase64(item.data);
  if (!size) return 0;
  return imageTokens(size.width, size.height, vision);
};

const callArgsText = (call: ChatToolCall): string => {
  const args = call.arguments?.trim();
  if (args) return `${call.name}\n${args}`;
  const argument = call.argument?.trim();
  return argument ? `${call.name}\n${argument}` : call.name;
};

export const toolCallTokens = (call: ChatToolCall, vision: VisionModel | null): number => {
  const args = estimateTokensFromText(callArgsText(call));
  const output = estimateTokensFromText(call.output ?? "");
  const size = toolImageSize(call);
  const image = size ? imageTokens(size.width, size.height, vision) : 0;
  return args + output + image;
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

const addCounted = (walk: MessageTokenWalk, tokens: number, role: ChatMessage["role"]): void => {
  if (tokens <= 0) return;
  walk.conversation += tokens;
  if (role === "user") walk.input += tokens;
  else walk.output += tokens;
};

const walkMessageTokens = (
  messages: ChatMessage[],
  vision: VisionModel | null,
): MessageTokenWalk => {
  const walk: MessageTokenWalk = {
    conversation: 0,
    skillOutputs: 0,
    input: 0,
    output: 0,
    reasoning: 0,
  };
  for (const message of messages) {
    const rounds = roundsForTokens(message);
    const roundContent = new Set(
      rounds.map((round) => round.content).filter((text) => text.length > 0),
    );
    const body = messageBody(message);
    if (!roundContent.has(body)) addCounted(walk, estimateTokensFromText(body), message.role);

    for (const item of message.attachments ?? []) {
      addCounted(walk, estimateTokensFromText(item.text ?? ""), message.role);
      addCounted(walk, attachmentImageTokens(item, vision), message.role);
    }

    const roundReasoning = new Set(
      rounds.map((round) => round.reasoning).filter((text) => text.length > 0),
    );
    const messageReasoning = message.reasoning ?? "";
    if (messageReasoning.length > 0 && !roundReasoning.has(messageReasoning)) {
      addTokens(walk, "conversation", messageReasoning);
      addTokens(walk, "reasoning", messageReasoning);
    }

    for (const round of rounds) {
      addCounted(walk, estimateTokensFromText(round.content), "assistant");
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
        const size = toolImageSize(call);
        if (size) {
          const image = imageTokens(size.width, size.height, vision);
          walk.conversation += image;
          walk.input += image;
        }
        if (call.name === "delete") {
          const removedLines = call.display?.linesRemoved ?? 0;
          if (removedLines > 0) {
            const removedTokens = estimateTokensFromText("\n".repeat(removedLines * 32));
            if (walk.conversation >= removedTokens) walk.conversation -= removedTokens;
            else walk.conversation = 0;
          }
        }
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

export const conversationTokens = (
  messages: ChatMessage[],
  vision: VisionModel | null = null,
): number => walkMessageTokens(messages, vision).conversation;

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

export const estimateLoadedSkillTokens = (
  messages: ChatMessage[],
  vision: VisionModel | null = null,
): number => walkMessageTokens(messages, vision).skillOutputs;

export const estimateMessageCostUsd = (
  messages: ChatMessage[],
  cost: ModelCost | undefined,
  extraInputTokens = 0,
  vision: VisionModel | null = null,
): number => {
  if (!cost) return 0;
  return costFromWalk(walkMessageTokens(messages, vision), cost, extraInputTokens);
};

export const buildContextUsage = ({
  windowTokens,
  extras,
  cost,
  messages,
  vision = null,
}: {
  windowTokens: number | undefined;
  extras?: ContextExtras;
  cost: ModelCost | undefined;
  messages: ChatMessage[];
  vision?: VisionModel | null;
}): ContextUsageSnapshot => {
  const window = windowTokens && windowTokens > 0 ? windowTokens : 0;
  const walk = walkMessageTokens(messages, vision);
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
