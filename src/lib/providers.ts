import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { ModelDraft, Provider, ProviderDraft, ProviderKind } from "@/types/providers";
import { isTauri } from "@/lib/platform";

export type ProviderMutationResult = {
  provider?: Provider;
  error?: string;
};

const invokeOrNone = async <T>(fn: () => Promise<T>, fallback: T): Promise<T> => {
  if (!isTauri()) return fallback;
  try {
    return await fn();
  } catch (error) {
    console.warn("provider command failed", error);
    throw error;
  }
};

type ProvidersStore = {
  providers: Provider[];
  loading: boolean;
  error?: string;
  load: () => Promise<void>;
  save: (draft: ProviderDraft) => Promise<ProviderMutationResult>;
  remove: (id: string) => Promise<{ error?: string }>;
  refresh: (id: string) => Promise<ProviderMutationResult>;
  refreshModel: (providerId: string, modelId: string) => Promise<ProviderMutationResult>;
  upsertModel: (providerId: string, draft: ModelDraft) => Promise<ProviderMutationResult>;
  removeModel: (providerId: string, modelId: string) => Promise<ProviderMutationResult>;
  setFavorite: (providerId: string, modelId: string, favorite: boolean) => Promise<ProviderMutationResult>;
};

export const useProvidersStore = create<ProvidersStore>((set) => ({
  providers: [],
  loading: false,

  load: async () => {
    set({ loading: true, error: undefined });
    try {
      const providers = await invokeOrNone<Provider[]>(
        () => invoke<Provider[]>("list_providers"),
        [],
      );
      set({ providers, loading: false });
    } catch (error) {
      set({ loading: false, error: toMessage(error) });
    }
  },

  save: async (draft) => {
    try {
      const payload = {
        id: draft.id ?? null,
        name: draft.name,
        kind: draft.kind,
        baseUrl: draft.baseUrl,
        apiKey: draft.apiKey ?? null,
        clearApiKey: Boolean(draft.clearApiKey),
      };
      const provider = await invoke<Provider>("save_provider", { input: payload });
      set((state) => {
        const idx = state.providers.findIndex((p) => p.id === provider.id);
        const next = [...state.providers];
        if (idx >= 0) next[idx] = provider;
        else next.push(provider);
        return { providers: next, error: undefined };
      });
      return { provider };
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  remove: async (id) => {
    try {
      await invoke("delete_provider", { id });
      set((state) => ({
        providers: state.providers.filter((p) => p.id !== id),
      }));
      return {};
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  refresh: async (id) => {
    try {
      const provider = await invoke<Provider>("refresh_provider_models", { id });
      set((state) => ({
        providers: state.providers.map((p) => (p.id === id ? provider : p)),
      }));
      return { provider };
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  refreshModel: async (providerId, modelId) => {
    try {
      const provider = await invoke<Provider>("refresh_single_model", {
        id: providerId,
        modelId,
      });
      set((state) => ({
        providers: state.providers.map((p) => (p.id === providerId ? provider : p)),
      }));
      return { provider };
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  upsertModel: async (providerId, draft) => {
    try {
      const provider = await invoke<Provider>("upsert_provider_model", {
        input: {
          providerId,
          originalId: draft.originalId ?? null,
          id: draft.id,
          displayName: draft.displayName ?? null,
          family: draft.family ?? null,
          contextWindow: draft.contextWindow ?? null,
          maxOutputTokens: draft.maxOutputTokens ?? null,
          multimodal: draft.multimodal,
        },
      });
      set((state) => ({
        providers: state.providers.map((item) => (item.id === provider.id ? provider : item)),
      }));
      return { provider };
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  removeModel: async (providerId, modelId) => {
    try {
      const provider = await invoke<Provider>("delete_provider_model", {
        id: providerId,
        modelId,
      });
      set((state) => ({
        providers: state.providers.map((item) => (item.id === provider.id ? provider : item)),
      }));
      return { provider };
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  setFavorite: async (providerId, modelId, favorite) => {
    try {
      const provider = await invoke<Provider>("set_model_favorite", {
        id: providerId,
        modelId,
        favorite,
      });
      set((state) => ({
        providers: state.providers.map((item) => (item.id === provider.id ? provider : item)),
      }));
      return { provider };
    } catch (error) {
      return { error: toMessage(error) };
    }
  },
}));

const toMessage = (error: unknown): string =>
  error instanceof Error ? error.message : typeof error === "string" ? error : "Unknown error";

export const providerKindLabel = (kind: ProviderKind): string => {
  switch (kind) {
    case "openai-like":
      return "OpenAI compatible";
    case "anthropic-like":
      return "Anthropic compatible";
    case "gemini-like":
      return "Gemini compatible";
  }
};
