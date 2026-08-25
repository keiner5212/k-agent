export type SelectedModel = {
  providerId: string;
  modelId: string;
};

export type ChatRole = "user" | "assistant";

export type ChatMessage = {
  id: string;
  role: ChatRole;
  content: string;
};

export type SendChatResult = {
  content: string;
};
