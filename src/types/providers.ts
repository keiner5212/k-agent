export type ProviderKind = "openai-like" | "anthropic-like" | "gemini-like";

export const PROVIDER_KINDS: readonly ProviderKind[] = [
  "openai-like",
  "anthropic-like",
  "gemini-like",
] as const;

export type ModelSource = "detected" | "custom";

export type ModelCost = {
  input: number;
  output: number;
  reasoning?: number;
  cacheRead?: number;
  cacheWrite?: number;
};

export type ModelInfo = {
  id: string;
  contextWindow?: number;
  maxOutputTokens?: number;
  displayName?: string;
  family?: string;
  knowledge?: string;
  input?: string[];
  output?: string[];
  reasoning?: boolean;
  toolCall?: boolean;
  structuredOutput?: boolean;
  attachment?: boolean;
  attachmentTypes?: string[];
  multimodal?: boolean;
  effortLevels?: string[];
  cost?: ModelCost;
  source?: ModelSource;
  userEdited?: boolean;
  favorite?: boolean;
};

export type ModelDraft = {
  originalId?: string;
  id: string;
  displayName?: string;
  family?: string;
  contextWindow?: number;
  maxOutputTokens?: number;
  multimodal: boolean;
  effortLevels?: string[];
};

export type Provider = {
  id: string;
  name: string;
  kind: ProviderKind;
  baseUrl: string;
  hasApiKey?: boolean;
  models: ModelInfo[];
  lastSyncedAt?: number;
};

export type ProviderDraft = {
  id?: string;
  name: string;
  kind: ProviderKind;
  baseUrl: string;
  apiKey?: string;
  clearApiKey?: boolean;
};

export const DEFAULT_BASE_URLS: Record<ProviderKind, string> = {
  "openai-like": "https://api.openai.com/v1",
  "anthropic-like": "https://api.anthropic.com",
  "gemini-like": "https://generativelanguage.googleapis.com",
};

export const formatTokenCount = (tokens: number): string => {
  const count = Math.max(0, Math.round(tokens));
  if (count >= 1_000_000) {
    return `${formatScaled(count / 1_000_000)}M`;
  }
  if (count >= 1_000) {
    return `${formatScaled(count / 1_000)}k`;
  }
  return String(count);
};

const formatScaled = (value: number): string => {
  if (value >= 100 || Number.isInteger(value)) return String(Math.round(value));
  return value.toFixed(1).replace(/\.0$/, "");
};

export const formatContextWindow = (tokens: number | undefined): string => {
  if (tokens === undefined) return "-";
  return formatTokenCount(tokens);
};

export const parseTokenAmount = (raw: string): number | undefined => {
  const trimmed = raw.trim().toLowerCase().replace(/[,\s]/g, "");
  if (!trimmed) return undefined;
  const match = trimmed.match(/^(\d+(?:\.\d+)?)(k|m)?$/);
  if (!match) return undefined;
  const value = Number(match[1]);
  if (!Number.isFinite(value) || value < 0) return undefined;
  const multiplier = match[2] === "m" ? 1_000_000 : match[2] === "k" ? 1_000 : 1;
  return Math.round(value * multiplier);
};
