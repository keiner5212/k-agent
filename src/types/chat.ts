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

export type ChatChunkKind = "content" | "reasoning";

export type ChatChunk = {
  kind: ChatChunkKind;
  text: string;
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
