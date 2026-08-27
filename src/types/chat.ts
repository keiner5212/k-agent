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

export type ChatChunkKind = "content" | "reasoning" | "tool" | "question";

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

export const parseToolChunkText = (text: string): ChatToolCall => {
  const nl = text.indexOf("\n");
  if (nl < 0) {
    const name = text.trim();
    return { name };
  }
  const name = text.slice(0, nl).trim();
  const argument = text.slice(nl + 1).trim();
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
};

export type ChatToolResultTurn = {
  callId: string;
  name: string;
  content: string;
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
