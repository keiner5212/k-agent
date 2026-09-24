export type SelectedModel = {
  providerId: string;
  modelId: string;
};

export type ChatRole = "user" | "assistant";

export type AttachmentKind = "image" | "pdf" | "video" | "audio" | "text" | "document";

export type ChatAttachment = {
  id: string;
  name: string;
  mime: string;
  kind: AttachmentKind;
  data?: string;
  text?: string;
  file?: string;
};

export type ChatChunkKind = "content" | "reasoning" | "tool" | "tool_result" | "question" | "todo";

export type TodoStatus = "pending" | "in_progress" | "completed" | "cancelled";

export type TodoItem = {
  id: string;
  content: string;
  status: TodoStatus;
  priority: number;
};

export type TodoDiff = {
  added?: TodoItem[];
  updated?: TodoItem[];
  removed?: string[];
  cleared?: boolean;
};

export type ChatChunk = {
  kind: ChatChunkKind;
  text: string;
};

export type AskUserOption = {
  label: string;
  description?: string;
  preview?: string;
};

export type AskUserQuestion = {
  id: string;
  header: string;
  question: string;
  options: AskUserOption[];
  multiSelect?: boolean;
  allowFreeText?: boolean;
};

export type AskUserQuestionChunk = {
  callId: string;
  questions: AskUserQuestion[];
  arguments?: string;
  thoughtSignature?: string;
};

export type AskUserAnswerEntry = {
  questionId: string;
  selected: string[];
  freeText: string;
  skipped?: boolean;
};

export type PendingQuestionState = {
  callId: string;
  messageId: string | null;
  sessionId: string | null;
  questions: AskUserQuestion[];
  answers: AskUserAnswerEntry[];
};

export type ToolDisplay = {
  kind?: string;
  path?: string;
  startLine?: number;
  endLine?: number;
  added?: number;
  removed?: number;
  status?: string;
  skillName?: string;
  linesRemoved?: number;
  imageData?: string;
  todos?: TodoItem[];
};

export type ChatToolCall = {
  id?: string;
  name: string;
  argument?: string;
  arguments?: string;
  thoughtSignature?: string;
  output?: string;
  display?: ToolDisplay;
};

const toolCallFromRecord = (parsed: {
  id?: unknown;
  name?: unknown;
  argument?: unknown;
  arguments?: unknown;
  thoughtSignature?: unknown;
}): ChatToolCall | null => {
  if (typeof parsed.name !== "string" || parsed.name.trim().length === 0) return null;
  const call: ChatToolCall = { name: parsed.name.trim() };
  if (typeof parsed.id === "string" && parsed.id.length > 0) call.id = parsed.id;
  if (typeof parsed.argument === "string" && parsed.argument.length > 0) {
    call.argument = parsed.argument;
  }
  if (typeof parsed.arguments === "string" && parsed.arguments.length > 0) {
    call.arguments = parsed.arguments;
  }
  if (typeof parsed.thoughtSignature === "string" && parsed.thoughtSignature.length > 0) {
    call.thoughtSignature = parsed.thoughtSignature;
  }
  return call;
};

export const parseToolResultChunk = (text: string): ChatToolCall | null => {
  try {
    const parsed = JSON.parse(text) as {
      id?: unknown;
      name?: unknown;
      argument?: unknown;
      arguments?: unknown;
      thoughtSignature?: unknown;
      output?: unknown;
      display?: ToolDisplay;
    };
    const call = toolCallFromRecord(parsed);
    if (!call) return null;
    if (typeof parsed.output === "string") call.output = parsed.output;
    if (parsed.display && typeof parsed.display === "object") {
      call.display = parsed.display;
    }
    return call;
  } catch {
    return null;
  }
};

export const parseToolChunkText = (text: string): ChatToolCall => {
  const trimmed = text.trim();
  if (trimmed.startsWith("{")) {
    try {
      const parsed = JSON.parse(trimmed) as {
        id?: unknown;
        name?: unknown;
        argument?: unknown;
        arguments?: unknown;
        thoughtSignature?: unknown;
      };
      const call = toolCallFromRecord(parsed);
      if (call) return call;
    } catch {
      // Older chunks are `name` or `name\\nargument`.
    }
  }
  const nl = trimmed.indexOf("\n");
  if (nl < 0) return { name: trimmed };
  const name = trimmed.slice(0, nl).trim();
  const argument = trimmed.slice(nl + 1).trim();
  return argument.length > 0 ? { name, argument } : { name };
};

const skillNameFromJson = (raw: string): string => {
  try {
    const parsed = JSON.parse(raw) as { name?: unknown };
    return typeof parsed.name === "string" ? parsed.name.trim() : "";
  } catch {
    return "";
  }
};

export const skillNameFromCall = (call: ChatToolCall): string => {
  const named = call.display?.skillName?.trim();
  if (named) return named;
  const argument = call.argument?.trim();
  if (argument) {
    if (argument.startsWith("{")) {
      const fromJson = skillNameFromJson(argument);
      if (fromJson) return fromJson;
    }
    return argument;
  }
  return call.arguments ? skillNameFromJson(call.arguments) : "";
};

export type PersistedToolCall = {
  id: string;
  name: string;
  argument?: string;
  arguments?: string;
  thoughtSignature?: string;
  output: string;
  display?: ToolDisplay;
};

export type ToolRoundTrace = {
  reasoning: string;
  reasoningSignature?: string;
  content?: string;
  calls: ChatToolCall[];
  thinkingMs?: number;
};

export type PendingAsk = {
  callId: string;
  questions: AskUserQuestion[];
};

export type ChatMessage = {
  id: string;
  role: ChatRole;
  content: string;
  reasoning?: string;
  reasoningSignature?: string;
  thinkingMs?: number;
  streaming?: boolean;
  kind?: "shell";
  shellAiSummary?: string;
  interrupted?: boolean;
  attachments?: ChatAttachment[];
  toolCalls?: ChatToolCall[];
  toolRounds?: ToolRoundTrace[];
  pendingAsk?: PendingAsk;
  todos?: TodoItem[];
  resumeTools?: boolean;
};

export type ChatToolResultTurn = {
  callId: string;
  name: string;
  content: string;
  imageData?: string;
};

export type ChatTurn = {
  role: ChatRole;
  content: string;
  reasoning?: string | null;
  reasoningSignature?: string | null;
  attachments?: ChatAttachment[];
  toolCalls?: ChatToolCall[];
  toolResult?: ChatToolResultTurn;
};

export type SendChatResult = {
  content: string;
  reasoning?: string;
  reasoningSignature?: string;
  toolRounds?: ToolRoundTrace[];
};
