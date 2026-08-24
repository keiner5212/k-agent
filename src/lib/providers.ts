import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { Provider, ProviderDraft, ProviderKind } from "@/types/providers";
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
