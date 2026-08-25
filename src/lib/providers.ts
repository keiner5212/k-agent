import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { ModelDraft, Provider, ProviderDraft } from "@/types/providers";
import { DESKTOP_REQUIRED, ipcErrorMessage, isTauri } from "@/lib/platform";
import { acquireWorkerCores, getWorkerCoreSnapshot } from "@/lib/worker-cores";

export type ProviderMutationResult = {
  provider?: Provider;
  error?: string;
};

const replaceProvider = (providers: Provider[], provider: Provider): Provider[] => {
  const idx = providers.findIndex((item) => item.id === provider.id);
  if (idx < 0) return [...providers, provider];
  const next = [...providers];
  next[idx] = provider;
  return next;
};

const withMaxCores = async <T>(label: string, run: (cores: number) => Promise<T>): Promise<T> => {
  const lease = acquireWorkerCores(label, getWorkerCoreSnapshot().limit);
  try {
    return await run(lease.cores);
  } finally {
    lease.release();
  }
};

const runMutation = async (
  work: () => Promise<Provider | undefined>,
): Promise<ProviderMutationResult> => {
  if (!isTauri()) return { error: DESKTOP_REQUIRED };
  try {
    const provider = await work();
    return provider ? { provider } : {};
  } catch (error) {
    return { error: ipcErrorMessage(error) };
  }
};

type ProvidersStore = {
  providers: Provider[];
  loading: boolean;
  error?: string;
  load: () => Promise<void>;
  save: (draft: ProviderDraft) => Promise<ProviderMutationResult>;
  remove: (id: string) => Promise<ProviderMutationResult>;
  refresh: (id: string) => Promise<ProviderMutationResult>;
  upsertModel: (providerId: string, draft: ModelDraft) => Promise<ProviderMutationResult>;
  removeModel: (providerId: string, modelId: string) => Promise<ProviderMutationResult>;
  setFavorite: (
    providerId: string,
    modelId: string,
    favorite: boolean,
  ) => Promise<ProviderMutationResult>;
};

export const useProvidersStore = create<ProvidersStore>((set) => ({
  providers: [],
  loading: false,

  load: async () => {
    set({ loading: true, error: undefined });
    if (!isTauri()) {
      set({ providers: [], loading: false });
      return;
    }
    try {
      const providers = await invoke<Provider[]>("list_providers");
      set({ providers, loading: false });
    } catch (error) {
      set({ loading: false, error: ipcErrorMessage(error) });
    }
  },

  save: async (draft) =>
    runMutation(async () =>
      withMaxCores("saveProvider", async (workerCores) => {
        const provider = await invoke<Provider>("save_provider", {
          input: {
            id: draft.id ?? null,
            name: draft.name,
            kind: draft.kind,
            baseUrl: draft.baseUrl,
            apiKey: draft.apiKey ?? null,
            clearApiKey: Boolean(draft.clearApiKey),
            workerCores,
          },
        });
        set((state) => ({
          providers: replaceProvider(state.providers, provider),
          error: undefined,
        }));
        return provider;
      }),
    ),

  remove: async (id) =>
    runMutation(async () => {
      await invoke("delete_provider", { id });
      set((state) => ({
        providers: state.providers.filter((item) => item.id !== id),
      }));
      return undefined;
    }),

  refresh: async (id) =>
    runMutation(async () =>
      withMaxCores("refreshProviderModels", async (workerCores) => {
        const provider = await invoke<Provider>("refresh_provider_models", { id, workerCores });
        set((state) => ({ providers: replaceProvider(state.providers, provider) }));
        return provider;
      }),
    ),

  upsertModel: async (providerId, draft) =>
    runMutation(async () => {
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
          effortLevels: draft.effortLevels ?? null,
        },
      });
      set((state) => ({ providers: replaceProvider(state.providers, provider) }));
      return provider;
    }),

  removeModel: async (providerId, modelId) =>
    runMutation(async () => {
      const provider = await invoke<Provider>("delete_provider_model", {
        id: providerId,
        modelId,
      });
      set((state) => ({ providers: replaceProvider(state.providers, provider) }));
      return provider;
    }),

  setFavorite: async (providerId, modelId, favorite) =>
    runMutation(async () => {
      const provider = await invoke<Provider>("set_model_favorite", {
        input: { id: providerId, modelId, favorite },
      });
      set((state) => ({ providers: replaceProvider(state.providers, provider) }));
      return provider;
    }),
}));
