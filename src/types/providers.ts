export type ProviderKind = "openai-like" | "anthropic-like" | "gemini-like";

export const PROVIDER_KINDS: readonly ProviderKind[] = [
  "openai-like",
  "anthropic-like",
  "gemini-like",
] as const;

export type ModelInfo = {
  id: string;
  contextWindow?: number;
  maxOutputTokens?: number;
  displayName?: string;
  family?: string;
  multimodal?: boolean;
};

export type Provider = {
  id: string;
  name: string;
  kind: ProviderKind;
  baseUrl: string;
  apiKey?: string;
  models: ModelInfo[];
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

export const formatContextWindow = (tokens: number | undefined): string => {
  if (tokens === undefined) return "-";
  if (tokens >= 1_000_000)
    return `${(tokens / 1_000_000).toFixed(tokens % 1_000_000 === 0 ? 0 : 1)}M`;
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(0)}k`;
  return String(tokens);
};
