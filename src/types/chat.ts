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
};

export type ChatChunkKind = "content" | "reasoning" | "tool";

export type ChatChunk = {
  kind: ChatChunkKind;
  text: string;
};

export type ChatToolCall = {
  name: string;
  argument?: string;
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
};

export type ChatTurn = {
  role: ChatRole;
  content: string;
  reasoning?: string | null;
  reasoningSignature?: string | null;
  attachments?: ChatAttachment[];
};

export type SendChatResult = {
  content: string;
  reasoning?: string;
  reasoningSignature?: string;
};
