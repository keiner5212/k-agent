export type ProviderKind = "openai-like" | "anthropic-like" | "gemini-like";

export const PROVIDER_KINDS: readonly ProviderKind[] = [
  "openai-like",
  "anthropic-like",
  "gemini-like",
] as const;

export type Provider = {
  id: string;
  name: string;
  kind: ProviderKind;
  baseUrl: string;
  apiKey?: string;
  models: string[];
  lastSyncedAt?: number;
};

export type ProviderDraft = {
  id?: string;
  name: string;
  kind: ProviderKind;
  baseUrl: string;
  apiKey?: string;
};

export const DEFAULT_BASE_URLS: Record<ProviderKind, string> = {
  "openai-like": "https://api.openai.com/v1",
  "anthropic-like": "https://api.anthropic.com",
  "gemini-like": "https://generativelanguage.googleapis.com",
};
