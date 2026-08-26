export type SelectedModel = {
  providerId: string;
  modelId: string;
};

export type ChatRole = "user" | "assistant";

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
};

export type ChatTurn = {
  role: ChatRole;
  content: string;
  reasoning?: string | null;
  reasoningSignature?: string | null;
};

export type SendChatResult = {
  content: string;
  reasoning?: string;
  reasoningSignature?: string;
};
